# Goal prompt spec: M8 — pace (v1.3.0 scope)

Authored at the top tier 2026-07-29 from the roadmap research the
owner accepted: pace markers, burn-rate forecast-to-limit. Sparklines
and on-disk history are DEFERRED to v1.4 — building them here is a
scope violation, not initiative.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
ZERO new dependencies. No §4.1 path is touched except Phase 0's
DECISIONS patch. No new network behavior, no new persistence of any
kind — everything below is arithmetic over data the app already has.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject is "docs(prompts): add M8 pace spec (v1.3.0 scope)";
   parent d212e7c. Tree clean. CI green. Workspace version 1.2.0 —
   and it STAYS 1.2.0; the version bump belongs to M8-RELEASE.
P2 DECISIONS.md contains "M7b Cipher Pine visual pass ✅" and no M8
   entry.

## PHASE 0 — §4a DECISIONS patch (byte-match, replace exactly once)

OLD: no vendor logos (owner decisions 2026-07-29).
NEW: no vendor logos (owner decisions 2026-07-29). ·
**M8 pace — underway 2026-07-29 (v1.3.0 scope)**: elapsed-time pace
markers on every bar; burn-rate forecast-to-limit from an in-memory
snapshot ring, surfaced only when projected exhaustion precedes the
reset; QuotaWindow gains duration_secs (new nullable JSON key);
sparklines/persistence deferred to v1.4 (owner decisions 2026-07-29).
(Insert unwrapped; byte-match OLD first.)
Commit: docs: open M8 — pace

## PHASE 1 — window duration (usage-core; one commit)

- `QuotaWindow` gains `pub duration_secs: Option<u64>` — the window's
  total length. Codex: pass `limit_window_seconds` through (both
  headline and per-model rows). Claude: derive from the limit kind —
  five-hour kinds 18000, weekly kinds 604800; anything else None. The
  legacy opus/sonnet fallback rows are weekly: 604800.
- CLI `--json`: the key serializes always (null when unknown), same
  pin-style test as reset_credits. This is a JSON surface ADDITION —
  record it in the end gate so the release CHANGELOG states it.
- Tests: Codex passthrough incl. per-model; Claude 5h/7d/unknown kind;
  JSON key present-and-null.

## PHASE 2 — pace math (usage-core, new `src/pace/mod.rs`; one commit)

Pure functions only; no clock reads inside the module — callers pass
now.
- `PaceSample { at_unix_secs: u64, used_fraction: f64 }`
- `PaceRing`: fixed capacity 256 per (provider, window-label); push;
  `reset_detected` clears when a new sample's fraction drops more
  than 0.05 below the last, or resets_in jumps up past the window
  duration's half (caller supplies the facts, ring stays dumb).
- `estimate(samples, now) -> Option<Burn>` where
  `Burn { per_hour: f64, exhaust_in_secs: Option<u64> }`:
  least-squares slope over samples within the trailing 7200 s;
  require >= 3 samples spanning >= 600 s; slope <= 0 -> exhaust None
  (burning nothing); else exhaust = (1.0 - latest)/slope, capped at
  14 days. NaN/degenerate inputs -> None, never panic.
- `at_risk(burn, resets_in_secs) -> bool`: exhaust is Some AND
  sooner than the reset.
- Tests: steady rise hits the algebraic answer; flat and falling give
  exhaust None; sparse (<3 or <600 s span) gives None; reset clears;
  a synthetic "80% at half-window" case is at_risk against its reset
  and a "20% at half-window" case is not.

## PHASE 3 — the pace tick (usage-ui; one commit)

On every bar whose window has BOTH used_fraction and duration_secs
plus resets_in_secs: a 1-px vertical tick across the bar's height at
x = elapsed_fraction * bar_width, where elapsed_fraction =
1 - resets_in/duration, clamped 0..=1. Color TEXT_DIM at alpha 200 in
BOTH themes (it is information, not theming). No tick when any input
is unknown. Reading: fill left of the tick = under budget; fill past
it = burning faster than time. Pure helper for the x position,
unit-tested at 0, mid, 1, and unknown-input None; harness asserts row
heights unchanged at 320 px.

## PHASE 4 — the at-risk line (usage-ui; one commit)

- Wire a PaceRing per (provider, headline window) fed on every poller
  Update; clear on detected reset. Per-model rows get ticks (Phase 3)
  but no rings in this slice.
- Per provider, compute Burn for each headline window; if any is
  at_risk, ONE line under that provider's bars: mono small,
  "at this pace: <label> full in ~<duration>" — AMBER; CARDINAL when
  exhaust_in < 21600 (6 h). No line otherwise — calm is silent. Two
  at-risk windows: show the sooner.
- Repaint discipline unchanged: everything recomputes on poll events
  only; zero new repaint scheduling.
- `--pace-demo` flag on the GUI binary: renders from a synthetic
  snapshot script (no polling, NO network at all in this mode, title
  shows "demo") that exercises tick positions, an at-risk amber line,
  and a cardinal line — this is how the owner reviews the feature
  without waiting hours, and how screenshots get made. One README
  sentence under Theming: flag exists, shows fake data, talks to
  nothing.
- Tests: at-risk selection (sooner wins), line text formatting,
  demo-mode snapshot generation is deterministic.

## VERIFY + SHIP

Full bar from cargo clean -p usage-core. Push. CI 7/7 green. No
Cargo.lock movement. Version still 1.2.0.

## END GATE — STOP (§4.5)

Report SHAs, CI run, test delta, the JSON keys added, and anything
where the spec's numbers (0.05 reset drop, 7200 s trail, 600 s span,
6 h cardinal) proved awkward — flag, don't retune. Then STOP for the
owner's visual pass: expect `--pace-demo` review first, then a real
soak. M8-RELEASE (v1.3.0) is a separate top-tier spec after
acceptance.
