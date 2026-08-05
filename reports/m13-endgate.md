# M13 end-gate — pace follow-ons (config.cfg, disk history, sparklines, alerts)

Floor session, headless under the M11d dispatcher. Spec:
`prompts/m13-pace-followons.md`. Four phases, four commits, in order,
plus one CI-driven fix and this report. **Acceptance is the owner's
(§4.8), and the visuals are the owner's eyes only (§4.5) — nothing
here is self-accepted, and no release is cut** (M13-RELEASE handles the
version bump and CHANGELOG for v1.6.0).

## Commits

Base: `1f89673` — *prompts: M13 spec + launcher — pace follow-ons
(v1.6.0 slice)*

| Phase | SHA | Subject |
|---|---|---|
| P1 | `b0ca0c5b6daf2750d2ae943f0cdff06d75644b98` | `ui: config.cfg — key=value preferences with theme.cfg migration` |
| P2 | `8f2dc3e16dff5773b2a03944d29409ad3a283933` | `core,ui: opt-in disk history; forecasts survive restart` |
| P3 | `943c8c50612773ef5bbaeb1b40a4950e80761561` | `ui: 24h sparkline strip per provider (history-fed)` |
| P4 | `033d20a359bdcc7ad4806e279187688f1e7d032a` | `ui: time-aware quota alerts — banner, tray ring, attention (dep-free)` |
| fix | `3687e6021bf9473dd6960013ccc7c0b9621f9170` | `ui: keep the alert tooltip prefix compiled on tray-less platforms` |

Each phase ran the full §3 bar green before its commit:
`cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `python3 tools/check-invariants.py`
(`OK: 7 invariants, 24 test bindings, tags and manifest set-equal,
SECURITY.md id set matches.`) — including in the P1 commit, which is
where the two §4a patches landed.

## CI

Run <https://github.com/cipherpine/quotapane/actions/runs/30971361646>
on `3687e60` — **conclusion: success**. All 8 required checks:

| Check | Conclusion |
|---|---|
| build & test (windows-latest) | success |
| build & test (ubuntu-latest) | success |
| build & test (macos-latest) | success |
| cargo-deny (licenses, bans, advisories, sources) | success |
| cargo-audit (RustSec advisories) | success |
| invariant 4 — no telemetry | success |
| gitleaks — full-history secret scan | success |
| invariants — manifest, docs, and tests agree | success |

The report commit's own run —
<https://github.com/cipherpine/quotapane/actions/runs/30971653421> on
`8cb622c` — is likewise **success** on all 8.

### The one red run, and why

Run <https://github.com/cipherpine/quotapane/actions/runs/30971133061>
on `033d20a` failed **only** `build & test (ubuntu-latest)`; Windows and
macOS were green, as were the other five checks. The log names it
exactly:

```
error: constant `ALERT_TOOLTIP_PREFIX` is never used
   --> crates/usage-ui/src/main.rs:948:7
    = note: `-D dead-code` implied by `-D warnings`
```

Fully explained by this session's own change: the tooltip is a tray
surface and `service_tray` is `cfg`-gated to Windows/macOS, so on Linux
the constant is written and never read. Fixed in `3687e60` with the
idiom this file already uses for `QuotaPaneApp::theme_overridden` —
`#[cfg_attr(not(any(windows, macos)), allow(dead_code))]` rather than
`cfg`-ing the constant away, because `cfg` would also remove the test
that pins its exact bytes from the Linux job, and `ALERT — ` is a
byte-level spec commitment worth checking on every target. Not a §4.6
condition: it was explained on the first look at the log.

## Test delta

| Target | Before | After | Δ |
|---|---|---|---|
| `usage-cli` unit (`src/main.rs`) | 64 | 64 | 0 |
| `usage-cli` integration (`tests/cli.rs`) | 13 | 13 | 0 |
| `usage-core` | 113 | 127 | **+14** |
| `usage-ui` | 131 | 175 | **+44** |
| **total** | **321** | **379** | **+58** |

New `usage-core` tests — all in the new `history` module:

