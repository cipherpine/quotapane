# M13-R1 end-gate — sparkline legibility iteration (round 2 of the §4.5 loop)

Floor session, headless under the M11d dispatcher. Spec:
`prompts/m13-r1-sparkline-iteration.md`. One commit plus this report.
**Nothing here is self-accepted.** The owner's round-1 verdict was
banner ✅, tray ring + tooltip ✅, sparkline ❌ — "hard to tell what it
is and what it means". This round changes what the strip draws; whether
it now reads is the owner's eyes only (§4.5). M13 and M13-R1 are
accepted together or iterated again, and M13-RELEASE cuts v1.6.0 only
after that. No version bump, no CHANGELOG, no release here.

## Commit

Base: `aaa4b03` — *reports: M13 end-gate — record the report commit's
own CI run*

| SHA | Subject |
|---|---|
| `ed4f1303d1d0fee791c1079889a2b9d499e49a41` | `M13-R1: make the sparkline read as an instrument` |

One file changed: `crates/usage-ui/src/main.rs`. Nothing else in the
tree was touched — the boundaries the spec set are all provable by that
one-line `--stat`:

```
$ git diff --stat aaa4b03..ed4f130
 crates/usage-ui/src/main.rs | 496 ++++++++++++++++++++++++++++--------
 1 file changed, 383 insertions(+), 113 deletions(-)
```

- **Zero new dependencies** — `Cargo.toml` and `Cargo.lock` are not in
  the diff. Painter primitives only (`Shape::mesh`, `Shape::line`,
  `circle_filled`, `Painter::text`), all already in `egui` 0.35.
- **No §4.1 bytes** — no `SECURITY.md`, `THREAT_MODEL.md`, `deny.toml`,
  `invariants.manifest`, `.github/**`, `.cargo/**`, `.claude/**`, and
  nothing under `crates/usage-core/src/egress` or
  `crates/usage-core/src/credentials`. §4a was not invoked, because
  there was nothing to invoke it for.
- **No `--json` change** — `usage-cli` and `usage-core::model` are not
  in the diff. The strip is drawn from data the pane already held.
- **No version bump** — `1.5.0` everywhere it was.

## The bar (§3)

Green before the commit, re-run after it:

```
cargo fmt --all --check                                          clean
cargo clippy --workspace --all-targets --locked -- -D warnings   clean
cargo test --workspace --locked                                  386 passed, 0 failed
python3 tools/check-invariants.py
  OK: 7 invariants, 24 test bindings, tags and manifest set-equal,
      SECURITY.md id set matches.
```

## CI

Run <https://github.com/cipherpine/quotapane/actions/runs/31012384300>
on `ed4f130` — **conclusion: success**. All 8 required checks:

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

No red runs this session.

## What changed, as numbers

The spec's five changes, before → after:

| Property | M13 (rejected) | M13-R1 | Constant |
|---|---|---|---|
| Strip height | 12.0 px | **16.0 px** | `SPARK_HEIGHT` |
| Stroke width | 1.0 px | **1.5 px** | `SPARK_STROKE_WIDTH` |
| Stroke colour | TEXT_DIM @ alpha **140** | TEXT_DIM @ alpha **255** | `spark_color()` |
| Fill under the curve | *(none)* | TEXT_DIM @ alpha **18** | `SPARK_FILL_ALPHA` |
| "now" dot | *(none)* | TEXT @ alpha 255, r = **2.5 px** | `SPARK_NOW_RADIUS` |
| Tag | *(none)* | `24h`, small, TEXT_FAINT, right-aligned | `SPARK_TAG` |
| Demo inner height | 240.0 px | **364.0 px** | `DEMO_WINDOW_HEIGHT` |
| Production inner height | 240.0 px | **240.0 px** (untouched) | `WINDOW_HEIGHT` |

`SPARK_ALPHA` is gone rather than retuned. It existed to encode "the
strip sits *behind* the pace tick" (`SPARK_ALPHA < PACE_TICK_ALPHA`),
and that rule is what produced a mark the owner could not identify. The
hierarchy is hue now — TEXT for the present, TEXT_DIM for the day,
TEXT_FAINT for the tag, and a near-floor alpha for the body alone —
pinned by `the_strips_hierarchy_is_hue_not_transparency`.

### Why the fill is a mesh

The region under a usage curve is **concave** in general (usage falls
at a reset, then climbs), and `egui`'s polygon fill is documented
convex-only — a concave series would flood across its own valley. The
fill is therefore two triangles per segment over **shared** vertices.
Shared, not per-segment quads: at alpha 18 a doubled seam draws a faint
vertical stripe at every reading, which is exactly the artifact the
demo history's wobble period is already chosen to avoid.

