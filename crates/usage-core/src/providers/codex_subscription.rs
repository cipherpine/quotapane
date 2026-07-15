//! `CodexSubscription` — the second provider (M3).
//!
//! Reads the local Codex CLI OAuth credentials (read-only) and queries the
//! ChatGPT-plan usage endpoint the Codex CLI itself polls:
//! `GET https://chatgpt.com/backend-api/wham/usage`. The token travels only
//! through the [`Egress`] chokepoint, only to `chatgpt.com`, and is held in
//! a [`Secret`] the whole time (SECURITY.md invariants 2, 3, 6).
//!
//! ## Verification (M3, "verify don't invent")
//! Endpoint, headers, and response shape were verified against the
//! open-source Codex CLI (`openai/codex`, `codex-rs/backend-client`) and its
//! issue tracker — notably: ChatGPT-plan usage is served by `chatgpt.com`,
//! **not** `api.openai.com`. The CLI sends `Authorization: Bearer <token>`,
//! `ChatGPT-Account-Id` (when known), and a `User-Agent` defaulting to
//! `"codex-cli"` — [`DEFAULT_USER_AGENT`] mirrors that verified default
//! rather than inventing a format. Presenting as the official client is a
//! deliberate, disclosed choice (README.md, SECURITY.md).
//!
//! ## Schema (pinned by a live run)
//! The endpoint is undocumented. The M3 live run pinned the real shape:
//! camelCase, windows wrapped under `rateLimits`, with `usedPercent` /
//! `windowDurationMins` / `resetsAt` (an absolute epoch number). The DTOs
//! below match that exactly; a flat root-level `primary`/`secondary` is also
//! accepted as cheap resilience, and `resetsAt` still parses as an RFC 3339
//! string or a relative count in case a future reshape uses one. Anything
//! unrecognized degrades the snapshot instead of crashing or leaking
//! (THREAT_MODEL.md R4).
//!
//! ## Credentials (`~/.codex/auth.json`, or `$CODEX_HOME/auth.json`)
//! Verified shape: `{ "OPENAI_API_KEY": …, "tokens": { "id_token",
//! "access_token", "refresh_token", "account_id" }, "last_refresh": … }`.
//! Only `access_token` (→ [`Secret`]) and `account_id` (header value, never
//! logged) are retained; the id/refresh tokens are never copied out of the
//! raw buffer, which is itself wrapped in [`Secret`] and zeroized. There is
//! no expiry field, so token expiry surfaces as a provider 401/403 →
//! [`ProviderError::TokenExpired`] (refresh is delegated to `codex login`,
//! invariant 6).

use crate::credentials::{load_credential_file, Secret};
use crate::egress::Egress;
use crate::model::{ProviderId, ProviderSnapshot, QuotaWindow, SnapshotSource};
use crate::providers::time::parse_rfc3339_to_unix;
use crate::providers::{Cadence, ProviderError, UsageProvider};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

/// The Codex (ChatGPT-plan) host — must be on the egress allowlist.
const HOST: &str = "chatgpt.com";
/// The subscription usage path the Codex CLI polls.
const PATH: &str = "/backend-api/wham/usage";
/// The `User-Agent` the Codex CLI sends by default (verified in its source).
/// Passed to [`CodexSubscription::with_default_path`] by callers unless they
/// have a more specific verified value.
pub const DEFAULT_USER_AGENT: &str = "codex-cli";

/// Threshold distinguishing an absolute epoch from a relative seconds count
/// when `resets_at` arrives as a bare number (~2001-09-09 in epoch terms; no
/// quota window is years long, and no epoch is this small).
const EPOCH_VS_RELATIVE_THRESHOLD: i64 = 1_000_000_000;

/// Provider for Codex (ChatGPT-plan) subscription quota.
pub struct CodexSubscription {
    /// Path to `auth.json` (or a test fixture).
    credentials_path: PathBuf,
    /// The `User-Agent` sent with the request. Use [`DEFAULT_USER_AGENT`]
    /// unless a more specific verified value is available.
    user_agent: String,
}

impl CodexSubscription {
    /// Construct with an explicit credential path and User-Agent.
    pub fn new(credentials_path: PathBuf, user_agent: impl Into<String>) -> Self {
        CodexSubscription {
            credentials_path,
            user_agent: user_agent.into(),
        }
    }

    /// Construct using the default `$CODEX_HOME/auth.json` /
    /// `~/.codex/auth.json` path (see `credentials::codex_credentials_path`).
    ///
    /// Returns `None` if no home directory can be resolved. Pass
    /// [`DEFAULT_USER_AGENT`] as `user_agent` unless you have a more
    /// specific verified value.
    pub fn with_default_path(user_agent: impl Into<String>) -> Option<Self> {
        crate::credentials::codex_credentials_path().map(|p| Self::new(p, user_agent))
    }
}

