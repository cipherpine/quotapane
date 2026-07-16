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
//! ## Schema (pinned by a live capture, 2026-07-15)
//! The endpoint is undocumented. A `usage-cli --debug-raw` capture pinned the
//! real shape: a top-level singular `rate_limit` object with
//! `primary_window`/`secondary_window`, each carrying `used_percent`,
//! `limit_window_seconds` (window DURATION in seconds), `reset_after_seconds`
//! (relative seconds until reset), and `reset_at` (absolute epoch fallback).
//! The DTOs below read only those fields. The response also contains account
//! PII (`user_id`, `account_id`, `email`) and a per-model
//! `additional_rate_limits` array — both are ignored by serde and never enter
//! a snapshot (the per-model breakdown is deferred to M5 depth). Anything
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

impl CodexSubscription {
    /// The one request this provider makes. Both [`Self::poll`] and
    /// [`Self::debug_raw_body`] go through here, so the debug dump is
    /// *guaranteed* to reflect the exact request normal polling sends
    /// (Ingress TB1 → Egress TB2). The token leaves the process only at the
    /// `http.get` call, wrapped in `Secret` until that moment.
    fn fetch(&self, http: &Egress) -> Result<crate::egress::EgressResponse, ProviderError> {
        // Ingress (TB1): load read-only, wrapped in Secret.
        let raw = load_credential_file(&self.credentials_path)
            .map_err(|e| ProviderError::Credential(e.to_string()))?;
        let creds = parse_auth(raw.expose())
            .ok_or_else(|| ProviderError::Credential("malformed auth.json".into()))?;

        // Egress (TB2): the ChatGPT-Account-Id header is sent when present,
        // matching the CLI.
        let mut headers: Vec<(&str, &str)> = vec![
            ("Content-Type", "application/json"),
            ("User-Agent", &self.user_agent),
        ];
        if let Some(account_id) = creds.account_id.as_deref() {
            headers.push(("ChatGPT-Account-Id", account_id));
        }
        Ok(http.get(HOST, PATH, Some(&creds.access_token), &headers)?)
    }

    /// Debug/diagnostic: perform the usage request and return the **raw**
    /// response as `"status: <code>\n<body>"`. Used by `usage-cli
    /// --debug-raw` to pin the endpoint's exact JSON shape without an ad-hoc
    /// token request outside the trust boundary. The body is provider usage
    /// data (percentages, timestamps) — non-secret; the bearer token and
    /// account id ride in request headers and never appear in the body.
    pub fn debug_raw_body(&self, http: &Egress) -> Result<String, ProviderError> {
        let resp = self.fetch(http)?;
        Ok(format!(
            "status: {}\n{}",
            resp.status,
            String::from_utf8_lossy(&resp.body)
        ))
    }
}

impl UsageProvider for CodexSubscription {
    fn id(&self) -> ProviderId {
        ProviderId::CodexSubscription
    }

    fn poll(&self, http: &Egress) -> Result<ProviderSnapshot, ProviderError> {
        let resp = self.fetch(http)?;

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

/// One rate-limit window. Fields pinned against a live capture:
/// `used_percent`, `limit_window_seconds` (window DURATION, seconds),
/// `reset_after_seconds` (relative seconds until reset), `reset_at`
/// (absolute epoch fallback). Everything optional: degrade, don't crash.
#[derive(Deserialize, Default)]
struct RawWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<u64>,
    reset_after_seconds: Option<i64>,
    reset_at: Option<i64>,
}

/// The `primary_window` / `secondary_window` pair nested under `rate_limit`.
#[derive(Deserialize, Default)]
struct RawRateLimit {
    primary_window: Option<RawWindow>,
    secondary_window: Option<RawWindow>,
}

/// Top-level response (verified 2026-07-15). Only the singular top-level
/// `rate_limit` is read; account PII and the per-model
/// `additional_rate_limits` array are ignored by serde (unknown fields) and
/// never enter a snapshot.
#[derive(Deserialize, Default)]
struct RawUsage {
    rate_limit: Option<RawRateLimit>,
}

/// Human label for a window from its duration in **seconds**:
/// `18000 → "5h"`, `604800 → "7d"`, `90 → "90s"`; `None`/`0 → fallback`.
fn seconds_label(secs: Option<u64>, fallback: &str) -> String {
    match secs {
        Some(s) if s > 0 && s % 86_400 == 0 => format!("{}d", s / 86_400),
        Some(s) if s > 0 && s % 3_600 == 0 => format!("{}h", s / 3_600),
        Some(s) if s > 0 && s % 60 == 0 => format!("{}m", s / 60),
        Some(s) if s > 0 => format!("{s}s"),
        _ => fallback.to_string(),
    }
}

/// Seconds until reset: prefer the relative `reset_after_seconds`, else
/// derive from the absolute `reset_at` epoch.
fn window_resets_in_secs(w: &RawWindow, now_unix_secs: u64) -> Option<u64> {
    if let Some(after) = w.reset_after_seconds {
        return Some(after.max(0) as u64);
    }
    w.reset_at
        .map(|at| at.saturating_sub(now_unix_secs as i64).max(0) as u64)
}

/// Build a normalized, non-secret snapshot from a usage response.
fn build_snapshot(usage: RawUsage, now_unix_secs: u64) -> ProviderSnapshot {
    let rate_limit = usage.rate_limit.unwrap_or_default();

    let mut windows = Vec::new();
    for (win, fallback) in [
        (rate_limit.primary_window, "primary"),
        (rate_limit.secondary_window, "secondary"),
    ] {
        if let Some(w) = win {
            windows.push(QuotaWindow {
                label: seconds_label(w.limit_window_seconds, fallback),
                used_fraction: w.used_percent.map(|u| (u / 100.0).clamp(0.0, 1.0)),
                resets_in_secs: window_resets_in_secs(&w, now_unix_secs),
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
        // Structurally identical to the 2026-07-15 `--debug-raw` capture:
        // top-level singular `rate_limit`, `primary_window`/`secondary_window`,
        // `used_percent` / `limit_window_seconds` / `reset_after_seconds`, and
        // ignored PII + `additional_rate_limits`. Regression test for the M3
        // live-run mismatch (no windows rendered).
        let now: u64 = 1_784_000_000;
        let json = format!(
            r#"{{
                "user_id":"user-REDACTED","account_id":"user-REDACTED","email":"x@example.com",
                "plan_type":"prolite",
                "rate_limit":{{
                    "allowed":true,"limit_reached":false,
                    "primary_window":  {{"used_percent":25,"limit_window_seconds":18000, "reset_after_seconds":3600,  "reset_at":{}}},
                    "secondary_window":{{"used_percent":18,"limit_window_seconds":604800,"reset_after_seconds":86400, "reset_at":{}}}
                }},
                "additional_rate_limits":[{{"limit_name":"GPT-5.3-Codex-Spark","rate_limit":{{"primary_window":{{"used_percent":0,"limit_window_seconds":604800}}}}}}],
                "rate_limit_reached_type":null
            }}"#,
            now + 3600,
            now + 86_400
        );
        let usage: RawUsage = serde_json::from_str(&json).unwrap();
        let snap = build_snapshot(usage, now);

        assert_eq!(snap.provider, ProviderId::CodexSubscription);
        // Only the top-level rate_limit's two windows; additional_rate_limits
        // are intentionally not emitted in M3.
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].label, "5h"); // 18000s
        assert_eq!(snap.windows[0].used_fraction, Some(0.25));
        assert_eq!(snap.windows[0].resets_in_secs, Some(3600)); // reset_after_seconds
        assert_eq!(snap.windows[1].label, "7d"); // 604800s
        assert_eq!(snap.windows[1].used_fraction, Some(0.18));
        assert_eq!(snap.windows[1].resets_in_secs, Some(86_400));
    }

