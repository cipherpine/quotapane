# M12 — CLI automation: --watch, --fail-at, stable JSON (ships as v1.5.0)

Authored at the top tier 2026-08-04. Owner decision: CLI automation is
the v1.5.0 slice; this is the first slice dispatched headlessly (M11d).
Read DECISIONS.md before touching anything — §3, §4, §4a, §4.4, §4.7.

## Why

The people finding QuotaPane are the people whose scripted/agentic runs
die mid-flight on quota. Give them a gate: a CLI that can watch quota
on an interval and fail loudly *before* a big run starts. This is also
the security-clean answer to the competitor's shell-trigger feature —
their config file executes commands; our flag inverts the risk so the
user's script polls and NOTHING in QuotaPane ever executes anything.

## Boundaries

- Zero new dependencies (sleeping is `std::thread::sleep`).
- No §4.1 path changes; no egress/credential changes of any kind.
- **No JSON key is added, removed, or renamed in this slice.**
- All user-visible strings below are byte-exact.
- End-gate report goes to reports/m12-endgate.md (reports/README.md).

## Feature spec

**`--watch <SECS>`** — the second mode (today `--once` is the only
one). Exactly one of `--once` / `--watch <SECS>` must be given; both or
neither is a usage error (exit 2, existing convention). SECS is an
integer ≥ 180 — the poller's MIN_INTERVAL floor applies to scripted
polling too; below 180 is a usage error naming the floor:

    --watch interval must be at least 180 seconds (the polling floor)

Each cycle polls the selected providers exactly as `--once` does, then
sleeps SECS. Text mode: each cycle prints the normal text block,
preceded by one separator line, exactly:

    --- <RFC3339 UTC timestamp> ---

JSON mode (`--watch` + `--json`): NDJSON — each cycle emits exactly ONE
line, the same object `--once --json` produces, serialized compactly
(no internal newlines). `--once --json` output is byte-for-byte
unchanged. A test asserts the NDJSON line contains no `\n`.

**`--fail-at <N>`** — N integer 1..=100, else usage error (exit 2).
After each poll (works with both modes), every window in every
successfully polled snapshot — headline AND per-model; a gate fails
safe, scripts wanting narrower semantics filter `--json` themselves —
is checked: used-fraction × 100, rounded as the UI rounds, ≥ N trips.
On trip, print one stderr line naming the worst offender (highest
percentage; ties broken by provider order then window order), exactly:

    fail-at: <provider> <window label> at <PCT>% >= <N>%

then exit with code **3**. Under `--watch`, the first tripping cycle
exits 3. Exit-code precedence per invocation: any trip → 3; else any
provider error → 1 (unchanged); else 0. The trip check runs only over
snapshots that polled successfully — an errored provider is exit-1
territory, never a silent pass.

**Exit codes documented in --help**, appended to the help text as its
own section, exactly:

    exit codes:
      0  success; with --fail-at: all windows under the threshold
      1  a provider or credential error
      2  usage error
      3  --fail-at tripped: a window reached the threshold

Help also gains `--watch <SECS>` and `--fail-at <N>` lines in the
existing option-list style (wording: floor's, matching current voice;
these two are the only non-byte-pinned strings in this spec).

**Pure logic requirement:** threshold evaluation is a pure function
over the normalized snapshots (no clock, no I/O) — e.g.
`fn worst_at_or_over(snapshots, n) -> Option<(provider, label, pct)>`
— unit-tested directly: empty input, exact-threshold boundary (== N
trips), rounding boundary, per-model bucket trips, tie-break order.
Interval validation is likewise pure and tested at 179/180/181.

## Phases (one commit each)

P1 `cli: --fail-at gate logic and flag parsing` — flags, validation,
   pure gate fn, exit-code plumbing for --once, tests.
P2 `cli: --watch mode with NDJSON output` — loop, separator line,
   compact serialization, watch+fail-at interaction, tests. Manual
   network testing is NOT required; the loop's per-cycle body must be
   the same function --once calls, tested at that seam.
P3 `docs: CLI automation + the JSON stability contract` — README CLI
   section gains the two flags and exit codes; NEW file
   docs/cli-json.md documenting every current --json key with type and
   nullability, plus the stability policy verbatim:

    Keys are never renamed or removed within a major version. New keys
    may be added in any release and are announced in the CHANGELOG.
    Consumers must ignore keys they do not recognize.

## The bar (§3), every phase

cargo fmt --all --check; cargo clippy --workspace --all-targets
--locked -- -D warnings; cargo test --workspace --locked. All green
before each commit. python3 tools/check-invariants.py must also pass
(it gates CI now); this slice should not touch anything it checks —
if it fails, STOP.

## End gate

Push all commits, wait for CI green on all 8 required checks, write
the complete end-gate report to reports/m12-endgate.md (SHAs, test
delta, verbatim-grep proof of every byte-pinned string, diff-stat
proof no §4.1 path changed, exit-code demonstration via the test
names), commit it as `reports: M12 end-gate`, push, CI green again,
STOP. Acceptance is the owner's (§4.8). No version bump, no CHANGELOG
— M12-RELEASE (first instantiation of prompts/release-template.md)
cuts v1.5.0 after acceptance.