impl UsageProvider for CodexSubscription {
    fn id(&self) -> ProviderId {
        ProviderId::CodexSubscription
    }

    fn poll(&self, http: &Egress) -> Result<ProviderSnapshot, ProviderError> {
        // Ingress (TB1): load read-only, wrapped in Secret.
        let raw = load_credential_file(&self.credentials_path)
            .map_err(|e| ProviderError::Credential(e.to_string()))?;
        let creds = parse_auth(raw.expose())
            .ok_or_else(|| ProviderError::Credential("malformed auth.json".into()))?;

        // Egress (TB2): the token leaves the process only here. The
        // ChatGPT-Account-Id header is sent when present, matching the CLI.
        let mut headers: Vec<(&str, &str)> = vec![
            ("Content-Type", "application/json"),
            ("User-Agent", &self.user_agent),
        ];
        if let Some(account_id) = creds.account_id.as_deref() {
            headers.push(("ChatGPT-Account-Id", account_id));
        }
        let resp = http.get(HOST, PATH, Some(&creds.access_token), &headers)?;

        match resp.status {
            200 => {
                let usage: RawUsage = serde_json::from_slice(&resp.body)
                    .map_err(|_| ProviderError::UnexpectedPayload)?;
                Ok(build_snapshot(usage, now_unix_secs()))
            }
            401 | 403 => Err(ProviderError::TokenExpired),
            429 => Err(ProviderError::RateLimited {
                retry_after_secs: retry_after_secs(&resp.headers),
            }),
            _ => Err(ProviderError::UnexpectedPayload),
        }
    }

    fn cadence(&self) -> Cadence {
        // The CLI itself polls aggressively (~60 s); we stay a far politer
        // client at Normal (~7 min), well above the 180 s floor.
        Cadence::Normal
    }
}

/// Parsed credentials: the bearer token (wrapped) and the account id.
struct ParsedAuth {
    access_token: Secret<String>,
    /// ChatGPT account id — a header value, not a bearer secret, but still
    /// an account identifier: sent in the request only, never logged.
    account_id: Option<String>,
}

/// Raw shape of `auth.json`. Only the fields we need are deserialized;
/// `id_token`/`refresh_token` are never copied out of the raw buffer (which
/// is itself a `Secret` and zeroized on drop).
#[derive(Deserialize)]
struct RawAuth {
    tokens: RawTokens,
}

#[derive(Deserialize)]
struct RawTokens {
    access_token: String,
    account_id: Option<String>,
}

/// Parse the credential JSON. Returns `None` on any structural problem; no
/// error value exists, so token bytes cannot leak through one.
fn parse_auth(raw_json: &str) -> Option<ParsedAuth> {
    let parsed: RawAuth = serde_json::from_str(raw_json).ok()?;
    Some(ParsedAuth {
        // Moves the String's heap buffer into Secret — no additional copy.
        access_token: Secret::new(parsed.tokens.access_token),
        account_id: parsed.tokens.account_id,
    })
}

/// `resets_at` as observed/plausible on the wire: an RFC 3339 string, an
/// absolute epoch-seconds number, or a relative seconds-until-reset number.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawResetsAt {
    Number(i64),
    Text(String),
}

/// One rate-limit window. Field names pinned against a real wire capture and
/// the Codex issue tracker: the endpoint serializes **camelCase**
/// (`usedPercent`, `windowDurationMins`, `resetsAt`); `resetsAt` is an
/// absolute epoch-seconds number. `#[serde(rename_all = "camelCase")]` maps
/// these snake_case Rust fields onto the wire names. Everything optional:
/// degrade, don't crash.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawWindow {
    used_percent: Option<f64>,
    window_duration_mins: Option<u64>,
    resets_at: Option<RawResetsAt>,
}

/// Windows pair as the endpoint models it (`primary` / `secondary`).
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawWindows {
    primary: Option<RawWindow>,
    secondary: Option<RawWindow>,
}

/// Top-level response. The verified shape wraps the windows under
/// `rateLimits`; a flat root-level `primary`/`secondary` is also accepted as
/// cheap resilience against a future reshape.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct RawUsage {
    rate_limits: Option<RawWindows>,
    primary: Option<RawWindow>,
    secondary: Option<RawWindow>,
}

