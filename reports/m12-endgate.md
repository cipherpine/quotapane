# M12 end-gate — CLI automation (`--fail-at`, `--watch`, JSON contract)

Floor session, headless under the M11d dispatcher. Spec:
`prompts/m12-cli-automation.md`. Three phases, three commits, in
order, plus this report. **Acceptance is the owner's (§4.8) — nothing
here is self-accepted, and no release is cut** (M12-RELEASE handles
the version bump and CHANGELOG).

## Commits

Base: `3cbd1f264bb9c833dae5cea00c370736882890b6` — *prompts: M12 spec +
launcher — CLI automation (v1.5.0 slice)*

| Phase | SHA | Subject |
|---|---|---|
| P1 | `8f0999b869219f5d8a9e9215e060fb41205ad5fa` | `cli: --fail-at gate logic and flag parsing` |
| P2 | `bac6b29937c4864900b03997c3dbdee75fb774d3` | `cli: --watch mode with NDJSON output` |
| P3 | `26fe69044f8931f3536cb94d8e8287c09fb65a9e` | `docs: CLI automation + the JSON stability contract` |

Each phase ran the full §3 bar green before its commit:
`cargo fmt --all --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `python3 tools/check-invariants.py`
(`OK: 7 invariants, 24 test bindings, tags and manifest set-equal,
SECURITY.md id set matches.`).

## CI

Run <https://github.com/cipherpine/quotapane/actions/runs/30869272471>
on `26fe690` — **conclusion: success**. All 8 required checks:

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

A second run for this report follows; it must be green before the
milestone is handed over.

## Test delta

| Target | Before | After | Δ |
|---|---|---|---|
| `usage-cli` unit (`src/main.rs`) | 40 | 64 | **+24** |
| `usage-cli` integration (`tests/cli.rs`) | 8 | 13 | **+5** |
| `usage-core` | 113 | 113 | 0 |
| `usage-ui` | 131 | 131 | 0 |
| **total** | **292** | **321** | **+29** |

New unit tests (P1 1–12, P2 13–24):

```
 1 fail_at_defaults_off_and_parses_a_threshold
 2 fail_at_accepts_only_1_through_100
 3 fail_at_with_a_bad_value_or_no_value_is_a_usage_error
 4 worst_at_or_over_returns_none_for_empty_input
 5 worst_at_or_over_trips_at_exactly_the_threshold
 6 worst_at_or_over_rounds_as_the_window_rounds
 7 worst_at_or_over_covers_per_model_buckets
 8 worst_at_or_over_picks_the_highest_percentage
 9 worst_at_or_over_breaks_ties_by_provider_then_window_order
10 worst_at_or_over_ignores_windows_with_unknown_usage
11 fail_at_line_is_byte_exact
12 help_documents_the_exit_codes_verbatim
13 exactly_one_mode_is_required
14 watch_interval_floor_is_the_pollers_own
15 watch_below_the_floor_names_the_floor_verbatim
16 watch_rejects_non_integer_intervals_and_a_missing_value
17 rfc3339_utc_formats_known_anchors
18 watch_separator_is_byte_exact
19 once_json_output_is_byte_for_byte_the_pretty_form
20 ndjson_is_one_compact_line_carrying_the_same_object
21 ndjson_is_the_watch_mode_and_nothing_else
22 only_watch_text_output_gets_a_separator
23 both_modes_run_the_same_cycle_body
24 help_documents_the_watch_mode_and_its_floor
```

New integration tests:

```
help_prints_the_exit_codes_block
fail_at_outside_one_to_hundred_exits_two_before_polling
help_lists_the_watch_mode_and_the_two_mode_usage_line
watch_below_the_polling_floor_exits_two_with_the_floor_message
the_two_modes_cannot_be_combined
```

P3 added no tests (documentation only).

## Byte-pinned strings — verbatim grep proof

Every string the spec pinned, as it exists in the tree at `26fe690`.

**1. The `--fail-at` trip line** — `crates/usage-cli/src/main.rs:334`
(the sole producer) and its full-equality test at `:1919`:

```
        "fail-at: {} {label} at {percent}% >= {n}%",
            "fail-at: claude 5h at 92% >= 90%"
