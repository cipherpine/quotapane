//! `--statusline` — the mode that sends nothing.
//!
//! Claude Code's `statusLine` feature runs a configured command on each render,
//! hands it a JSON document on stdin, and displays the first line of its
//! stdout. That document already carries the subscription quota numbers, so
//! QuotaPane can answer the question without asking anyone: this mode opens no
//! credential file, constructs no chokepoint, and sends no byte anywhere. It is
//! the only surface in the product that reports usage with zero egress, and
//! `main.rs` pins that structurally — this module's source is scanned for the
//! names it must never contain, and the mode is pinned to return from `main`
//! before an `Egress` is ever constructed.
//!
//! ## The pinned schema
//!
//! Taken from the official statusline documentation
//! (`docs.claude.com/en/docs/claude-code/statusline`, fetched 2026-08-09):
//!
//! ```json
//! "rate_limits": {
//!   "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
//!   "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 }
//! }
//! ```
//!
//! `used_percentage` is 0–100 — a percentage already, not a fraction, which is
//! the one place this differs from every other number in the codebase.
//! `resets_at` is Unix epoch seconds, an absolute instant rather than the
//! countdown a provider snapshot carries.
//!
//! ## Defensive by rule
//!
//! The same documentation states that `rate_limits` "appears only for Claude.ai
//! subscribers (Pro/Max) after the first API response in the session", and that
//! each window may be independently absent; anthropics/claude-code#40094
//! reports it missing for further plan/auth combinations. So nothing here is
//! required to be present. Invalid JSON, an absent `rate_limits`, an empty one,
//! a percentage that is not a finite number — each yields no output and exit 0.
//! A status line must never break its host, and a host that renders a status
//! bar has nowhere to put an error anyway.
//!
//! ## The aperture
//!
//! Percentages and reset times, and nothing else. The same payload carries the
//! working directory, the model id, the session id, the transcript path, the
//! open PR and the worktree — none of it is parsed, and a test plants a
//! sentinel in every one of those fields and proves it cannot reach stdout.
//! That is invariant 8's welded-aperture idea applied to a second reader,
//! enforced by test rather than by a new invariant.

use std::io::Read;

use serde_json::Value;

use crate::format_reset;

/// The windows this mode reports, as `(payload key, printed label)`, in output
/// order: the session window first, then the weekly one.
///
/// Fixed here rather than read from the payload's key order, so a reordered
/// JSON object cannot reorder the line. The labels are the product's own — the
/// same `5h` and `7d` the window and the text summary print.
const WINDOWS: &[(&str, &str)] = &[("five_hour", "5h"), ("seven_day", "7d")];

/// At or past this percentage, a segment gains a `!`.
///
/// Inclusive, like `--fail-at`'s threshold and for the same reason: a boundary
/// the user can see should fire at the number they can see, not one past it.
const BANG_AT: i64 = 80;

/// What joins the segments of the line.
const SEPARATOR: &str = " · ";

/// A payload percentage as the whole number the line prints, or `None` when it
/// is not a measurement.
///
/// Rounded and clamped exactly as the window rounds and clamps, so the bang
/// fires on the number the reader sees rather than on a hidden fraction. A
/// non-finite value is skipped rather than read as 0: "unknown" is not
/// "untouched", the same rule `--fail-at` applies to a window with no usage.
fn percent(used_percentage: f64) -> Option<i64> {
    if !used_percentage.is_finite() {
        return None;
    }
    Some(used_percentage.round().clamp(0.0, 100.0) as i64)
}

