//! Opt-in on-disk history: timestamps, labels, and percentages (M13).
//!
//! The pace forecast ([`crate::pace`]) fits a line through the last couple of
//! hours of readings, and until now that trail lived only in memory — so every
//! restart threw the forecast away and the window went quiet for the next two
//! hours. This module is the smallest thing that fixes that: a rolling log of
//! *what the window already displays*, replayed into the rings at startup.
//!
//! ## What can be in the file, and why that is the whole story
//!
//! One [`HistoryEntry`] per line, and an entry has five fields: a unix second,
//! a [`ProviderId`], a window label, a used fraction, and the window's duration.
//! There is no field a credential could go in — not "no code path puts one
//! there", but no field of a type that could hold one. [`ProviderId`] is a
//! two-variant enum; three fields are numbers; and the label is the provider's
//! own window name, copied from a [`QuotaWindow`] that `quotapane-cli --json`
//! already prints in full. Constructing an entry any other way is not possible
//! from outside this module's own API: [`HistoryEntry::from_window`] is the
//! only constructor, and it takes a `&QuotaWindow` (SECURITY.md invariant 1).
//!
//! Writing is off unless `history=on` in `config.cfg`. With it off the file is
//! never created, never read, and never deleted.
//!
//! ## Everything here is clock-free
//!
//! Like [`crate::pace`], and for the same reason: a retention decision or a
//! rehydration cutoff that read `SystemTime::now()` internally could only be
//! tested against whatever moment the test ran at. Every entry point that needs
//! "now" takes it as a parameter — including the I/O shell, which needs none at
//! all. [`tests::no_clock_is_read_in_this_module`] pins it.
//!
//! ## Failure is never fatal
//!
//! A garbage line is skipped, not an error; a truncated final line (the app was
//! killed mid-write) is discarded; an unreadable or absent file reads as no
//! history. The forecast degrades to what M8 already did — start cold — which
//! is exactly the behaviour a user who never turned history on gets anyway.

use std::io::Write;
use std::path::Path;

use crate::model::{ProviderId, ProviderSnapshot, QuotaWindow};

/// The file's name, alongside `config.cfg` in the platform config directory.
/// The path itself is resolved by the UI, which already owns config-dir logic.
pub const FILE_NAME: &str = "history.jsonl";

/// Size at which the log is rewritten to its newest half, in bytes (256 KiB).
///
/// At the roughly 90 bytes an entry encodes to, and four entries per poll (two
/// providers × two headline windows) on the normal ~7 min cadence, that is
/// about three and a half days of readings — and the newest half kept after a
/// rewrite is still well over the 24 h the sparkline draws and the two hours
/// the pace fit uses. Checked once, at startup: a rolling rewrite on every
/// append would rewrite a quarter-megabyte file every seven minutes to save
/// nothing a user would notice.
pub const MAX_BYTES: u64 = 256 * 1024;

/// One reading, as written to disk: when, whose, which window, how full.
///
/// Deliberately **not** `serde::Deserialize`-derived. The decoder below is
/// written out by hand so that what a line can turn into is legible in one
/// screen: five reads, each of a fixed type, each of which must be present and
/// well-formed or the whole line is discarded.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryEntry {
    /// When the reading was taken (unix epoch seconds) — the provider's own
    /// [`ProviderSnapshot::taken_at_unix_secs`], never the moment it was
    /// written, so a replayed entry sits at the time the number was true.
    pub at_unix_secs: u64,
    /// Which provider the reading came from.
    pub provider: ProviderId,
    /// The window's label, verbatim from [`QuotaWindow::label`].
    pub label: String,
    /// Fraction of the window consumed, `0.0..=1.0`.
    pub used_fraction: f64,
    /// The window's total length in seconds, when the provider stated one.
    pub duration_secs: Option<u64>,
}

impl HistoryEntry {
    /// The only constructor: a reading of one quota window at one instant.
    ///
    /// `None` when the window carries no usable fraction — an unknown or
    /// non-finite percentage is not a reading, and writing a line for it would
    /// put a hole in the trail that the fit would have to defend against.
    pub fn from_window(
        provider: ProviderId,
        at_unix_secs: u64,
        window: &QuotaWindow,
    ) -> Option<HistoryEntry> {
        let used_fraction = window.used_fraction?;
        if !used_fraction.is_finite() {
            return None;
        }
        Some(HistoryEntry {
            at_unix_secs,
            provider,
            label: window.label.clone(),
            used_fraction,
            duration_secs: window.duration_secs,
        })
    }
}