```
no_clock_is_read_in_this_module
an_entry_round_trips_through_a_line
the_encoded_line_carries_five_keys_and_nothing_else
the_provider_word_is_the_one_json_already_prints
garbage_lines_decode_to_nothing_and_never_panic
a_missing_duration_is_allowed_but_a_missing_reading_is_not
decode_all_skips_junk_and_tolerates_a_truncated_final_line
only_headline_windows_with_a_reading_are_recorded
a_window_without_a_finite_reading_makes_no_entry
retention_triggers_only_above_the_cap
keeping_the_newest_half_is_deterministic_at_every_boundary
rehydration_keeps_the_trail_and_drops_the_rest
within_is_the_same_filter_at_any_width
the_log_appends_reads_and_retains_on_disk
```

New `usage-ui` tests, by phase:

**P1 — `config.rs` (11 new, 4 pre-existing kept):**

```
defaults_are_off_pace_and_eighty
an_empty_or_garbage_file_parses_to_the_defaults
every_key_parses
keys_and_values_are_trimmed_and_case_folded
comments_and_blank_lines_are_ignored
unknown_keys_are_ignored
a_garbage_value_keeps_that_key_at_its_default
alert_at_out_of_range_or_garbage_is_eighty
the_last_line_for_a_key_wins
render_emits_every_key_under_a_two_line_header
render_then_parse_is_identity
config_file_round_trips_migrates_and_degrades   (rewritten: adds migration)
```

**P2 — history + rehydration (8):**

```
a_forecast_survives_a_restart
rehydration_takes_only_this_provider_and_only_the_trail
a_reset_inside_the_replayed_span_still_clears_the_trail
history_off_records_nothing_anywhere
recording_appends_to_disk_and_survives_a_reopen
the_day_trail_forgets_readings_older_than_a_day
the_demo_fabricates_a_day_of_history_and_writes_none_of_it
the_demo_history_is_deterministic
```

**P3 — sparkline (9):**

```
the_strip_runs_a_day_left_to_right_and_fills_upward
the_strip_clips_what_is_outside_its_day
a_fraction_outside_zero_to_one_is_clamped_into_the_strip
an_empty_series_maps_to_no_points
the_series_is_this_windows_readings_in_time_order
one_reading_or_none_draws_no_strip_and_costs_no_space
a_pane_without_history_renders_exactly_as_it_did_before
the_strip_is_the_same_colour_in_both_themes
the_demo_draws_a_strip_for_every_pane
```

**P4 — alerts (16):**

```
alerts_off_is_silent_no_matter_the_numbers
an_unknown_or_impossible_reading_says_nothing
threshold_mode_fires_on_every_crossing_regardless_of_the_clock
pace_mode_only_speaks_when_spending_beats_the_clock
an_unknown_duration_falls_back_to_threshold_semantics
an_alert_fires_once_per_crossing_and_re_arms_on_refill
the_threshold_is_the_one_from_the_config
the_banner_and_refill_lines_are_byte_exact
the_pane_banners_the_worst_offender_only
only_a_new_crossing_asks_for_attention
a_window_the_provider_stops_reporting_does_not_stay_armed
the_alert_lines_cost_a_row_only_when_there_is_one
the_tray_alert_variant_rings_the_icon_and_prefixes_the_tooltip
folding_an_update_reports_the_new_alert_to_its_caller
the_demo_raises_exactly_one_alert
the_demo_forces_the_alert_settings_but_leaves_the_look_alone
```

`the_demo_scenario_fits_the_window` was rewritten rather than added: it
now lays each demo pane out **twice** — once as shipped and once with
M13's trail and alert lines removed — holds the M13-free height to the
previously accepted budget, and bounds the difference to M13's own
cost. See "Findings for the owner" below.

## §4a proof — the two protected-path patches

Both patches were authored at the top tier inside
`prompts/m13-pace-followons.md` and applied by
`tools/m13-apply-p1-patches.py`, which **extracts OLD and NEW from the
spec file's own bytes** (lines beginning `OLD: ` / `NEW: `, taken in
document order) and refuses to write unless OLD occurs exactly once in
the target and NEW occurs exactly once afterwards. Nothing was retyped.
The script is committed so the operation is reproducible by a reviewer.

Verification, re-run against the tree at `3687e60`:

```
SECURITY.md:         NEW occurs 1x, OLD occurs 0x   (sha256(NEW)[:16] = 2071fa6afa196f9d)
invariants.manifest: NEW occurs 1x, OLD occurs 0x   (sha256(NEW)[:16] = 31829e78eaff5a47)
```

The complete protected-path diff for the whole slice — two files, one
line each, both the authorized lines:

