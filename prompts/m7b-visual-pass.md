# Goal prompt spec: M7B — Cipher Pine visual pass

Authored at the top tier (Cowork bridge) 2026-07-29. Owner decisions,
all resolved: direction B (Cipher Pine terminal) from the comparison
mockup; org avatar = mark 1b, repo mark = 1c "Two panes"; tray/taskbar
icon = LIVE miniature of 1c; no vendor logos ever (trademark ruling);
no per-provider hues in-window; cursor is a status indicator, not
decoration. Design tokens from the owner's design doc: pine #2d7a4f,
cardinal #c41e3a, ground #0a0f0d.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Phase 0b carries an exact DECISIONS.md patch (§4a).
Everything else is yours — `usage-ui` is an ordinary path. `assets/`
already exists, committed at the top tier: read it, never modify it. No dependency may be added: every visual below is egui
`Style` + painter primitives + a hand-rasterized RGBA buffer, in the
M5a-triangle tradition. If any step seems to need an image/font crate,
STOP — that is a design error to hand back, not a crate to add.

## PRECONDITIONS (mismatch = STOP and report)

P1 v1.1.0 is tagged and PUBLISHED; workspace version is 1.1.0;
   DECISIONS.md contains "**M7a per-model truth ✅" (the M7-RELEASE
   phase-4 stamp landed). CI green on the tip.
P2 Tree CLEAN — this spec, the launcher index, the brand kit
   (assets/, 18 files), and the README banner were all committed at the
   top tier after v1.1.0 closed. assets/quotapane-1c.svg exists and
   README.md's first line embeds assets/quotapane-readme-banner.png.
   Any dirt, or a missing asset: STOP.

## THE PALETTE (Color32, exact — use these names as consts)

    GROUND        (10, 15, 13)    window fill        #0a0f0d
    PANEL         (14, 16, 15)    titlebar fill      #0e100f
    HAIRLINE      (30, 36, 34)    borders/grid base  #1e2422
    TEXT          (205,214,209)   primary            #cdd6d1
    TEXT_DIM      (138,147,142)   labels/resets      #8a938e
    TEXT_FAINT    (92, 102, 95)   updated-line       #5c665f
    PINE          (45, 122, 79)   healthy fill       #2d7a4f
    OPER_GREEN    (63, 174, 106)  operational dot    #3fae6a
    AMBER         (217,161, 59)   caution fill       #d9a13b
    CARDINAL      (196, 30, 58)   prompt/cursor/critical/stale #c41e3a

Semantic fill mapping (replaces fraction_color's colors; thresholds:
< 0.50 PINE, 0.50–0.79 AMBER, ≥ 0.80 CARDINAL). Unknown fraction stays
the existing neutral treatment.

## PHASE 0 — §4a DECISIONS patch only (spec already committed)

Replace exactly once:
OLD: while CLI/JSON stay truthful (owner decisions 2026-07-29).
NEW: while CLI/JSON stay truthful (owner decisions 2026-07-29). ·
**M7b Cipher Pine visual pass — underway 2026-07-29**: direction B
(terminal: grid, mono, // labels, cardinal prompt, status cursor),
live-miniature tray icon painting mark 1c from real usage, marks 1b/1c
adopted, no vendor logos (owner decisions 2026-07-29).
(Insert unwrapped; byte-match OLD first.)
Commit: docs: open M7b — Cipher Pine visual pass

## PHASE 1 — theme (one commit)

- All egui TextStyles → Monospace family (egui's built-in mono; no font
  asset). Sizes: heading 15, body 12, small 10.5 — then let the layout
  harness arbitrate: every existing width/height test must still pass
  at 320px; if mono overflows a row, shrink type, never widen layout.
- Window bg GROUND; titlebar PANEL with 1px HAIRLINE bottom border.
- Blueprint grid: painter lines every 40px both axes across the body,
  color PINE at alpha 12 (of 255) — texture, not noise.
- Titlebar text: "> quotapane" — the ">" in CARDINAL, rest TEXT, mono.
- Provider headers become "// CLAUDE" / "// CODEX": "// " in CARDINAL,
  name uppercase TEXT_DIM, small. (No letter-spacing in egui — plain
  uppercase mono is the approximation, accepted.)
- Bars: fill by the semantic mapping; trough PANEL with HAIRLINE
  border, 3px rounding. Percent text stays white-on-fill.
- "updated Ns ago" TEXT_FAINT with a small OPER_GREEN dot prefix when
  fresh; the whole line CARDINAL (dot too) when STALE. Stale = snapshot
  age > 600 s. "resets available: N" stays TEXT_DIM.

## PHASE 2 — status cursor (one commit)

A painted block cursor (rect, ~7×13 at titlebar scale, CARDINAL) after
the titlebar text: SOLID when idle and all providers fresh; BLINKING
(1.1 s period, steps) only while a poll is in flight or any provider is
stale. Repaint discipline: request_repaint_after only while blinking is
active — an idle healthy window must not tick repaints for the cursor.
Tests: pure state→(visible, needs_repaint) function, unit-tested for
idle/polling/stale; harness asserts titlebar height unchanged.

## PHASE 3 — live icon (one commit)

New `usage-ui/src/icon.rs`, pure and dependency-free:
    pub fn render_icon(claude: Option<f32>, codex: Option<f32>,
                       size: u32) -> Vec<u8>   // RGBA, size*size*4
Geometry = mark 1c: GROUND rounded square; window outline in TEXT_DIM;
two dots (CARDINAL, PINE); two horizontal bars — top Claude, bottom
Codex — trough HAIRLINE, fill by the semantic mapping of the fraction,
None → trough only. Plain rects at small sizes; corner rounding may be
approximated (skip corner pixels).
- Wire: eframe viewport icon (IconData) at startup from
  render_icon(None, None, 32) — the brand look; tray icon (Windows)
  re-rendered on every poller Update using the representative headline
  fractions, set via tray-icon's set_icon, ONLY when the rendered bytes
  changed (cache the last buffer).
- Tests (pure fn): buffer length; determinism; a bar-interior pixel is
  PINE at 0.2, AMBER at 0.6, CARDINAL at 0.9; None leaves trough color;
  0.0 vs 1.0 buffers differ.

## PHASE 4 — assets + README: VERIFY ONLY (no commit)

The brand kit and README banner landed at the top tier (commit
fb19db8, "docs(readme): adopt the Cipher Pine brand kit"). Verify
assets/quotapane-1c.svg exists and README.md's first line embeds
assets/quotapane-readme-banner.png, then move on. Commit nothing in
this phase; editing README.md or assets/ anywhere in this session is a
§4.7 stop.

## VERIFY + SHIP

Full bar from cargo clean -p usage-core. Push. CI 7/7 green. No
Cargo.lock movement of any kind.

## END GATE — STOP (the heaviest §4.5 gate yet)

Report SHAs, CI, test count, and any place mono type forced a size
tradeoff. Then STOP for the owner's visual pass — expect iteration:
the owner will send screenshots and adjustment rounds through the top
tier; do not self-assess the aesthetics. Version stays 1.1.0; the
v1.2.0 release prompt comes only after the owner accepts the look.
OWNER'S OWN LIST (report verbatim): none — the avatar, the social
preview, and the asset exports are already done.