```

Spec: `fail-at: <provider> <window label> at <PCT>% >= <N>%` — the
format string substitutes provider name, label, percent, threshold in
that order, and the test pins the rendered bytes for both providers.

**2. The `--watch` interval floor error** — the constant, its
unit-test expectation, and the process-level assertion:

```
crates/usage-cli/src/main.rs: const WATCH_FLOOR_ERROR: &str = "--watch interval must be at least 180 seconds (the polling floor)";
crates/usage-cli/src/main.rs:1987:            "--watch interval must be at least 180 seconds (the polling floor)"
crates/usage-cli/tests/cli.rs:340:            stderr.contains("--watch interval must be at least 180 seconds (the polling floor)"),
```

**3. The watch separator line** — producer at `:223`, pinned bytes at
`:2025`:

```
    format!("--- {} ---", format_rfc3339_utc(unix_secs))
            "--- 2026-01-01T00:00:00Z ---"
```

Spec: `--- <RFC3339 UTC timestamp> ---`.

**4. The `--help` exit-codes section** — in `HELP` at `:112` and
asserted verbatim (as one multi-line literal, unit test at `:1933` and
integration test) :

```
exit codes:
  0  success; with --fail-at: all windows under the threshold
  1  a provider or credential error
  2  usage error
  3  --fail-at tripped: a window reached the threshold
```

**5. The JSON stability policy** — `docs/cli-json.md:13-15`, exactly
the three lines the spec supplied, line breaks included:

```
Keys are never renamed or removed within a major version. New keys
may be added in any release and are announced in the CHANGELOG.
Consumers must ignore keys they do not recognize.
```

## Exit codes — where each one is proven

| Code | Proven by |
|---|---|
| `2` usage error | `fail_at_outside_one_to_hundred_exits_two_before_polling` (`0`, `101`, `ninety`, and a missing value), `watch_below_the_polling_floor_exits_two_with_the_floor_message` (`1`, `60`, `179`), `the_two_modes_cannot_be_combined`, plus the pre-existing `missing_required_mode_still_errors` and `unknown_flag_still_errors`. All assert `Some(2)` from the real binary. |
| `1` provider error | Unchanged, and still pinned by the pre-existing `proxy_env_without_the_flag_fails_closed_and_prints_the_hint` / `lowercase_proxy_env_fails_closed_too`, which assert `Some(1)` from the real binary. |
| `0` success | The no-trip path: `worst_at_or_over_returns_none_for_empty_input`, and the `N = 91` arm of `worst_at_or_over_trips_at_exactly_the_threshold` (a 90 % window does not trip a 91 % gate → `None` → the gate does not fire). |
| `3` trip | The decision is `worst_at_or_over` (tests 4–10) and the line is `fail_at_line` (test 11); the single `ExitCode::from(3)` call site consumes exactly those two. **Stated plainly: exit 3 is not demonstrated end-to-end by a process test.** Reaching it requires a *successful* poll, i.e. real credentials and a live request — excluded by §4.4 and by the spec ("network testing is not required"). What is proven is every input that decides it, plus that nothing else can produce it. |

## Mutation check (beyond spec)

Tests that cannot fail prove nothing, so each new behavior was
mutated and the suite re-run before the phase was committed. Eight
mutations, eight caught:

| Mutation | Caught by |
|---|---|
| `--fail-at` range widened to `0..=1000` | `fail_at_accepts_only_1_through_100`, `fail_at_with_a_bad_value_or_no_value_is_a_usage_error` |
| tie-break flipped (`>` → `>=`, i.e. last wins) | `worst_at_or_over_breaks_ties_by_provider_then_window_order` |
| exit-codes block reworded in `HELP` only | `help_prints_the_exit_codes_block` (integration) |
| floor error reworded | `watch_below_the_floor_names_the_floor_verbatim` |
| `--once` + `--watch` silently accepted | `exactly_one_mode_is_required` |
| interval floor check disabled | `watch_interval_floor_is_the_pollers_own` |
| `json_is_ndjson` forced to `false` (watch emits pretty JSON) | `ndjson_is_the_watch_mode_and_nothing_else` |
| `prints_cycle_separator` forced to `false` | `only_watch_text_output_gets_a_separator` |

The last two mutations initially **escaped** — the first pass tested
`render_json` directly but nothing pinned that `--watch` selects the
compact form, so `--watch --json` could have silently emitted
multi-line documents. Two named seams (`json_is_ndjson`,
`prints_cycle_separator`) were added with their call sites pinned, and
the battery was re-run. This is reported because it is the one place
the delivered tests are stronger than the spec required, and because
the gap existed at all.

## Hard limits — verified, not assumed

**No §4.1 protected path changed.** Complete diff-stat for the slice:

```
$ git diff --stat 3cbd1f2..HEAD
 README.md                     |  29 +-
 crates/usage-cli/src/main.rs  | 987 ++++++++++++++++++++++++++++++++++++++----
 crates/usage-cli/tests/cli.rs | 121 ++++++
 docs/cli-json.md              | 161 +++++++
 4 files changed, 1207 insertions(+), 91 deletions(-)