### Byte pin — the tag

Spec: byte-exact tag text `24h`. One constant, one paint site, and the
bytes are asserted twice — once against the constant, once against the
galley that actually reaches the screen:

```
const SPARK_TAG: &str = "24h";
            SPARK_TAG,                              // the sole paint site
        assert_eq!(SPARK_TAG, "24h");               // the_strip_is_drawn_at_the_sizes_the_spec_names
        assert_eq!(texts[0].galley.text(), SPARK_TAG);  // the_strip_paints_...
```

Codepoint dump of the literal: `['0x32', '0x34', '0x68']` — ASCII `2`,
`4`, `h`. No space, no unit separator, no look-alike.

## The demo window height, and where 364 comes from

`--pace-demo` — **and only `--pace-demo`** — now asks for a 364px inner
height. This is not a number chosen by eye. `render_panes` was
extracted from `App::ui` (the exact loop it ran inline, separators
included) so the layout harness can measure the *window's* body rather
than one pane at a time, and `lay_out_sized` measures it against the
demo's own screen height with the central panel's margins read from the
installed style rather than assumed.

Measured 2026-08-05, at `ed4f130`:

| Theme | Titlebar | Panel chrome | Demo body | Needed |
|---|---|---|---|---|
| Cipher Pine | 24.0 | 16.0 | 323.8125 | **363.8125** |
| Plain | 24.0 | 16.0 | 305.625 | 345.625 |

Cipher Pine binds — its monospace is taller per row, the same reason it
binds every width assertion in the file — so `DEMO_WINDOW_HEIGHT` is
`ceil(363.8125) = 364.0`. Plain opens with ~18px of slack, which is the
correct direction to be wrong in: a little empty ground, never a scroll
bar.

Two things worth stating plainly:

1. **This is the as-launched, collapsed state.** Expanding a per-model
   disclosure is a click, and it still scrolls — the expanded-state
   bottom cutoff is the polish DECISIONS.md §2 records as accepted and
   queued post-1.0, and this session did not quietly re-scope it.
2. **The production window is untouched at 240px.** Every visual
   acceptance to date was given against 320×240, and a synthetic review
   scenario does not get to resize the app real subscribers run.
   `only_the_demo_gets_the_taller_window` pins both halves, and
   `a_pane_without_history_renders_exactly_as_it_did_before` (the
   renders-as-v1.5.0 pin) is unchanged and green.

## Test delta

| Target | Before (`aaa4b03`) | After (`ed4f130`) | Δ |
|---|---|---|---|
| `usage-cli` unit | 64 | 64 | 0 |
| `usage-cli` integration | 13 | 13 | 0 |
| `usage-core` | 127 | 127 | 0 |
| `usage-ui` | 175 | 182 | **+7** |
| **total** | **379** | **386** | **+7** |

Seven new:

```
the_strip_paints_a_body_a_line_a_now_dot_and_its_tag
the_strip_is_drawn_at_the_sizes_the_spec_names
the_strips_hierarchy_is_hue_not_transparency
the_fill_spans_every_segment_down_to_the_baseline
a_fill_needs_a_segment
the_strip_costs_its_tag_row_on_top_of_its_height
only_the_demo_gets_the_taller_window
```

Two rewritten:

- `the_strip_is_the_same_colour_in_both_themes` — was
  `spark_color() == TEXT_DIM@140` and `SPARK_ALPHA < PACE_TICK_ALPHA`;
  is now full-alpha stroke + alpha-18 fill.
- `the_demo_scenario_fits_the_window` — per the spec, an **equality**
  claim against the demo's requested height instead of a recorded
  overflow. It now asserts both directions: under-size and the owner
  reviews through a scroll bar again; over-size by more than one row
  (24px) and `DEMO_WINDOW_HEIGHT` has stopped being derived from
  anything. The old "231px against 216px, within one row, flagged at
  the M8 gate" wording is gone because the condition it described is
  gone — for the demo. It was never a claim about the production
  window, which is unchanged.

Unchanged and still green, deliberately: the five M13 geometry tests
(`the_strip_runs_a_day_left_to_right_and_fills_upward`,
`the_strip_clips_what_is_outside_its_day`,
`a_fraction_outside_zero_to_one_is_clamped_into_the_strip`,
`an_empty_series_maps_to_no_points`,
`the_series_is_this_windows_readings_in_time_order`),
`one_reading_or_none_draws_no_strip_and_costs_no_space`,
`the_demo_draws_a_strip_for_every_pane`, and the two
renders-exactly-as-before pins.

### The new kind of test: shapes, not sizes

