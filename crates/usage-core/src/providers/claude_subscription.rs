//! `ClaudeSubscription` — the first provider (M1).
//!
//! Reads the local Claude Code OAuth token (read-only) and queries the
//! subscription usage endpoint the official `/usage` command uses:
//! `GET https://api.anthropic.com/api/oauth/usage`. The token travels only
//! through the [`Egress`] chokepoint, only to `api.anthropic.com`, and is
//! held in a [`Secret`] the whole time (SECURITY.md invariants 2, 3, 6).
//!
//! ## Undocumented endpoint — honest posture
//! This endpoint is not part of any published contract. QuotaPane must send
//! `User-Agent: claude-code/<version>`; without it the request lands in an
//! aggressively rate-limited bucket. That means QuotaPane presents itself as
//! the official client — disclosed in README.md and SECURITY.md. The schema
//! may change without notice, so every field is optional and parsing **fails
//! closed**: a missing or renamed field degrades the snapshot rather than
//! leaking or crashing (THREAT_MODEL.md R4).
//!
//! ## Scope (M1)
//! The primary usage-endpoint path is implemented and tested. The
//! Messages-API rate-limit-header *fallback* named in the spec requires a
//! second, verified endpoint and is deferred (same "verify, don't invent"
//! rule applied to the Codex host): a 429 surfaces [`ProviderError::RateLimited`]
//! with any `retry-after` hint rather than a guessed secondary call.

use crate::credentials::{load_credential_file, Secret};
use crate::egress::Egress;
use crate::model::{ProviderId, ProviderSnapshot, QuotaWindow, SnapshotSource};
use crate::providers::time::parse_rfc3339_to_unix;
use crate::providers::{Cadence, ProviderError, UsageProvider};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

/// The Anthropic host (must be on the egress allowlist).
const HOST: &str = "api.anthropic.com";
/// The subscription usage path.
const PATH: &str = "/api/oauth/usage";
/// The OAuth beta header the endpoint requires.
const ANTHROPIC_BETA: &str = "oauth-2025-04-20";

/// Provider for Claude Code subscription quota.
pub struct ClaudeSubscription {
    /// Path to `~/.claude/.credentials.json` (or a test fixture).
    credentials_path: PathBuf,
    /// The `claude-code` client version string sent in `User-Agent`. Must be
    /// a real Claude Code version to avoid the throttled bucket; the caller
    /// supplies it (detecting it via the official CLI is deferred to M2/M3).
    client_version: String,
}

impl ClaudeSubscription {
    /// Construct with an explicit credential path and client version.
    pub fn new(credentials_path: PathBuf, client_version: impl Into<String>) -> Self {
        ClaudeSubscription {
            credentials_path,
            client_version: client_version.into(),
        }
    }

    /// Construct using the default `~/.claude/.credentials.json` path.
    ///
    /// Returns `None` if no home directory can be resolved.
    pub fn with_default_path(client_version: impl Into<String>) -> Option<Self> {
        crate::credentials::claude_credentials_path().map(|p| Self::new(p, client_version))
    }
}

impl ClaudeSubscription {
    /// The one request this provider makes. Both [`UsageProvider::poll`] and
    /// [`Self::debug_raw_body`] go through here, so the debug dump is
    /// *guaranteed* to reflect the exact request normal polling sends
    /// (Ingress TB1 → Egress TB2). The token leaves the process only at the
    /// `http.get` call, wrapped in `Secret` until that moment.
    fn fetch(&self, http: &Egress) -> Result<crate::egress::EgressResponse, ProviderError> {
        // Ingress (TB1): load read-only, wrapped in Secret.
        let raw = load_credential_file(&self.credentials_path)
            .map_err(|e| ProviderError::Credential(e.to_string()))?;
        let creds = parse_credentials(raw.expose())
            .ok_or_else(|| ProviderError::Credential("malformed .credentials.json".into()))?;

        // Fail before any network call if the token is known-expired.
        if let Some(expires_at_ms) = creds.expires_at_unix_ms {
            if expires_at_ms <= now_unix_millis() {
                return Err(ProviderError::TokenExpired);
            }
        }

        // Egress (TB2): the token leaves the process only here.
        let user_agent = format!("claude-code/{}", self.client_version);
        Ok(http.get(
            HOST,
            PATH,
            Some(&creds.access_token),
            &[
                ("anthropic-beta", ANTHROPIC_BETA),
                ("Content-Type", "application/json"),
                ("User-Agent", &user_agent),
            ],
        )?)
    }

