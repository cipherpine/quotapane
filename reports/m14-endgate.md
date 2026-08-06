# M14 end-gate — the density pass (resizable height, freshness dot)

Floor session, headless under the M11d dispatcher. Spec:
`prompts/m14-density.md`. Two phases, two commits, in order, plus this
report. **Acceptance is the owner's (§4.8), and the visuals are the
owner's eyes only (§4.5) — nothing here is self-accepted, and no release
is cut.** Version stays `1.6.0`; no CHANGELOG entry (both belong to
M14-RELEASE).

## Commits

Base: `e77e5d0` — *prompts: M14 density-pass spec + launcher*. An
unrelated `db5db97` (*prompts: M15 agents-pane spec + launcher*) landed
locally between session start and the first commit; it is
`prompts/`-only and was carried up in the same push, unmodified.

| Phase | SHA | Subject |
|---|---|---|
| P1 | `618b0b9e224ab438ef49af372acf08a5c48561e0` | `M14: a height of your own — resizable window, grip, snap-to-fit` |
| P2 | `25415b8ec34bad86bb2e09823ca66de63672abde` | `M14: the freshness dot — a row back from the footer` |

Phase 2 was not started until Phase 1's commit was green on all 8
required checks, waited for in the foreground
(`gh run watch <id> --exit-status`). No background watchers were used.

## CI

Both runs waited in the foreground to completion; both **success** on
all 8 required checks.

| Phase | Run | SHA | Conclusion |
|---|---|---|---|
| P1 | <https://github.com/cipherpine/quotapane/actions/runs/31109564402> | `618b0b9` | success |
| P2 | <https://github.com/cipherpine/quotapane/actions/runs/31110729614> | `25415b8` | success |

Per-check, run 31110729614 (the tip):

| Check | Conclusion |
|---|---|
| build & test (windows-latest) | success |
| build & test (ubuntu-latest) | success |
| build & test (macos-latest) | success |
| cargo-deny (licenses, bans, advisories, sources) | success |
| cargo-audit (RustSec advisories) | success |
| gitleaks — full-history secret scan | success |
| invariant 4 — no telemetry | success |
| invariants — manifest, docs, and tests agree | success |

## The §3 bar

Run green locally before **each** push, on the exact tree that was
pushed:

```
cargo fmt --all --check                                      (clean, no output)
cargo clippy --workspace --all-targets --locked -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s)
cargo test --workspace --locked
    usage-cli   (unit)   64 passed; 0 failed
    usage-cli   (integ)  13 passed; 0 failed
    usage-core          127 passed; 0 failed
    usage-ui            207 passed; 0 failed      (196 after P1, 182 at base)
    doc-tests             0 passed; 0 failed
python3 tools/check-invariants.py
    OK: 7 invariants, 24 test bindings, tags and manifest set-equal,
    SECURITY.md id set matches.
```

Test count: the `usage-ui` binary went **182 → 196 → 207** across the two
phases (+25). By file, `#[test]` functions: `main.rs` 156 → 178,
`config.rs` 16 → 19, `icon.rs` 10 unchanged.

## What shipped

### Phase 1 — resizable height, remembered

- **Viewport.** `with_resizable(true)`, with `WINDOW_WIDTH` declared as
  *both* `with_min_inner_size` and `with_max_inner_size`, so winit pins
  the width even against the OS-provided borderless edge-drag Windows
  gives a resizable undecorated window. Height floors at a new
  `MIN_WINDOW_HEIGHT = 160.0`.
- **The grip.** An 8px full-width bottom strip carrying a right-aligned
  `▞`, TEXT_FAINT at rest and TEXT on hover, cursor `ResizeVertical`.
  Drag start sends `ViewportCommand::BeginResize(ResizeDirection::South)`
  so the OS owns the drag. It is chrome: an `egui::Panel::bottom` shown
  before the `CentralPanel`, exactly as `TITLEBAR_HEIGHT` is carved out
  at the top, so it never overlaps the ScrollArea's content.