/// Format one status line from one payload.
///
/// Pure — the clock is an argument, so the countdown is testable without
/// waiting for one. Returns the empty string for "nothing to say", which the
/// caller prints as nothing at all rather than as a blank line.
pub(crate) fn line(payload: &str, now_unix_secs: u64) -> String {
    let Ok(document) = serde_json::from_str::<Value>(payload) else {
        return String::new();
    };
    let Some(limits) = document.get("rate_limits") else {
        return String::new();
    };

    let mut segments: Vec<String> = Vec::new();
    // The most-used window and the reset time it reported, for the countdown
    // below. A strict `>` keeps the earlier window on a tie, which given
    // `WINDOWS`' order means the session one.
    let mut most_used: Option<(i64, Option<u64>)> = None;

    for (key, label) in WINDOWS {
        let Some(window) = limits.get(key) else {
            continue;
        };
        let Some(used) = window
            .get("used_percentage")
            .and_then(Value::as_f64)
            .and_then(percent)
        else {
            continue;
        };

        let bang = if used >= BANG_AT { "!" } else { "" };
        segments.push(format!("{label} {used}%{bang}"));

        if most_used.is_none_or(|(highest, _)| used > highest) {
            most_used = Some((used, window.get("resets_at").and_then(Value::as_u64)));
        }
    }

    if segments.is_empty() {
        return String::new();
    }

    // One countdown, for the window closest to running out — the number the
    // reader is actually worried about. A reset already in the past says
    // nothing useful (the window has turned over, or the payload is stale), so
    // it is dropped rather than printed as `0s`.
    if let Some((_, Some(resets_at))) = most_used {
        let remaining = resets_at.saturating_sub(now_unix_secs);
        if remaining > 0 {
            segments.push(format!("resets {}", format_reset(remaining)));
        }
    }

    segments.join(SEPARATOR)
}