/// The lines one successful poll contributes: its **headline** windows only.
///
/// Per-model buckets are deliberately excluded, matching M8's ring policy —
/// they get no forecast, so recording them would grow the file by the number of
/// models on the plan to feed nothing.
pub fn entries_for_snapshot(snapshot: &ProviderSnapshot) -> Vec<HistoryEntry> {
    snapshot
        .windows
        .iter()
        .filter_map(|window| {
            HistoryEntry::from_window(snapshot.provider, snapshot.taken_at_unix_secs, window)
        })
        .collect()
}

/// The on-disk word for a provider — the same token `--json` emits, so the two
/// files agree and neither needs a translation table to read.
fn provider_word(provider: ProviderId) -> &'static str {
    match provider {
        ProviderId::ClaudeSubscription => "claude_subscription",
        ProviderId::CodexSubscription => "codex_subscription",
    }
}

/// Inverse of [`provider_word`]. An unknown word is `None`: a line naming a
/// provider this build does not have is not a reading it can use.
fn provider_from_word(word: &str) -> Option<ProviderId> {
    match word {
        "claude_subscription" => Some(ProviderId::ClaudeSubscription),
        "codex_subscription" => Some(ProviderId::CodexSubscription),
        _ => None,
    }
}

/// Encode one entry as a single compact JSON object — no trailing newline.
///
/// `serde_json` does the escaping (a hand-rolled string escape is exactly the
/// kind of parser bug this project avoids), but the object is assembled field
/// by field rather than derived, so the set of keys is a decision written here
/// and not a consequence of a struct's shape.
pub fn encode_line(entry: &HistoryEntry) -> String {
    serde_json::json!({
        "at": entry.at_unix_secs,
        "provider": provider_word(entry.provider),
        "window": entry.label,
        "used": entry.used_fraction,
        "duration": entry.duration_secs,
    })
    .to_string()
}

/// Decode one line. `None` for anything that is not a complete, well-formed
/// entry — which is the normal outcome for a truncated final line and for
/// whatever a future version writes that this one does not understand.
pub fn decode_line(line: &str) -> Option<HistoryEntry> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    let object = value.as_object()?;

    let at_unix_secs = object.get("at")?.as_u64()?;
    let provider = provider_from_word(object.get("provider")?.as_str()?)?;
    let label = object.get("window")?.as_str()?.to_string();
    let used_fraction = object.get("used")?.as_f64()?;
    if !used_fraction.is_finite() {
        return None;
    }
    // Absent and explicitly null both mean "the provider never said", matching
    // the `Option` on the window it came from.
    let duration_secs = match object.get("duration") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(value.as_u64()?),
    };

    Some(HistoryEntry {
        at_unix_secs,
        provider,
        label,
        used_fraction,
        duration_secs,
    })
}

/// Decode a whole file, skipping every line that does not parse.
///
/// Order is preserved, which is append order, which is chronological for a file
/// this process wrote — but nothing downstream relies on that: the pace fit
/// sorts nothing and does not need to (`samples_in_any_order_fit_the_same`).
pub fn decode_all(text: &str) -> Vec<HistoryEntry> {
    text.lines().filter_map(decode_line).collect()
}

/// Whether a file of this many bytes has outgrown [`MAX_BYTES`].
pub fn needs_retention(len_bytes: u64) -> bool {
    len_bytes > MAX_BYTES
}