/// Human label for a window from its duration in minutes: `300 → "5h"`,
/// `10080 → "7d"`, `90 → "90m"`; `None → fallback`.
fn minutes_label(minutes: Option<u64>, fallback: &str) -> String {
    match minutes {
        Some(m) if m > 0 && m % 1440 == 0 => format!("{}d", m / 1440),
        Some(m) if m > 0 && m % 60 == 0 => format!("{}h", m / 60),
        Some(m) if m > 0 => format!("{m}m"),
        _ => fallback.to_string(),
    }
}

/// Seconds until reset from whichever `resets_at` form arrived.
fn resets_in_secs(resets_at: Option<&RawResetsAt>, now_unix_secs: u64) -> Option<u64> {
    match resets_at? {
        RawResetsAt::Text(s) => parse_rfc3339_to_unix(s)
            .map(|reset| reset.saturating_sub(now_unix_secs as i64).max(0) as u64),
        RawResetsAt::Number(n) if *n >= EPOCH_VS_RELATIVE_THRESHOLD => {
            // Absolute epoch seconds.
            Some(n.saturating_sub(now_unix_secs as i64).max(0) as u64)
        }
        RawResetsAt::Number(n) if *n >= 0 => Some(*n as u64), // relative seconds
        RawResetsAt::Number(_) => None,                       // negative: nonsense
    }
}

/// Build a normalized, non-secret snapshot from a usage response.
fn build_snapshot(usage: RawUsage, now_unix_secs: u64) -> ProviderSnapshot {
    // Prefer the wrapped shape; fall back to root-level windows.
    let (primary, secondary) = match usage.rate_limits {
        Some(rl) => (rl.primary, rl.secondary),
        None => (usage.primary, usage.secondary),
    };

    let mut windows = Vec::new();
    for (win, fallback) in [(primary, "primary"), (secondary, "secondary")] {
        if let Some(w) = win {
            windows.push(QuotaWindow {
                label: minutes_label(w.window_duration_mins, fallback),
                used_fraction: w.used_percent.map(|u| (u / 100.0).clamp(0.0, 1.0)),
                resets_in_secs: resets_in_secs(w.resets_at.as_ref(), now_unix_secs),
            });
        }
    }
    ProviderSnapshot {
        provider: ProviderId::CodexSubscription,
        taken_at_unix_secs: now_unix_secs,
        windows,
        source: SnapshotSource::UsageEndpoint,
    }
}