- **Snap-to-fit.** Double-clicking the grip sends
  `ViewportCommand::InnerSize([WINDOW_WIDTH, snapped])` where `snapped =
  TITLEBAR_HEIGHT + panel chrome + content + GRIP_HEIGHT`, floored at
  `MIN_WINDOW_HEIGHT` and left to egui's own monitor clamp at the top
  end. The content height is this frame's `ScrollAreaOutput::content_size.y`
  and the chrome is measured from the live style, the way the layout
  harness measures it — so the snap is acted on *after* the layout, not
  from the previous frame.
- **Persistence.** New `config.cfg` key `height`, u32 pixels. Absent,
  unparsable, or outside `160..=4096` → 240, key by key, the `alert_at`
  idiom (fall back, do not clamp). `--pace-demo` keeps
  `DEMO_WINDOW_HEIGHT` and neither reads nor writes the saved height.
- **Doc corrections in the same commit** (the same-change rule): the
  "**not resizable**" claim on `WINDOW_HEIGHT`, the ScrollArea's "fixed
  240px with no resize", and `per_model_row_is_visible`'s "320px window
  that cannot be resized" (now: fixed *width*).

### Phase 2 — the freshness dot

- Three states welded to `STALE_AFTER` and a new `AGING_AFTER = 300s`
  beside it: OPER_GREEN under aging, AMBER between, CARDINAL at or past
  stale. `const _: () = assert!(AGING_AFTER.as_secs() < STALE_AFTER.as_secs())`
  keeps the order — a compile-time fact checked at compile time.
- `is_stale` keeps its meaning and every caller, M10's pace-line mute
  included. A test pins that the dot's CARDINAL and `is_stale` agree at
  every age, so the two surfaces cannot drift apart.
- Hover text from one producer, byte-pinned fresh and stale, with the
  age straight from `format_age`.
- Placement right-aligned on the provider header row (right-to-left
  outer layout, left-to-right inner — the shipped titlebar idiom). The
  footer row is gone in both themes; Plain gets the identical dot.
- Error states untouched: M10's expired-token copy still renders, in
  CARDINAL, with the dot beside it reporting age only.
- `DEMO_WINDOW_HEIGHT` re-derived twice as the chrome changed: 364 → 372
  (P1 adds the grip) → 330 (P2 removes two footer rows). Its doc table is
  re-measured, and `demo_window_height_needed` now goes through the
  shipped `snapped_height` rather than a retyped sum, so the demo's
  constant and the grip's double-click answer the same question with the
  same arithmetic.

## Deviations from the spec

Three, all deliberate, all measured. **Items 1 and 2 want a top-tier
ruling.**

### 1. The tooltip's clock is UTC, and says so

**Spec (Phase 2.2):** `updated <AGE> ago at <HH:MM:SS>` where the clock
is "the local wall-clock time of the last successful poll".
**Shipped:** `updated 5s ago at 09:14:22 UTC` (and
`… — stale` past `STALE_AFTER`).

