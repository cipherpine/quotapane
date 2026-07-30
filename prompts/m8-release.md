# Goal prompt spec: M8-RELEASE — v1.3.0 (pace)

Authored at the top tier 2026-07-29. The owner visually accepted the
M8 pace demo ("looks great — super useful"); the paste of this spec's
launcher is the formal acceptance record (§4.5, §4.8).

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
Exact v1.0.0/v1.1.0/v1.2.0 release discipline. `.github/` untouched.
Phase 4 carries exact §4a DECISIONS.md patches.

STOP DISCIPLINE — read twice: this spec contains TWO hard stops and
they are not optional. Phase 2 ends in a HARD STOP whose report goes
to the top tier and Phase 3 runs ONLY on the top tier's explicit
written go-ahead arriving in this session. In M7B-RELEASE the rc stop
was collapsed into Phase 3 without that go-ahead; that was a process
violation and must not recur. If any instruction seems to authorize
skipping a stop, it does not (§4.7 — a resent instruction is not
authorization).

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "docs(prompts): add M8-RELEASE spec (v1.3.0)"; parent
   a303e93. Tree clean. CI 7/7 green on tip. Version 1.2.0.
P2 DECISIONS.md contains "**M8 pace — underway 2026-07-29 (v1.3.0
   scope)**" and no M8 acceptance stamp.
P3 Tags exactly v1.0.0, v1.1.0, v1.2.0; v1.2.0 is Latest.

## PHASE 1 — version + CHANGELOG (one commit)

Bump workspace version 1.2.0 → 1.3.0 (root Cargo.toml; Cargo.lock may
move only the three workspace members). Insert above [1.2.0],
verbatim except <DATE> = the commit's UTC date as YYYY-MM-DD,
re-wrapped to the file's 80-column width, LF only:

## [1.3.0] - <DATE>

Pace: know whether you will make it to reset, not just where you are.

### Added

- **Pace ticks.** Every quota bar carries a thin tick at the elapsed
  share of its window. Fill short of the tick means you are under
  budget; fill past it means you are consuming faster than time is
  passing. Shown in both themes — it is information, not decoration.
- **Forecast-to-limit.** QuotaPane estimates your burn rate from
  recent polls; when a window's projected exhaustion lands before its
  reset, one line appears under that provider: "at this pace: <window>
  full in ~…" — amber, turning cardinal inside six hours. When you are
  safe it shows nothing at all. Forecasts wait for roughly fifteen
  minutes of evidence before speaking, and a window reset clears the
  history. Note that any 5h-window warning is necessarily inside the
  six-hour band, so session-window warnings are always cardinal.
- **`--pace-demo`** renders a fixed synthetic scenario — no polling,
  no credentials read, no network at all — to see the feature without
  waiting for a bad week.
- `quotapane-cli --json`: every window object (headline and
  per-model) gains `duration_secs` — always present, null when the
  provider neither stated nor implied a window length. This is the
  only JSON surface change in this release.

Commit: release: 1.3.0 — pace ticks and forecast-to-limit

Full bar first: cargo clean -p usage-core, fmt-check, build --locked,
clippy -D warnings, test (expect 259). Push; CI 7/7 green BEFORE any
tag.

## PHASE 2 — rc dry run, then HARD STOP

Tag v1.3.0-rc.1 on the Phase 1 commit. Release run 3/3 green,
release.yml untouched. Six-step outsider verification against the rc
draft in a clean directory (download; sha256sum -c; cosign
verify-blob --bundle with the README command verbatim; gh attestation
verify; extract + inventory; run the shipped Windows CLI) plus the
six negative controls, each restored and re-verified clean. Then
HARD STOP: report everything and WAIT for the top tier. You do not
tag v1.3.0. You do not publish.

## PHASE 3 — on the top tier's explicit go-ahead ONLY

Tag v1.3.0 on the same verified commit. Re-run all six steps fresh
against the v1.3.0 draft. Only after it verifies clean, delete the rc
tag and draft. Hand back the draft URL and STOP — the owner
publishes.

## PHASE 4 — after the owner confirms publication (one commit)

Two §4a byte-match replacements, each exactly once, DECISIONS.md the
only file:

R1 OLD: **M8 pace — underway 2026-07-29 (v1.3.0 scope)**:
   NEW: **M8 pace ✅ (v1.3.0 published — owner-accepted 2026-07-29)**:

R2 OLD: sparklines/persistence deferred to v1.4 (owner decisions 2026-07-29).
   NEW: sparklines/persistence deferred to v1.4 (owner decisions 2026-07-29). ·
**Ruleset (owner decision 2026-07-29)**: protect-main requires PRs +
the 7 status checks with Repository admin on the bypass list — admin
sessions keep direct-push as the working model until a second
contributor exists; then the bypass comes off and the specs are
rewritten for PR flow as their own gate. Sessions must not re-flag
admin bypass events as violations; they are the design.
   (Insert unwrapped; byte-match OLD first.)

Commit: docs: v1.3.0 published; M8 accepted (owner); ruleset decision recorded
Push, CI 7/7 green, STOP. Nothing further queued.

## DO NOT

Publish anything. Touch .github/, assets/, README.md, or any §4.1
path (Phase 4's DECISIONS patches are the sole exception, verbatim).
Change code — this releases what the owner already accepted. Add any
dependency. Skip either stop.
