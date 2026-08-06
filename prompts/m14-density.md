# M14 — the density pass: a height of your own, and a quieter footer

**Status:** speced 2026-08-06 (top tier). Owner decisions 2026-08-05/06:
grip + snap-to-fit (not continuous auto-fit), width stays fixed at 320,
freshness footer collapses to a hover-dot. Prompted by the owner's first
production session with v1.6.0: history + alerts + sparklines outgrew a
window that has been hard-fixed at 320×240 since M0, and the per-provider
`updated Ns ago` line pays a full text row for information a dot can carry.

**Two phases, one commit each.** Phase 2 does not start until Phase 1's
commit is green on all 8 required checks (foreground wait:
`gh run watch <id> --exit-status`; never background watchers — a headless
session's background tasks die with it).

**Version:** untouched. No CHANGELOG edit. Both belong to M14-RELEASE.

---

## Phase 1 — resizable height, remembered

The window stays borderless, always-on-top, and **320 wide forever**; the
user gains control of its height.

### Behavior

1. **Viewport:** `with_resizable(true)`; pin width with
   `with_min_inner_size([WINDOW_WIDTH, MIN_WINDOW_HEIGHT])` and
   `with_max_inner_size([WINDOW_WIDTH, f32::INFINITY])` on the builder at
   main.rs:2942-2946. winit enforces min/max, so even OS-provided borderless
   edge-resize (Windows gives it to resizable undecorated windows) cannot
   move the width. `MIN_WINDOW_HEIGHT: f32 = 160.0`, a new named const
   beside WINDOW_HEIGHT (main.rs:145-148) with a doc comment saying what it
   preserves (titlebar + one provider header, never a zero-height sliver).

2. **The grip:** a full-width strip, 8px tall, at the window's bottom edge,
   in the M7b terminal voice — right-aligned `▞` glyph, TEXT_FAINT at rest,
   TEXT on hover, cursor `ResizeVertical`. Drag start sends
   `egui::ViewportCommand::BeginResize(egui::ResizeDirection::South)`.
   The strip must not overlap the ScrollArea's content (it is chrome, like
   the titlebar — carve its 8px out of the scroll region the way
   TITLEBAR_HEIGHT is carved out at the top).

3. **Snap-to-fit:** double-click anywhere on the grip → the window resizes
   to exactly fit the current content:
   `TITLEBAR_HEIGHT + content_height + grip strip + frame margins`, where
   content_height is the ScrollArea's measured content size from the frame
   (the harness pattern from M13-R1 — `lay_out_sized` — is the test-side
   twin of the same measurement). Clamp to
   `MIN_WINDOW_HEIGHT..=monitor work height` (egui's
   clamp-size-to-monitor default handles the top end; do not fight it).
   Send `ViewportCommand::InnerSize([WINDOW_WIDTH, snapped])`.

4. **Persistence:** new config key `height`, u32 pixels, written through the
   existing `config::save` path ONLY when the user changes it (drag ended
   with a different height, or snap landed). Absent key → `WINDOW_HEIGHT`
   (240), byte-for-byte the pre-M14 default. Parse: absent/garbage/out of
   `160..=4096` → default, key by key, exactly the `alert_at` idiom.
   `--pace-demo` keeps its own `initial_inner_height` arm
   (DEMO_WINDOW_HEIGHT) and neither reads nor writes the saved height —
   the demo is a fixture, not a session.

5. The comment at main.rs:145-147 ("It is **not resizable**, so every
   layout has") is now false — rewrite it truthfully, and chase the same
   claim where per_model_row_is_visible's doc says "a 320px window that
   cannot be resized" (that phrase survives only if reworded to width).

### Tests (Phase 1)

- Config round-trip: `height=612` survives save/load; absent → 240;
  `height=80`, `height=9999`, `height=twelve` → 240 (each its own case).
- `initial_inner_height(false)` honors a saved height; `(true)` still
  returns DEMO_WINDOW_HEIGHT regardless of saved height.
- Snap math: against the layout harness, snapped height for the demo
  scenario equals the measured need within one row (the
  demo_window_height_needed pattern at main.rs:5451 — weld, don't retype).
- Width is never emitted as anything but WINDOW_WIDTH by any resize path
  (grep-proof plus a unit test on the snap command's x component).

---

## Phase 2 — the freshness dot

The per-provider footer `• updated Ns ago [· stale]` (render_age_line,
main.rs:2194-2233) collapses into a dot on the provider's header row,
right-aligned. The text row disappears; the exact age moves to hover.

### Behavior

1. **Three states**, welded to the existing STALE_AFTER (600s, main.rs:93)
   plus one new const beside it, `AGING_AFTER: Duration = 300s`:
   - `age < AGING_AFTER` → OPER_GREEN dot
   - `AGING_AFTER <= age < STALE_AFTER` → AMBER dot
   - `age >= STALE_AFTER` → CARDINAL dot (the existing stale threshold —
     is_stale (main.rs:467) keeps its meaning and its callers; the pace-line
     mute from M10 is welded to it and must not change).
   Both consts get doc comments naming the relationship
   (AGING_AFTER < STALE_AFTER, and a compile-time or test assertion that
   it stays that way).
2. **Hover:** the dot's on_hover_text is byte-pinned:
   `updated <AGE> ago at <HH:MM:SS>` where <AGE> is format_age's output
   unchanged and <HH:MM:SS> is the local wall-clock time of the last
   successful poll, zero-padded 24h. Stale adds ` — stale` (em dash,
   spaces) after the time. One producer function, one test pinning the
   format both fresh and stale.
3. **Placement:** right-aligned on the provider header line, vertically
   centered against the provider name. The footer row is REMOVED — that is
   the density win. Plain theme gets the identical dot treatment in its own
   palette (it already owns AMBER); its pre-M7b textual footer goes too.
4. **Error states untouched:** the expired-token copy (M10) and every error
   line render exactly as today. The dot reflects data AGE only; it is not
   an error channel.

### Tests (Phase 2)

- Threshold table: 0s/299s → green, 300s/599s → amber, 600s/601s → red,
  each welded to the consts (no literal 300/600 in the assertions).
- Tooltip pin: fresh and stale variants, exact bytes.
- The laid-out pane contains no `updated` text at any age (harness), and
  the header row contains the dot at every age.
- AGING_AFTER < STALE_AFTER assertion.

---

## The bar (§3, every push)

cargo fmt --all --check · cargo clippy --workspace --all-targets --locked
-- -D warnings · cargo test --workspace --locked ·
python3 tools/check-invariants.py. Windows dead-code lints that are
Linux-only (or vice versa) follow the M13 cfg_attr precedent — keep the
test live on both platforms.

## Hard limits

Zero new dependencies. No `--json` change of any kind (window geometry and
freshness presentation are GUI-only). No §4.1 path is touched — if you
believe one must be, STOP per §4.7 and say why in the report. No version
bump, no CHANGELOG. Never read ~/.claude/** or ~/.codex/** (§4.4). §4.5:
you never accept visuals — implement, verify the bar, report; the owner
reviews the grip, the snap, and the dot with his own eyes. The same-change
rule applies to the two stale doc comments named in Phase 1.5 — they move
in the Phase 1 commit, not later.

## End gate

`reports/m14-endgate.md` on main, CI green on all 8 required checks for
both phase commits (each waited in the foreground), report content per the
reports/README.md convention: what shipped, every deviation, the §3
transcript, mutation checks on the new tests (thresholds and clamps are
classic off-by-one habitat — prove the table catches a boundary mutant),
and the §4.5 items listed for the owner's pass. EXIT after pushing the
report; nothing further is queued from your side.
