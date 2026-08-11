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
//!
//! M12 makes the CLI usable as a gate rather than only as a reporter.
//! `--fail-at <N>` exits **3** when any window has reached N percent, so a
//! script can stop *before* a long run dies mid-flight; `--watch <SECS>` is
//! the second mode, polling on an interval (floored at the poller's own 180 s)
//! and emitting NDJSON under `--json`. Both are deliberately inert: QuotaPane
//! never executes anything on the user's behalf — it reports, and the user's
//! own script decides what to do about it.
//!
//! M18a adds `--statusline`, a third mode beside `--once` and `--watch` and the
//! only one that reports usage without asking anyone: Claude Code's statusline
//! feature already hands its command the quota numbers on stdin, so the mode
//! reads one JSON document, prints one line, and exits 0 — no credential file,
//! no `Egress`, no byte sent. It lives in its own module so "sends nothing" can
//! be pinned by scanning that module's source (see `mod statusline`).
//!
//! `--allow-proxy` (M9b) is the **only** proxy opt-in surface in the product.
//! Egress refuses to send anything while a proxy environment variable is set
//! and the user has not opted in — it fails closed, it does not quietly
//! connect directly. This flag is how a user says "yes, I know a
//! TLS-inspecting proxy can read my bearer token, do it anyway", for one run.
//! The window has no equivalent: it constructs its egress proxy-off
//! unconditionally (SECURITY.md invariant 7).

mod statusline;

use std::process::ExitCode;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use usage_core::egress::{Egress, EgressError};
use usage_core::model::{ProviderId, ProviderSnapshot};
use usage_core::providers::{
    ClaudeSubscription, CodexSubscription, ProviderError, UsageProvider, CODEX_DEFAULT_USER_AGENT,
};
use usage_core::update;

/// Sent when `--client-version` is omitted. Real Claude Code versions avoid
/// the provider's aggressively rate-limited fallback bucket (see
/// `claude_subscription` module docs in usage-core).
const DEFAULT_CLIENT_VERSION: &str = "0.0.0";

/// `--help` output. Every flag `parse_args` accepts must appear here verbatim;
/// a test scans the parser's own source for its accepted literals and asserts
/// each one is present, so a future flag cannot ship undocumented.
const HELP: &str = "\
QuotaPane CLI — read your own Claude and Codex subscription usage locally.

usage: quotapane-cli (--once | --watch <SECS>) [--json]
                     [--provider claude|codex|all] [--fail-at <N>]
                     [--client-version <VER>] [--debug-raw]
                     [--debug-raw-unsafe] [--allow-proxy]
       quotapane-cli --statusline
       quotapane-cli --check-update

Options:
  --once                  Poll once and exit. Exactly one of --once and
                          --watch is required.
  --watch <SECS>          Poll every SECS seconds until interrupted. SECS must
                          be at least 180 — the polling floor applies to
                          scripted polling just as it does to the window. Text
                          output precedes each cycle with a separator line,
                          `--- <RFC 3339 UTC timestamp> ---`; with --json each
                          cycle prints one compact line instead (NDJSON).
  --statusline            Read one Claude Code statusline JSON document from
                          stdin, print one line of quota, and exit 0. The third
                          mode, and the only one that sends nothing: the
                          numbers are already in the payload, so no credential
                          file is opened and no request is made. Combines with
                          no other flag, --client-version included: there is no
                          request for a version string to ride on. A payload
                          with no quota in it prints nothing and still exits
                          0 — a status line must never break its host. The line
                          is a human-readable surface and is NOT covered by the
                          --json stability contract.
  --check-update          Ask GitHub for the latest release tag, print one line,
                          and exit. The fourth mode, and the only request this
                          tool makes carrying no credential: one anonymous GET
                          with a fixed User-Agent and
                          no identifier of any kind — no version string, no OS,
                          no account — from which exactly one field is read.
                          Running this command IS the opt-in, so no preference
                          file is consulted; the window has its own separate
                          setting, off until you answer it. Exits 0 whether or
                          not a newer version exists, and 1 if the check could
                          not complete. Combines with no other flag.
  --fail-at <N>           Exit 3 if any window is at or over N percent used
                          (N is 1–100). Checked after the normal output is
                          printed, over every window of every provider that
                          polled successfully — headline and per-model both.
                          Under --watch, the first tripping cycle exits.
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
  --allow-proxy           Send this run through the proxy in your environment.
                          Off by default: while a proxy variable is set and
                          this flag is absent, egress sends nothing and fails
                          with an error. A TLS-inspecting proxy can read the
                          bearer token, so opting in is explicit, per run.
  -h, --help              Print this help and exit.
  --version               Print the version and exit.

Credentials are read from your local claude/codex CLI files, read-only; they
are never written, logged, or persisted. If a token has expired, run `claude`
or `codex` to refresh it.

exit codes:
  0  success; with --fail-at: all windows under the threshold
  1  a provider or credential error; with --check-update: the check failed
  2  usage error
  3  --fail-at tripped: a window reached the threshold
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

/// How this invocation polls: once and out, or every `SECS` until interrupted
/// — or, in the one case that polls nothing, the update check.
///
/// Exactly one is chosen at parse time, so nothing downstream has to handle
/// "neither" or "both".
///
/// [`Mode::CheckUpdate`] lives here rather than beside [`Invocation::Statusline`]
/// for one structural reason: the statusline mode must return *before* an
/// [`Egress`] exists, and this one needs exactly that `Egress` — the same one,
/// from the same `--allow-proxy` seam, so the CLI keeps its single chokepoint
/// constructor (pinned by `the_only_egress_constructor_call_is_fed_by_the_seam`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Once,
    Watch(u64),
    /// `--check-update`: one anonymous request for the latest release tag,
    /// one line of output, exit. No provider is polled and no credential file
    /// is opened — see `usage_core::update`.
    CheckUpdate,
}

/// The shortest `--watch` interval, taken from the poller's own floor rather
/// than restated.
///
/// A script polling faster than the window does would be the same endpoint
/// pressure the poller exists to avoid (DECISIONS.md §1: ≥180 s per provider),
/// so scripted polling gets the same limit — and gets it from the same
/// constant, so the two cannot drift apart.
const WATCH_MIN_INTERVAL_SECS: u64 = usage_core::poller::MIN_INTERVAL.as_secs();

/// Usage error for `--watch` below [`WATCH_MIN_INTERVAL_SECS`]. Names the
/// floor and why it exists: a bare "invalid value" would leave the script
/// author guessing at the number.
const WATCH_FLOOR_ERROR: &str = "--watch interval must be at least 180 seconds (the polling floor)";

/// Parse `--watch <SECS>`: whole seconds, at or above the polling floor.
fn parse_watch_interval(s: &str) -> Result<u64, String> {
    let secs: u64 = s
        .parse()
        .map_err(|_| format!("--watch requires a whole number of seconds (got {s:?})"))?;
    if secs < WATCH_MIN_INTERVAL_SECS {
        return Err(WATCH_FLOOR_ERROR.to_string());
    }
    Ok(secs)
}

/// Wall-clock seconds since the Unix epoch.
///
/// The CLI's only clock, and deliberately outside the threshold gate — that
/// stays a pure function of the snapshots. A clock set before 1970 yields 0
/// rather than panicking; a timestamp is a log ornament, not a reason to fail
/// a poll.
fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Civil (proleptic Gregorian) year/month/day for a day count since the epoch.
/// Howard Hinnant's `civil_from_days` — the inverse of the `days_from_civil`
/// usage-core parses timestamps with, and the reason this needs no dependency.
fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m as u64, d as u64)
}

