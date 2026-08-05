# M13-R1 — sparkline legibility iteration (round 2 of the §4.5 loop)

Authored at the top tier 2026-08-05. Owner round-1 verdict on the M13
visual pass: banner ✅, tray ring + tooltip ✅, sparkline ❌ — "hard to
tell what it is and what it means." This round makes the strip read as
a deliberate instrument instead of a faint scribble. Same charter
rules as M13 (§3, §4, §4.4, §4.5, §4.7); headless under the dispatcher.

## Boundaries

Zero new dependencies. No §4.1 bytes. No --json changes. No version
bump/CHANGELOG. Painter primitives only. End-gate report to
reports/m13-r1-endgate.md; then STOP for the owner's round-2 look.

## Changes (one commit unless the bar forces a second)

1. **The strip becomes legible.** Height 12 -> 16 px. Stroke: TEXT_DIM
   at FULL alpha, width 1.5 (was 1.0 at alpha 140). Add a fill under
   the curve, TEXT_DIM at alpha 18, from the line down to the strip's
   baseline — the trend should read as a shape, not a wire. Neutral
   ink on purpose: pine/amber/cardinal are status semantics and the
   strip is history, not status.
2. **A "now" anchor.** A filled dot, radius 2.5 px, TEXT at full
   alpha, at the newest point (right edge). This is the one bright
   mark — it tells the eye the line flows toward the present.
3. **Self-describing tag.** The text `24h` in the small style,
   TEXT_FAINT, right-aligned directly above the strip's right edge.
   Byte-exact tag text: `24h`
4. **Demo review window fits its content.** --pace-demo (and ONLY the
   demo) requests a taller initial inner size so the full two-pane
   scenario — banners, strips, per-model, footer — renders without
   scrolling. Production default size is UNTOUCHED (the
   renders-exactly-as-v1.5.0 pins must still pass). Size the demo
   height from the existing layout-harness measurement, not a magic
   number pulled from air; name the number and its derivation in the
   report.
5. Tests: update the strip-geometry unit tests for the new height and
   the dot/tag presence (real-font layout harness where measured);
   `the_demo_scenario_fits_the_window` becomes an equality-style claim
   against the demo's requested height rather than a recorded
   overflow. Existing "no history -> renders exactly as before" pins
   unchanged and still green.

## The bar (§3)

fmt, clippy -D warnings (all targets, locked), full tests, python3
tools/check-invariants.py — green before the commit.

## End gate

Push, CI green on all 8 required checks, write reports/m13-r1-endgate.md
(byte-pins: `24h`; geometry values as shipped; before/after alpha and
stroke numbers), commit it, push, CI green, EXIT. Then STOP: the owner
looks again with `cargo run --bin quotapane -- --pace-demo`. M13 and
M13-R1 are accepted together or iterated again; M13-RELEASE cuts
v1.6.0 only after that acceptance.