    #[test]
    fn single_window_secondary_null() {
        // The exact shape Justin's account returned: only primary_window,
        // secondary null.
        let json = r#"{"rate_limit":{"primary_window":{"used_percent":0,"limit_window_seconds":604800,"reset_after_seconds":604060},"secondary_window":null}}"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows.len(), 1);
        assert_eq!(snap.windows[0].label, "7d");
        assert_eq!(snap.windows[0].used_fraction, Some(0.0));
        assert_eq!(snap.windows[0].resets_in_secs, Some(604_060));
    }

    #[test]
    fn reset_at_used_when_reset_after_absent() {
        let now: u64 = 1_784_000_000;
        let json = format!(
            r#"{{"rate_limit":{{"primary_window":{{"used_percent":5,"limit_window_seconds":18000,"reset_at":{}}}}}}}"#,
            now + 1800
        );
        let usage: RawUsage = serde_json::from_str(&json).unwrap();
        let snap = build_snapshot(usage, now);
        assert_eq!(snap.windows[0].resets_in_secs, Some(1800));
    }

    #[test]
    fn unknown_shape_degrades_to_empty_snapshot() {
        let usage: RawUsage = serde_json::from_str(r#"{"something_else": 1}"#).unwrap();
        let snap = build_snapshot(usage, 0);
        assert!(snap.windows.is_empty()); // degrade, never crash
    }

    #[test]
    fn missing_window_duration_uses_fallback_label() {
        let json = r#"{"rate_limit":{"primary_window":{"used_percent":10.0}}}"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows[0].label, "primary");
    }

    #[test]
    fn used_percent_is_clamped() {
        let json = r#"{"rate_limit":{"primary_window":{"used_percent":250.0,"limit_window_seconds":18000}}}"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows[0].used_fraction, Some(1.0));
    }

    #[test]
    fn seconds_labels() {
        assert_eq!(seconds_label(Some(18_000), "x"), "5h");
        assert_eq!(seconds_label(Some(604_800), "x"), "7d");
        assert_eq!(seconds_label(Some(5_400), "x"), "90m");
        assert_eq!(seconds_label(Some(45), "x"), "45s");
        assert_eq!(seconds_label(None, "primary"), "primary");
        assert_eq!(seconds_label(Some(0), "x"), "x");
    }

    #[test]
    fn window_resets_prefers_relative_then_epoch() {
        // reset_after_seconds wins when present (even if reset_at also set).
        let w = RawWindow {
            reset_after_seconds: Some(1800),
            reset_at: Some(9_999_999_999),
            ..Default::default()
        };
        assert_eq!(window_resets_in_secs(&w, 1_784_000_000), Some(1800));
        // Negative relative clamps to zero.
        let w = RawWindow {
            reset_after_seconds: Some(-5),
            ..Default::default()
        };
        assert_eq!(window_resets_in_secs(&w, 0), Some(0));
        // Falls back to reset_at epoch when reset_after absent.
        let w = RawWindow {
            reset_at: Some(1_784_003_600),
            ..Default::default()
        };
        assert_eq!(window_resets_in_secs(&w, 1_784_000_000), Some(3600));
        // Neither → None.
        assert_eq!(window_resets_in_secs(&RawWindow::default(), 0), None);
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