```
$ git diff --stat 1f89673..HEAD -- SECURITY.md THREAT_MODEL.md deny.toml \
    invariants.manifest .github .cargo .claude \
    crates/usage-core/src/egress crates/usage-core/src/credentials
 SECURITY.md         | 2 +-
 invariants.manifest | 2 +-
 2 files changed, 2 insertions(+), 2 deletions(-)
```

```diff
-1. **No credential persistence.** … Since v1.2.0 the app writes exactly one non-credential file: `theme.cfg`, a single word (`plain` or `cipherpine`) …
+1. **No credential persistence.** … Since v1.6.0 the app writes at most two non-credential files under the platform config directory: `config.cfg`, a handful of key=value preference lines (theme, history, alerts — see the README), and, only when `history=on`, `history.jsonl` — timestamps, window labels, and usage percentages, nothing else. The legacy one-word `theme.cfg` is still read as a fallback but never written. …

-invariant 1: No credential persistence — no code path writes a token; the only file ever written is theme.cfg (one word, preferences only).
+invariant 1: No credential persistence — no code path writes a token; the only files ever written are config.cfg (key=value preferences) and, when history=on, history.jsonl (timestamps, labels, and percentages only).
```

`python3 tools/check-invariants.py` passes **in the P1 commit itself**
(the claim headline `No credential persistence` still prefixes the
manifest summary, so the F2 headline check holds), and at every commit
since.

**Note for the owner, stated plainly:** the authorized SECURITY.md and
manifest bytes describe the *end state of the whole slice* — they name
`history.jsonl`, which P1 does not yet write. The spec required them in
the P1 commit, byte-exact, so that is where they landed; the file they
describe exists from P2 onward, and the slice ships as one unit with no
release cut in between. The floor-authored README and ARCHITECTURE
prose was written to match those bytes for the same reason. Flagged
because it is the one place a commit's docs run one commit ahead of its
code.

## Byte-pinned strings — verbatim grep proof

**1. Alert banner** — spec: `alert: <provider> <label> at <PCT>% >= <N>% (<mode>)`.
Sole producer in `crates/usage-ui/src/main.rs` (1 occurrence):

```
        "alert: {} {label} at {} >= {alert_at}% ({})",
```

Substituting `provider_label(provider)`, `format_percent(Some(used))`
(which appends the `%`), `alert_at`, `alert_mode.as_word()`. Rendered
bytes pinned by full equality in three tests:

```
"alert: Codex 5h at 82% >= 80% (pace)"
"alert: Claude 7d at 95% >= 90% (threshold)"
"alert: Codex 5h at 80% >= 80% (pace)"      (the demo's single alert)
```

**2. Refill line** — spec: `refilled: <provider> <label> back under <N>%`.
Sole producer (1 occurrence):

```
        "refilled: {} {label} back under {alert_at}%",
```

Rendered bytes pinned:

```
"refilled: Codex 5h back under 80%"
"refilled: Claude weekly back under 42%"
"refilled: Claude 5h back under 80%"
```

**3. Tray tooltip prefix** — spec: exactly `ALERT — `. One constant, one
consumer, one test:

```
const ALERT_TOOLTIP_PREFIX: &str = "ALERT — ";
                format!("{ALERT_TOOLTIP_PREFIX}{tooltip}")
        assert_eq!(ALERT_TOOLTIP_PREFIX, "ALERT \u{2014} ");
```

Codepoint dump of the constant's literal:
`['0x41','0x4c','0x45','0x52','0x54','0x20','0x2014','0x20']` — `ALERT`,
space, **U+2014**, space. The test asserts the escape form, so a
look-alike hyphen or en dash fails it (mutation-verified below).

**4. Commit subjects** — the four spec-mandated subjects, verbatim, in
the table at the top of this report.

## Mutation check (beyond spec)

Tests that cannot fail prove nothing, so every new behavior was mutated
and the suite re-run. **23 mutations, 23 caught** — 22 by a named test,
one at compile time — after one escape was found and closed.

P2 — history (6/6):

| Mutation | Caught by |
|---|---|
| rehydration seeds no rings | `a_forecast_survives_a_restart` |
| rehydration ignores the provider filter | `rehydration_takes_only_this_provider_and_only_the_trail` |
| the disk append is removed | `recording_appends_to_disk_and_survives_a_reopen` |
| retention rounds the half down instead of up | `keeping_the_newest_half_is_deterministic_at_every_boundary` |
| `decode_line` accepts a missing/non-finite reading | `garbage_lines_decode_to_nothing_and_never_panic` |
| `within` lets future timestamps through | `rehydration_keeps_the_trail_and_drops_the_rest` |