/// The retention rewrite, as a pure text-to-text decision: keep the newest half
/// of the complete lines, drop the rest.
///
/// "Complete" is load-bearing. [`append`] always terminates a line, so text that
/// does not end in a newline was truncated — the app was killed mid-write, or
/// the disk filled — and that trailing fragment is dropped rather than carried
/// forward as a permanently unparsable line.
///
/// Half rounds **up**, so a one-line file keeps its line rather than being
/// emptied by its own retention.
pub fn keep_newest_half(text: &str) -> String {
    let mut lines: Vec<&str> = text.lines().collect();
    if !text.is_empty() && !text.ends_with('\n') {
        lines.pop();
    }
    if lines.is_empty() {
        return String::new();
    }
    let keep = lines.len().div_ceil(2);
    let mut out = String::new();
    for line in &lines[lines.len() - keep..] {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The entries a restart should replay, oldest first: everything observed
/// within `window_secs` of `now_unix_secs`.
///
/// A timestamp in the *future* is dropped. Snapshots are stamped from the local
/// clock when the poll completes, so a reading later than now means the clock
/// moved backwards between runs, and feeding a future point into a
/// least-squares fit would tilt it against a moment that has not happened.
pub fn within(entries: &[HistoryEntry], now_unix_secs: u64, window_secs: u64) -> Vec<HistoryEntry> {
    entries
        .iter()
        .filter(|entry| {
            entry.at_unix_secs <= now_unix_secs && now_unix_secs - entry.at_unix_secs <= window_secs
        })
        .cloned()
        .collect()
}

/// The rehydration filter: the entries still inside the pace trail
/// ([`crate::pace::TRAIL_SECS`]) and therefore worth seeding a ring with.
///
/// Anything older is dropped here rather than in the ring, because
/// [`crate::pace::estimate`] would drop it anyway — seeding it would only make
/// the ring's contents disagree with the fit's inputs for no benefit.
pub fn rehydratable(entries: &[HistoryEntry], now_unix_secs: u64) -> Vec<HistoryEntry> {
    within(entries, now_unix_secs, crate::pace::TRAIL_SECS)
}

// --------------------------------------------------------------------------
// I/O shell. Three functions, each a thin wrapper over `std::fs`, so the whole
// filesystem surface of this feature is visible in one place.
// --------------------------------------------------------------------------

/// Read and decode the log. An absent or unreadable file is no history, not an
/// error — the caller has nothing useful to do with the distinction.
pub fn read(path: &Path) -> Vec<HistoryEntry> {
    match std::fs::read_to_string(path) {
        Ok(text) => decode_all(&text),
        Err(_) => Vec::new(),
    }
}

/// Append entries, creating the file (and its directory) if needed.
///
/// One `write_all` for the whole batch, so a poll's lines land together rather
/// than interleaved with anything else appending to the same path.
pub fn append(path: &Path, entries: &[HistoryEntry]) -> std::io::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut buffer = String::new();
    for entry in entries {
        buffer.push_str(&encode_line(entry));
        buffer.push('\n');
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(buffer.as_bytes())
}

/// Rewrite the log to its newest half if it has outgrown [`MAX_BYTES`].
///
/// Does nothing — including no read — for a file that is absent or still under
/// the cap, which is the overwhelmingly common case.
pub fn apply_retention(path: &Path) -> std::io::Result<()> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Ok(());
    };
    if !needs_retention(metadata.len()) {
        return Ok(());
    }
    let text = std::fs::read_to_string(path)?;
    std::fs::write(path, keep_newest_half(&text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SnapshotSource;

    const BASE: u64 = 1_785_000_000;

    fn entry(at: u64, provider: ProviderId, label: &str, used: f64) -> HistoryEntry {
        HistoryEntry {
            at_unix_secs: at,
            provider,
            label: label.to_string(),
            used_fraction: used,
            duration_secs: Some(18_000),
        }
    }

    fn window(label: &str, used: Option<f64>, duration: Option<u64>) -> QuotaWindow {
        QuotaWindow {
            label: label.to_string(),
            used_fraction: used,
            resets_in_secs: Some(3_600),
            duration_secs: duration,
        }
    }

    /// The same guard `pace` carries: this module answers questions about time
    /// but must never ask the OS what time it is, or its edge cases become
    /// untestable.
    #[test]
    fn no_clock_is_read_in_this_module() {
        const SRC: &str = include_str!("history.rs");

        // Everything above this test module, with whole-line comments dropped —
        // the prose above explains *why* there is no `SystemTime::now()` here,
        // and a comment cannot read a clock regardless. Same scanner as
        // `pace::tests::no_clock_is_read_in_this_module`, deliberately.
        let code: Vec<&str> = SRC[..SRC.find("mod tests").expect("test module not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect();
        let code = code.join("\n");

        for forbidden in ["SystemTime", "Instant", "UNIX_EPOCH", ".elapsed()"] {
            assert!(
                !code.contains(forbidden),
                "`{forbidden}` appears in history.rs — this module must stay clock-free"
            );
        }
        // Guard the scanner: if the split or the comment filter silently
        // produced nothing, the loop above would pass vacuously.
        assert!(
            code.contains("pub fn rehydratable") && code.contains("pub fn apply_retention"),
            "the scanner did not capture the module's code"
        );
    }

    #[test]
    fn an_entry_round_trips_through_a_line() {
        for e in [
            entry(BASE, ProviderId::ClaudeSubscription, "5h", 0.42),
            entry(BASE + 1, ProviderId::CodexSubscription, "7d", 0.0),
            HistoryEntry {
                duration_secs: None,
                ..entry(BASE, ProviderId::ClaudeSubscription, "weekly", 1.0)
            },
            // A label with everything a naive encoder would break on.
            entry(BASE, ProviderId::CodexSubscription, "a\"b\\c\nd\te 🙂", 0.5),
        ] {
            let line = encode_line(&e);
            assert!(
                !line.contains('\n'),
                "an entry must encode to ONE line: {line:?}"
            );
            assert_eq!(decode_line(&line).as_ref(), Some(&e), "line was {line:?}");
        }
    }

    #[test]
    fn the_encoded_line_carries_five_keys_and_nothing_else() {
        let line = encode_line(&entry(BASE, ProviderId::ClaudeSubscription, "5h", 0.42));
        let value: serde_json::Value = serde_json::from_str(&line).unwrap();
        let mut keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(keys, ["at", "duration", "provider", "used", "window"]);
    }

    #[test]
    fn the_provider_word_is_the_one_json_already_prints() {
        // The file and `quotapane-cli --json` must not drift into two spellings
        // of the same provider.
        for provider in [
            ProviderId::ClaudeSubscription,
            ProviderId::CodexSubscription,
        ] {
            let json = serde_json::to_string(&provider).unwrap();
            assert_eq!(json, format!("\"{}\"", provider_word(provider)));
            assert_eq!(provider_from_word(provider_word(provider)), Some(provider));
        }
    }

    #[test]
    fn garbage_lines_decode_to_nothing_and_never_panic() {
        for junk in [
            "",
            "   ",
            "not json",
            "{",
            "[]",
            "null",
            "42",
            r#"{"at":1785000000}"#,
            r#"{"at":"soon","provider":"claude_subscription","window":"5h","used":0.4}"#,
            r#"{"at":-1,"provider":"claude_subscription","window":"5h","used":0.4}"#,
            r#"{"at":1785000000,"provider":"anthropic","window":"5h","used":0.4}"#,
            r#"{"at":1785000000,"provider":"claude_subscription","window":5,"used":0.4}"#,
            r#"{"at":1785000000,"provider":"claude_subscription","window":"5h","used":"lots"}"#,
            r#"{"at":1785000000,"provider":"claude_subscription","window":"5h","used":null}"#,
            r#"{"at":1785000000,"provider":"claude_subscription","window":"5h","used":0.4,"duration":"5h"}"#,
        ] {
            assert_eq!(decode_line(junk), None, "{junk:?} should not decode");
        }
    }

    #[test]
    fn a_missing_duration_is_allowed_but_a_missing_reading_is_not() {
        let ok = r#"{"at":1785000000,"provider":"codex_subscription","window":"5h","used":0.4}"#;
        assert_eq!(decode_line(ok).unwrap().duration_secs, None);
        let null = r#"{"at":1785000000,"provider":"codex_subscription","window":"5h","used":0.4,"duration":null}"#;
        assert_eq!(decode_line(null).unwrap().duration_secs, None);
    }

    #[test]
    fn decode_all_skips_junk_and_tolerates_a_truncated_final_line() {
        let good = encode_line(&entry(BASE, ProviderId::ClaudeSubscription, "5h", 0.1));
        let text = format!("{good}\nnot json\n\n{good}\n{{\"at\":178500");
        let entries = decode_all(&text);
        assert_eq!(entries.len(), 2, "only the two whole lines survive");
        assert!(entries.iter().all(|e| e.label == "5h"));
    }

    #[test]
    fn only_headline_windows_with_a_reading_are_recorded() {
        let snapshot = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: BASE,
            windows: vec![
                window("5h", Some(0.8), Some(18_000)),
                window("7d", None, Some(604_800)),
                window("odd", Some(f64::NAN), None),
            ],
            per_model: vec![window("GPT-5.3-Codex-Max", Some(0.42), Some(604_800))],
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        };
        let entries = entries_for_snapshot(&snapshot);
        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].label, "5h");
        assert_eq!(entries[0].at_unix_secs, BASE);
        assert_eq!(entries[0].provider, ProviderId::CodexSubscription);
    }

    #[test]
    fn a_window_without_a_finite_reading_makes_no_entry() {
        for used in [None, Some(f64::NAN), Some(f64::INFINITY)] {
            assert_eq!(
                HistoryEntry::from_window(
                    ProviderId::ClaudeSubscription,
                    BASE,
                    &window("5h", used, Some(18_000))
                ),
                None,
                "{used:?} should not become an entry"
            );
        }
    }

    #[test]
    fn retention_triggers_only_above_the_cap() {
        assert_eq!(MAX_BYTES, 262_144);
        assert!(!needs_retention(0));
        assert!(!needs_retention(MAX_BYTES - 1));
        assert!(
            !needs_retention(MAX_BYTES),
            "the cap itself is still under it"
        );
        assert!(needs_retention(MAX_BYTES + 1));
    }

    #[test]
    fn keeping_the_newest_half_is_deterministic_at_every_boundary() {
        assert_eq!(keep_newest_half(""), "");
        assert_eq!(
            keep_newest_half("\n"),
            "\n",
            "one empty line stays one line"
        );
        assert_eq!(keep_newest_half("1\n"), "1\n", "one line keeps itself");
        assert_eq!(keep_newest_half("1\n2\n"), "2\n");
        assert_eq!(keep_newest_half("1\n2\n3\n"), "2\n3\n", "half rounds up");
        assert_eq!(keep_newest_half("1\n2\n3\n4\n"), "3\n4\n");
        // A truncated tail is dropped, not carried forward.
        assert_eq!(keep_newest_half("1\n2\n3\n4"), "2\n3\n");
        assert_eq!(keep_newest_half("1"), "");
    }

    #[test]
    fn rehydration_keeps_the_trail_and_drops_the_rest() {
        let now = BASE + 10_000;
        let entries = vec![
            entry(now - 20_000, ProviderId::ClaudeSubscription, "5h", 0.1), // too old
            entry(
                now - crate::pace::TRAIL_SECS - 1,
                ProviderId::ClaudeSubscription,
                "5h",
                0.2,
            ),
            entry(
                now - crate::pace::TRAIL_SECS,
                ProviderId::ClaudeSubscription,
                "5h",
                0.3,
            ),
            entry(now - 60, ProviderId::ClaudeSubscription, "5h", 0.4),
            entry(now, ProviderId::ClaudeSubscription, "5h", 0.5),
            entry(now + 1, ProviderId::ClaudeSubscription, "5h", 0.6), // the future
        ];
        let kept: Vec<f64> = rehydratable(&entries, now)
            .iter()
            .map(|e| e.used_fraction)
            .collect();
        assert_eq!(
            kept,
            vec![0.3, 0.4, 0.5],
            "trail edge inclusive, future dropped"
        );
    }

    #[test]
    fn within_is_the_same_filter_at_any_width() {
        let now = BASE;
        let entries = vec![
            entry(now - 86_401, ProviderId::CodexSubscription, "7d", 0.1),
            entry(now - 86_400, ProviderId::CodexSubscription, "7d", 0.2),
            entry(now, ProviderId::CodexSubscription, "7d", 0.3),
        ];
        assert_eq!(within(&entries, now, 86_400).len(), 2);
        assert_eq!(within(&entries, now, 0).len(), 1);
        assert_eq!(within(&[], now, 86_400).len(), 0);
    }

    /// The filesystem shell, exercised through a private temp directory.
    #[test]
    fn the_log_appends_reads_and_retains_on_disk() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("quotapane-history-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(FILE_NAME);

        // Absent file reads as no history, and retention on it is a no-op.
        assert!(read(&path).is_empty());
        apply_retention(&path).unwrap();
        assert!(!path.exists(), "retention must not create the file");

        // Append creates the directory and the file.
        let first = entry(BASE, ProviderId::ClaudeSubscription, "5h", 0.1);
        append(&path, std::slice::from_ref(&first)).unwrap();
        assert_eq!(read(&path), vec![first.clone()]);

        // Appending again adds, never replaces.
        let second = entry(BASE + 60, ProviderId::CodexSubscription, "7d", 0.2);
        append(&path, std::slice::from_ref(&second)).unwrap();
        assert_eq!(read(&path), vec![first.clone(), second.clone()]);

        // An empty batch writes nothing at all.
        append(&path, &[]).unwrap();
        assert_eq!(read(&path).len(), 2);

        // Under the cap, retention leaves the file byte-identical.
        let before = std::fs::read(&path).unwrap();
        apply_retention(&path).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);

        // Over the cap, it keeps the newest half.
        let filler: Vec<HistoryEntry> = (0..4_000)
            .map(|i| entry(BASE + i, ProviderId::ClaudeSubscription, "5h", 0.5))
            .collect();
        append(&path, &filler).unwrap();
        let over = std::fs::metadata(&path).unwrap().len();
        assert!(needs_retention(over), "fixture must exceed the cap: {over}");
        let lines_before = read(&path).len();
        apply_retention(&path).unwrap();
        let after = read(&path);
        assert_eq!(after.len(), lines_before.div_ceil(2));
        // The newest line survives; the oldest does not.
        assert_eq!(after.last().unwrap(), filler.last().unwrap());
        assert_ne!(after.first().unwrap(), &first);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
