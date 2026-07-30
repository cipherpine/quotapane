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
//! `--debug-raw` prints a provider's wire response instead of a snapshot, for
//! pinning an undocumented endpoint's schema without making an ad-hoc token
//! request outside the trust boundary. It is supported by **both** providers;
//! it used to apply to Codex only and be silently ignored for Claude.
//!
//! Since M9b that dump is **redacted by default**. The Codex usage response
//! carries account PII (`email`, `user_id`, `account_id`) that never enters a
//! snapshot but is right there in the bytes, and a debug dump is precisely the
//! output people paste into an issue. So the default path parses the body as
//! JSON and replaces the value of every PII-named key at any depth before
//! printing; a body that is not valid JSON is withheld entirely rather than
//! dumped unexamined (fail closed). `--debug-raw-unsafe` restores the
//! byte-exact dump behind an explicit stderr warning — for the schema-pinning
//! case where the exact bytes are the point.

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

/// `--help` output. Every flag `parse_args` accepts must appear here verbatim;
/// a test scans the parser's own source for its accepted literals and asserts
/// each one is present, so a future flag cannot ship undocumented.
const HELP: &str = "\
QuotaPane CLI — read your own Claude and Codex subscription usage locally.

usage: quotapane-cli --once [--json] [--provider claude|codex|all]
                     [--client-version <VER>] [--debug-raw]
                     [--debug-raw-unsafe]

Options:
  --once                  Poll once and exit. Required — the only mode today.
  --json                  Print the normalized snapshot as JSON instead of a
                          text summary. With --provider all, prints an array.
  --provider <WHICH>      Which provider to poll: claude, codex, or all.
                          Default: claude.
  --client-version <VER>  claude-code version string to send. Default: 0.0.0,
                          which the provider throttles aggressively — pass a
                          real version for normal use.
  --debug-raw             Print the provider's wire response instead of a
                          snapshot, for pinning an undocumented endpoint's
                          schema. Takes precedence over --json. Account
                          identifiers (email, user_id, account_id, id) are
                          replaced with «redacted» at any depth; a body that
                          is not valid JSON is withheld rather than dumped.
  --debug-raw-unsafe      Like --debug-raw but byte-exact: no redaction, no
                          withholding. The output may contain your email and
                          account identifiers — do not paste it anywhere.
  -h, --help              Print this help and exit.
  --version               Print the version and exit.

Credentials are read from your local claude/codex CLI files, read-only; they
are never written, logged, or persisted. If a token has expired, run `claude`
or `codex` to refresh it.
";

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

/// Object keys whose **values** are account identifiers or contact details.
///
/// Matched by name at any depth, in objects and through arrays — the shape of
/// an undocumented endpoint's response is not something to hard-code a path
/// into. `id` is included deliberately even though it is the broadest: a
/// debug dump is a diagnostic, and over-redacting a harmless `id` costs a
/// re-run with `--debug-raw-unsafe`, while under-redacting costs the user an
/// identifier they pasted into a public issue.
const PII_KEYS: &[&str] = &["email", "user_id", "account_id", "id"];

/// What a redacted value is replaced with. Distinctive on purpose: seeing it
/// in a dump should read as "the tool removed this", not as endpoint data.
const REDACTED: &str = "«redacted»";

/// Printed in place of a body that could not be parsed as JSON, so nothing
/// unexamined reaches stdout.
const WITHHELD_NOTICE: &str =
    "(body withheld: not valid JSON — use --debug-raw-unsafe for exact bytes)";

/// Stderr warning printed once before a `--debug-raw-unsafe` dump.
const UNSAFE_WARNING: &str = "warning: --debug-raw-unsafe prints the response body byte-for-byte, \
     with no redaction; it may contain your email address and account identifiers. Do not paste \
     the output into an issue, a chat, or a screenshot.";

/// Holds no credential material — `client_version` is a public version
/// string — so deriving `Debug` here cannot leak a token.
#[derive(Debug)]
struct Args {
    json: bool,
    provider: ProviderSel,
    client_version: String,
    client_version_defaulted: bool,
    debug_raw: bool,
    /// Byte-exact dump. Implies `debug_raw`; never set on its own.
    debug_raw_unsafe: bool,
}

