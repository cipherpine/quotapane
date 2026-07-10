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

impl UsageProvider for ClaudeSubscription {
    fn id(&self) -> ProviderId {
        ProviderId::ClaudeSubscription
    }

    fn poll(&self, http: &Egress) -> Result<ProviderSnapshot, ProviderError> {
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
        let resp = http.get(
            HOST,
            PATH,
            Some(&creds.access_token),
            &[
                ("anthropic-beta", ANTHROPIC_BETA),
                ("Content-Type", "application/json"),
                ("User-Agent", &user_agent),
            ],
        )?;

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
#[derive(Deserialize, Default)]
struct RawUsage {
    five_hour: Option<RawWindow>,
    seven_day: Option<RawWindow>,
    seven_day_opus: Option<RawWindow>,
    seven_day_sonnet: Option<RawWindow>,
}

#[derive(Deserialize)]
struct RawWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

/// Build a normalized, non-secret snapshot from a usage response.
fn build_snapshot(usage: RawUsage, now_unix_secs: u64) -> ProviderSnapshot {
    let mut windows = Vec::new();
    for (label, win) in [
        ("5h", usage.five_hour),
        ("7d", usage.seven_day),
        ("7d-opus", usage.seven_day_opus),
        ("7d-sonnet", usage.seven_day_sonnet),
    ] {
        if let Some(w) = win {
            windows.push(QuotaWindow {
                label: label.to_string(),
                used_fraction: w.utilization.map(|u| (u / 100.0).clamp(0.0, 1.0)),
                resets_in_secs: w
                    .resets_at
                    .as_deref()
                    .and_then(parse_rfc3339_to_unix)
                    .map(|reset| reset.saturating_sub(now_unix_secs as i64).max(0) as u64),
            });
        }
    }
    ProviderSnapshot {
        provider: ProviderId::ClaudeSubscription,
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

/// Parse an RFC 3339 / ISO 8601 timestamp to a Unix second count.
///
/// Dependency-free (keeps the trust boundary's tree minimal). Handles the
/// forms the endpoint returns: `YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`.
/// Fractional seconds are ignored; the offset is applied. Returns `None` on
/// any malformed input — callers treat that as "reset unknown".
fn parse_rfc3339_to_unix(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    // Split date and time on 'T' (accept lowercase 't' defensively).
    let t_pos = bytes.iter().position(|&b| b == b'T' || b == b't')?;
    let (date, rest) = (&s[..t_pos], &s[t_pos + 1..]);

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Locate the offset marker: 'Z'/'z', or a '+'/'-' after the time.
    let (time_str, offset_secs) = split_offset(rest)?;

    // Strip any fractional part.
    let time_core = time_str.split('.').next()?;
    let mut time_parts = time_core.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    if time_parts.next().is_some()
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3_600 + minute * 60 + second;
    // A positive offset means local time is ahead of UTC → subtract to get UTC.
    Some(secs - offset_secs)
}

/// Split the time-plus-offset tail into (time_without_offset, offset_seconds).
fn split_offset(rest: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = rest.strip_suffix(['Z', 'z']) {
        return Some((stripped, 0));
    }
    // Find the sign that introduces the numeric offset. It follows the time,
    // so search from the part after position 0 (hour digits can't be signed).
    let rb = rest.as_bytes();
    for i in 1..rb.len() {
        if rb[i] == b'+' || rb[i] == b'-' {
            let (time, off) = (&rest[..i], &rest[i..]);
            let sign = if rb[i] == b'+' { 1 } else { -1 };
            let mut op = off[1..].split(':');
            let oh: i64 = op.next()?.parse().ok()?;
            let om: i64 = op.next().unwrap_or("0").parse().ok()?;
            if op.next().is_some() || !(0..=23).contains(&oh) || !(0..=59).contains(&om) {
                return None;
            }
            return Some((time, sign * (oh * 3_600 + om * 60)));
        }
    }
    // No offset marker at all — treat as UTC (endpoint always sends one, but
    // fail open to UTC rather than reject a bare local time).
    Some((rest, 0))
}

/// Days since the Unix epoch for a civil (proleptic Gregorian) date.
/// Howard Hinnant's `days_from_civil` algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
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
        // Opus was null → skipped; the other three present.
        assert_eq!(snap.windows.len(), 3);

        let five = &snap.windows[0];
        assert_eq!(five.label, "5h");
        assert_eq!(five.used_fraction, Some(0.33));
        assert_eq!(five.resets_in_secs, Some(3600));

        let sonnet = snap
            .windows
            .iter()
            .find(|w| w.label == "7d-sonnet")
            .unwrap();
        assert_eq!(sonnet.used_fraction, Some(0.125));
        assert_eq!(sonnet.resets_in_secs, None); // resets_at was null
    }

    #[test]
    fn missing_fields_degrade_not_crash() {
        // Empty object → a valid, empty snapshot (fail closed, not panic).
        let usage: RawUsage = serde_json::from_str("{}").unwrap();
        let snap = build_snapshot(usage, 0);
        assert!(snap.windows.is_empty());

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
    fn rfc3339_known_anchors() {
        assert_eq!(parse_rfc3339_to_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_to_unix("2000-01-01T00:00:00Z"),
            Some(946_684_800)
        );
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T00:00:00Z"),
            Some(1_767_225_600)
        );
        // Fractional seconds ignored; explicit +00:00 offset.
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T00:00:00.528743+00:00"),
            Some(1_767_225_600)
        );
    }

    #[test]
    fn rfc3339_applies_offset() {
        // 01:00:00+01:00 is the same instant as 00:00:00Z.
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T01:00:00+01:00"),
            Some(1_767_225_600)
        );
        // 23:00:00-01:00 on the prior day is also 2026-01-01T00:00:00Z.
        assert_eq!(
            parse_rfc3339_to_unix("2025-12-31T23:00:00-01:00"),
            Some(1_767_225_600)
        );
    }

    #[test]
    fn rfc3339_rejects_garbage() {
        assert_eq!(parse_rfc3339_to_unix("not-a-date"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T25:00:00Z"), None);
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

        std::fs::remove_file(&path).unwrap();
    }
}