`painted_shapes` runs a render and flattens `FullOutput.shapes`, so the
round-2 spec is checked against **what reaches the screen** rather than
against the constants feeding it. This matters: the M13 strip would
have passed every layout assertion in the file while drawing nothing at
all — height and width cannot see paint. The one test asserts, from the
emitted shapes: exactly one mesh whose vertices are all
`spark_fill_color()` and whose curve vertices equal the line's points
and whose floor is one flat y at the strip's own bottom; exactly one
open path at width 1.5 in solid `spark_color()`; exactly one circle of
radius 2.5 filled TEXT, centred on the line's last point; and exactly
one text shape whose galley is `24h`, in TEXT_FAINT, ending within 1px
of the strip's right edge and positioned above it.

## Mutation check (beyond spec)

Tests that cannot fail prove nothing. **14 mutations run, 14 caught** —
after one escape was found and closed before the commit.

| Mutation | Caught by |
|---|---|
| the "now" dot is never painted | `the_strip_paints_a_body_a_line_a_now_dot_and_its_tag` |
| the dot is put on the *oldest* reading | `the_strip_paints_...` |
| the fill mesh is never added to the painter | `the_strip_paints_...` |
| the tag paints an empty string | `the_strip_paints_...` |
| the tag is left-aligned instead of right | `the_strip_paints_...` |
| the fill's floor is the curve's low point, not the strip's | `the_strip_paints_...` |
| the fill covers only the first segment | `the_fill_spans_every_segment_down_to_the_baseline` |
| `SPARK_TAG` becomes `24 h` | `the_strip_is_drawn_at_the_sizes_the_spec_names` |
| stroke width back to 1.0 | `the_strip_is_drawn_at_the_sizes_the_spec_names` |
| strip height back to 12.0 | `the_strip_is_drawn_at_the_sizes_the_spec_names`, `the_demo_scenario_fits_the_window` |
| the line goes back to alpha 140 | `the_strip_is_the_same_colour_in_both_themes` |
| the fill goes to alpha 140 | `the_strip_is_the_same_colour_in_both_themes` |
| the production window also opens tall | `only_the_demo_gets_the_taller_window` |
| `DEMO_WINDOW_HEIGHT` set to 240 / to 500 | compile-time `const` assert / `the_demo_scenario_fits_the_window` |

**The escape, stated honestly.** On the first pass, changing the call
site to `spark_fill_mesh(&points, rect.center().y)` — a fill that stops
at the middle of the strip instead of its floor, leaving every trough
hollow — was **not caught**. `the_fill_spans_every_segment_down_to_the_baseline`
passes the baseline in itself, so it cannot see the call site, and the
shape test at that point only counted vertices and checked their
colour. Closed by asserting, from the painted mesh, that the curve
vertices equal the line's points and that the floor sits exactly
`SPARK_HEIGHT × lowest_fraction` below the lowest reading. Re-run:
caught.

## Deviations from the spec

None. All five changes landed as written, in one commit (the bar never
forced a second).

Two judgement calls worth naming, neither a departure:

- The spec says the tag goes "directly above the strip's right edge".
  It is a real allocated row inside a zero-`item_spacing` `ui.vertical`
  group, not paint inside the strip's own rect — painting it inside
  would put it on top of the line. That costs the pane one small text
  row per strip, which is in the measured 364px and pinned by
  `the_strip_costs_its_tag_row_on_top_of_its_height`.
- "Size the demo height from the existing layout-harness measurement"
  required extracting `render_panes` from `App::ui` and adding a
  `screen_height` parameter + a measured `panel_chrome_height` to the
  harness. Both are extensions of the existing harness, not
  replacements; `lay_out` and `lay_out_themed` keep their signatures
  and every existing caller is unchanged.

## §4 conditions hit

None. No protected path, no dependency, no egress change, no
credentials, no CI failure, no conflict between the spec and the tree.
§4.5 is the standing one and is why this session stops here.

## What the owner does next

1. `cargo run --bin quotapane -- --pace-demo` — the demo now opens at
   320×364 and the whole two-pane scenario should render unscrolled.
   Look at the strip under each provider's bars: a filled body under a
   solid line, a bright dot on its right end, `24h` above that end.
2. Verdict on M13 + M13-R1 together — accept, or name what still does
   not read and this iterates again.
3. Only after acceptance: M13-RELEASE cuts v1.6.0 (version bump,
   CHANGELOG, tag, the release pipeline). Nothing in this session did
   any of that.

Not verified by this session, and deliberately so: **how any of it
looks.** The tests prove the marks are emitted, at the specified sizes
and colours, at the specified coordinates. They cannot prove the strip
now reads as an instrument — that is the judgement the §4.5 loop exists
to get from the owner, and this is round 2 of it.