Local time is not reachable under the milestone's own hard limit. `std`
exposes no timezone; the offset requires either a dependency (§4.2 hard
stop, and the spec says zero new dependencies) or per-platform `unsafe`
FFI — `GetLocalTime` on Windows, `localtime_r` on unix — which is raw FFI
in the window crate of a project whose thesis is a small auditable
surface, for a hover string. Feeding a UTC clock into a field labelled as
a local one would produce a number confidently wrong by the reader's own
offset: exactly the failure `usage-core::providers::time` rejects bare
local timestamps to avoid ("a bare local time read as UTC is off by the
reader's own timezone, quietly"), and exactly why
`quotapane-cli --watch` stamps `Z`. So the suffix is the honest version
of the same information.

Swapping it for a local clock is one function (`freshness_tooltip`) plus
whatever the top tier decides about how to obtain the offset. The
producer already takes the timestamp as a parameter, so nothing else
moves.

### 2. The maximum inner height is finite (4096), not `f32::INFINITY`

**Spec (Phase 1.1):** `with_max_inner_size([WINDOW_WIDTH, f32::INFINITY])`.
**Shipped:** `with_max_inner_size([WINDOW_WIDTH, MAX_WINDOW_HEIGHT])`
with `MAX_WINDOW_HEIGHT = 4096.0`.

Following the letter here would have broken the milestone's own feature
on its primary target. egui guards the *command* path against infinity —
`egui-winit-0.35.0/src/lib.rs:1791` refuses a non-finite
`MaxInnerSize` — but the **builder** path (`lib.rs:2076-2078`) hands the
value straight to winit as a `LogicalSize`. From there:
`winit-0.30.13` `to_physical` casts via `f64::round() as u32`, which
saturates infinity to `u32::MAX`; `window_state.rs:454-462`'s
`adjust_size` then writes that into `RECT.bottom` as `width as i32` =
`-1`; and `event_loop.rs:2190-2210`'s `WM_GETMINMAXINFO` handler sets
`ptMaxTrackSize` from the result. The window would end up with a maximum
track height of a few pixels — unresizable, the opposite of M14.

4096 logical points is far beyond any monitor's work area, and is the
same ceiling `config::height` already accepts up to, so the stored bound
and the viewport bound are one bound (welded by a `const` assertion).

### 3. Two ARCHITECTURE.md bullets moved in the Phase 2 commit

§8's **Modes** bullet said "one fixed, non-resizable size … 
`.with_resizable(false)`" and its **Liveness** bullet described the age
line. Both became false with this milestone. The spec named only the two
in-code doc comments, and §3 does not pre-approve ARCHITECTURE.md edits;
ARCHITECTURE.md is also not a §4.1 path, so this was not a stop. The
edits are descriptions of shipped behaviour, not architecture decisions,
and the project's same-change rule (M9b precedent) says a false doc line
does not get to outlive the change that falsified it. Flagged here for
the top tier to confirm or revert.

### Smaller, in-scope judgement calls (not spec deviations)

- **The grip's glyph is painted with an explicit monospace `FontId` in
  both themes.** Measured: Hack (the monospace family) has U+259E;
  Ubuntu-Light (Plain's proportional default) and egui's emoji fallbacks
  do not, so Plain would have drawn a replacement box. Two tests hold
  this — one against the atlas rectangle a laid-out glyph points at, one
  against the source.
- **`config::save` is now routed through one writer** (`persist_preferences`).
  It refuses to write under `--pace-demo` and refuses to write back a
  run-only `--plain`/`--themed` theme. The demo guard closes a
  pre-existing hole: `demo_config` forces `alerts=on`, so before M14 a
  tray theme toggle during `--pace-demo` would have written the demo's
  forced alert settings into the user's real `config.cfg`. Reported
  rather than silently absorbed.

## Mutation checks on the new tests

Thirteen mutants, each applied to the shipped source alone, tests run
unmodified, then reverted. **All thirteen were caught.** Thresholds and
clamps got explicit boundary mutants, as asked.

| # | Mutant | Caught by |
|---|---|---|
| 1 | `config::height` range `..=` → `..` | `height_out_of_range_or_garbage_is_the_default`, `render_then_parse_is_identity` |
| 2 | `snapped_height` drops `GRIP_HEIGHT` | `snapping_adds_exactly_the_chrome_the_content_does_not_get`, `snapping_never_goes_under_the_minimum`, `the_demo_scenario_fits_the_window` |
| 3 | `snapped_height` drops the `MIN_WINDOW_HEIGHT` clamp | `snapping_never_goes_under_the_minimum` |
| 4 | `height_action` settle `>=` → `>` | `a_height_is_written_only_once_the_user_stops_dragging` |
| 5 | `initial_inner_height` ignores the saved height | `a_saved_height_is_what_the_window_opens_at`, `the_opening_height_is_clamped_to_the_viewport_bounds` |
| 6 | snap command emits `WINDOW_WIDTH + 1.0` | `the_snap_command_can_only_name_the_fixed_width` |
| 7 | grip painted in the proportional family | `the_grip_paints_its_mark_in_the_monospace_family` |
| 8 | `freshness_color` aging `>=` → `>` | `the_dot_goes_green_amber_cardinal_on_the_two_thresholds` |
| 9 | `freshness_color` stale `>=` → `>` | same, plus `the_dots_cardinal_is_exactly_the_stale_predicate` |
| 10 | tooltip drops the hour's zero pad | `the_tooltip_is_byte_exact_fresh_and_stale` |
| 11 | tooltip drops ` — stale` | same |
| 12 | header stops drawing the dot | `the_header_carries_the_dot_at_every_age_in_either_theme`, `the_error_lines_are_untouched_by_the_dot` |
| 13 | `AGING_AFTER` raised to 600 (= `STALE_AFTER`) | **compile error** — `const _: () = assert!(AGING_AFTER < STALE_AFTER)` |

One test-authoring escape was found and closed on the way: the first
draft of `the_grip_glyph_is_in_the_shipped_fonts` used
`Fonts::has_glyph`, which for the monospace family compares Hack against
itself as the replacement face and returns `false` for every character,
ASCII included. It was replaced with the atlas-rectangle comparison
described above, which discriminates.

## Hard limits — held

| Limit | Status |
|---|---|
| Zero new dependencies | `Cargo.toml`, `Cargo.lock` and every crate manifest: **no diff** across `e77e5d0..HEAD` |
| No `--json` change | `crates/usage-cli` and `crates/usage-core`: **no diff** across `e77e5d0..HEAD` |
| No version bump, no CHANGELOG | `version = "1.6.0"` unchanged; `CHANGELOG.md` untouched |
| No §4.1 path | `egress/**`, `credentials/**`, `deny.toml`, `SECURITY.md`, `THREAT_MODEL.md`, `.github/**`, `.cargo/**`, `.claude/**`: **no diff** |
| No security-invariant test touched | `tools/check-invariants.py` green, unchanged: 7 invariants, 24 test bindings |
| §4.4 — no credential material | Nothing read from `~/.claude/**` or `~/.codex/**`; no token value anywhere in this work or this report |
| §4.5 — visuals not self-accepted | See below |
| Git lock/tmp sweep | Checked after every git operation; `.git` carried no `*.lock` or `tmp_obj*` at any point, so `_to_delete/git-stale/` received nothing |

## §4.5 — for the owner's eyes

Nothing visual is accepted here. What needs a look, in `--pace-demo` and
in a live window:

1. **The grip at rest and on hover** — is `▞` at 8px legible as a resize
   handle in both themes, and is TEXT_FAINT → TEXT enough of a change to
   read as an affordance?
2. **The drag** — does the bottom edge resize smoothly, does the width
   genuinely never move, and does the floor at 160px feel right?
3. **The snap** — double-click the grip on a pane with the per-model
   disclosure open and closed. Does it land exactly on the content, with
   no dead band and no scroll bar?
4. **The remembered height** — resize, quit, relaunch. (Verify
   `config.cfg` gained a `height=` line, and that `--pace-demo` still
   opens at its own 330px regardless.)
5. **The dot** — position and weight on the header row against the
   provider name, in both themes, and whether green/amber/cardinal read
   correctly at a glance. The demo pane's dot starts green and ages
   through amber to cardinal on the real clock.
6. **The hover** — the tooltip's bytes, and specifically **the `UTC`
   suffix** (deviation 1). If a local clock is wanted, that is a top-tier
   decision about how to obtain the offset, not a floor edit.
7. **The density win itself** — two rows back across two panes. Worth it?

## Untested hops, stated honestly

The window cannot be launched in this environment, so three things are
proven by construction and tests rather than by running them:

- The OS-side resize drag (`BeginResize`) and the min/max width pin.
  Enforced by winit; the code path is one `send_viewport_cmd`.
- `ViewportCommand::InnerSize` actually resizing the window on
  double-click. The command's contents are unit-tested; its delivery is
  egui's.
- The settle-then-write persistence loop against a real OS drag.
  `height_action` is unit-tested at both sides of the boundary and the
  observation is one `ctx.viewport_rect().height()` read per frame, but
  nobody has watched `config.cfg` gain a line after letting go of an
  edge. Item 4 above is the owner's check for this.

## What the owner must do next

1. Review the §4.5 list above with his own eyes (both themes;
   `--pace-demo` covers most of it, item 4 needs a live run).
2. Rule on **deviation 1** (UTC vs local tooltip clock) and
   **deviation 2** (finite max height) — both are recorded in the code's
   doc comments as well as here.
3. Confirm or revert **deviation 3** (the two ARCHITECTURE.md bullets).
4. Accept or bounce M14 (§4.8 — milestone acceptance is the owner's).
   M14-RELEASE owns the version bump and CHANGELOG.

Nothing further is queued from this session.