P3 — sparkline (6/6):

| Mutation | Caught by |
|---|---|
| the strip fills downward | `the_strip_runs_a_day_left_to_right_and_fills_upward` |
| the 24 h clip is removed | `the_strip_clips_what_is_outside_its_day` |
| a single reading draws a strip | `one_reading_or_none_draws_no_strip_and_costs_no_space` |
| the series is left unsorted | `the_series_is_this_windows_readings_in_time_order` |
| the `0..=1` fraction clamp is removed | `a_fraction_outside_zero_to_one_is_clamped_into_the_strip` |
| the renderer is never called from the pane | `a_pane_without_history_renders_exactly_as_it_did_before` |

P4 — alerts (11/11):

| Mutation | Caught by |
|---|---|
| `alerts=off` ignored | `alerts_off_is_silent_no_matter_the_numbers` |
| debounce removed (fires every poll) | `an_alert_fires_once_per_crossing_and_re_arms_on_refill` |
| pace comparison `<=` → `<` (equal counts as ahead) | `pace_mode_only_speaks_when_spending_beats_the_clock` |
| unknown duration goes silent instead of fail-safe | `an_unknown_duration_falls_back_to_threshold_semantics` |
| banner picks the *least* offender | `the_pane_banners_the_worst_offender_only` |
| stale firing labels never pruned | `a_window_the_provider_stops_reporting_does_not_stay_armed` |
| tooltip prefix uses `-` instead of U+2014 | `the_tray_alert_variant_rings_the_icon_and_prefixes_the_tooltip` |
| the alert ring is never painted | `the_tray_alert_variant_rings_the_icon_and_prefixes_the_tooltip` |
| the banner drops the `%` sign | `the_banner_and_refill_lines_are_byte_exact` |
| `apply_update` swallows the "raised" flag | `folding_an_update_reports_the_new_alert_to_its_caller` |
| `drain` swallows the "raised" flag | compile error (unused result under `-D warnings`) |

### The one escape, and how it was closed

The first P4 pass had **no test covering `drain`'s alert hop**: deleting
`raised |=` from the snapshot arm compiled clean and passed 174 tests.
The cause is structural — `drain` needs a live `PollerHandle`, which a
unit test must not spawn — and it is the same shape as the gap
`pane_wants_blink` was extracted to close in M8. The per-update fold is
now `ProviderPane::apply_update(update, config) -> bool`, taking a plain
`Update`, and `folding_an_update_reports_the_new_alert_to_its_caller`
asserts the crossing, the repeat, and that a `Failure` is never an
alert. Re-mutated: both the `apply_update` and `drain` variants are now
caught.

### The one hop still untested, stated plainly

`eframe::App::logic` turning `alert_raised == true` into
`ViewportCommand::RequestUserAttention(Informational)` is **not**
covered by a test. It needs a real eframe frame and a real window
manager, which this session cannot run. What *is* proven is every input
that decides it (the eleven mutations above) and that nothing else sets
the flag. Confirming the taskbar actually flashes belongs to the owner's
`--pace-demo` pass.

## Findings for the owner

**1. The known 240px overflow deepens.** `the_demo_scenario_fits_the_window`
already recorded, since M8, that the collapsed demo — two panes, both
warning, Codex also reporting a reset credit — wants 231px against the
216px the central panel offers, reachable via the ScrollArea but not
visible by default, and that the window's size is the owner's call. M13
adds one 12px strip per pane and, in the demo, one banner row: the same
scenario now wants **261px**. The test was rewritten to measure this
rather than absorb it — it lays out each pane with and without M13's
additions, still holds the M13-free height to the old budget, and bounds
M13's own cost to one strip per pane plus one small row. So unbounded
growth still fails the test while the known, now-deeper overflow does
not read as green. **This is reported, not decided (§4.5).**

Worth knowing: this is the *demo's* worst case with alerts forced on. A
default install (`history=off`, `alerts=off`) lays out exactly as v1.5.0
did — same rows, same width, same height — because `render_sparkline`
returns before it allocates and both alert lines are absent. Pinned by
`a_pane_without_history_renders_exactly_as_it_did_before`,
`one_reading_or_none_draws_no_strip_and_costs_no_space`, and
`the_alert_lines_cost_a_row_only_when_there_is_one`. That is a layout
claim, measured through the real font harness; it is not a pixel
comparison, which this session has no way to make (§4.5).