/// Format a Unix second count as an RFC 3339 UTC timestamp,
/// `YYYY-MM-DDTHH:MM:SSZ`.
///
/// UTC with an explicit `Z`, never local time: a watcher's log is read later,
/// often on another machine, and an offsetless local timestamp is the exact
/// ambiguity usage-core's parser refuses to accept from a provider.
fn format_rfc3339_utc(unix_secs: u64) -> String {
    let (days, rem) = (
        (unix_secs / 86_400) as i64,
        // Seconds into the day; `days` above already floored the division.
        unix_secs % 86_400,
    );
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// The line that precedes each `--watch` text cycle.
fn watch_separator(unix_secs: u64) -> String {
    format!("--- {} ---", format_rfc3339_utc(unix_secs))
}

/// Does this invocation's JSON output become NDJSON — one compact line per
/// cycle — or stay the pretty document `--once --json` has always printed?
///
/// A named seam rather than an inline `matches!`, so "watch ⇒ NDJSON, once ⇒
/// byte-unchanged" is unit-testable and its single call site can be pinned.
fn json_is_ndjson(mode: Mode) -> bool {
    matches!(mode, Mode::Watch(_))
}

/// Does this cycle print the `--- <timestamp> ---` separator first?
///
/// `--watch` text output only. `--once` prints one block and has nothing to
/// delimit; a JSON cycle is already exactly one line, and a separator in that
/// stream would break every NDJSON consumer.
fn prints_cycle_separator(args: &Args) -> bool {
    matches!(args.mode, Mode::Watch(_)) && !args.json
}

/// Serialize a cycle's snapshots exactly as this invocation prints them.
///
/// `compact` is what makes `--watch --json` NDJSON: one line per cycle, so a
/// consumer can read the stream line by line as it arrives instead of waiting
/// for a document to close. Without it the output is the pretty form
/// `--once --json` has always emitted, byte for byte.
///
/// A single provider that failed to poll yields an empty string, which the
/// caller prints as nothing at all — not as `null`, and not as a blank line.
fn render_json(
    snapshots: &[ProviderSnapshot],
    multi: bool,
    compact: bool,
) -> serde_json::Result<String> {
    if multi {
        // `all` → array, even when partial or empty.
        if compact {
            serde_json::to_string(snapshots)
        } else {
            serde_json::to_string_pretty(snapshots)
        }
    } else {
        match snapshots.first() {
            Some(snapshot) if compact => serde_json::to_string(snapshot),
            Some(snapshot) => serde_json::to_string_pretty(snapshot),
            None => Ok(String::new()),
        }
    }
}

/// Parse the `--fail-at` value: a whole percentage in `1..=100`.
///
/// Neither end is clamped. `0` would trip on an untouched account and `101`
/// could never trip at all — both are a script author's mistake, and a gate
/// that silently "corrects" one is worse than a gate that refuses to start.
fn parse_fail_at(s: &str) -> Result<u32, String> {
    match s.parse::<u32>() {
        Ok(n) if (1..=100).contains(&n) => Ok(n),
        _ => Err(format!(
            "--fail-at must be a whole number from 1 to 100 (got {s:?})"
        )),
    }
}

/// A used fraction as a whole percentage, rounded **exactly as the window
/// rounds it** (`usage-ui`: `(f * 100.0).round().clamp(0.0, 100.0)`).
///
/// The agreement is the point: a gate that rounded differently would fail a
/// script at a number the user never saw on screen. The clamp also keeps a
/// provider reporting over-quota usage from printing `137%`.
fn window_percent(used_fraction: f64) -> i64 {
    (used_fraction * 100.0).round().clamp(0.0, 100.0) as i64
}

/// The worst window at or over `n` percent, or `None` if nothing reaches it.
///
/// Pure: no clock, no I/O, no exit — just the normalized snapshots the poll
/// already produced, so the whole gate is unit-testable without a network.
///
/// Every window of every snapshot counts, **headline and per-model**: a gate
/// fails safe, and a script that wants narrower semantics can filter `--json`
/// itself. A window with unknown usage is skipped rather than read as 0 or
/// 100 — "unknown" is not a measurement. Ties go to the earlier one, which
/// given the caller's ordering means provider order, then window order with
/// headline windows before per-model rows.
fn worst_at_or_over(snapshots: &[ProviderSnapshot], n: u32) -> Option<(ProviderId, &str, i64)> {
    let mut worst: Option<(ProviderId, &str, i64)> = None;
    for snapshot in snapshots {
        for window in snapshot.windows.iter().chain(snapshot.per_model.iter()) {
            let Some(fraction) = window.used_fraction else {
                continue;
            };
            let percent = window_percent(fraction);
            if percent < i64::from(n) {
                continue;
            }
            if worst.is_none_or(|(_, _, highest)| percent > highest) {
                worst = Some((snapshot.provider, window.label.as_str(), percent));
            }
        }
    }
    worst
}

/// The single stderr line a tripped `--fail-at` prints, byte-exact.
///
/// A named function rather than an inline `eprintln!` so the exact bytes are
/// unit-testable: this string ends up in other people's CI logs.
fn fail_at_line(provider: ProviderId, label: &str, percent: i64, n: u32) -> String {
    format!(
        "fail-at: {} {label} at {percent}% >= {n}%",
        provider_cli_name(provider)
    )
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

/// Stderr warning printed once before any request when `--allow-proxy` is on.
const PROXY_OPT_IN_WARNING: &str =
    "warning: --allow-proxy routes this run through the proxy in your environment. A \
     TLS-inspecting proxy terminates TLS, so at its decryption point it can observe the bearer \
     token QuotaPane sends. Opt in only on a network you trust with that token.";

/// Appended once after a run that egress refused because of the proxy gate.
///
/// The error itself already names the offending variable (whatever its
/// casing), so this line does not re-enumerate variable names — it only points
/// at the two ways forward.
const PROXY_GATE_HINT: &str =
    "hint: re-run with --allow-proxy to opt in, or unset the proxy variable.";

/// Holds no credential material — `client_version` is a public version
/// string — so deriving `Debug` here cannot leak a token.
#[derive(Debug)]
struct Args {
    /// `--once` or `--watch <SECS>` — exactly one, resolved at parse time.
    mode: Mode,
    json: bool,
    provider: ProviderSel,
    client_version: String,
    client_version_defaulted: bool,
    debug_raw: bool,
    /// Byte-exact dump. Implies `debug_raw`; never set on its own.
    debug_raw_unsafe: bool,
    /// Opt in to the proxy environment for this run (SECURITY.md invariant 7).
    allow_proxy: bool,
    /// `--fail-at <N>`: exit 3 when a window reaches N percent used.
    fail_at: Option<u32>,
}

/// The `proxy_opt_in` argument this run hands to [`Egress::new`].
///
/// Deliberately trivial, and deliberately a named function: it is the single
/// seam where the CLI decides whether a proxy-enabled chokepoint can exist.
/// A test pins both halves — that `--allow-proxy` is the only input that can
/// make it true, and that `main` has exactly one `Egress::new` call site fed
/// by this function — without reaching into the egress module, which this
/// change does not touch.
fn egress_proxy_opt_in(args: &Args) -> bool {
    args.allow_proxy
}

/// Did egress refuse this request because a proxy variable is set and the user
/// has not opted in (SECURITY.md invariant 7)?
///
/// Matched on the typed variant rather than on message text, so the CLI reacts
/// to the chokepoint's own refusal and a reworded error cannot silently drop
/// the hint.
fn is_proxy_gate_error(err: &ProviderError) -> bool {
    matches!(
        err,
        ProviderError::Egress(EgressError::ProxyNotOptedIn { .. })
    )
}

/// Appended after an expired-token failure: the exact command that refreshes
/// this provider's token.
///
/// Per provider, because the two CLIs differ — `claude` refreshes as a side
/// effect of starting work, `codex` has an explicit `login`. A pure function of
/// the id so the text is unit-testable without a poll.
fn token_expired_hint(id: ProviderId) -> &'static str {
    match id {
        ProviderId::ClaudeSubscription => {
            "hint: start any claude session (even `claude -p hi`) to refresh the token, then rerun"
        }
        ProviderId::CodexSubscription => "hint: run `codex login` to refresh the token, then rerun",
    }
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
    /// `--statusline`: format one stdin payload and exit. Carries no [`Args`]
    /// because there is nothing to configure — no provider to choose, no
    /// output shape to pick, and nothing to poll.
    Statusline,
    Run(Args),
}

/// The flags `--statusline` refuses to share an invocation with, in the order
/// they are reported.
///
/// Every one of them describes a request this mode never makes: most configure
/// polling, and `--client-version` names the version string a poll would send.
/// An invocation carrying both asked for two different programs, so the parser
/// refuses rather than silently ignoring one (the same reason `--once` and
/// `--watch` cannot be combined). The order is fixed here so the message a
/// script sees is deterministic; `--debug-raw-unsafe` precedes `--debug-raw`
/// because it sets both, and the flag the user actually typed is the one worth
/// naming.
///
/// `--client-version` was accepted-and-ignored when M18a shipped; the owner's
/// D3 ruling closed that, on this codebase's own standard that a silently
/// dropped flag reads as "the tool produced nothing", not "that flag does not
/// apply here".
fn statusline_conflict(seen: PollingFlags) -> Option<&'static str> {
    [
        ("--once", seen.once),
        ("--watch", seen.watch),
        ("--json", seen.json),
        ("--provider", seen.provider),
        ("--fail-at", seen.fail_at),
        ("--debug-raw-unsafe", seen.debug_raw_unsafe),
        ("--debug-raw", seen.debug_raw),
        ("--allow-proxy", seen.allow_proxy),
        ("--client-version", seen.client_version),
        ("--check-update", seen.check_update),
    ]
    .into_iter()
    .find(|(_, present)| *present)
    .map(|(flag, _)| flag)
}