    /// Debug/diagnostic: perform the usage request and return the **raw**
    /// response as `"status: <code>\n<body>"`. Used by `quotapane-cli --debug-raw`
    /// to pin the endpoint's exact JSON shape without an ad-hoc token request
    /// outside the trust boundary — the same "verify, don't invent" tool that
    /// pinned the Codex schema. The body is provider usage data (utilization
    /// percentages, reset timestamps) — non-secret; the bearer token rides in
    /// a request header and never appears in the body.
    pub fn debug_raw_body(&self, http: &Egress) -> Result<String, ProviderError> {
        let resp = self.fetch(http)?;
        Ok(format!(
            "status: {}\n{}",
            resp.status,
            String::from_utf8_lossy(&resp.body)
        ))
    }
}

impl UsageProvider for ClaudeSubscription {
    fn id(&self) -> ProviderId {
        ProviderId::ClaudeSubscription
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
        // The endpoint is safe at >=180s; Normal (~7 min) stays well clear.
        Cadence::Normal
    }
}

/// Parsed credentials, with the token re-wrapped in [`Secret`].
struct ParsedCredentials {
    access_token: Secret<String>,
    /// OAuth expiry in Unix epoch milliseconds, if present.
    expires_at_unix_ms: Option<i64>,
}

/// Raw shape of `~/.claude/.credentials.json`. The token is deserialized into
/// a plain `String` only transiently, then moved into a [`Secret`] (its buffer
/// is not copied) so it is wiped on drop.
#[derive(Deserialize)]
struct RawCredentials {
    #[serde(rename = "claudeAiOauth")]
    claude_ai_oauth: RawOauth,
}