```

Four files. None is under `crates/usage-core/src/egress/**`,
`crates/usage-core/src/credentials/**`, `.github/**`, `.cargo/**`,
`.claude/**`; none is `invariants.manifest`,
`tools/check-invariants.py`, `deny.toml`, `SECURITY.md`, or
`THREAT_MODEL.md`; no security-invariant test was edited (the `INV:`-
tagged tests in `tests/cli.rs` are untouched — the new tests were
appended, and `check-invariants.py` passes unchanged at 24 bindings).

**Zero new dependencies.** Neither `Cargo.toml` nor `Cargo.lock`
appears in the diff. Sleeping is `std::thread::sleep`; the RFC 3339
timestamp is formatted by a local `civil_from_days` (the inverse of
the conversion `usage-core` already parses timestamps with), so no
date crate was needed. `cargo-deny` and `cargo-audit` green.

**No JSON key added, removed, or renamed.** `crates/usage-core/src/model/`
is not in the diff, and it is the only place snapshot keys are
defined. `--once --json` bytes are pinned against the exact pre-M12
expression by `once_json_output_is_byte_for_byte_the_pretty_form`;
`--watch --json` differs only in whitespace, asserted by parsing both
forms to `serde_json::Value` and comparing.

**No version bump, no CHANGELOG.** Version stays `1.4.1`; both belong
to M12-RELEASE.

**No credential file read by this session.** `~/.claude/**` and
`~/.codex/**` were never opened (§4.4), and no live poll was run — the
new process-level tests all exit at argument parsing, before any
credential or network access.

## Deviations from the spec

None that change the specified behavior. Recorded for review:

1. **Two extra seams.** The spec named `worst_at_or_over` and the pure
   interval validation. `json_is_ndjson(mode)` and
   `prints_cycle_separator(args)` were added for the reason in the
   mutation section above — without them the mode→output-shape wiring
   was untested. Both are pure and additive.
2. **`worst_at_or_over` returns `Option<(ProviderId, &str, i64)>.`**
   The spec sketched `Option<(provider, label, pct)>`; the label
   borrows from the snapshot rather than allocating.
3. **Serialization-failure handling moved.** A `serde_json` failure
   used to `return ExitCode::FAILURE` from `main`; inside the shared
   cycle it now sets `had_error`, which is the same exit code for
   `--once` and keeps a `--watch` alive rather than killing the
   watcher over one bad cycle.
4. **Floor-authored wording** (permitted by the spec) for: the
   `--fail-at` range error (`--fail-at must be a whole number from 1
   to 100 (got …)`), the mode-conflict error (`--once and --watch
   cannot be combined; pass exactly one`), the missing-mode error (`a
   mode is required: --once or --watch <SECS>`), the non-integer
   interval error, and the two new `--help` option entries.
5. **One README line beyond the CLI section.** The feature list gained
   a "gate for scripted runs" bullet next to the existing "Headless
   mode" one; the spec scoped P3 to the CLI section and the new doc.
   It states no new capability — flag it if it is unwanted.
6. **`--help` mode wording.** `--once` no longer says "the only mode
   today", and the usage line is now
   `(--once | --watch <SECS>)`; the previous text became false with P2.

## §4 conditions hit

**None.** No protected path was reached, no dependency was needed, no
precondition mismatched, no CI failure occurred, and no credential was
touched. Nothing was worked around.

## What the owner must do next

1. **Review and accept M12** (§4.8) — the milestone gate is yours.
   Nothing in this slice is self-accepted.
2. **Nothing to look at visually.** This slice is CLI-only; the window
   is untouched.
3. **After acceptance: M12-RELEASE** — the first instantiation of
   `prompts/release-template.md`, cutting **v1.5.0** with the version
   bump and CHANGELOG entry that were deliberately left out here.
4. Optional call on deviation 5 (the extra README bullet) before the
   release spec is authored.