/// Which polling flags this command line carried, for the check above.
///
/// A named struct rather than nine positional booleans: they are all the same
/// type, so a transposed pair would compile silently and report the wrong flag.
#[derive(Debug, Clone, Copy, Default)]
struct PollingFlags {
    once: bool,
    watch: bool,
    json: bool,
    provider: bool,
    fail_at: bool,
    debug_raw: bool,
    debug_raw_unsafe: bool,
    allow_proxy: bool,
    client_version: bool,
    check_update: bool,
}

fn parse_args<I: IntoIterator<Item = String>>(argv: I) -> Result<Invocation, String> {
    let mut once = false;
    let mut watch: Option<u64> = None;
    let mut json = false;
    let mut provider: Option<ProviderSel> = None;
    let mut client_version: Option<String> = None;
    let mut debug_raw = false;
    let mut debug_raw_unsafe = false;
    let mut allow_proxy = false;
    let mut fail_at: Option<u32> = None;
    let mut statusline = false;
    let mut check_update = false;

    let mut iter = argv.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            // Answered before any other validation, so `--help` works on its
            // own — without it the parser would reject the invocation for a
            // missing mode.
            "--help" | "-h" => return Ok(Invocation::Help),
            "--version" => return Ok(Invocation::Version),
            "--once" => once = true,
            "--statusline" => statusline = true,
            "--check-update" => check_update = true,
            "--watch" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--watch requires a value".to_string())?;
                watch = Some(parse_watch_interval(&value)?);
            }
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
            "--allow-proxy" => allow_proxy = true,
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
            "--fail-at" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--fail-at requires a value".to_string())?;
                fail_at = Some(parse_fail_at(&value)?);
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
    }

    // The statusline mode is resolved first and separately: it shares no
    // configuration with the polling modes, so it is answered here rather than
    // threaded through `Args` as a third `Mode` that every polling field would
    // then have to be meaningless for.
    if statusline {
        if let Some(flag) = statusline_conflict(PollingFlags {
            once,
            watch: watch.is_some(),
            json,
            provider: provider.is_some(),
            fail_at: fail_at.is_some(),
            debug_raw,
            debug_raw_unsafe,
            allow_proxy,
            client_version: client_version.is_some(),
            check_update,
        }) {
            return Err(format!(
                "--statusline cannot be combined with {flag}; it reads one JSON document from stdin and prints one line"
            ));
        }
        return Ok(Invocation::Statusline);
    }

    // The update check is resolved the same way and for the same reason: it
    // shares no configuration with the polling modes. It refuses the identical
    // flag set — every one of those flags describes a provider poll, and this
    // request is not one. `--statusline` is absent from the list because it was
    // answered above; an invocation carrying both never reaches here.
    if check_update {
        if let Some(flag) = statusline_conflict(PollingFlags {
            once,
            watch: watch.is_some(),
            json,
            provider: provider.is_some(),
            fail_at: fail_at.is_some(),
            debug_raw,
            debug_raw_unsafe,
            allow_proxy,
            client_version: client_version.is_some(),
            // Not its own conflict.
            check_update: false,
        }) {
            return Err(format!(
                "--check-update cannot be combined with {flag}; it asks GitHub for the latest release tag and prints one line"
            ));
        }
        return Ok(Invocation::Run(Args {
            mode: Mode::CheckUpdate,
            json: false,
            provider: ProviderSel::Claude,
            client_version: DEFAULT_CLIENT_VERSION.to_string(),
            // Nothing is polled, so there is no throttle note to suppress or
            // emit — the flag it refers to is a conflict in this mode anyway.
            client_version_defaulted: false,
            debug_raw: false,
            debug_raw_unsafe: false,
            allow_proxy: false,
            fail_at: None,
        }));
    }

    // Exactly one mode. "Both" is not a merge of two intents and "neither" is
    // not a default — either way the CLI would be guessing, so it refuses.
    let mode = match (once, watch) {
        (true, None) => Mode::Once,
        (false, Some(secs)) => Mode::Watch(secs),
        (true, Some(_)) => {
            return Err("--once and --watch cannot be combined; pass exactly one".to_string())
        }
        (false, None) => {
            return Err(
                "a mode is required: --once, --watch <SECS>, --statusline, or --check-update"
                    .to_string(),
            )
        }
    };

    let client_version_defaulted = client_version.is_none();
    Ok(Invocation::Run(Args {
        mode,
        json,
        provider: provider.unwrap_or(ProviderSel::Claude),
        client_version: client_version.unwrap_or_else(|| DEFAULT_CLIENT_VERSION.to_string()),
        client_version_defaulted,
        debug_raw,
        debug_raw_unsafe,
        allow_proxy,
        fail_at,
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

/// What one poll cycle produced.
struct Cycle {
    /// Snapshots from the providers that polled successfully, in output order.
    snapshots: Vec<ProviderSnapshot>,
    /// Whether any selected provider failed (credential, egress, or schema).
    had_error: bool,
}

/// Poll every selected provider once and print this cycle's output.
///
/// The entire per-cycle body, and the **only** one: `--once` calls it exactly
/// once, `--watch` calls it every tick. Keeping the two modes on one function
/// is what guarantees a watched cycle prints what a one-shot run prints — a
/// test pins the single call site so a watch-only path cannot appear later.
///
/// Printing lives here, not in `main`, because the output shape is per cycle:
/// under `--watch --json` each cycle is one NDJSON line, and a document that
/// spanned cycles could not be read until the watcher was killed.
fn run_cycle(
    args: &Args,
    ids: &[ProviderId],
    egress: &Egress,
    proxy_hint_shown: &mut bool,
) -> Cycle {
    let multi = matches!(args.provider, ProviderSel::All);
    let mut snapshots: Vec<ProviderSnapshot> = Vec::new();
    let mut had_error = false;

    // Poll each selected provider independently: one signed-out or erroring
    // provider records a clean diagnostic and flips the exit code, but never
    // aborts the others (`all` still emits whatever succeeded).
    for &id in ids {
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
                        .map(|p| p.debug_raw_body(egress))
                }
                ProviderId::CodexSubscription => {
                    CodexSubscription::with_default_path(CODEX_DEFAULT_USER_AGENT)
                        .map(|p| p.debug_raw_body(egress))
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
                    report_provider_error(id, &e, proxy_hint_shown);
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
            Some(provider) => match provider.poll(egress) {
                Ok(snapshot) => snapshots.push(snapshot),
                Err(e) => {
                    report_provider_error(id, &e, proxy_hint_shown);
                    had_error = true;
                }
            },
        }
    }

    if args.json {
        // `all` → array (even if partial/empty); single provider → object,
        // preserving the exact M1 output shape for the default invocation.
        // Compact only under `--watch`, which makes that stream NDJSON.
        match render_json(&snapshots, multi, json_is_ndjson(args.mode)) {
            Ok(s) if !s.is_empty() => println!("{s}"),
            Ok(_) => {} // single provider failed: nothing on stdout
            Err(e) => {
                eprintln!("error: failed to serialize snapshot(s): {e}");
                had_error = true;
            }
        }
    } else {
        for snapshot in &snapshots {
            print_summary(snapshot);
        }
    }

    Cycle {
        snapshots,
        had_error,
    }
}

/// `--check-update`: one anonymous request, one line, an exit code.
///
/// Three outcomes and three exit codes, because a script may want to act on
/// this: newer (0), current (0), and "could not tell" (1). The CLI is allowed
/// to be honest about a failed check in a way the window is not — the user
/// typed this command and is owed an answer — but it can say no more than that
/// it failed. `usage_core::update` has no error type to say more with, which
/// is deliberate: a failure detail is where a proxy variable's name or a URL
/// would leak into a terminal.
///
/// Passing `Some(true)` is the opt-in: `config.cfg` is not read here at all,
/// because typing the command IS asking. The window's stored preference has no
/// business gating a command the user just ran.
fn run_check_update(egress: &Egress) -> ExitCode {
    let (line, ok) = check_update_report(
        &update::check_outcome(egress, Some(true)),
        env!("CARGO_PKG_VERSION"),
    );
    if ok {
        println!("{line}");
        ExitCode::SUCCESS
    } else {
        eprintln!("{line}");
        ExitCode::FAILURE
    }
}

/// The line `--check-update` prints, and whether the run succeeded.
///
/// Pure, and separate from [`run_check_update`], so all three outcomes are
/// testable without a network: the only untested hop left is the one that
/// actually dials, which no test in this repository is allowed to do.
fn check_update_report(outcome: &update::CheckOutcome, running: &str) -> (String, bool) {
    match outcome {
        update::CheckOutcome::Newer(notice) => (
            format!(
                "quotapane {running} — {} available: {}",
                notice.version, notice.url
            ),
            true,
        ),
        update::CheckOutcome::Current => (format!("quotapane {running} — up to date"), true),
        // Deliberately no detail: `usage_core::update` has none to give, and a
        // reason here is where a URL or a proxy variable's name would surface.
        update::CheckOutcome::Inconclusive => ("update check failed".to_string(), false),
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
        // Answered here, and this arm returns: the statusline mode must be
        // resolved before the `Egress::new` below ever runs. Nothing in this
        // path opens a credential file or constructs a chokepoint — a test
        // pins both the ordering and the module's contents.
        Ok(Invocation::Statusline) => {
            let line = statusline::line(&statusline::read_payload(), now_unix_secs());
            // Nothing to say prints nothing at all, not a blank line.
            if !line.is_empty() {
                println!("{line}");
            }
            return ExitCode::SUCCESS;
        }
        Ok(Invocation::Run(a)) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: quotapane-cli (--once | --watch <SECS>) [--json] [--provider claude|codex|all] [--fail-at <N>] [--client-version <VER>] [--debug-raw] [--debug-raw-unsafe] [--allow-proxy]"
            );
            eprintln!("       quotapane-cli --statusline");
            eprintln!("       quotapane-cli --check-update");
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

    // Same rule for the proxy opt-in: warn once, before anything is sent.
    if args.allow_proxy {
        eprintln!("{PROXY_OPT_IN_WARNING}");
    }

    // The one and only `Egress::new` in this binary. Without `--allow-proxy`
    // this is `false` and the chokepoint's gate refuses to send while a proxy
    // variable is set — the CLI does not decide that, and does not route
    // around it; it only surfaces the way out (see `report_provider_error`).
    let egress = Egress::new(egress_proxy_opt_in(&args));

    // The update check, answered here and returning: it needs the chokepoint
    // above and nothing below — no provider is built, no credential file is
    // opened, and the poll loop is never entered.
    if args.mode == Mode::CheckUpdate {
        return run_check_update(&egress);
    }

    // The proxy hint is a once-per-run line, for the same reason the warnings
    // above are: `--provider all` failing twice on one gate does not need to
    // say it twice. It survives across watch cycles for the same reason.
    let mut proxy_hint_shown = false;

    loop {
        // Text cycles are delimited so a watcher's log can be read back: one
        // timestamped line, then the block a `--once` run would have printed.
        // JSON cycles need no delimiter — each is already exactly one line.
        if prints_cycle_separator(&args) {
            println!("{}", watch_separator(now_unix_secs()));
        }

        let cycle = run_cycle(&args, &ids, &egress, &mut proxy_hint_shown);

        // The gate runs after the cycle's output, so a script that trips still
        // has the full picture in its log. It sees only snapshots that actually
        // polled: an errored provider is exit-1 territory (below), never a
        // silent pass through the gate.
        if let Some(n) = args.fail_at {
            if let Some((provider, label, percent)) = worst_at_or_over(&cycle.snapshots, n) {
                eprintln!("{}", fail_at_line(provider, label, percent, n));
                // Precedence: a trip outranks a provider error. The script
                // asked "am I about to run out?", and the answer is yes.
                // Under --watch this is the first tripping cycle, and it ends
                // the run — a gate that kept watching after tripping would
                // have nothing left to say.
                return ExitCode::from(3);
            }
        }

        match args.mode {
            // `--check-update` returned before this loop was entered; if it
            // ever reached here it has nothing to repeat, so it leaves the way
            // `--once` does rather than looping forever.
            Mode::Once | Mode::CheckUpdate => {
                return if cycle.had_error {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                };
            }
            // A failed cycle does not end a watch: a watcher exists to survive
            // the transient failure that would otherwise kill the script it
            // guards. Sleeping (not scheduling) means the interval is the gap
            // *between* polls, so it can drift later than SECS but never
            // sooner — never under the floor.
            Mode::Watch(secs) => std::thread::sleep(Duration::from_secs(secs)),
        }
    }
}

/// Print one provider failure, plus — the first time a run is refused by the
/// proxy gate — the hint naming the way out.
///
/// The gate lives in the egress chokepoint and is not weakened here: a refused
/// run stays refused and still exits non-zero. All this adds is the sentence
/// that turns "egress denied" into something actionable.
///
/// An expired token earns the same treatment, per provider. Unlike the proxy
/// hint it is not deduplicated across a `--provider all` run: the two providers
/// need different commands, so each failing one says its own.
fn report_provider_error(id: ProviderId, err: &ProviderError, proxy_hint_shown: &mut bool) {
    eprintln!("error: {}: {err}", provider_cli_name(id));
    if is_proxy_gate_error(err) && !*proxy_hint_shown {
        eprintln!("{PROXY_GATE_HINT}");
        *proxy_hint_shown = true;
    }
    // Matched on the typed variant, not on message text — same discipline as
    // the proxy hint above.
    if matches!(err, ProviderError::TokenExpired) {
        eprintln!("{}", token_expired_hint(id));
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

/// Format a reset countdown compactly: `45s`, `12m`, `3h12m`, and — past two
/// days — `3d0h`.
///
/// The day unit is the M18a §8.2 ruling. Without it a weekly window three days
/// out read `72h0m`, which is arithmetic rather than an answer; the window has
/// always said `resets in 5d 17h` for the same span, so the two surfaces now
/// agree on the unit even though this one stays space-free for a status bar.
///
/// The switch is at **48 hours**, not the window's 24: a status line is read at
/// a glance and `36h0m` is still a number a reader holds in their head, while
/// the window has room to be gentler about it. Above the boundary the minutes
/// go — `3d0h` is the same information as `3d 0h 14m` for a countdown nobody is
/// timing to the minute.
fn format_reset(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs <= 172_800 {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d{}h", secs / 86_400, (secs % 86_400) / 3600)
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

    // --- token_expired_hint: the refresh instruction ---

    #[test]
    fn token_expired_hints_name_the_exact_command() {
        // Full equality: the milestone is the exact words, and `contains` would
        // pass on a hint that had lost the command or the "then rerun".
        assert_eq!(
            token_expired_hint(ProviderId::ClaudeSubscription),
            "hint: start any claude session (even `claude -p hi`) to refresh the token, then rerun"
        );
        assert_eq!(
            token_expired_hint(ProviderId::CodexSubscription),
            "hint: run `codex login` to refresh the token, then rerun"
        );
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

    // --- M9b: --allow-proxy is the only proxy opt-in surface ---

    // INV:7 — registered in invariants.manifest (checked in CI)
    #[test]
    fn allow_proxy_defaults_off_and_is_the_only_input_that_turns_it_on() {
        // The default must be off: the whole point of invariant 7 is that
        // routing a bearer token past a TLS terminator is a deliberate act.
        assert!(!parse_run(&["--once"]).allow_proxy);
        assert!(!egress_proxy_opt_in(&parse_run(&["--once"])));

        // No other flag can flip it — not the debug ones, not --json.
        let loaded = parse_run(&[
            "--once",
            "--json",
            "--debug-raw",
            "--debug-raw-unsafe",
            "--provider",
            "all",
            "--client-version",
            "1.2.3",
        ]);
        assert!(!egress_proxy_opt_in(&loaded));

        // And the flag itself does, in any position.
        assert!(egress_proxy_opt_in(&parse_run(&[
            "--once",
            "--allow-proxy"
        ])));
        assert!(egress_proxy_opt_in(&parse_run(&[
            "--allow-proxy",
            "--provider",
            "codex",
            "--once"
        ])));
    }

    #[test]
    fn the_only_egress_constructor_call_is_fed_by_the_seam() {
        // `egress_proxy_opt_in` being correct is worth nothing if `main`
        // constructs its `Egress` some other way. Pin the wiring by scanning
        // this file's own source: exactly one `Egress::new(` call site, and it
        // takes the seam — so `Egress::new(true)` is reachable only via
        // --allow-proxy, and a future edit that hardcodes `true` fails here.
        const SRC: &str = include_str!("main.rs");

        // Real code only: slice the test module off, then drop comment lines —
        // the call site's own comment necessarily discusses the constructor.
        let code: String = SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(
            code.matches("Egress::new(").count(),
            1,
            "the CLI must construct exactly one Egress"
        );
        assert!(
            code.contains("Egress::new(egress_proxy_opt_in(&args))"),
            "the sole Egress must be constructed from the --allow-proxy seam"
        );
        // A hardcoded opt-in would bypass the flag entirely.
        assert!(
            !code.contains("Egress::new(true)"),
            "proxy-enabled egress must be reachable only through --allow-proxy"
        );
    }

    // INV:7 — registered in invariants.manifest (checked in CI)
    #[test]
    fn proxy_gate_refusal_is_matched_on_the_variant_not_the_message() {
        use usage_core::egress::EgressError;

        assert!(is_proxy_gate_error(&ProviderError::Egress(
            EgressError::ProxyNotOptedIn {
                variable: "HTTPS_PROXY".to_string()
            }
        )));
        // Casing is the egress module's business — any variable name qualifies.
        assert!(is_proxy_gate_error(&ProviderError::Egress(
            EgressError::ProxyNotOptedIn {
                variable: "all_proxy".to_string()
            }
        )));

        // Nothing else earns the hint — an unrelated failure must not tell the
        // user to go turn a proxy flag on.
        for other in [
            ProviderError::Egress(EgressError::HostNotAllowlisted("evil.example".to_string())),
            ProviderError::Egress(EgressError::Transport("connection reset".to_string())),
            ProviderError::Credential("malformed auth.json".to_string()),
            ProviderError::TokenExpired,
            ProviderError::RateLimited {
                retry_after_secs: Some(60),
            },
            ProviderError::UnexpectedPayload,
        ] {
            assert!(
                !is_proxy_gate_error(&other),
                "{other:?} must not earn the hint"
            );
        }
    }

    // INV:7 — registered in invariants.manifest (checked in CI)
    #[test]
    fn help_documents_the_proxy_flag_and_its_fail_closed_default() {
        assert!(HELP.contains("--allow-proxy"), "{HELP}");
        // The consequence, not just the flag name: that the default refuses.
        assert!(HELP.contains("fails"), "{HELP}");
        assert!(HELP.contains("bearer token"), "{HELP}");
        // The warning and hint the user will actually see are honest about it.
        assert!(PROXY_OPT_IN_WARNING.contains("bearer token"));
        assert!(PROXY_GATE_HINT.contains("--allow-proxy"));
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
            "--statusline",
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

    // --- M12 P1: --fail-at, the scripted quota gate ---

    use usage_core::model::{QuotaWindow, SnapshotSource};

    /// A window with only the two fields the gate reads.
    fn win(label: &str, used_fraction: Option<f64>) -> QuotaWindow {
        QuotaWindow {
            label: label.to_string(),
            used_fraction,
            resets_in_secs: Some(3600),
            duration_secs: Some(18_000),
        }
    }

    /// A snapshot carrying the given headline and per-model windows.
    fn snap(
        provider: ProviderId,
        windows: Vec<QuotaWindow>,
        per_model: Vec<QuotaWindow>,
    ) -> ProviderSnapshot {
        ProviderSnapshot {
            provider,
            taken_at_unix_secs: 1_784_000_000,
            windows,
            per_model,
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        }
    }

    #[test]
    fn fail_at_defaults_off_and_parses_a_threshold() {
        assert_eq!(parse_run(&["--once"]).fail_at, None);
        assert_eq!(parse_run(&["--once", "--fail-at", "90"]).fail_at, Some(90));
        // Order-independent, like every other flag.
        assert_eq!(
            parse_run(&["--fail-at", "1", "--provider", "codex", "--once"]).fail_at,
            Some(1)
        );
    }

    #[test]
    fn fail_at_accepts_only_1_through_100() {
        assert_eq!(parse_fail_at("1"), Ok(1));
        assert_eq!(parse_fail_at("100"), Ok(100));
        // A gate at 0 would trip on an untouched account, and >100 could never
        // trip — both are user error, not a silently clamped value.
        for bad in ["0", "101", "-1", "50.5", "", "ninety", "90%", " 90"] {
            assert!(parse_fail_at(bad).is_err(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn fail_at_with_a_bad_value_or_no_value_is_a_usage_error() {
        assert!(parse_args(args(&["--once", "--fail-at"])).is_err());
        assert!(parse_args(args(&["--once", "--fail-at", "0"])).is_err());
        assert!(parse_args(args(&["--once", "--fail-at", "101"])).is_err());
        assert!(parse_args(args(&["--once", "--fail-at", "lots"])).is_err());
    }

    #[test]
    fn worst_at_or_over_returns_none_for_empty_input() {
        assert_eq!(worst_at_or_over(&[], 1), None);
        // A snapshot with no windows at all is the same story.
        let empty = snap(ProviderId::ClaudeSubscription, vec![], vec![]);
        assert_eq!(worst_at_or_over(std::slice::from_ref(&empty), 1), None);
    }

    #[test]
    fn worst_at_or_over_trips_at_exactly_the_threshold() {
        // The boundary the spec pins: == N trips, N-1 does not.
        let s = [snap(
            ProviderId::ClaudeSubscription,
            vec![win("5h", Some(0.90))],
            vec![],
        )];
        assert_eq!(
            worst_at_or_over(&s, 90),
            Some((ProviderId::ClaudeSubscription, "5h", 90))
        );
        assert_eq!(worst_at_or_over(&s, 91), None);
        assert_eq!(
            worst_at_or_over(&s, 89),
            Some((ProviderId::ClaudeSubscription, "5h", 90))
        );
    }

    #[test]
    fn worst_at_or_over_rounds_as_the_window_rounds() {
        // usage-ui renders `(f * 100.0).round().clamp(0.0, 100.0)`. The gate
        // must agree to the percentage point, or a script fails on a number
        // the user never saw.
        let half_up = [snap(
            ProviderId::ClaudeSubscription,
            vec![win("5h", Some(0.895))],
            vec![],
        )];
        assert_eq!(
            worst_at_or_over(&half_up, 90),
            Some((ProviderId::ClaudeSubscription, "5h", 90)),
            "0.895 renders as 90% and must trip a 90% gate"
        );

        let just_under = [snap(
            ProviderId::ClaudeSubscription,
            vec![win("5h", Some(0.894))],
            vec![],
        )];
        assert_eq!(
            worst_at_or_over(&just_under, 90),
            None,
            "0.894 renders as 89%"
        );

        // Over-quota fractions clamp to 100 rather than reporting 137%.
        let over = [snap(
            ProviderId::ClaudeSubscription,
            vec![win("5h", Some(1.37))],
            vec![],
        )];
        assert_eq!(
            worst_at_or_over(&over, 100),
            Some((ProviderId::ClaudeSubscription, "5h", 100))
        );
    }

    #[test]
    fn worst_at_or_over_covers_per_model_buckets() {
        // A gate fails safe: a per-model bucket at the threshold is a real
        // exhaustion for the model a script is about to use.
        let s = [snap(
            ProviderId::CodexSubscription,
            vec![win("5h", Some(0.10))],
            vec![win("GPT-5.3-Codex-Max", Some(0.97))],
        )];
        assert_eq!(
            worst_at_or_over(&s, 90),
            Some((ProviderId::CodexSubscription, "GPT-5.3-Codex-Max", 97))
        );
    }

    #[test]
    fn worst_at_or_over_picks_the_highest_percentage() {
        let s = [
            snap(
                ProviderId::ClaudeSubscription,
                vec![win("5h", Some(0.91)), win("7d", Some(0.95))],
                vec![win("7d-opus", Some(0.93))],
            ),
            snap(
                ProviderId::CodexSubscription,
                vec![win("5h", Some(0.99))],
                vec![],
            ),
        ];
        assert_eq!(
            worst_at_or_over(&s, 90),
            Some((ProviderId::CodexSubscription, "5h", 99))
        );
    }

    #[test]
    fn worst_at_or_over_breaks_ties_by_provider_then_window_order() {
        // Same percentage everywhere: the first provider in output order wins,
        // and within a provider the first window — headline before per-model.
        let s = [
            snap(
                ProviderId::ClaudeSubscription,
                vec![win("5h", Some(0.92)), win("7d", Some(0.92))],
                vec![win("7d-opus", Some(0.92))],
            ),
            snap(
                ProviderId::CodexSubscription,
                vec![win("5h", Some(0.92))],
                vec![],
            ),
        ];
        assert_eq!(
            worst_at_or_over(&s, 90),
            Some((ProviderId::ClaudeSubscription, "5h", 92))
        );

        // With the headline windows below the threshold, the tie is decided
        // among the per-model rows in their own order.
        let per_model_only = [snap(
            ProviderId::CodexSubscription,
            vec![win("5h", Some(0.10))],
            vec![win("first", Some(0.92)), win("second", Some(0.92))],
        )];
        assert_eq!(
            worst_at_or_over(&per_model_only, 90),
            Some((ProviderId::CodexSubscription, "first", 92))
        );
    }

    #[test]
    fn worst_at_or_over_ignores_windows_with_unknown_usage() {
        // "Unknown" is not "under the threshold" and not "over" it — the gate
        // simply has nothing to judge, and must not read it as 0 or as 100.
        let s = [snap(
            ProviderId::ClaudeSubscription,
            vec![win("5h", None), win("7d", Some(0.50))],
            vec![win("7d-opus", None)],
        )];
        assert_eq!(worst_at_or_over(&s, 90), None);
        assert_eq!(
            worst_at_or_over(&s, 50),
            Some((ProviderId::ClaudeSubscription, "7d", 50))
        );
    }

    #[test]
    fn fail_at_line_is_byte_exact() {
        // The line a script's log will show. Pinned in full: this is the
        // milestone's user-visible contract.
        assert_eq!(
            fail_at_line(ProviderId::ClaudeSubscription, "5h", 92, 90),
            "fail-at: claude 5h at 92% >= 90%"
        );
        assert_eq!(
            fail_at_line(ProviderId::CodexSubscription, "GPT-5.3-Codex-Max", 100, 100),
            "fail-at: codex GPT-5.3-Codex-Max at 100% >= 100%"
        );
    }

    #[test]
    fn help_documents_the_exit_codes_verbatim() {
        // Scripts branch on these numbers; the block is part of the contract.
        assert!(
            HELP.contains(
                "\
exit codes:
  0  success; with --fail-at: all windows under the threshold
  1  a provider or credential error; with --check-update: the check failed
  2  usage error
  3  --fail-at tripped: a window reached the threshold
"
            ),
            "{HELP}"
        );
    }

    // --- M12 P2: --watch, the second mode ---

    #[test]
    fn exactly_one_mode_is_required() {
        assert_eq!(parse_run(&["--once"]).mode, Mode::Once);
        assert_eq!(parse_run(&["--watch", "300"]).mode, Mode::Watch(300));
        assert_eq!(
            parse_run(&["--json", "--watch", "180", "--provider", "all"]).mode,
            Mode::Watch(180)
        );

        // Both is a usage error — the two modes mean different things and
        // guessing which one was meant is not the CLI's call.
        assert!(parse_args(args(&["--once", "--watch", "300"])).is_err());
        assert!(parse_args(args(&["--watch", "300", "--once"])).is_err());
        // Neither is still a usage error, as it was before --watch existed.
        assert!(parse_args(args(&["--json"])).is_err());
        assert!(parse_args(args(&[])).is_err());
    }

    #[test]
    fn watch_interval_floor_is_the_pollers_own() {
        // 179/180/181: the floor is inclusive, and it is not a rounded-off
        // approximation of the poller's — it IS the poller's constant.
        assert_eq!(
            WATCH_MIN_INTERVAL_SECS,
            usage_core::poller::MIN_INTERVAL.as_secs(),
            "the scripted floor must be the poller's own floor"
        );
        assert_eq!(WATCH_MIN_INTERVAL_SECS, 180);

        assert!(parse_watch_interval("179").is_err());
        assert_eq!(parse_watch_interval("180"), Ok(180));
        assert_eq!(parse_watch_interval("181"), Ok(181));
        assert_eq!(parse_watch_interval("3600"), Ok(3600));
    }

    #[test]
    fn watch_below_the_floor_names_the_floor_verbatim() {
        // Byte-exact: a script author who sees this line should learn the
        // number, not just that something was wrong.
        assert_eq!(
            parse_watch_interval("179").unwrap_err(),
            "--watch interval must be at least 180 seconds (the polling floor)"
        );
        assert_eq!(parse_watch_interval("0").unwrap_err(), WATCH_FLOOR_ERROR);
        assert_eq!(parse_watch_interval("1").unwrap_err(), WATCH_FLOOR_ERROR);
        // The message and the constant cannot drift apart silently.
        assert!(WATCH_FLOOR_ERROR.contains(&WATCH_MIN_INTERVAL_SECS.to_string()));
    }

    #[test]
    fn watch_rejects_non_integer_intervals_and_a_missing_value() {
        for bad in ["", "300.5", "-300", "five minutes", "300s"] {
            assert!(
                parse_watch_interval(bad).is_err(),
                "{bad:?} must be rejected"
            );
        }
        assert!(parse_args(args(&["--watch"])).is_err());
        assert!(parse_args(args(&["--watch", "abc"])).is_err());
    }

    #[test]
    fn rfc3339_utc_formats_known_anchors() {
        assert_eq!(format_rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_rfc3339_utc(946_684_800), "2000-01-01T00:00:00Z");
        // A leap day, and the second before a year boundary — the two places a
        // hand-rolled civil-date conversion goes wrong.
        assert_eq!(format_rfc3339_utc(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(format_rfc3339_utc(1_767_225_599), "2025-12-31T23:59:59Z");
        assert_eq!(format_rfc3339_utc(1_767_225_600), "2026-01-01T00:00:00Z");
        // Time-of-day fields, all three non-zero.
        assert_eq!(format_rfc3339_utc(1_784_000_000), "2026-07-14T03:33:20Z");
    }

    #[test]
    fn watch_separator_is_byte_exact() {
        // The line that delimits cycles in a watcher's log.
        assert_eq!(
            watch_separator(1_767_225_600),
            "--- 2026-01-01T00:00:00Z ---"
        );
        assert_eq!(watch_separator(0), "--- 1970-01-01T00:00:00Z ---");
    }

    #[test]
    fn once_json_output_is_byte_for_byte_the_pretty_form() {
        // The M12 promise to existing scripts: --once --json did not change.
        // Compared against the exact expression the pre-M12 CLI used.
        let claude = snap(
            ProviderId::ClaudeSubscription,
            vec![win("5h", Some(0.25))],
            vec![win("7d-opus", Some(0.5))],
        );
        let codex = snap(
            ProviderId::CodexSubscription,
            vec![win("5h", Some(0.1))],
            vec![],
        );

        assert_eq!(
            render_json(std::slice::from_ref(&claude), false, false).unwrap(),
            serde_json::to_string_pretty(&claude).unwrap()
        );
        let both = vec![claude, codex];
        assert_eq!(
            render_json(&both, true, false).unwrap(),
            serde_json::to_string_pretty(&both).unwrap()
        );
        // A single provider that failed to poll prints nothing at all, as before.
        assert_eq!(render_json(&[], false, false).unwrap(), "");
    }

    #[test]
    fn ndjson_is_one_compact_line_carrying_the_same_object() {
        let claude = snap(
            ProviderId::ClaudeSubscription,
            vec![win("5h", Some(0.25)), win("7d", None)],
            vec![win("7d-opus", Some(0.5))],
        );

        let line = render_json(std::slice::from_ref(&claude), false, true).unwrap();
        assert!(
            !line.contains('\n'),
            "an NDJSON cycle must be exactly one line: {line}"
        );
        assert!(!line.is_empty());

        // Same object, only the whitespace differs — one line per cycle must
        // not mean less data per cycle.
        let pretty = render_json(std::slice::from_ref(&claude), false, false).unwrap();
        let compact: serde_json::Value = serde_json::from_str(&line).unwrap();
        let expanded: serde_json::Value = serde_json::from_str(&pretty).unwrap();
        assert_eq!(compact, expanded);
        assert!(line.len() < pretty.len(), "compact form is not compact");

        // The --provider all array form is one line too.
        let both = vec![claude, snap(ProviderId::CodexSubscription, vec![], vec![])];
        let array_line = render_json(&both, true, true).unwrap();
        assert!(!array_line.contains('\n'), "{array_line}");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&array_line)
                .unwrap()
                .as_array()
                .unwrap()
                .len(),
            2
        );
    }

    /// This file's own real (non-test, non-comment) source, for the wiring
    /// pins below. Same technique as the `Egress::new` pin above: a pure
    /// function being right is worth nothing if nothing calls it.
    fn real_code() -> String {
        const SRC: &str = include_str!("main.rs");
        SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn ndjson_is_the_watch_mode_and_nothing_else() {
        assert!(
            !json_is_ndjson(Mode::Once),
            "--once --json must keep the pretty form"
        );
        assert!(json_is_ndjson(Mode::Watch(180)));
        assert!(json_is_ndjson(Mode::Watch(3600)));

        // And the cycle body's only JSON call is fed by that seam — so
        // `--once --json` cannot be switched to compact output, nor
        // `--watch --json` to a multi-line document, without failing here.
        let code = real_code();
        assert_eq!(
            code.matches("render_json(").count(),
            2,
            "expected one definition and exactly one call site"
        );
        assert!(
            code.contains("render_json(&snapshots, multi, json_is_ndjson(args.mode))"),
            "the JSON output shape must come from the mode seam"
        );
    }

    #[test]
    fn only_watch_text_output_gets_a_separator() {
        // The separator delimits cycles. One-shot output has nothing to
        // delimit, and a JSON cycle is already exactly one line — a stray
        // separator there would break every NDJSON consumer.
        let cases = [
            (&["--once"][..], false),
            (&["--once", "--json"][..], false),
            (&["--watch", "180"][..], true),
            (&["--watch", "180", "--json"][..], false),
        ];
        for (argv, expected) in cases {
            assert_eq!(
                prints_cycle_separator(&parse_run(argv)),
                expected,
                "wrong separator decision for {argv:?}"
            );
        }

        let code = real_code();
        assert_eq!(
            code.matches("watch_separator(").count(),
            2,
            "expected one definition and exactly one call site"
        );
        assert!(
            code.contains("if prints_cycle_separator(&args) {"),
            "the separator must be gated by the seam"
        );
    }

    #[test]
    fn both_modes_run_the_same_cycle_body() {
        // The spec's seam: --watch must not grow its own poll/print path that
        // could drift from --once. Pinned by scanning this file's own source —
        // exactly one definition and one call site, so a second, watch-only
        // body fails here. Same technique as the Egress::new pin above.
        const SRC: &str = include_str!("main.rs");

        let code: String = SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(code.contains("fn run_cycle("), "the cycle seam is missing");
        assert_eq!(
            code.matches("run_cycle(").count(),
            2,
            "expected one definition and exactly one call site"
        );
    }

    #[test]
    fn help_documents_the_watch_mode_and_its_floor() {
        assert!(HELP.contains("--watch <SECS>"), "{HELP}");
        // The floor is a number a script author must know before writing a
        // loop, so it appears in the help, not just in the error.
        assert!(
            HELP.contains(&WATCH_MIN_INTERVAL_SECS.to_string()),
            "{HELP}"
        );
        assert!(HELP.contains("NDJSON"), "{HELP}");
    }

    // --- M18a: --statusline, the third mode ---

    #[test]
    fn statusline_is_its_own_invocation_and_needs_no_other_mode() {
        // It does not satisfy "--once or --watch" — it replaces the question.
        assert!(matches!(
            parse_args(args(&["--statusline"])),
            Ok(Invocation::Statusline)
        ));
    }

    /// M18a D3, ruled 2026-08-11: `--client-version` is a conflict, not an
    /// inert extra. The flag names the version string a request would carry,
    /// and this mode makes no request — so accepting it silently would be the
    /// one thing the rest of this parser refuses to do.
    #[test]
    fn statusline_refuses_client_version_rather_than_ignoring_it() {
        for v in [
            &["--statusline", "--client-version", "1.2.3"][..],
            &["--client-version", "1.2.3", "--statusline"][..],
        ] {
            let err = parse_args(args(v))
                .err()
                .unwrap_or_else(|| panic!("{v:?} must be a usage error"));
            assert!(
                err.contains("--statusline cannot be combined with --client-version"),
                "{v:?} produced the wrong error: {err}"
            );
        }
    }

    #[test]
    fn statusline_refuses_every_polling_flag_and_names_the_one_it_found() {
        // Each conflict, in both orders — the flag before --statusline and
        // after it — because a parser that only checked one order would let
        // `--statusline --fail-at 85` through.
        let cases: [(&[&str], &str); 9] = [
            (&["--once"], "--once"),
            (&["--watch", "300"], "--watch"),
            (&["--json"], "--json"),
            (&["--provider", "all"], "--provider"),
            (&["--fail-at", "85"], "--fail-at"),
            (&["--debug-raw"], "--debug-raw"),
            (&["--debug-raw-unsafe"], "--debug-raw-unsafe"),
            (&["--allow-proxy"], "--allow-proxy"),
            (&["--client-version", "1.2.3"], "--client-version"),
        ];

        for (conflicting, expected) in cases {
            // --statusline before the flag and after it. A parser that checked
            // only one order would let `--statusline --fail-at 85` through.
            for statusline_first in [true, false] {
                let mut v: Vec<&str> = Vec::with_capacity(conflicting.len() + 1);
                if statusline_first {
                    v.push("--statusline");
                }
                v.extend_from_slice(conflicting);
                if !statusline_first {
                    v.push("--statusline");
                }

                let err = parse_args(args(&v))
                    .err()
                    .unwrap_or_else(|| panic!("{v:?} must be a usage error"));
                assert!(
                    err.contains("--statusline cannot be combined with"),
                    "{v:?} produced the wrong error: {err}"
                );
                assert!(
                    err.contains(expected),
                    "{v:?} must name {expected}, got: {err}"
                );
            }
        }
    }

    #[test]
    fn the_reported_statusline_conflict_is_the_first_in_a_fixed_order() {
        // With several present the message must be deterministic, not
        // whichever the argument order happened to surface.
        assert_eq!(
            statusline_conflict(PollingFlags {
                once: true,
                watch: true,
                json: true,
                provider: true,
                fail_at: true,
                debug_raw: true,
                debug_raw_unsafe: true,
                allow_proxy: true,
                client_version: true,
                check_update: true,
            }),
            Some("--once")
        );
        // Last in the order: reported only when it is the only one present.
        assert_eq!(
            statusline_conflict(PollingFlags {
                client_version: true,
                ..PollingFlags::default()
            }),
            Some("--client-version")
        );
        assert_eq!(
            statusline_conflict(PollingFlags {
                allow_proxy: true,
                client_version: true,
                ..PollingFlags::default()
            }),
            Some("--allow-proxy")
        );
        assert_eq!(
            statusline_conflict(PollingFlags {
                json: true,
                fail_at: true,
                ..PollingFlags::default()
            }),
            Some("--json")
        );
        // --debug-raw-unsafe sets both flags; the one the user typed is named.
        assert_eq!(
            statusline_conflict(PollingFlags {
                debug_raw: true,
                debug_raw_unsafe: true,
                ..PollingFlags::default()
            }),
            Some("--debug-raw-unsafe")
        );
        assert_eq!(
            statusline_conflict(PollingFlags {
                debug_raw: true,
                ..PollingFlags::default()
            }),
            Some("--debug-raw")
        );
        // Nothing conflicting: the mode stands alone.
        assert_eq!(statusline_conflict(PollingFlags::default()), None);
    }

    #[test]
    fn the_statusline_mode_is_resolved_before_any_egress_is_constructed() {
        // The ordering, not just the absence: "this mode sends nothing" must
        // not rest on `Egress::new` happening to be harmless to call. The arm
        // has to appear — and return — ahead of the constructor in `main`.
        // The `=>` matters: `parse_args` also *returns* `Ok(Invocation::
        // Statusline)`, and that one is not the arm under test.
        let code = real_code();
        let arm = code
            .find("Ok(Invocation::Statusline) =>")
            .expect("main does not handle Invocation::Statusline");
        let egress = code
            .find("Egress::new(")
            .expect("the Egress construction moved");
        assert!(
            arm < egress,
            "the statusline arm must come before the Egress construction"
        );

        let next_arm = arm
            + code[arm..]
                .find("Ok(Invocation::Run")
                .expect("the Run arm moved");
        assert!(
            code[arm..next_arm].contains("return ExitCode::SUCCESS"),
            "the statusline arm must return, not fall through to the poll path"
        );
    }

    #[test]
    fn the_statusline_module_is_the_only_place_that_formats_the_line() {
        // One call site, fed by the module's own reader — so a future edit
        // cannot grow a second statusline path in `main` that skips the
        // module's zero-egress guarantees.
        let code = real_code();
        assert_eq!(
            code.matches("statusline::line(").count(),
            1,
            "expected exactly one statusline::line call site"
        );
        assert!(
            code.contains("statusline::line(&statusline::read_payload(), now_unix_secs())"),
            "the statusline arm must read its payload from the module's reader"
        );
    }

    #[test]
    fn help_documents_the_statusline_mode_and_what_it_does_not_do() {
        assert!(HELP.contains("--statusline"), "{HELP}");
        // Its own synopsis line: it combines with no polling flag, and folding
        // it into the bracketed one would say the opposite.
        assert!(
            HELP.contains("       quotapane-cli --statusline\n"),
            "the statusline mode needs its own usage line: {HELP}"
        );
        // The two claims a reader has to be able to check: it sends nothing,
        // and its output is not the JSON contract.
        assert!(HELP.contains("sends nothing"), "{HELP}");
        assert!(HELP.contains("stability"), "{HELP}");
    }

    #[test]
    fn the_missing_mode_error_names_all_four_modes() {
        let err = parse_args(args(&["--json"])).unwrap_err();
        for mode in ["--once", "--watch", "--statusline", "--check-update"] {
            assert!(err.contains(mode), "the mode error omits {mode}: {err}");
        }
    }

    // --- M18b: --check-update, the fourth mode ---

    #[test]
    fn check_update_is_its_own_mode_and_needs_no_other() {
        assert_eq!(
            parse_run(&["--check-update"]).mode,
            Mode::CheckUpdate,
            "--check-update must satisfy the mode requirement by itself"
        );
    }

    /// The mode polls nothing, so it inherits none of the polling
    /// configuration: the defaults it carries exist only to keep one `Args`
    /// type, and the parser must not let a user set any of them.
    #[test]
    fn check_update_carries_no_polling_configuration() {
        let parsed = parse_run(&["--check-update"]);
        assert!(!parsed.json);
        assert!(!parsed.debug_raw);
        assert!(!parsed.debug_raw_unsafe);
        assert_eq!(parsed.fail_at, None);
        // Never proxy-enabled: --allow-proxy is a conflict, so the seam this
        // mode hands the chokepoint can only ever be false.
        assert!(!parsed.allow_proxy);
        assert!(!egress_proxy_opt_in(&parsed));
        // And it prints no throttle note: there is no poll for a client
        // version to be missing from.
        assert!(!parsed.client_version_defaulted);
    }

    #[test]
    fn check_update_refuses_every_other_flag_and_names_the_one_it_found() {
        let cases: [(&[&str], &str); 10] = [
            (&["--once"], "--once"),
            (&["--watch", "300"], "--watch"),
            (&["--json"], "--json"),
            (&["--provider", "all"], "--provider"),
            (&["--fail-at", "85"], "--fail-at"),
            (&["--debug-raw"], "--debug-raw"),
            (&["--debug-raw-unsafe"], "--debug-raw-unsafe"),
            (&["--allow-proxy"], "--allow-proxy"),
            (&["--client-version", "1.2.3"], "--client-version"),
            (&["--statusline"], "--statusline"),
        ];

        for (conflicting, expected) in cases {
            // Both orders, for the reason the statusline cases test both.
            for check_first in [true, false] {
                let mut v: Vec<&str> = Vec::with_capacity(conflicting.len() + 1);
                if check_first {
                    v.push("--check-update");
                }
                v.extend_from_slice(conflicting);
                if !check_first {
                    v.push("--check-update");
                }

                let err = parse_args(args(&v))
                    .err()
                    .unwrap_or_else(|| panic!("{v:?} must be a usage error"));
                assert!(
                    err.contains("cannot be combined with"),
                    "{v:?} produced the wrong error: {err}"
                );
                assert!(
                    err.contains(expected),
                    "{v:?} must name {expected}, got: {err}"
                );
            }
        }
    }

    /// `--statusline --check-update` asked for two modes that both refuse
    /// company. Whichever message wins, it must be a refusal naming the other
    /// flag — never a silent win for one of them.
    #[test]
    fn the_two_non_polling_modes_refuse_each_other() {
        for v in [
            &["--statusline", "--check-update"][..],
            &["--check-update", "--statusline"][..],
        ] {
            let err = parse_args(args(v))
                .err()
                .unwrap_or_else(|| panic!("{v:?} must be a usage error"));
            assert!(
                err.contains("--statusline cannot be combined with --check-update"),
                "{v:?} produced: {err}"
            );
        }
    }

    #[test]
    fn the_update_check_returns_before_the_poll_path_is_entered() {
        // The structural half of "this mode polls nothing": the arm appears
        // after the one Egress construction (it needs the chokepoint) and
        // before any provider is built (it needs nothing else). Same technique
        // as the statusline ordering pin.
        let code = real_code();
        let egress = code
            .find("Egress::new(")
            .expect("the Egress construction moved");
        let arm = code
            .find("if args.mode == Mode::CheckUpdate {")
            .expect("main does not dispatch the update check");
        // The poll path's entry point, as called from `main` — providers are
        // built inside `run_cycle`, which is defined above `main`, so the call
        // is what to look for rather than the definition.
        let poll = arm
            + code[arm..]
                .find("run_cycle(&args,")
                .expect("no run_cycle call after the update-check arm");
        let returns = arm
            + code[arm..]
                .find("return run_check_update(&egress);")
                .expect("the update-check arm does not return");
        assert!(
            egress < arm,
            "the update check needs the chokepoint, so it must follow it"
        );
        assert!(
            returns < poll,
            "the update-check arm must return, not fall through to the poll path"
        );
    }

    #[test]
    fn the_update_check_is_the_only_caller_of_the_update_module() {
        // One call site, and it passes the literal opt-in: running the command
        // IS the consent, and config.cfg is never consulted here.
        let code = real_code();
        assert_eq!(
            code.matches("update::check_outcome(").count(),
            1,
            "expected exactly one update::check_outcome call site"
        );
        assert!(
            code.contains("update::check_outcome(egress, Some(true))"),
            "the CLI's opt-in is the command itself, passed as a literal"
        );
        // The window's preference file has no say over a command the user just
        // typed — and the CLI has no way to read it: the preferences module
        // lives in usage-ui and is never named here.
        let outside_help = code.split("\";").nth(1).expect("HELP is not terminated");
        assert!(
            !outside_help.contains("config"),
            "the CLI must not consult the window's preferences"
        );
    }

    /// All three outcomes, byte-exact. This is the surface a human reads and a
    /// script may grep, so it is pinned in full rather than by substring.
    #[test]
    fn the_three_update_check_outcomes_are_byte_exact() {
        use usage_core::update::{CheckOutcome, UpdateNotice};

        let newer = CheckOutcome::Newer(UpdateNotice {
            version: "v1.8.0".to_string(),
            url: usage_core::update::RELEASES_URL,
        });
        assert_eq!(
            check_update_report(&newer, "1.7.0"),
            (
                "quotapane 1.7.0 — v1.8.0 available: github.com/cipherpine/quotapane/releases"
                    .to_string(),
                true
            )
        );

        assert_eq!(
            check_update_report(&CheckOutcome::Current, "1.7.0"),
            ("quotapane 1.7.0 — up to date".to_string(), true)
        );

        // The failure line says that it failed and nothing else — no host, no
        // URL, no reason. The `false` is exit 1.
        let (line, ok) = check_update_report(&CheckOutcome::Inconclusive, "1.7.0");
        assert_eq!(line, "update check failed");
        assert!(!ok, "a failed check must exit non-zero");
        assert!(!line.contains("github"), "the failure line names no host");
        assert!(
            !line.contains("1.7.0"),
            "the failure line claims no version"
        );
    }

    #[test]
    fn help_documents_the_update_check_and_its_anonymity() {
        assert!(HELP.contains("--check-update"), "{HELP}");
        // Its own synopsis line, like --statusline: it combines with nothing.
        assert!(
            HELP.contains("       quotapane-cli --check-update\n"),
            "the update check needs its own usage line: {HELP}"
        );
        // The two claims a reader must be able to check before running it.
        assert!(HELP.contains("no credential"), "{HELP}");
        assert!(HELP.contains("no identifier"), "{HELP}");
    }

    // --- M18a §8.2, ruled: the countdown grows a day unit ---

    /// The whole format as a table, boundaries included.
    ///
    /// A table rather than one assertion per case: the units are a single
    /// decision and reading them in a column is how you see a gap. The two rows
    /// that matter are `172_800` and `172_801` — the ruling is "above 48h", so
    /// exactly two days is still hours.
    #[test]
    fn format_reset_renders_days_only_above_forty_eight_hours() {
        for (secs, expected) in [
            (0_u64, "0s"),
            (1, "1s"),
            (59, "59s"),
            (60, "1m"),
            (3_599, "59m"),
            (3_600, "1h0m"),
            (7_830, "2h10m"),
            (86_400, "24h0m"),
            (172_799, "47h59m"),
            // Exactly 48h is not "above" it: the hour form holds.
            (172_800, "48h0m"),
            // One second past, and the unit changes.
            (172_801, "2d0h"),
            // The ruling's own example: three days out reads 3d0h, not 72h0m.
            (259_200, "3d0h"),
            (446_400, "5d4h"),
            (604_800, "7d0h"),
        ] {
            assert_eq!(
                format_reset(secs),
                expected,
                "format_reset({secs}) should be {expected:?}"
            );
        }
    }

    /// The ruling's stated purpose, as a property rather than a sample: past
    /// the boundary nothing reads in bare hours any more.
    #[test]
    fn no_countdown_past_two_days_is_reported_in_hours() {
        for days in 3..=14_u64 {
            let rendered = format_reset(days * 86_400 + 3_600);
            assert!(
                rendered.contains('d'),
                "{days} days rendered without a day unit: {rendered}"
            );
            assert!(
                !rendered.contains('m'),
                "a multi-day countdown should not carry minutes: {rendered}"
            );
        }
    }

    /// The statusline's own segment, end to end — the surface the ruling was
    /// about, not just the helper underneath it.
    #[test]
    fn the_statusline_countdown_uses_the_day_unit_for_a_weekly_window() {
        let doc = format!(
            r#"{{"rate_limits": {{"seven_day": {{"used_percentage": 41, "resets_at": {}}}}}}}"#,
            1_784_000_000_u64 + 259_200
        );
        assert_eq!(
            statusline::line(&doc, 1_784_000_000),
            "7d 41% · resets 3d0h"
        );
    }
}