/// Replace the value of every [`PII_KEYS`] entry with [`REDACTED`], recursing
/// through objects and arrays.
///
/// Replaces the value **whatever its type** — a `user_id` that arrives as a
/// number or an object is redacted just as a string one is, so a schema change
/// cannot quietly reopen the hole.
fn redact_pii(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                if PII_KEYS.contains(&key.as_str()) {
                    *child = serde_json::Value::String(REDACTED.to_string());
                } else {
                    redact_pii(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                redact_pii(item);
            }
        }
        _ => {}
    }
}

/// Render a provider's `debug_raw_body` output for printing.
///
/// The provider hands back `"status: <code>\n<body>"`. With `byte_exact` the
/// whole thing is passed through unchanged (`--debug-raw-unsafe`). Otherwise
/// the status line is kept, the body is parsed as JSON, PII-named values are
/// replaced, and the result is pretty-printed. A body that does not parse is
/// **withheld**: the point of the default path is that nothing unexamined
/// reaches stdout, and "not JSON" means we cannot examine it.
fn render_debug_raw(raw: &str, byte_exact: bool) -> String {
    if byte_exact {
        return raw.to_string();
    }
    let (status_line, body) = raw.split_once('\n').unwrap_or((raw, ""));
    let rendered = match serde_json::from_str::<serde_json::Value>(body) {
        Ok(mut value) => {
            redact_pii(&mut value);
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| WITHHELD_NOTICE.to_string())
        }
        Err(_) => WITHHELD_NOTICE.to_string(),
    };
    format!("{status_line}\n{rendered}")
}

/// What the command line asked for. `--help`/`--version` are answered and
/// exit 0 without polling anything — the first command a stranger types must
/// not fail.
#[derive(Debug)]
enum Invocation {
    Help,
    Version,
    Run(Args),
}