/// Read the payload the host piped in.
///
/// A failed read, or bytes that are not UTF-8, yield an empty payload — which
/// [`line`] turns into no output and exit 0. There is deliberately no error
/// path: the caller is a status bar, not a script that could handle one.
pub(crate) fn read_payload() -> String {
    let mut buffer = String::new();
    match std::io::stdin().read_to_string(&mut buffer) {
        Ok(_) => buffer,
        Err(_) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A clock far from any window boundary, so a test that does not care about
    /// the countdown gets no countdown.
    const NOW: u64 = 1_784_000_000;

    /// Build a `rate_limits` payload from raw JSON for the two windows.
    fn payload(rate_limits: &str) -> String {
        format!(r#"{{"rate_limits": {rate_limits}}}"#)
    }

    /// A single window object.
    fn window(used: &str, resets_at: Option<u64>) -> String {
        match resets_at {
            Some(at) => format!(r#"{{"used_percentage": {used}, "resets_at": {at}}}"#),
            None => format!(r#"{{"used_percentage": {used}}}"#),
        }
    }

    // --- the pinned schema, rendered ---

    #[test]
    fn the_documented_example_payload_renders_the_documented_numbers() {
        // The docs' own example, verbatim: 23.5 and 41.2 with both reset times.
        // The weekly window is the most used, so its reset is the one shown —
        // two hours and ten minutes before it, here.
        let doc = payload(&format!(
            r#"{{"five_hour": {}, "seven_day": {}}}"#,
            window("23.5", Some(1_738_425_600)),
            window("41.2", Some(1_738_857_600))
        ));
        assert_eq!(
            line(&doc, 1_738_857_600 - 7_800),
            "5h 24% · 7d 41% · resets 2h10m"
        );
    }

    #[test]
    fn the_plain_two_window_line_is_the_specs_shape() {
        let doc = payload(&format!(
            r#"{{"five_hour": {}, "seven_day": {}}}"#,
            window("12", None),
            window("48", None)
        ));
        assert_eq!(line(&doc, NOW), "5h 12% · 7d 48%");
    }

    #[test]
    fn a_window_at_or_past_the_threshold_gains_a_bang() {
        let doc = payload(&format!(
            r#"{{"five_hour": {}, "seven_day": {}}}"#,
            window("12", None),
            window("83", None)
        ));
        assert_eq!(line(&doc, NOW), "5h 12% · 7d 83%!");
    }

    // --- the bang boundary (mutation target: `>=` flipped to `>`) ---

    #[test]
    fn the_bang_threshold_is_inclusive_at_eighty() {
        // 79 / 80 / 81. The middle case is the whole point: flipping the
        // comparison to `>` loses exactly this one, and nothing else.
        let at = |used: &str| {
            line(
                &payload(&format!(r#"{{"five_hour": {}}}"#, window(used, None))),
                NOW,
            )
        };

        assert_eq!(at("79"), "5h 79%", "79% must not be flagged");
        assert_eq!(
            at("80"),
            "5h 80%!",
            "80% is AT the threshold and must be flagged"
        );
        assert_eq!(at("81"), "5h 81%!", "81% must be flagged");
        assert_eq!(at("100"), "5h 100%!");
        assert_eq!(at("0"), "5h 0%");
    }

    #[test]
    fn the_bang_follows_the_rounded_number_the_reader_sees() {
        // 79.5 renders as 80, so it must carry the bang: the flag and the digits
        // cannot disagree on screen.
        let at = |used: &str| {
            line(
                &payload(&format!(r#"{{"five_hour": {}}}"#, window(used, None))),
                NOW,
            )
        };
        assert_eq!(at("79.5"), "5h 80%!");
        assert_eq!(at("79.4"), "5h 79%");
    }

    #[test]
    fn percentages_are_rounded_and_clamped_like_the_window() {
        assert_eq!(percent(23.5), Some(24));
        assert_eq!(percent(41.2), Some(41));
        assert_eq!(percent(0.0), Some(0));
        // A provider reporting over quota reads as 100, never as 137.
        assert_eq!(percent(137.0), Some(100));
        // Negative is nonsense; it clamps rather than printing `-3%`.
        assert_eq!(percent(-3.0), Some(0));
        // Not a measurement.
        assert_eq!(percent(f64::NAN), None);
        assert_eq!(percent(f64::INFINITY), None);
    }

    // --- ordering (mutation target: the two segments swapped) ---

    #[test]
    fn the_session_window_prints_first_whatever_order_the_payload_used() {
        // The payload lists the weekly window first; the line must not.
        let doc = payload(&format!(
            r#"{{"seven_day": {}, "five_hour": {}}}"#,
            window("48", None),
            window("12", None)
        ));
        assert_eq!(line(&doc, NOW), "5h 12% · 7d 48%");
    }

    #[test]
    fn a_single_window_prints_its_own_segment_only() {
        let five_only = payload(&format!(r#"{{"five_hour": {}}}"#, window("12", None)));
        assert_eq!(line(&five_only, NOW), "5h 12%");

        let seven_only = payload(&format!(r#"{{"seven_day": {}}}"#, window("48", None)));
        assert_eq!(line(&seven_only, NOW), "7d 48%");
    }

    // --- the countdown ---

    #[test]
    fn the_countdown_belongs_to_the_most_used_window() {
        // The weekly window is at 83% and the session one at 12%: the reader
        // needs to know when the weekly one turns over, not the session one.
        let doc = payload(&format!(
            r#"{{"five_hour": {}, "seven_day": {}}}"#,
            window("12", Some(NOW + 600)),
            window("83", Some(NOW + 7_800))
        ));
        assert_eq!(line(&doc, NOW), "5h 12% · 7d 83%! · resets 2h10m");

        // Flip which one is worse and the countdown follows it.
        let flipped = payload(&format!(
            r#"{{"five_hour": {}, "seven_day": {}}}"#,
            window("83", Some(NOW + 600)),
            window("12", Some(NOW + 7_800))
        ));
        assert_eq!(line(&flipped, NOW), "5h 83%! · 7d 12% · resets 10m");
    }

    #[test]
    fn a_tie_keeps_the_session_windows_countdown() {
        let doc = payload(&format!(
            r#"{{"five_hour": {}, "seven_day": {}}}"#,
            window("50", Some(NOW + 600)),
            window("50", Some(NOW + 7_800))
        ));
        assert_eq!(line(&doc, NOW), "5h 50% · 7d 50% · resets 10m");
    }

    #[test]
    fn a_window_with_no_reset_time_simply_has_no_countdown() {
        let doc = payload(&format!(
            r#"{{"five_hour": {}, "seven_day": {}}}"#,
            window("12", Some(NOW + 600)),
            window("48", None)
        ));
        // The weekly window is the most used and reported no reset, so there is
        // nothing to count down — the session window's is NOT borrowed for it.
        assert_eq!(line(&doc, NOW), "5h 12% · 7d 48%");
    }

    #[test]
    fn a_reset_already_in_the_past_is_dropped_rather_than_shown_as_zero() {
        let doc = payload(&format!(
            r#"{{"five_hour": {}}}"#,
            window("12", Some(NOW - 60))
        ));
        assert_eq!(line(&doc, NOW), "5h 12%");
        // Exactly now is the same case: the window has turned over.
        let at_now = payload(&format!(r#"{{"five_hour": {}}}"#, window("12", Some(NOW))));
        assert_eq!(line(&at_now, NOW), "5h 12%");
    }

    // --- never break the host (mutation target: absent rate_limits erroring) ---

    #[test]
    fn an_absent_or_empty_rate_limits_prints_nothing() {
        // The #40094 case, and the "before the first API response" case: the
        // payload is perfectly valid and simply has no quota in it.
        assert_eq!(line(r#"{"cwd": "/x", "version": "2.1.90"}"#, NOW), "");
        assert_eq!(line(r#"{"rate_limits": {}}"#, NOW), "");
        assert_eq!(line(r#"{"rate_limits": null}"#, NOW), "");
        // A `rate_limits` of the wrong type is not a crash either.
        assert_eq!(line(r#"{"rate_limits": []}"#, NOW), "");
        assert_eq!(line(r#"{"rate_limits": "soon"}"#, NOW), "");
    }

    #[test]
    fn garbage_or_empty_stdin_prints_nothing() {
        for junk in [
            "",
            "   ",
            "not json at all",
            "{",
            r#"{"rate_limits": {"five_hour": "#,
            "null",
            "[]",
            "42",
        ] {
            assert_eq!(line(junk, NOW), "", "{junk:?} must produce no output");
        }
    }

    #[test]
    fn a_window_whose_percentage_is_missing_or_unusable_is_skipped() {
        let cases = [
            r#"{"resets_at": 1738425600}"#,       // no percentage at all
            r#"{"used_percentage": null}"#,       // explicitly unknown
            r#"{"used_percentage": "23.5"}"#,     // a string, not a number
            r#"{"used_percentage": {"pct": 5}}"#, // an object
        ];
        for window_json in cases {
            let doc = payload(&format!(r#"{{"five_hour": {window_json}}}"#));
            assert_eq!(line(&doc, NOW), "", "{window_json} must be skipped");
        }

        // And a skipped window does not take a good one down with it.
        let mixed = payload(&format!(
            r#"{{"five_hour": {{"used_percentage": null}}, "seven_day": {}}}"#,
            window("48", None)
        ));
        assert_eq!(line(&mixed, NOW), "7d 48%");
    }

    #[test]
    fn unknown_fields_are_ignored_everywhere() {
        // Parse only what you print: a payload gaining new keys — at the top
        // level, inside `rate_limits`, or inside a window — changes nothing.
        let doc = r#"{
          "brand_new_top_level": {"anything": [1, 2, 3]},
          "rate_limits": {
            "five_hour": {"used_percentage": 12, "brand_new": "x", "resets_at": null},
            "monthly": {"used_percentage": 99},
            "seven_day": {"used_percentage": 48, "nested": {"deep": true}}
          }
        }"#;
        assert_eq!(line(doc, NOW), "5h 12% · 7d 48%");
    }

    // --- the aperture: nothing else in the payload can reach stdout ---

    /// Planted in every field this mode must not read. Shaped as an obvious
    /// placeholder so secret scanners have nothing to flag, and distinctive
    /// enough that a substring search for it means something.
    const SENTINEL: &str = "SENTINEL-DO-NOT-PRINT";

    #[test]
    fn no_field_other_than_the_quota_numbers_can_reach_the_line() {
        // The full documented payload with a sentinel planted in every string
        // field — cwd, session id, transcript path, model, workspace, version,
        // agent, PR, worktree, output style, vim mode — plus a sentinel key
        // inside `rate_limits` itself. Only the percentages and reset times
        // are read, so not one of them can appear in the output.
        let doc = format!(
            r#"{{
  "cwd": "/{SENTINEL}/cwd",
  "session_id": "{SENTINEL}-session",
  "session_name": "{SENTINEL}-name",
  "prompt_id": "{SENTINEL}-prompt",
  "transcript_path": "/{SENTINEL}/transcript.jsonl",
  "model": {{"id": "{SENTINEL}-model-id", "display_name": "{SENTINEL}-model"}},
  "workspace": {{
    "current_dir": "/{SENTINEL}/current",
    "project_dir": "/{SENTINEL}/project",
    "added_dirs": ["/{SENTINEL}/added"],
    "git_worktree": "{SENTINEL}-worktree",
    "repo": {{"host": "{SENTINEL}.example", "owner": "{SENTINEL}", "name": "{SENTINEL}-repo"}}
  }},
  "version": "{SENTINEL}-version",
  "output_style": {{"name": "{SENTINEL}-style"}},
  "cost": {{"total_cost_usd": 0.01234, "total_duration_ms": 45000}},
  "context_window": {{"total_input_tokens": 15500, "used_percentage": 77}},
  "exceeds_200k_tokens": false,
  "vim": {{"mode": "{SENTINEL}-NORMAL"}},
  "agent": {{"name": "{SENTINEL}-agent"}},
  "pr": {{"number": 1234, "url": "https://{SENTINEL}.example/pull/1234"}},
  "worktree": {{"name": "{SENTINEL}-wt", "path": "/{SENTINEL}/wt", "branch": "{SENTINEL}-br"}},
  "rate_limits": {{
    "note": "{SENTINEL}-inside-rate-limits",
    "five_hour": {{"used_percentage": 12, "resets_at": {resets}, "label": "{SENTINEL}-window"}},
    "seven_day": {{"used_percentage": 48}}
  }}
}}"#,
            resets = NOW + 600
        );

        let out = line(&doc, NOW);

        // It produced the real line — so this is not passing because parsing
        // failed and everything was dropped.
        assert_eq!(out, "5h 12% · 7d 48%");
        assert!(
            !out.contains(SENTINEL),
            "a field outside the quota numbers reached the output: {out}"
        );

        // `context_window.used_percentage` is the one number in the payload
        // shaped exactly like the two we do read — same key name, same 0–100
        // range. It is a different quota entirely and must not appear.
        assert!(!out.contains("77"), "context_window usage leaked: {out}");
    }

    // --- the structural pin lives in main.rs; this one guards the module ---

    #[test]
    fn this_module_names_nothing_that_could_send_a_byte() {
        // The claim is "this mode sends nothing". The cheapest way to keep it
        // true as the file changes is to pin that the module cannot even NAME
        // the machinery that would send something. Same technique as the
        // `Egress::new` single-call-site pin in main.rs. Comment lines are
        // dropped first — the doc comment above necessarily discusses egress.
        const SRC: &str = include_str!("statusline.rs");
        let code: String = SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        // Guard the scanner: if the slice were empty these would pass vacuously.
        assert!(
            code.contains("pub(crate) fn line("),
            "the scanner lost the module body"
        );

        for forbidden in [
            "Egress",
            "egress",
            "UsageProvider",
            "build_provider",
            "with_default_path",
            "ClaudeSubscription",
            "CodexSubscription",
            "credential",
            "Credential",
            "ureq",
            "reqwest",
            "TcpStream",
            "http",
        ] {
            assert!(
                !code.contains(forbidden),
                "the statusline module must not name `{forbidden}` — it is the zero-egress path"
            );
        }
    }
}