**2. Which window the sparkline draws.** The spec says "the headline
window's used fraction" without naming which of the one-or-two headline
windows. The strip follows `representative_window` — the one closest to
its limit — which is what the tray miniature and the tray tooltip
already use, so all three now speak about the same window per provider.
`representative_window` was un-`cfg`-gated for this. Say if a different
window was intended.

**3. `tools/m13-apply-p1-patches.py` is committed.** A one-shot scratch
tool, kept so the §4a claim above is reproducible rather than asserted.
Safe to delete at M13-RELEASE if the audit trail in this report is
enough.

## Hard limits — verified, not assumed

**Zero new dependencies.** Neither `Cargo.toml`, nor any crate's
`Cargo.toml`, nor `Cargo.lock` appears in the slice's diff
(`git diff --stat 1f89673..HEAD -- Cargo.lock Cargo.toml crates/*/Cargo.toml`
is empty; `git diff --numstat` on `Cargo.lock` returns 0 lines).
`config.cfg` is hand-parsed with `std` only; `history.jsonl` uses the
`serde_json` already inside `usage-core`; the alert surfaces use
`tray-icon` and `eframe` APIs already present. `cargo-deny` and
`cargo-audit` green.

**No `--json` key added, removed, or renamed.**
`crates/usage-core/src/model/` does not appear in the diff, and it is
the only place snapshot keys are defined. `usage-cli` is untouched
entirely (0 test delta, no source change). `history.jsonl` has its own
five keys (`at`, `provider`, `window`, `used`, `duration`) which are
**not** `--json` keys; `the_provider_word_is_the_one_json_already_prints`
pins that the two files at least agree on the provider spelling.

**No §4.1 bytes beyond the two authorized patches.** Full slice
diff-stat:

```
$ git diff --stat 1f89673..HEAD
 ARCHITECTURE.md                  |   17 +-
 README.md                        |   27 +-
 SECURITY.md                      |    2 +-
 crates/usage-core/src/history.rs |  612 ++++++++++++
 crates/usage-core/src/lib.rs     |   11 +-
 crates/usage-ui/src/config.rs    |  484 +++++++++-
 crates/usage-ui/src/icon.rs      |   43 +-
 crates/usage-ui/src/main.rs      | 1983 +++++++++++++++++++++++++++++++++++---
 invariants.manifest              |    2 +-
 tools/m13-apply-p1-patches.py    |   69 ++
 10 files changed, 3047 insertions(+), 203 deletions(-)
```

Of these, only `SECURITY.md` and `invariants.manifest` are §4.1 paths,
and both changed by exactly the one authorized line each. Nothing under
`crates/usage-core/src/egress/**` or `crates/usage-core/src/credentials/**`,
nothing in `.github/**`, `.cargo/**`, `.claude/**`, not `deny.toml`, not
`THREAT_MODEL.md`, not `tools/check-invariants.py`.

**No security-invariant test edited.** Every file hosting a
manifest-named `// INV:`-tagged test —
`crates/usage-core/src/credentials/mod.rs`, `credentials/secret.rs`,
`poller/mod.rs`, `egress/mod.rs`, `crates/usage-cli/src/main.rs`,
`crates/usage-cli/tests/cli.rs` — is absent from the slice's diff
(verified: the diff-stat filtered to those six paths is empty).
`check-invariants.py` still reports 24 bindings, unchanged.

**`usage_core::pace` untouched.** Not in the diff. Rehydration seeds
rings through the existing public `PaceRing::observe`; the ring's own
reset detection therefore still applies to replayed samples
(`a_reset_inside_the_replayed_span_still_clears_the_trail`).

**No version bump, no CHANGELOG.** Version stays `1.5.0`; both belong to
M13-RELEASE, which cuts v1.6.0.

**No credential file read by this session.** `~/.claude/**` and
`~/.codex/**` were never opened (§4.4), and no live poll was run. Every
new filesystem test writes to a private directory under the system temp
dir, keyed by process id, and removes it on drop; `QUOTAPANE_CONFIG_DIR`
redirects the config tests away from the real config directory exactly
as before.

**No visuals accepted.** §4.5 — the window was never launched by this
session and no screenshot was taken.

## Deviations from the spec

None that change the specified behavior. Recorded for review:

