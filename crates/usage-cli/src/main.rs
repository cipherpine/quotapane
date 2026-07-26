//! QuotaPane headless CLI — `--json` (M1) plus multi-provider `--provider` (M3).
//!
//! This binary is how the pipeline is proven before any UI exists, and how
//! security-conscious users verify egress behavior under a packet capture
//! (SECURITY.md, hardening guidance §3).
//!
//! M3 adds `--provider claude|codex|all` so both subscription providers can be
//! polled headlessly. The default stays `claude` (backward-compatible with the
//! M1 CLI, which emitted a single snapshot object). `--provider all` polls both
//! and emits a JSON **array** (text mode prints both summaries); a provider that
//! is signed out (absent credential file) produces a clean stderr diagnostic and
//! a non-zero exit — never a panic — without stopping the other provider.
//!
//! `--debug-raw` prints a provider's exact wire response instead of a snapshot,
//! for pinning an undocumented endpoint's schema without making an ad-hoc token
//! request outside the trust boundary. It is supported by **both** providers;
//! it used to apply to Codex only and be silently ignored for Claude.

use std::process::ExitCode;

use usage_core::egress::Egress;
use usage_core::model::{ProviderId, ProviderSnapshot};
use usage_core::providers::{
    ClaudeSubscription, CodexSubscription, UsageProvider, CODEX_DEFAULT_USER_AGENT,
};

/// Sent when `--client-version` is omitted. Real Claude Code versions avoid
/// the provider's aggressively rate-limited fallback bucket (see
/// `claude_subscription` module docs in usage-core).
const DEFAULT_CLIENT_VERSION: &str = "0.0.0";

/// Which provider(s) to poll (`--provider`). Defaults to Claude for backward
/// compatibility with the M1 single-provider CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProviderSel {
    Claude,
    Codex,
    All,
}

/// Parse the `--provider` value.
fn parse_provider(s: &str) -> Result<ProviderSel, String> {
    match s {
        "claude" => Ok(ProviderSel::Claude),
        "codex" => Ok(ProviderSel::Codex),
        "all" => Ok(ProviderSel::All),
        other => Err(format!(
            "--provider must be one of claude|codex|all (got {other:?})"
        )),
    }
}

/// The provider ids a selection expands to, in output order.
fn selected_ids(sel: ProviderSel) -> Vec<ProviderId> {
    match sel {
        ProviderSel::Claude => vec![ProviderId::ClaudeSubscription],
        ProviderSel::Codex => vec![ProviderId::CodexSubscription],
        ProviderSel::All => vec![
            ProviderId::ClaudeSubscription,
            ProviderId::CodexSubscription,
        ],
    }
}

/// Short CLI name for a provider, used in stderr diagnostics.
fn provider_cli_name(id: ProviderId) -> &'static str {
    match id {
        ProviderId::ClaudeSubscription => "claude",
        ProviderId::CodexSubscription => "codex",
    }
}

struct Args {
    json: bool,
    provider: ProviderSel,
    client_version: String,
    client_version_defaulted: bool,
    debug_raw: bool,
}

fn parse_args<I: IntoIterator<Item = String>>(argv: I) -> Result<Args, String> {
    let mut once = false;
    let mut json = false;
    let mut provider: Option<ProviderSel> = None;
    let mut client_version: Option<String> = None;
    let mut debug_raw = false;

    let mut iter = argv.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--once" => once = true,
            "--json" => json = true,
            "--debug-raw" => debug_raw = true,
            "--provider" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--provider requires a value".to_string())?;
                provider = Some(parse_provider(&value)?);
            }
            "--client-version" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--client-version requires a value".to_string())?;
                client_version = Some(value);
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
    }

    if !once {
        return Err("--once is required (the only supported mode for now)".to_string());
    }

    let client_version_defaulted = client_version.is_none();
    Ok(Args {
        json,
        provider: provider.unwrap_or(ProviderSel::Claude),
        client_version: client_version.unwrap_or_else(|| DEFAULT_CLIENT_VERSION.to_string()),
        client_version_defaulted,
        debug_raw,
    })
}