#[derive(Deserialize)]
struct RawOauth {
    #[serde(rename = "accessToken")]
    access_token: String,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

/// Parse the credential JSON. Returns `None` on any structural problem; never
/// includes token bytes in an error (there is no error value at all).
fn parse_credentials(raw_json: &str) -> Option<ParsedCredentials> {
    let parsed: RawCredentials = serde_json::from_str(raw_json).ok()?;
    Some(ParsedCredentials {
        // Moves the String's heap buffer into Secret — no additional copy.
        access_token: Secret::new(parsed.claude_ai_oauth.access_token),
        expires_at_unix_ms: parsed.claude_ai_oauth.expires_at,
    })
}

/// Raw usage response. Every field optional: the endpoint is undocumented and
/// may change, so parsing degrades instead of failing hard.
///
/// `limits` is the generalized array the endpoint grew (observed 2026-07-29);
/// the legacy `seven_day_opus`/`seven_day_sonnet` keys still exist but are now
/// null on current accounts, so both shapes are parsed and the legacy pair is
/// kept as the fallback (see [`build_snapshot`]).
#[derive(Deserialize, Default)]
struct RawUsage {
    five_hour: Option<RawWindow>,
    seven_day: Option<RawWindow>,
    seven_day_opus: Option<RawWindow>,
    seven_day_sonnet: Option<RawWindow>,
    limits: Option<Vec<RawLimit>>,
}

#[derive(Deserialize)]
struct RawWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

/// One entry of the generalized `limits` array.
///
/// Only the four fields the snapshot actually uses are declared. Deliberately
/// **not** parsed: `group`, `severity`, `is_active`, `scope.surface`, and — most
/// importantly — `scope.model.id`. The field-ignoring defense is structural:
/// serde drops every key with no matching field, so a value that has nowhere to
/// land cannot reach a snapshot, a log, or the CLI's JSON. Declaring a field
/// "for completeness" would quietly undo that, so don't.
///
/// `kind` earns its field (M8): it is the only thing in the response that says
/// how long a limit's window *is*, which the pace markers need. It is consumed
/// by [`duration_from_kind`] into a plain number of seconds and is never itself
/// stored — no provider vocabulary reaches a snapshot through it. That is the
/// bar for declaring anything here: a field the snapshot genuinely needs, read
/// for a derived value, not carried along.
#[derive(Deserialize)]
struct RawLimit {
    kind: Option<String>,
    percent: Option<f64>,
    resets_at: Option<String>,
    scope: Option<RawScope>,
}

/// A five-hour window, in seconds — Claude's "session" limit.
const FIVE_HOUR_SECS: u64 = 18_000;
/// A weekly window, in seconds.
const WEEKLY_SECS: u64 = 604_800;

/// Window length in seconds for a `limits[].kind`, or `None` for a kind this
/// crate has not verified.
///
/// The endpoint states a limit's *kind*, never its duration, so this mapping is
/// the derivation — and it stays a closed list plus one family rather than a
/// clever parse. Observed vocabulary (2026-07-29): `session` is the five-hour
/// window; `weekly_all` and `weekly_scoped` are both weekly, which is why the
/// weekly side matches the family prefix — the endpoint already ships two
/// members of it, so a third would be weekly too, while a genuinely new kind
/// (`monthly_…`, say) must fall through to `None`.
///
/// `None` is the honest answer for an unrecognized kind: it costs the pace tick
/// on that row and nothing else. Guessing would put a marker on a bar at a
/// position derived from a duration the provider never stated.
fn duration_from_kind(kind: Option<&str>) -> Option<u64> {
    match kind? {
        "session" | "five_hour" => Some(FIVE_HOUR_SECS),
        k if k == "weekly" || k.starts_with("weekly_") => Some(WEEKLY_SECS),
        _ => None,
    }
}

/// `limits[].scope` — an unscoped (subscription-wide) entry has `null` here.
#[derive(Deserialize)]
struct RawScope {
    model: Option<RawScopeModel>,
}

/// `limits[].scope.model` — presence of `display_name` is what marks an entry
/// as per-model. `id` is deliberately absent (see [`RawLimit`]).
#[derive(Deserialize)]
struct RawScopeModel {
    display_name: Option<String>,
}

/// Percent (0–100) → the `used_fraction` convention (0.0–1.0), clamped.
///
/// One place for the conversion so the legacy-window path and the `limits`
/// path cannot drift into two conventions.
fn percent_to_fraction(percent: Option<f64>) -> Option<f64> {
    percent.map(|p| (p / 100.0).clamp(0.0, 1.0))
}

/// Seconds until an RFC 3339 reset timestamp, never negative. Shared by both
/// per-model paths for the same reason as [`percent_to_fraction`].
fn resets_in_secs(resets_at: Option<&str>, now_unix_secs: u64) -> Option<u64> {
    resets_at
        .and_then(parse_rfc3339_to_unix)
        .map(|reset| reset.saturating_sub(now_unix_secs as i64).max(0) as u64)
}

/// Normalize one raw window into a [`QuotaWindow`] under `label`.
///
/// `duration_secs` is passed in rather than derived here: for the top-level
/// windows the JSON *key* is the kind (`five_hour`, `seven_day`), so the caller
/// is the only place that knows it.
fn to_quota_window(
    label: &str,
    w: RawWindow,
    now_unix_secs: u64,
    duration_secs: Option<u64>,
) -> QuotaWindow {
    QuotaWindow {
        label: label.to_string(),
        used_fraction: percent_to_fraction(w.utilization),
        resets_in_secs: resets_in_secs(w.resets_at.as_deref(), now_unix_secs),
        duration_secs,
    }
}

/// Build a normalized, non-secret snapshot from a usage response.
///
/// Windows split by *scope*, not by kind. `five_hour`/`seven_day` describe the
/// subscription as a whole and are the headline rows — they stay the headline
/// source even now that the `limits` array duplicates them as its unscoped
/// `session`/`weekly_all` entries, because they are the shape this provider
/// has always verified against.
///
/// `per_model` prefers the generalized `limits` array: an entry is per-model
/// exactly when it carries a `scope.model.display_name`, and that name becomes
/// the row's label **verbatim** — the provider's own name for a model is the
/// honest one, and normalizing it here would invent a vocabulary the UI would
/// then have to parse back (same rule as the Codex provider's `limit_name`).
/// If `limits` is absent, or present but yields no model-scoped entries, the
/// legacy `seven_day_opus`/`seven_day_sonnet` keys populate `per_model` as
/// before, under their unchanged `"7d-opus"`/`"7d-sonnet"` labels.
fn build_snapshot(usage: RawUsage, now_unix_secs: u64) -> ProviderSnapshot {
    let mut windows = Vec::new();
    // The key names *are* the kinds here, so the durations are known outright:
    // `five_hour` is a five-hour window and `seven_day` a weekly one. Written
    // as literals beside their keys rather than routed through
    // `duration_from_kind`, because inventing the strings "session"/"weekly_all"
    // to look them up would be pretending the endpoint said something it didn't.
    for (label, win, duration) in [
        ("5h", usage.five_hour, FIVE_HOUR_SECS),
        ("7d", usage.seven_day, WEEKLY_SECS),
    ] {
        if let Some(w) = win {
            windows.push(to_quota_window(label, w, now_unix_secs, Some(duration)));
        }
    }

    let mut per_model: Vec<QuotaWindow> = usage
        .limits
        .unwrap_or_default()
        .into_iter()
        .filter_map(|limit| {
            let duration_secs = duration_from_kind(limit.kind.as_deref());
            // Unscoped entries (`scope: null`) duplicate the headline windows
            // and are dropped here; only a model-scoped one becomes a row.
            let label = limit.scope?.model?.display_name?;
            Some(QuotaWindow {
                label,
                used_fraction: percent_to_fraction(limit.percent),
                resets_in_secs: resets_in_secs(limit.resets_at.as_deref(), now_unix_secs),
                duration_secs,
            })
        })
        .collect();

    if per_model.is_empty() {
        // The legacy pair is weekly by construction — the keys say `seven_day`.
        for (label, win) in [
            ("7d-opus", usage.seven_day_opus),
            ("7d-sonnet", usage.seven_day_sonnet),
        ] {
            if let Some(w) = win {
                per_model.push(to_quota_window(label, w, now_unix_secs, Some(WEEKLY_SECS)));
            }
        }
    }

    ProviderSnapshot {
        provider: ProviderId::ClaudeSubscription,
        taken_at_unix_secs: now_unix_secs,
        windows,
        per_model,
        // Claude reports no reset-credit equivalent.
        reset_credits: None,
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

fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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

    const SYNTHETIC_TOKEN: &str = "synthetic-oauth-QQQ-not-a-real-token";

    fn creds_json(expires_at: Option<i64>) -> String {
        let exp = match expires_at {
            Some(v) => v.to_string(),
            None => "null".to_string(),
        };
        format!(
            r#"{{"claudeAiOauth":{{"accessToken":"{SYNTHETIC_TOKEN}","refreshToken":"rt-synthetic","expiresAt":{exp},"scopes":["user:inference"]}}}}"#
        )
    }

    #[test]
    fn parses_credentials_and_wraps_token_redacted() {
        let parsed = parse_credentials(&creds_json(Some(1_800_000_000_000))).unwrap();
        assert_eq!(parsed.access_token.expose(), SYNTHETIC_TOKEN);
        assert_eq!(parsed.expires_at_unix_ms, Some(1_800_000_000_000));
        // Redaction still holds on the re-wrapped token.
        assert!(!format!("{:?}", parsed.access_token).contains("QQQ"));
    }

    #[test]
    fn malformed_credentials_return_none_without_secrets() {
        assert!(parse_credentials("not json").is_none());
        assert!(parse_credentials(r#"{"claudeAiOauth":{}}"#).is_none());
    }

    #[test]
    fn builds_snapshot_from_usage_json() {
        let json = r#"{
            "five_hour": {"utilization": 33.0, "resets_at": "2026-04-11T07:00:00.528743+00:00"},
            "seven_day": {"utilization": 80.0, "resets_at": "2026-04-18T07:00:00+00:00"},
            "seven_day_opus": null,
            "seven_day_sonnet": {"utilization": 12.5, "resets_at": null}
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        // now = 2026-04-11T06:00:00Z → 5h resets in 3600s.
        let now = parse_rfc3339_to_unix("2026-04-11T06:00:00Z").unwrap() as u64;
        let snap = build_snapshot(usage, now);

        assert_eq!(snap.provider, ProviderId::ClaudeSubscription);
        assert_eq!(snap.source, SnapshotSource::UsageEndpoint);

        // Headline windows: the subscription-wide 5h and 7d, and only those.
        assert_eq!(snap.windows.len(), 2);
        assert_eq!(snap.windows[0].label, "5h");
        assert_eq!(snap.windows[1].label, "7d");

        let five = &snap.windows[0];
        assert_eq!(five.used_fraction, Some(0.33));
        assert_eq!(five.resets_in_secs, Some(3600));

        let seven = &snap.windows[1];
        assert_eq!(seven.used_fraction, Some(0.80));

        // Per-model: opus was null → skipped, so sonnet alone. Labels are
        // unchanged by the split.
        assert_eq!(snap.per_model.len(), 1);
        let sonnet = &snap.per_model[0];
        assert_eq!(sonnet.label, "7d-sonnet");
        assert_eq!(sonnet.used_fraction, Some(0.125));
        assert_eq!(sonnet.resets_in_secs, None); // resets_at was null

        // The per-model rows must not also appear in the headline list.
        assert!(!snap.windows.iter().any(|w| w.label.contains("opus")));
        assert!(!snap.windows.iter().any(|w| w.label.contains("sonnet")));
    }

    #[test]
    fn both_per_model_windows_are_split_out() {
        // All four windows present: two headline, two per-model.
        let json = r#"{
            "five_hour": {"utilization": 10.0, "resets_at": null},
            "seven_day": {"utilization": 20.0, "resets_at": null},
            "seven_day_opus": {"utilization": 30.0, "resets_at": null},
            "seven_day_sonnet": {"utilization": 40.0, "resets_at": null}
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);

        let headline: Vec<&str> = snap.windows.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(headline, vec!["5h", "7d"]);

        let per_model: Vec<&str> = snap.per_model.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(per_model, vec!["7d-opus", "7d-sonnet"]);

        assert_eq!(snap.per_model[0].used_fraction, Some(0.30));
        assert_eq!(snap.per_model[1].used_fraction, Some(0.40));
    }

    #[test]
    fn no_per_model_windows_yields_an_empty_vec() {
        // Headline windows only — per_model is empty, never an error.
        let json = r#"{"five_hour":{"utilization":10.0},"seven_day":{"utilization":20.0}}"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows.len(), 2);
        assert!(snap.per_model.is_empty());
    }

    #[test]
    fn missing_fields_degrade_not_crash() {
        // Empty object → a valid, empty snapshot (fail closed, not panic).
        let usage: RawUsage = serde_json::from_str("{}").unwrap();
        let snap = build_snapshot(usage, 0);
        assert!(snap.windows.is_empty());
        assert!(snap.per_model.is_empty());

        // Utilization present, resets_at absent.
        let usage: RawUsage =
            serde_json::from_str(r#"{"five_hour":{"utilization":50.0}}"#).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows[0].used_fraction, Some(0.5));
        assert_eq!(snap.windows[0].resets_in_secs, None);
    }

    #[test]
    fn utilization_is_clamped() {
        let usage: RawUsage =
            serde_json::from_str(r#"{"five_hour":{"utilization":150.0}}"#).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows[0].used_fraction, Some(1.0));
    }

    #[test]
    fn retry_after_parsed_from_headers() {
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("retry-after".to_string(), "120".to_string()),
        ];
        assert_eq!(retry_after_secs(&headers), Some(120));
        assert_eq!(retry_after_secs(&[]), None);
    }

    #[test]
    fn expired_token_is_detected_before_network() {
        // A provider pointed at a fixture with a past expiry must not dial.
        // We exercise the expiry branch directly via a fixture file.
        use std::io::Write as _;
        let mut path = std::env::temp_dir();
        path.push(format!("quotapane-cs-{}-creds.json", std::process::id()));
        std::fs::File::create(&path)
            .unwrap()
            .write_all(creds_json(Some(1)).as_bytes()) // expires 1ms after epoch → long past
            .unwrap();

        let provider = ClaudeSubscription::new(path.clone(), "2.0.0");
        let egress = Egress::new(false);
        let err = provider.poll(&egress).unwrap_err();
        assert!(matches!(err, ProviderError::TokenExpired));

        // `debug_raw_body` shares `fetch`, so it must refuse a known-expired
        // token before dialing too — the diagnostic path is not a way around
        // the pre-network expiry check.
        let err = provider.debug_raw_body(&egress).unwrap_err();
        assert!(matches!(err, ProviderError::TokenExpired));

        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn debug_raw_missing_credentials_is_a_clean_secret_free_error() {
        // Mirrors the Codex provider's contract: an absent credential file
        // surfaces a Credential error containing "not found", never a panic
        // and never token bytes.
        let mut path = std::env::temp_dir();
        path.push(format!("quotapane-cs-{}-missing.json", std::process::id()));
        let provider = ClaudeSubscription::new(path, "2.0.0");
        let err = provider.debug_raw_body(&Egress::new(false)).unwrap_err();
        match err {
            ProviderError::Credential(msg) => {
                assert!(msg.contains("not found"), "message was: {msg}");
                assert!(!msg.contains(SYNTHETIC_TOKEN));
            }
            other => panic!("expected Credential, got: {other:?}"),
        }
    }

    // --- generalized `limits` array (M7a) ---

    // Synthetic PII, deliberately distinctive so the absence assertions below
    // are meaningful. `.invalid` is the reserved never-resolvable TLD.
    const PII_USER_ID: &str = "user-SYNTHETIC-PII-4b8e";
    const PII_ACCOUNT_ID: &str = "acct-SYNTHETIC-PII-2d61";
    const PII_EMAIL: &str = "not-real-person@example.invalid";
    const PII_MODEL_ID: &str = "model-SYNTHETIC-PII-a70f";

    #[test]
    fn limits_array_supplies_per_model_and_carries_no_pii() {
        // Mirrors the shape observed 2026-07-29: the legacy per-model keys are
        // null, the headline keys remain, and `limits` carries two unscoped
        // entries (duplicating the headline) plus one model-scoped entry.
        // Every value here is synthetic.
        let json = format!(
            r#"{{
                "user_id":"{PII_USER_ID}","account_id":"{PII_ACCOUNT_ID}","email":"{PII_EMAIL}",
                "five_hour": {{"utilization": 15.0, "resets_at": "2026-04-11T07:00:00+00:00"}},
                "seven_day": {{"utilization": 42.0, "resets_at": "2026-04-18T07:00:00+00:00"}},
                "seven_day_opus": null,
                "seven_day_sonnet": null,
                "limits":[
                  {{"kind":"session","group":"session","percent":15,"severity":"low",
                    "resets_at":"2026-04-11T07:00:00+00:00","scope":null,"is_active":true}},
                  {{"kind":"weekly_all","group":"weekly","percent":42,"severity":"low",
                    "resets_at":"2026-04-18T07:00:00+00:00","scope":null,"is_active":true}},
                  {{"kind":"weekly_scoped","group":"weekly","percent":40,"severity":"low",
                    "resets_at":"2026-04-11T07:00:00+00:00",
                    "scope":{{"model":{{"id":"{PII_MODEL_ID}","display_name":"TestModel"}},
                              "surface":null}},
                    "is_active":false}}
                ]
            }}"#
        );
        // Guard against a vacuous test: the fixture must really carry the
        // values whose absence is asserted below.
        for pii in [PII_USER_ID, PII_ACCOUNT_ID, PII_EMAIL, PII_MODEL_ID] {
            assert!(json.contains(pii), "fixture lost its PII: {pii}");
        }

        let usage: RawUsage = serde_json::from_str(&json).unwrap();
        // now = 2026-04-11T06:00:00Z → the 07:00 resets are 3600s out.
        let now = parse_rfc3339_to_unix("2026-04-11T06:00:00Z").unwrap() as u64;
        let snap = build_snapshot(usage, now);

        // Headline stays the legacy top-level pair; the unscoped `limits`
        // entries that duplicate it must not become extra rows.
        let headline: Vec<&str> = snap.windows.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(headline, vec!["5h", "7d"]);
        assert_eq!(snap.windows[0].used_fraction, Some(0.15));
        assert_eq!(snap.windows[1].used_fraction, Some(0.42));

        // Exactly the one model-scoped entry, labeled with `display_name`
        // verbatim.
        assert_eq!(snap.per_model.len(), 1);
        assert_eq!(snap.per_model[0].label, "TestModel");
        assert_eq!(snap.per_model[0].used_fraction, Some(0.40));
        assert_eq!(snap.per_model[0].resets_in_secs, Some(3600));

        // Positive absence check: no PII value — including the model `id`
        // sitting right beside the `display_name` we do read — appears in the
        // snapshot's `Debug` or in the JSON the CLI emits. This fails the
        // moment anyone declares a field for one, which is the point.
        let debug = format!("{snap:?}");
        let json_out = serde_json::to_string(&snap).unwrap();
        for pii in [PII_USER_ID, PII_ACCOUNT_ID, PII_EMAIL, PII_MODEL_ID] {
            assert!(!debug.contains(pii), "PII leaked into Debug: {pii}");
            assert!(!json_out.contains(pii), "PII leaked into JSON: {pii}");
        }
        // The local parts alone must not survive either.
        assert!(!debug.contains("SYNTHETIC-PII"));
        assert!(!debug.contains("example.invalid"));
    }

    #[test]
    fn claude_snapshots_never_carry_reset_credits() {
        // Reset credits are a Codex concept; Claude has no equivalent, so the
        // field stays None and the window renders nothing for it. Asserted on
        // a fully-populated response so this cannot pass vacuously.
        let json = r#"{
            "five_hour": {"utilization": 15.0},
            "seven_day": {"utilization": 42.0},
            "limits":[{"percent":40,"scope":{"model":{"display_name":"TestModel"}}}]
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert!(!snap.windows.is_empty());
        assert_eq!(snap.per_model.len(), 1);
        assert!(snap.reset_credits.is_none());
    }

    #[test]
    fn limits_absent_falls_back_to_legacy_per_model_keys() {
        // Accounts still on the old shape send no `limits` at all.
        let json = r#"{
            "five_hour": {"utilization": 10.0, "resets_at": null},
            "seven_day": {"utilization": 20.0, "resets_at": null},
            "seven_day_opus": {"utilization": 30.0, "resets_at": null},
            "seven_day_sonnet": {"utilization": 40.0, "resets_at": null}
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);

        let per_model: Vec<&str> = snap.per_model.iter().map(|w| w.label.as_str()).collect();
        assert_eq!(per_model, vec!["7d-opus", "7d-sonnet"]);
        assert_eq!(snap.per_model[0].used_fraction, Some(0.30));
        assert_eq!(snap.per_model[1].used_fraction, Some(0.40));
    }

    #[test]
    fn limits_without_model_scope_yields_empty_per_model() {
        // `limits` present but every entry unscoped, and the legacy keys null
        // (the 2026-07-29 reality for an account with no model-scoped quota):
        // per_model is empty, never an error and never a duplicated headline.
        let json = r#"{
            "five_hour": {"utilization": 10.0},
            "seven_day": {"utilization": 20.0},
            "seven_day_opus": null,
            "seven_day_sonnet": null,
            "limits":[
              {"kind":"session","group":"session","percent":10,"scope":null},
              {"kind":"weekly_all","group":"weekly","percent":20,"scope":{"model":null,"surface":null}}
            ]
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);

        assert_eq!(snap.windows.len(), 2);
        assert!(snap.per_model.is_empty());
    }

    #[test]
    fn model_scoped_limits_win_over_the_legacy_keys() {
        // Both shapes populated at once: the new array is authoritative, so
        // the legacy fallback must not append duplicate rows behind it.
        let json = r#"{
            "seven_day_opus": {"utilization": 30.0},
            "seven_day_sonnet": {"utilization": 40.0},
            "limits":[
              {"percent":55,"scope":{"model":{"display_name":"TestModel"}}}
            ]
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);

        assert_eq!(snap.per_model.len(), 1);
        assert_eq!(snap.per_model[0].label, "TestModel");
        assert_eq!(snap.per_model[0].used_fraction, Some(0.55));
    }