1. **`history.jsonl` key names are floor-chosen.** The spec fixed the
   entry's *fields* (unix seconds, provider id string, window label,
   used fraction, nullable duration) but not the JSON key spelling.
   Chosen: `at`, `provider`, `window`, `used`, `duration` — self-
   describing, because a user-visible file the product asks to be
   trusted should be readable without a decoder ring.
2. **`provider` is a typed `ProviderId` in memory, a string on disk.**
   The spec says "provider id string", which is what the file carries;
   holding it as the two-variant enum is what makes "no field a
   credential could occupy" true by type rather than by convention.
   `usage_core::model` was **not** modified for this — the encoder and
   decoder are hand-written, so `ProviderId` gained no `Deserialize`.
3. **`render_windows` gained a `PaneLines` struct** rather than four more
   positional parameters. Required: clippy's `too_many_arguments` fires
   at 8, and the spec's P3 and P4 both add arguments to that function.
4. **`ProviderPane::apply_update` extracted from `drain`.** Not in the
   spec; added to close the mutation escape documented above.
5. **`elapsed_fraction` extracted and shared** by `pace_tick_x` and the
   alert decision, so the tick on the bar and the rule behind a pace
   alert are one piece of arithmetic. `pace_tick_x`'s behavior is
   unchanged and still pinned by its pre-existing M8 tests.
6. **`representative_window` un-`cfg`-gated** — see finding 2.
7. **Extra doc truth beyond the two paragraphs the spec named.** The
   spec named the README Theming section and ARCHITECTURE's "one file,
   one word" paragraph. Two further lines made the same now-false claim
   and were corrected in the same commit: README's Install paragraph
   ("The only file QuotaPane ever writes is `theme.cfg`") and
   ARCHITECTURE §4's enforced-invariants bullet. The README's Theming
   section was also renamed to "Theming and preferences" and given a
   key/value table; no anchor links to `#theming` exist in the repo
   (checked).
8. **`--pace-demo` forces all three alert settings**, not just `alerts`.
   The spec says it "forces one alert active"; forcing only `alerts=on`
   would leave a reviewer whose own file says `alert_at=95` looking at a
   demo with no alert in it. The theme is deliberately left alone.
9. **The refill notice's "once" is one poll cycle**, not one frame — the
   window repaints roughly every second, so a frame-scoped notice would
   be invisible. It is set on the poll that detects the drop and cleared
   by the next poll.

## §4 conditions hit

**None.** No protected path was reached beyond the §4a-authorized bytes,
no dependency was needed, no precondition mismatched, no credential was
touched, and nothing was worked around. The one CI failure was explained
by this session's own change on the first look at the log (§4.6 does not
apply) and is documented above.

## What the owner must do next

1. **The §4.5 visual pass — this slice needs one.** Run the demo:

   ```
   cargo run -p usage-ui --bin quotapane -- --pace-demo
   ```

   Four things to look at, none of which this session may accept:
   - a **12px sparkline strip** under each provider's headline bars,
     TEXT_DIM at alpha 140, one bar wide, rising left-to-right over a
     fabricated day;
   - the CARDINAL banner **`alert: Codex 5h at 80% >= 80% (pace)`** just
     above the Codex pane's freshness footer, and nothing on the Claude
     pane;
   - the **tray icon's 1px red ring** and the tooltip reading
     `ALERT — Claude 7d 55% | Codex 5h 80%`;
   - whether the **taskbar button flashed once** at startup — the one
     hop no test covers.

   Also worth a look in both themes (`--pace-demo --plain`) and with the
   window's height in mind: see finding 1 above.

2. **Optionally exercise the real thing.** Put `history=on` and
   `alerts=on` into `config.cfg` in your config directory
   (`%APPDATA%\quotapane\config.cfg`) and run `quotapane` normally; the
   README's "Theming and preferences" table documents every key. A
   restart after a couple of hours is what demonstrates P2's headline
   claim. `theme.cfg` is left where it is and is not read once
   `config.cfg` exists.

3. **Accept or reject M13** (§4.8) — the milestone gate is yours.

4. **Calls to make**, if any: findings 1 (the 240px overflow), 2 (which
   window the strip draws), and 3 (whether to keep the patch script);
   deviations 1, 7 and 8 are the other places floor judgement filled a
   gap in the spec.

5. **After acceptance: M13-RELEASE** — `prompts/release-template.md`,
   cutting **v1.6.0** with the version bump and CHANGELOG entry
   deliberately left out here.