/// Construct the selected provider as a trait object, or `None` if the
/// credential *path* cannot be resolved at all (no home directory).
fn build_provider(id: ProviderId, client_version: &str) -> Option<Box<dyn UsageProvider>> {
    match id {
        ProviderId::ClaudeSubscription => {
            ClaudeSubscription::with_default_path(client_version.to_string())
                .map(|p| Box::new(p) as Box<dyn UsageProvider>)
        }
        ProviderId::CodexSubscription => {
            CodexSubscription::with_default_path(CODEX_DEFAULT_USER_AGENT)
                .map(|p| Box::new(p) as Box<dyn UsageProvider>)
        }
    }
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: usage-cli --once [--json] [--provider claude|codex|all] [--client-version <VER>] [--debug-raw]"
            );
            return ExitCode::from(2);
        }
    };

    let ids = selected_ids(args.provider);

    // The throttle note is Claude-specific; only surface it when Claude is
    // actually being polled with the placeholder version.
    if args.client_version_defaulted && ids.contains(&ProviderId::ClaudeSubscription) {
        eprintln!(
            "note: no --client-version given; using \"{DEFAULT_CLIENT_VERSION}\" — pass a real claude-code version to avoid provider throttling"
        );
    }

    // `--debug-raw` prints the provider's raw wire response instead of a
    // normalized snapshot, so there is no snapshot for `--json` to serialize.
    // Say so rather than dropping the flag silently — a silently ignored flag
    // reads as "the tool produced no JSON", not "that flag does not apply".
    if args.debug_raw && args.json {
        eprintln!(
            "note: --json does not apply to --debug-raw; printing the raw response body instead"
        );
    }

    let egress = Egress::new(false);
    let multi = matches!(args.provider, ProviderSel::All);
    let mut snapshots: Vec<ProviderSnapshot> = Vec::new();
    let mut had_error = false;

    // Poll each selected provider independently: one signed-out or erroring
    // provider records a clean diagnostic and flips the exit code, but never
    // aborts the others (`all` still emits whatever succeeded).
    for id in ids {
        // `--debug-raw` bypasses the normal snapshot path, printing the exact
        // wire response through the same `fetch` the normal poll uses
        // (`debug_raw_body`), so the dump is guaranteed to reflect the real
        // request. Supported by **both** providers: the flag used to be
        // silently ignored for Claude, which made it look like the endpoint
        // returned nothing rather than that the flag did not apply.
        if args.debug_raw {
            // `None` means the credential *path* could not be resolved at all.
            let dumped = match id {
                ProviderId::ClaudeSubscription => {
                    ClaudeSubscription::with_default_path(args.client_version.clone())
                        .map(|p| p.debug_raw_body(&egress))
                }
                ProviderId::CodexSubscription => {
                    CodexSubscription::with_default_path(CODEX_DEFAULT_USER_AGENT)
                        .map(|p| p.debug_raw_body(&egress))
                }
            };
            match dumped {
                None => {
                    eprintln!(
                        "error: {}: could not resolve a home directory for the credentials path",
                        provider_cli_name(id)
                    );
                    had_error = true;
                }
                Some(Ok(raw)) => println!("{raw}"),
                Some(Err(e)) => {
                    eprintln!("error: {}: {e}", provider_cli_name(id));
                    had_error = true;
                }
            }
            continue;
        }

        match build_provider(id, &args.client_version) {
            None => {
                eprintln!(
                    "error: {}: could not resolve a home directory for the credentials path",
                    provider_cli_name(id)
                );
                had_error = true;
            }
            Some(provider) => match provider.poll(&egress) {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(e) => {
                    eprintln!("error: {}: {e}", provider_cli_name(id));
                    had_error = true;
                }
            },
        }
    }

    if args.json {
        // `all` → array (even if partial/empty); single provider → object,
        // preserving the exact M1 output shape for the default invocation.
        let serialized = if multi {
            serde_json::to_string_pretty(&snapshots)
        } else {
            match snapshots.first() {
                Some(snapshot) => serde_json::to_string_pretty(snapshot),
                None => Ok(String::new()), // single provider failed: nothing on stdout
            }
        };
        match serialized {
            Ok(s) if !s.is_empty() => println!("{s}"),
            Ok(_) => {}
            Err(e) => {
                eprintln!("error: failed to serialize snapshot(s): {e}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        for snapshot in &snapshots {
            print_summary(snapshot);
        }
    }

    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_summary(snapshot: &ProviderSnapshot) {
    println!("provider: {:?}", snapshot.provider);
    for w in &snapshot.windows {
        let percent = w
            .used_fraction
            .map(|f| format!("{:.1}%", f * 100.0))
            .unwrap_or_else(|| "unknown".to_string());
        let reset = w
            .resets_in_secs
            .map(format_reset)
            .unwrap_or_else(|| "unknown".to_string());
        println!("  {} — {percent} used, resets in {reset}", w.label);
    }
}

fn format_reset(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // --- existing M1 behavior (backward compatibility) ---

    #[test]
    fn once_alone_defaults_json_off_version_defaulted_and_provider_claude() {
        let parsed = parse_args(args(&["--once"])).unwrap();
        assert!(!parsed.json);
        assert_eq!(parsed.client_version, DEFAULT_CLIENT_VERSION);
        assert!(parsed.client_version_defaulted);
        assert_eq!(parsed.provider, ProviderSel::Claude);
    }

    #[test]
    fn json_flag_is_recognized() {
        let parsed = parse_args(args(&["--once", "--json"])).unwrap();
        assert!(parsed.json);
    }

    #[test]
    fn client_version_flag_overrides_default() {
        let parsed = parse_args(args(&["--once", "--client-version", "1.2.3"])).unwrap();
        assert_eq!(parsed.client_version, "1.2.3");
        assert!(!parsed.client_version_defaulted);
    }

    #[test]
    fn missing_once_is_an_error() {
        assert!(parse_args(args(&["--json"])).is_err());
    }

    #[test]
    fn unrecognized_flag_is_an_error() {
        assert!(parse_args(args(&["--once", "--bogus"])).is_err());
    }

    #[test]
    fn client_version_without_value_is_an_error() {
        assert!(parse_args(args(&["--once", "--client-version"])).is_err());
    }

    #[test]
    fn flags_can_appear_in_any_order() {
        let parsed = parse_args(args(&["--json", "--client-version", "9.9.9", "--once"])).unwrap();
        assert!(parsed.json);
        assert_eq!(parsed.client_version, "9.9.9");
    }

    // --- parse_provider (new) ---

    #[test]
    fn parse_provider_accepts_the_three_values() {
        assert_eq!(parse_provider("claude").unwrap(), ProviderSel::Claude);
        assert_eq!(parse_provider("codex").unwrap(), ProviderSel::Codex);
        assert_eq!(parse_provider("all").unwrap(), ProviderSel::All);
    }

    #[test]
    fn parse_provider_rejects_unknown_values() {
        assert!(parse_provider("both").is_err());
        assert!(parse_provider("").is_err());
        assert!(parse_provider("Claude").is_err()); // case-sensitive
    }

    // --- selected_ids (new) ---

    #[test]
    fn selected_ids_expand_correctly() {
        assert_eq!(
            selected_ids(ProviderSel::Claude),
            vec![ProviderId::ClaudeSubscription]
        );
        assert_eq!(
            selected_ids(ProviderSel::Codex),
            vec![ProviderId::CodexSubscription]
        );
        assert_eq!(
            selected_ids(ProviderSel::All),
            vec![
                ProviderId::ClaudeSubscription,
                ProviderId::CodexSubscription
            ]
        );
    }

    // --- provider_cli_name (new) ---

    #[test]
    fn provider_cli_names_map_correctly() {
        assert_eq!(provider_cli_name(ProviderId::ClaudeSubscription), "claude");
        assert_eq!(provider_cli_name(ProviderId::CodexSubscription), "codex");
    }

    // --- --provider parsing through parse_args (new) ---

    #[test]
    fn provider_flag_selects_codex() {
        let parsed = parse_args(args(&["--once", "--provider", "codex"])).unwrap();
        assert_eq!(parsed.provider, ProviderSel::Codex);
    }

    #[test]
    fn provider_flag_selects_all() {
        let parsed = parse_args(args(&["--once", "--json", "--provider", "all"])).unwrap();
        assert_eq!(parsed.provider, ProviderSel::All);
        assert!(parsed.json);
    }

    #[test]
    fn provider_flag_with_invalid_value_is_an_error() {
        assert!(parse_args(args(&["--once", "--provider", "nope"])).is_err());
    }

    #[test]
    fn provider_flag_without_value_is_an_error() {
        assert!(parse_args(args(&["--once", "--provider"])).is_err());
    }

    // --- --json surface (M5a) ---

    #[test]
    fn json_output_includes_per_model() {
        // The CLI serializes `ProviderSnapshot` wholesale rather than
        // hand-building its JSON, so `per_model` rides along for free. This
        // pins that: if anyone replaces the derive with a hand-rolled writer,
        // the field has to be carried deliberately.
        use usage_core::model::{QuotaWindow, SnapshotSource};

        let snapshot = ProviderSnapshot {
            provider: ProviderId::ClaudeSubscription,
            taken_at_unix_secs: 1_784_000_000,
            windows: vec![QuotaWindow {
                label: "5h".to_string(),
                used_fraction: Some(0.25),
                resets_in_secs: Some(3600),
            }],
            per_model: vec![QuotaWindow {
                label: "7d-opus".to_string(),
                used_fraction: Some(0.5),
                resets_in_secs: None,
            }],
            source: SnapshotSource::UsageEndpoint,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"per_model\""), "missing per_model: {json}");
        assert!(json.contains("7d-opus"));

        // And the `--provider all` array form carries it too.
        let array = serde_json::to_string(&vec![snapshot]).unwrap();
        assert!(array.contains("\"per_model\""));
    }

    #[test]
    fn json_output_keeps_per_model_present_when_empty() {
        // No `skip_serializing_if`: consumers can rely on the key existing.
        use usage_core::model::SnapshotSource;

        let snapshot = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 0,
            windows: vec![],
            per_model: vec![],
            source: SnapshotSource::UsageEndpoint,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"per_model\":[]"), "was: {json}");
    }

    // --- --debug-raw parsing (new) ---

    #[test]
    fn debug_raw_flag_defaults_off() {
        let parsed = parse_args(args(&["--once"])).unwrap();
        assert!(!parsed.debug_raw);
    }

    #[test]
    fn debug_raw_flag_is_recognized_with_codex_provider() {
        let parsed = parse_args(args(&["--once", "--debug-raw", "--provider", "codex"])).unwrap();
        assert!(parsed.debug_raw);
        assert_eq!(parsed.provider, ProviderSel::Codex);
    }

    #[test]
    fn debug_raw_flag_can_appear_in_any_order() {
        let parsed = parse_args(args(&["--provider", "codex", "--once", "--debug-raw"])).unwrap();
        assert!(parsed.debug_raw);
    }
}