fn parse_args<I: IntoIterator<Item = String>>(argv: I) -> Result<Invocation, String> {
    let mut once = false;
    let mut json = false;
    let mut provider: Option<ProviderSel> = None;
    let mut client_version: Option<String> = None;
    let mut debug_raw = false;
    let mut debug_raw_unsafe = false;

    let mut iter = argv.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            // Answered before any other validation, so `--help` works on its
            // own — without it the parser would reject the invocation for a
            // missing `--once`.
            "--help" | "-h" => return Ok(Invocation::Help),
            "--version" => return Ok(Invocation::Version),
            "--once" => once = true,
            "--json" => json = true,
            "--debug-raw" => debug_raw = true,
            // Implies --debug-raw: it is the same mode, minus the redaction,
            // so `--debug-raw-unsafe` alone works and combining them is not an
            // error. Not a rename — existing --debug-raw scripts keep working,
            // just safer.
            "--debug-raw-unsafe" => {
                debug_raw = true;
                debug_raw_unsafe = true;
            }
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
    Ok(Invocation::Run(Args {
        json,
        provider: provider.unwrap_or(ProviderSel::Claude),
        client_version: client_version.unwrap_or_else(|| DEFAULT_CLIENT_VERSION.to_string()),
        client_version_defaulted,
        debug_raw,
        debug_raw_unsafe,
    }))
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
        Ok(Invocation::Help) => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Ok(Invocation::Version) => {
            println!("quotapane-cli {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        Ok(Invocation::Run(a)) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: quotapane-cli --once [--json] [--provider claude|codex|all] [--client-version <VER>] [--debug-raw] [--debug-raw-unsafe]"
            );
            eprintln!("try `quotapane-cli --help` for the full list of options");
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

    // One warning per run, before any body reaches stdout — not one per
    // provider, so `--provider all` does not train the reader to skip it.
    if args.debug_raw_unsafe {
        eprintln!("{UNSAFE_WARNING}");
    }

    let egress = Egress::new(false);
    let multi = matches!(args.provider, ProviderSel::All);
    let mut snapshots: Vec<ProviderSnapshot> = Vec::new();
    let mut had_error = false;

    // Poll each selected provider independently: one signed-out or erroring
    // provider records a clean diagnostic and flips the exit code, but never
    // aborts the others (`all` still emits whatever succeeded).
    for id in ids {
        // `--debug-raw` bypasses the normal snapshot path, reading the wire
        // response through the same `fetch` the normal poll uses
        // (`debug_raw_body`), so the dump is guaranteed to reflect the real
        // request. What reaches stdout is redacted unless the user asked for
        // byte-exact output — see `render_debug_raw`. Supported by **both**
        // providers: the flag used to be
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
                Some(Ok(raw)) => println!("{}", render_debug_raw(&raw, args.debug_raw_unsafe)),
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

    /// Parse an invocation expected to be a normal run, unwrapping to `Args`.
    fn parse_run(v: &[&str]) -> Args {
        match parse_args(args(v)) {
            Ok(Invocation::Run(a)) => a,
            other => panic!("expected a run invocation, got {other:?}"),
        }
    }

    // --- existing M1 behavior (backward compatibility) ---

    #[test]
    fn once_alone_defaults_json_off_version_defaulted_and_provider_claude() {
        let parsed = parse_run(&["--once"]);
        assert!(!parsed.json);
        assert_eq!(parsed.client_version, DEFAULT_CLIENT_VERSION);
        assert!(parsed.client_version_defaulted);
        assert_eq!(parsed.provider, ProviderSel::Claude);
    }

    #[test]
    fn json_flag_is_recognized() {
        let parsed = parse_run(&["--once", "--json"]);
        assert!(parsed.json);
    }

    #[test]
    fn client_version_flag_overrides_default() {
        let parsed = parse_run(&["--once", "--client-version", "1.2.3"]);
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
        let parsed = parse_run(&["--json", "--client-version", "9.9.9", "--once"]);
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
        let parsed = parse_run(&["--once", "--provider", "codex"]);
        assert_eq!(parsed.provider, ProviderSel::Codex);
    }

    #[test]
    fn provider_flag_selects_all() {
        let parsed = parse_run(&["--once", "--json", "--provider", "all"]);
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
                duration_secs: Some(18_000),
            }],
            per_model: vec![QuotaWindow {
                label: "7d-opus".to_string(),
                used_fraction: Some(0.5),
                resets_in_secs: None,
                duration_secs: Some(604_800),
            }],
            reset_credits: None,
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
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(json.contains("\"per_model\":[]"), "was: {json}");
    }

    #[test]
    fn zero_usage_per_model_buckets_stay_in_json() {
        // M7a: the window *hides* untouched per-model rows (0% or unknown
        // usage) because providers enumerate every bucket on the plan. That
        // filter is display-only and lives in `usage-ui`. This pins the other
        // half of the decision: `--json` stays the full truth, so a script
        // reading it still sees every bucket the provider reported.
        //
        // Deliberately a guard against a *future* change: if anyone ever
        // "cleans up" by pushing the UI filter down into the snapshot, this
        // test is what fails.
        use usage_core::model::{QuotaWindow, SnapshotSource};

        let bucket = |label: &str, used_fraction: Option<f64>| QuotaWindow {
            label: label.to_string(),
            used_fraction,
            resets_in_secs: Some(604_800),
            duration_secs: Some(604_800),
        };

        let snapshot = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 1_784_000_000,
            windows: vec![bucket("5h", Some(0.25))],
            per_model: vec![
                bucket("GPT-5.3-Codex-Spark", Some(0.0)), // untouched — hidden in the window
                bucket("GPT-5.3-Codex-Max", Some(0.42)),  // used — shown in the window
                bucket("GPT-5.3-Codex-Mini", None),       // unknown — hidden in the window
            ],
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        };

        // Round-trip through `Value` so the assertions pin the data, not the
        // float formatting.
        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let per_model = parsed["per_model"].as_array().unwrap();

        assert_eq!(per_model.len(), 3, "a bucket was dropped: {json}");
        let labels: Vec<&str> = per_model
            .iter()
            .map(|w| w["label"].as_str().unwrap())
            .collect();
        assert_eq!(
            labels,
            vec![
                "GPT-5.3-Codex-Spark",
                "GPT-5.3-Codex-Max",
                "GPT-5.3-Codex-Mini"
            ]
        );

        // The zero is a real zero, not a dropped/nulled field.
        assert_eq!(per_model[0]["used_fraction"].as_f64(), Some(0.0));
        assert_eq!(per_model[1]["used_fraction"].as_f64(), Some(0.42));
        assert!(per_model[2]["used_fraction"].is_null());

        // Same for the `--provider all` array form.
        let array = serde_json::to_string(&vec![snapshot]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&array).unwrap();
        assert_eq!(parsed[0]["per_model"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn reset_credits_key_is_always_present_in_json() {
        // M7a2: `reset_credits` gets no `skip_serializing_if`, so the key is
        // in every snapshot — `null` for a provider with no such concept
        // (Claude), an object for one that reports them (Codex). A consumer
        // can therefore read `.reset_credits` unconditionally, and "absent"
        // and "zero" stay distinguishable.
        use usage_core::model::{QuotaWindow, ResetCredits, SnapshotSource};

        let claude = ProviderSnapshot {
            provider: ProviderId::ClaudeSubscription,
            taken_at_unix_secs: 1_784_000_000,
            windows: vec![QuotaWindow {
                label: "5h".to_string(),
                used_fraction: Some(0.18),
                resets_in_secs: Some(2805),
                duration_secs: Some(18_000),
            }],
            per_model: vec![],
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        };
        let codex = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 1_784_000_000,
            windows: vec![],
            per_model: vec![],
            reset_credits: Some(ResetCredits {
                available: 1,
                applicable_now: Some(0),
            }),
            source: SnapshotSource::UsageEndpoint,
        };

        // Claude: the key exists and is null — not omitted.
        let json = serde_json::to_string(&claude).unwrap();
        assert!(
            json.contains("\"reset_credits\":null"),
            "Claude must emit an explicit null: {json}"
        );

        // Codex: an object carrying both counts, with the zero preserved.
        let parsed: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&codex).unwrap()).unwrap();
        assert_eq!(parsed["reset_credits"]["available"].as_u64(), Some(1));
        assert_eq!(parsed["reset_credits"]["applicable_now"].as_u64(), Some(0));

        // And the `--provider all` array form carries both shapes.
        let array = serde_json::to_string(&vec![claude, codex]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&array).unwrap();
        assert!(
            parsed[0].get("reset_credits").is_some(),
            "key missing from the array form: {array}"
        );
        assert!(parsed[0]["reset_credits"].is_null());
        assert_eq!(parsed[1]["reset_credits"]["available"].as_u64(), Some(1));
    }

    #[test]
    fn duration_secs_key_is_always_present_in_json() {
        // M8: `duration_secs` gets no `skip_serializing_if`, so the key is on
        // every window — a number when the provider stated or implied the
        // window's length, an explicit `null` when it did not. Same contract as
        // `reset_credits`: a consumer reads `.duration_secs` unconditionally,
        // and "unknown" stays distinguishable from "absent field".
        use usage_core::model::{QuotaWindow, SnapshotSource};

        let snapshot = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 1_784_000_000,
            windows: vec![
                QuotaWindow {
                    label: "5h".to_string(),
                    used_fraction: Some(0.25),
                    resets_in_secs: Some(3600),
                    duration_secs: Some(18_000),
                },
                QuotaWindow {
                    // The endpoint gave no window length for this one.
                    label: "primary".to_string(),
                    used_fraction: Some(0.10),
                    resets_in_secs: None,
                    duration_secs: None,
                },
            ],
            per_model: vec![QuotaWindow {
                label: "GPT-5.3-Codex-Spark".to_string(),
                used_fraction: Some(0.42),
                resets_in_secs: Some(604_800),
                duration_secs: None,
            }],
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        };

        let json = serde_json::to_string(&snapshot).unwrap();
        assert!(
            json.contains("\"duration_secs\":null"),
            "an unknown duration must emit an explicit null: {json}"
        );

        // Round-trip through `Value` so the assertions pin the data rather than
        // the number formatting, and so "key exists" is checked as such.
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let windows = parsed["windows"].as_array().unwrap();
        assert_eq!(windows[0]["duration_secs"].as_u64(), Some(18_000));
        assert!(
            windows[1].get("duration_secs").is_some(),
            "key missing from the unknown-duration window: {json}"
        );
        assert!(windows[1]["duration_secs"].is_null());

        // Per-model rows carry the key too — they are the same type.
        let per_model = parsed["per_model"].as_array().unwrap();
        assert!(per_model[0].get("duration_secs").is_some());
        assert!(per_model[0]["duration_secs"].is_null());

        // And the `--provider all` array form.
        let array = serde_json::to_string(&vec![snapshot]).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&array).unwrap();
        assert_eq!(
            parsed[0]["windows"][0]["duration_secs"].as_u64(),
            Some(18_000)
        );
        assert!(parsed[0]["windows"][1]["duration_secs"].is_null());
    }

    // --- --debug-raw parsing (new) ---

    #[test]
    fn debug_raw_flag_defaults_off() {
        let parsed = parse_run(&["--once"]);
        assert!(!parsed.debug_raw);
    }

    #[test]
    fn debug_raw_flag_is_recognized_with_codex_provider() {
        let parsed = parse_run(&["--once", "--debug-raw", "--provider", "codex"]);
        assert!(parsed.debug_raw);
        assert_eq!(parsed.provider, ProviderSel::Codex);
    }

    #[test]
    fn debug_raw_flag_can_appear_in_any_order() {
        let parsed = parse_run(&["--provider", "codex", "--once", "--debug-raw"]);
        assert!(parsed.debug_raw);
    }

    // --- M9b: --debug-raw redacts by default, --debug-raw-unsafe does not ---

    /// Synthetic PII markers. Deliberately shaped as obvious placeholders (not
    /// as plausible credentials) so secret scanners have nothing to flag, and
    /// distinctive enough that a substring search for them is meaningful.
    const SENTINEL_EMAIL: &str = "sentinel-person-DO-NOT-PRINT@example.invalid";
    const SENTINEL_USER_ID: &str = "sentinel-user-id-DO-NOT-PRINT";
    const SENTINEL_ACCOUNT_ID: &str = "sentinel-account-id-DO-NOT-PRINT";
    const SENTINEL_ID: &str = "sentinel-bare-id-DO-NOT-PRINT";

    /// A response body shaped like the real Codex one: PII at the top level,
    /// nested inside an object, and inside array elements.
    fn body_with_pii() -> String {
        format!(
            r#"{{
  "email": "{SENTINEL_EMAIL}",
  "rate_limit": {{
    "primary_window": {{ "used_percent": 42.5, "reset_after_seconds": 900 }},
    "owner": {{ "user_id": "{SENTINEL_USER_ID}", "account_id": "{SENTINEL_ACCOUNT_ID}" }}
  }},
  "additional_rate_limits": [
    {{ "name": "GPT-5.3-Codex-Max", "used_percent": 10, "id": "{SENTINEL_ID}" }},
    {{ "name": "GPT-5.3-Codex-Mini", "used_percent": 0,
       "meta": {{ "deeply": {{ "nested": {{ "email": "{SENTINEL_EMAIL}" }} }} }} }}
  ]
}}"#
        )
    }

    fn raw_with_pii() -> String {
        format!("status: 200\n{}", body_with_pii())
    }

    #[test]
    fn debug_raw_unsafe_flag_is_recognized_and_implies_debug_raw() {
        let parsed = parse_run(&["--once", "--debug-raw-unsafe"]);
        assert!(parsed.debug_raw_unsafe);
        assert!(
            parsed.debug_raw,
            "--debug-raw-unsafe must imply --debug-raw"
        );

        // Order-independent, and combining the two flags is not an error.
        let parsed = parse_run(&["--debug-raw-unsafe", "--provider", "codex", "--once"]);
        assert!(parsed.debug_raw_unsafe && parsed.debug_raw);
        let parsed = parse_run(&["--once", "--debug-raw", "--debug-raw-unsafe"]);
        assert!(parsed.debug_raw_unsafe && parsed.debug_raw);
    }

    #[test]
    fn debug_raw_unsafe_defaults_off_so_plain_debug_raw_is_the_safe_path() {
        assert!(!parse_run(&["--once"]).debug_raw_unsafe);
        assert!(!parse_run(&["--once", "--debug-raw"]).debug_raw_unsafe);
    }

    #[test]
    fn default_debug_raw_redacts_pii_at_every_depth() {
        let out = render_debug_raw(&raw_with_pii(), false);

        // Not one sentinel survives — top level, nested object, array element,
        // or three levels down inside an array element.
        for sentinel in [
            SENTINEL_EMAIL,
            SENTINEL_USER_ID,
            SENTINEL_ACCOUNT_ID,
            SENTINEL_ID,
        ] {
            assert!(
                !out.contains(sentinel),
                "PII survived redaction ({sentinel}):\n{out}"
            );
        }

        // ...and it was removed by redaction, not by dropping the body: the
        // marker appears once per redacted key (5 of them).
        assert_eq!(
            out.matches(REDACTED).count(),
            5,
            "expected one marker per PII key:\n{out}"
        );

        // The usage data — the reason to run --debug-raw at all — is intact,
        // and so is the status line.
        assert!(out.starts_with("status: 200\n"), "{out}");
        for kept in [
            "primary_window",
            "used_percent",
            "42.5",
            "reset_after_seconds",
            "GPT-5.3-Codex-Max",
            "additional_rate_limits",
        ] {
            assert!(
                out.contains(kept),
                "redaction ate usage data ({kept}):\n{out}"
            );
        }

        // The output is still parseable JSON below the status line, so the
        // dump stays useful for pinning a schema.
        let body = out.split_once('\n').unwrap().1;
        let parsed: serde_json::Value =
            serde_json::from_str(body).expect("redacted body must still be JSON");
        assert_eq!(parsed["email"].as_str(), Some(REDACTED));
        assert_eq!(
            parsed["rate_limit"]["owner"]["user_id"].as_str(),
            Some(REDACTED)
        );
        assert_eq!(
            parsed["rate_limit"]["owner"]["account_id"].as_str(),
            Some(REDACTED)
        );
        assert_eq!(
            parsed["additional_rate_limits"][0]["id"].as_str(),
            Some(REDACTED)
        );
        assert_eq!(
            parsed["additional_rate_limits"][1]["meta"]["deeply"]["nested"]["email"].as_str(),
            Some(REDACTED)
        );
    }

    #[test]
    fn redaction_replaces_non_string_pii_values_too() {
        // A schema change that turns `user_id` into a number or an object must
        // not reopen the hole: the VALUE is replaced whatever its type.
        let raw = r#"status: 200
{"user_id": 1234567890, "account_id": {"kind": "org", "id": "nested-sentinel"}, "ids": [1, 2]}"#;
        let out = render_debug_raw(raw, false);
        assert!(!out.contains("1234567890"), "{out}");
        assert!(!out.contains("nested-sentinel"), "{out}");
        // `ids` is not in the key list and carries no identifier — untouched.
        assert!(out.contains("\"ids\""), "{out}");
        assert_eq!(out.matches(REDACTED).count(), 2, "{out}");
    }

    #[test]
    fn unsafe_debug_raw_is_byte_exact() {
        let raw = raw_with_pii();
        assert_eq!(
            render_debug_raw(&raw, true),
            raw,
            "--debug-raw-unsafe must pass the bytes through unchanged"
        );
        // Including a body the default path would have withheld.
        let not_json = "status: 502\n<html><body>Gateway Timeout</body></html>";
        assert_eq!(render_debug_raw(not_json, true), not_json);
    }

    #[test]
    fn non_json_body_is_withheld_not_dumped() {
        // Fail closed: if we cannot parse it, we cannot claim it is PII-free.
        let raw =
            "status: 502\n<html><body>Gateway Timeout — user=someone@example.invalid</body></html>";
        let out = render_debug_raw(raw, false);

        assert!(out.contains(WITHHELD_NOTICE), "{out}");
        assert!(
            out.contains("--debug-raw-unsafe"),
            "the notice must name the escape hatch: {out}"
        );
        assert!(!out.contains("Gateway Timeout"), "the body leaked: {out}");
        assert!(
            !out.contains("someone@example.invalid"),
            "the body leaked: {out}"
        );
        // The status line still comes through — it is ours, not the body.
        assert!(out.starts_with("status: 502\n"), "{out}");

        // An empty body, and a truncated/partial JSON body, are withheld too.
        assert!(render_debug_raw("status: 204\n", false).contains(WITHHELD_NOTICE));
        assert!(render_debug_raw("status: 200\n{\"email\": \"x", false).contains(WITHHELD_NOTICE));
        // A response with no newline at all degrades cleanly rather than
        // treating the status line itself as a body.
        assert!(render_debug_raw("status: 200", false).contains(WITHHELD_NOTICE));
    }

    #[test]
    fn help_documents_both_debug_raw_flags_and_the_default() {
        assert!(HELP.contains("--debug-raw "), "{HELP}");
        assert!(HELP.contains("--debug-raw-unsafe"), "{HELP}");
        // The default behavior is stated, not just the flag name.
        assert!(
            HELP.contains(REDACTED),
            "help must show the redaction marker"
        );
        for key in PII_KEYS {
            assert!(
                HELP.contains(key),
                "help must name the redacted key `{key}`"
            );
        }
    }

    // --- --help / --version (M6-NAME) ---

    #[test]
    fn help_and_version_are_recognized_on_their_own() {
        // Neither requires --once: a bare `--help` must not fail the way it
        // did before (exit 2, "unrecognized argument").
        assert!(matches!(
            parse_args(args(&["--help"])),
            Ok(Invocation::Help)
        ));
        assert!(matches!(parse_args(args(&["-h"])), Ok(Invocation::Help)));
        assert!(matches!(
            parse_args(args(&["--version"])),
            Ok(Invocation::Version)
        ));
    }

    #[test]
    fn help_wins_over_other_flags_and_over_parse_errors() {
        // Asking for help never turns into an error, whatever else is present.
        assert!(matches!(
            parse_args(args(&["--once", "--json", "--help"])),
            Ok(Invocation::Help)
        ));
        assert!(matches!(
            parse_args(args(&["--help", "--bogus"])),
            Ok(Invocation::Help)
        ));
    }

    #[test]
    fn unknown_flags_still_error_after_adding_help() {
        // The regression guard for this change: adding --help must not turn
        // the parser permissive.
        assert!(parse_args(args(&["--once", "--bogus"])).is_err());
        assert!(parse_args(args(&["--help-me"])).is_err());
        assert!(parse_args(args(&["-x"])).is_err());
    }

    #[test]
    fn help_text_documents_every_flag_the_parser_accepts() {
        // Enumerate the accepted set from `parse_args`'s OWN SOURCE rather
        // than from a hand-kept list, so a flag added to the parser without a
        // help entry fails this test instead of shipping undocumented.
        const SRC: &str = include_str!("main.rs");

        let start = SRC.find("fn parse_args").expect("parse_args not found");
        let body = &SRC[start..];
        // rustfmt puts the function's closing brace at column 0.
        let end = body.find("\n}\n").expect("end of parse_args not found");
        let body = &body[..end];

        // String literals are the odd-indexed pieces when splitting on `"`.
        // A flag literal starts with `-` and contains no whitespace, which
        // excludes the parser's error messages ("--provider requires a value").
        let flags: Vec<&str> = body
            .split('"')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, lit)| lit)
            .filter(|lit| lit.starts_with('-') && !lit.contains(char::is_whitespace))
            .collect();

        // Guard the scanner itself: if it silently matched nothing, or the
        // source layout moved, the assertions below would pass vacuously.
        for expected in [
            "--once",
            "--json",
            "--debug-raw",
            "--provider",
            "--client-version",
            "--help",
            "-h",
            "--version",
        ] {
            assert!(
                flags.contains(&expected),
                "scanner missed {expected}; found {flags:?}"
            );
        }

        for flag in &flags {
            assert!(
                HELP.contains(flag),
                "`{flag}` is accepted by parse_args but absent from --help text"
            );
        }
    }

    #[test]
    fn help_text_names_the_renamed_binary() {
        assert!(HELP.contains("quotapane-cli"));
        assert!(!HELP.contains("usage-cli"));
    }
}