    // --- window duration (M8) ---

    #[test]
    fn kind_to_duration_is_a_closed_list_plus_the_weekly_family() {
        // The verified kinds.
        assert_eq!(duration_from_kind(Some("session")), Some(18_000));
        assert_eq!(duration_from_kind(Some("weekly_all")), Some(604_800));
        assert_eq!(duration_from_kind(Some("weekly_scoped")), Some(604_800));
        // The family, so a further weekly member needs no code change.
        assert_eq!(duration_from_kind(Some("weekly")), Some(604_800));
        assert_eq!(
            duration_from_kind(Some("weekly_something_new")),
            Some(604_800)
        );
        // Anything else is unknown, and unknown means None — not a guess. In
        // particular a kind that merely *contains* "weekly" is not a match.
        assert_eq!(duration_from_kind(Some("monthly_all")), None);
        assert_eq!(duration_from_kind(Some("not_weekly_at_all")), None);
        assert_eq!(duration_from_kind(Some("")), None);
        assert_eq!(duration_from_kind(None), None);
    }

    #[test]
    fn headline_windows_carry_their_durations() {
        // The key name is the kind: `five_hour` is 5h, `seven_day` is weekly.
        let json = r#"{
            "five_hour": {"utilization": 33.0, "resets_at": null},
            "seven_day": {"utilization": 80.0, "resets_at": null}
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.windows[0].label, "5h");
        assert_eq!(snap.windows[0].duration_secs, Some(18_000));
        assert_eq!(snap.windows[1].label, "7d");
        assert_eq!(snap.windows[1].duration_secs, Some(604_800));
    }

    #[test]
    fn per_model_duration_comes_from_the_limit_kind() {
        // Three model-scoped entries: a weekly kind, a five-hour kind, and one
        // whose kind this crate has never seen — which must degrade to None
        // rather than inherit a neighbour's duration.
        let json = r#"{
            "limits":[
              {"kind":"weekly_scoped","percent":40,
               "scope":{"model":{"display_name":"WeeklyModel"}}},
              {"kind":"session","percent":10,
               "scope":{"model":{"display_name":"SessionModel"}}},
              {"kind":"fortnightly_scoped","percent":5,
               "scope":{"model":{"display_name":"StrangeModel"}}},
              {"kind":"missing_kind_entirely","percent":5,
               "scope":{"model":{"display_name":"NoKindModel"}}}
            ]
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);

        assert_eq!(snap.per_model.len(), 4);
        assert_eq!(snap.per_model[0].label, "WeeklyModel");
        assert_eq!(snap.per_model[0].duration_secs, Some(604_800));
        assert_eq!(snap.per_model[1].label, "SessionModel");
        assert_eq!(snap.per_model[1].duration_secs, Some(18_000));
        assert_eq!(snap.per_model[2].label, "StrangeModel");
        assert_eq!(snap.per_model[2].duration_secs, None);
        assert_eq!(snap.per_model[3].label, "NoKindModel");
        assert_eq!(snap.per_model[3].duration_secs, None);
    }

    #[test]
    fn per_model_entry_without_a_kind_has_no_duration() {
        // An entry that omits `kind` altogether: the row still renders, just
        // without a pace marker.
        let json = r#"{
            "limits":[{"percent":40,"scope":{"model":{"display_name":"TestModel"}}}]
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.per_model[0].used_fraction, Some(0.40));
        assert_eq!(snap.per_model[0].duration_secs, None);
    }

    #[test]
    fn legacy_per_model_fallback_rows_are_weekly() {
        // `seven_day_opus` / `seven_day_sonnet` say weekly in their key names.
        let json = r#"{
            "seven_day_opus": {"utilization": 30.0},
            "seven_day_sonnet": {"utilization": 40.0}
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);
        assert_eq!(snap.per_model[0].label, "7d-opus");
        assert_eq!(snap.per_model[0].duration_secs, Some(604_800));
        assert_eq!(snap.per_model[1].label, "7d-sonnet");
        assert_eq!(snap.per_model[1].duration_secs, Some(604_800));
    }

    #[test]
    fn the_limit_kind_string_never_reaches_the_snapshot() {
        // `kind` is read for a derived number and nothing else. A distinctive
        // synthetic kind proves the string itself is not carried into `Debug`
        // or the CLI's JSON — the same structural check the PII tests make.
        let json = r#"{
            "limits":[{"kind":"weekly_MARKER_KIND_5d1c","percent":40,
                       "scope":{"model":{"display_name":"TestModel"}}}]
        }"#;
        assert!(json.contains("MARKER_KIND"), "fixture lost its marker");

        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);

        // Read, and read correctly — the weekly family matched.
        assert_eq!(snap.per_model[0].duration_secs, Some(604_800));

        let debug = format!("{snap:?}");
        let json_out = serde_json::to_string(&snap).unwrap();
        assert!(!debug.contains("MARKER_KIND"), "kind leaked into Debug");
        assert!(!json_out.contains("MARKER_KIND"), "kind leaked into JSON");
    }

    #[test]
    fn limit_percent_is_clamped_and_missing_fields_degrade() {
        // Same fail-closed contract as the legacy path: out-of-range percent
        // clamps, an absent percent/resets_at degrades to None, not a panic.
        let json = r#"{
            "limits":[
              {"percent":150,"scope":{"model":{"display_name":"OverModel"}}},
              {"scope":{"model":{"display_name":"BareModel"}}}
            ]
        }"#;
        let usage: RawUsage = serde_json::from_str(json).unwrap();
        let snap = build_snapshot(usage, 0);

        assert_eq!(snap.per_model.len(), 2);
        assert_eq!(snap.per_model[0].used_fraction, Some(1.0));
        assert_eq!(snap.per_model[1].label, "BareModel");
        assert_eq!(snap.per_model[1].used_fraction, None);
        assert_eq!(snap.per_model[1].resets_in_secs, None);
    }
}