/// Extract a `retry-after` value (seconds) from response headers, if present.
fn retry_after_secs(headers: &[(String, String)]) -> Option<u64> {
    headers
        .iter()
        .find(|(name, _)| name == "retry-after")
        .and_then(|(_, value)| value.trim().parse::<u64>().ok())
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC_TOKEN: &str = "synthetic-codex-token-ZZTOP-not-real";

    fn auth_json() -> String {
        format!(
            r#"{{"OPENAI_API_KEY":null,"tokens":{{"id_token":"idt-synthetic","access_token":"{SYNTHETIC_TOKEN}","refresh_token":"rt-synthetic","account_id":"acct-1234"}},"last_refresh":"2026-07-15T00:00:00Z"}}"#
        )
    }

    #[test]
    fn parses_auth_and_wraps_token_redacted() {
        let parsed = parse_auth(&auth_json()).unwrap();
        assert_eq!(parsed.access_token.expose(), SYNTHETIC_TOKEN);
        assert_eq!(parsed.account_id.as_deref(), Some("acct-1234"));
        assert!(!format!("{:?}", parsed.access_token).contains("ZZTOP"));
    }

    #[test]
    fn auth_without_account_id_still_parses() {
        let json = r#"{"tokens":{"access_token":"tok-synthetic"}}"#;
        let parsed = parse_auth(json).unwrap();
        assert_eq!(parsed.account_id, None);
    }

    #[test]
    fn malformed_auth_returns_none_without_secrets() {
        assert!(parse_auth("not json").is_none());
        assert!(parse_auth(r#"{"tokens":{}}"#).is_none());
        assert!(parse_auth(r#"{"OPENAI_API_KEY":"x"}"#).is_none());
    }

    #[test]
    fn builds_snapshot_from_verified_wire_shape() {
        // Byte-for-byte the real endpoint shape: camelCase, `rateLimits`
        // wrapper, `resetsAt` as an absolute epoch number (from the Codex
        // issue tracker capture). This is the regression test for the M3
        // live-run mismatch — the first build used snake_case and rendered
        // no bars.
        let now: u64 = 1_779_000_000;
        let json = format!(
            r#"{{"rateLimits":{{
                "limitId":"codex",
                "primary":   {{"usedPercent": 25, "windowDurationMins": 300,   "resetsAt": {}}},
                "secondary": {{"usedPercent": 18, "windowDurationMins": 10080, "resetsAt": {}}},
                "planType":"prolite","rateLimitReachedType":null
            }}}}"#,
            now + 3600,
            now + 86_400
        );
        let usage: RawUsage = serde_json::from_str(&json).unwrap();
        let snap = build_snapshot(usage, now);

        assert_eq!(snap.provider, ProviderId::CodexSubscription);
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].label, "5h");
        assert_eq!(snap.windows[0].used_fraction, Some(0.25));
        assert_eq!(snap.windows[0].resets_in_secs, Some(3600));
        assert_eq!(snap.windows[1].label, "7d");
        assert_eq!(snap.windows[1].used_fraction, Some(0.18));
        assert_eq!(snap.windows[1].resets_in_secs, Some(86_400));
    }

    #[test]
    fn builds_snapshot_from_flat_shape() {
        let json = r#"{"primary": {"usedPercent": 50.0, "windowDurationMins": 300}}"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows.len(), 1);
        assert_eq!(snap.windows[0].used_fraction, Some(0.5));
        assert_eq!(snap.windows[0].resets_in_secs, None);
    }

    #[test]
    fn unknown_shape_degrades_to_empty_snapshot() {
        let usage: RawUsage = serde_json::from_str(r#"{"something_else": 1}"#).unwrap();
        let snap = build_snapshot(usage, 0);
        assert!(snap.windows.is_empty()); // degrade, never crash
    }

    #[test]
    fn missing_window_duration_uses_fallback_label() {
        let json = r#"{"primary": {"usedPercent": 10.0}}"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows[0].label, "primary");
    }

    #[test]
    fn used_percent_is_clamped() {
        let json = r#"{"primary": {"usedPercent": 250.0}}"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows[0].used_fraction, Some(1.0));
    }

    #[test]
    fn minutes_labels() {
        assert_eq!(minutes_label(Some(300), "x"), "5h");
        assert_eq!(minutes_label(Some(10080), "x"), "7d");
        assert_eq!(minutes_label(Some(90), "x"), "90m");
        assert_eq!(minutes_label(Some(1440), "x"), "1d");
        assert_eq!(minutes_label(None, "primary"), "primary");
        assert_eq!(minutes_label(Some(0), "x"), "x");
    }

    #[test]
    fn resets_at_number_forms() {
        // Relative seconds (small number).
        assert_eq!(
            resets_in_secs(Some(&RawResetsAt::Number(1800)), 1_800_000_000),
            Some(1800)
        );
        // Absolute epoch (large number): 3600s after "now".
        assert_eq!(
            resets_in_secs(Some(&RawResetsAt::Number(1_800_003_600)), 1_800_000_000),
            Some(3600)
        );
        // Past epoch clamps to zero rather than underflowing.
        assert_eq!(
            resets_in_secs(Some(&RawResetsAt::Number(1_700_000_000)), 1_800_000_000),
            Some(0)
        );
        // Negative is nonsense.
        assert_eq!(resets_in_secs(Some(&RawResetsAt::Number(-5)), 0), None);
        // Absent.
        assert_eq!(resets_in_secs(None, 0), None);
    }

    #[test]
    fn missing_credentials_error_is_detectable_and_secret_free() {
        // Contract the UI relies on: an absent auth.json surfaces a
        // Credential error whose message contains "not found".
        let mut path = std::env::temp_dir();
        path.push(format!(
            "quotapane-codex-{}-missing.json",
            std::process::id()
        ));
        let provider = CodexSubscription::new(path, DEFAULT_USER_AGENT);
        let err = provider.poll(&Egress::new(false)).unwrap_err();
        match err {
            ProviderError::Credential(msg) => {
                assert!(msg.contains("not found"), "message was: {msg}");
                assert!(!msg.contains(SYNTHETIC_TOKEN));
            }
            other => panic!("expected Credential, got: {other:?}"),
        }
    }

    #[test]
    fn malformed_credentials_file_is_a_clean_error() {
        use std::io::Write as _;
        let mut path = std::env::temp_dir();
        path.push(format!("quotapane-codex-{}-bad.json", std::process::id()));
        std::fs::File::create(&path)
            .unwrap()
            .write_all(b"{ not json")
            .unwrap();
        let provider = CodexSubscription::new(path.clone(), DEFAULT_USER_AGENT);
        let err = provider.poll(&Egress::new(false)).unwrap_err();
        assert!(matches!(err, ProviderError::Credential(_)));
        std::fs::remove_file(&path).unwrap();
    }
}
