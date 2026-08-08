# M16b end-gate — the pane opens on what is alive

Attended session on **Opus 5** (top tier). Spec: `prompts/m16-agents-refine.md`,
**§2.1–§2.6 only**. Phase 1 was already on `main` as `eb5ae3e` and was not
revisited, reverted, or re-issued; D1 and D2 of `reports/m16-endgate.md` are
accepted and stand.

Acceptance is the owner's (§4.8) and the visuals are the owner's eyes only
(§4.5): **nothing here is self-accepted**, no screen was captured, no release is
cut. Version stays `1.6.0`, no CHANGELOG entry. **Zero new dependencies.** No
`--json` change — the CLI does not name `usage_core::agents` at all, and Phase 2
touched no core crate.

The headline: **CI is green on all 8 required checks**, and because Actions
never created a run for `eb5ae3e` or `93c1e70`, this run is the first that
covers Phase 1 as well as Phase 2. Phase 1 is now CI-verified.

## Commits

Base: `93c1e70` — *reports: M16 end-gate*. Working tree clean at session start,
`origin/main` in sync.

| # | SHA | Subject |
|---|---|---|
| 1 | `b4ed4c7efb0581f823be213d3d5fce651034660f` | `M16: the pane opens on what is alive` |
| 2 | this commit | `reports: M16b end-gate — phase 2 shipped, CI green for phase 1 too` |

Two files, and only two: `crates/usage-ui/src/main.rs` and `README.md`. No
§4.1-protected path was read for modification or touched — Phase 2 needed none
of them, exactly as the goal prompt predicted.

## What landed, section by section

### §2.1 — recent by default, older on demand

`AGENTS_RECENT_WINDOW = 2h`, beside the other agents-view consts. The scanned
list splits on `age <= AGENTS_RECENT_WINDOW` (`is_older_session` is the single
predicate, and the boundary belongs to the **recent** side — the inclusive rule
every `usage_core::agents` threshold already follows). The 24 h lookback is
untouched and stays a scanning bound.

The recent set renders exactly as M15 rendered sessions: same provider grouping,
same headers, same order. Below everything, when the older set is non-empty, one
dim clickable foot line — `// 7 older today` collapsed (`// 1 older today` for
one), `// hide older` expanded. Expanded, the older rows join **their own**
provider groups rather than gathering under a second set of headers; `scan`
already orders newest-write-first, so they arrive at the foot of the group they
belong to. They are dimmed to `TEXT_FAINT` / `weak_text_color`.

`render_agents` grew `show_older: &mut bool` and flips it itself; the app holds
`agents_show_older`, **not persisted**, with the doc comment pointing at `View`
for the reason.

Two empty states now: `NO_AGENTS_LINE` when nothing scanned at all, and
`NO_RECENT_AGENTS_LINE` (`// nothing active in the last 2h`) above the toggle
when something scanned but none of it is recent. The 24 h line is never shown
over a list that has rows one click below it.

### §2.2 — the second line

A live row becomes two lines when it has a second thing to say. Line one is
byte-for-byte M15's; line two is small, dim, and inset by the dot's own measured
width plus the row gap, so it starts under the identity rather than under the
dot.

Order is by value under truncation: **pulse, turn, duration, version**. Any part
the scan could not name is absent, not empty; a row where all four are absent
draws no second line at all. `Recent` rows are one line by rule, which is what
keeps the expanded older list from doubling in height.

`TURN_IN_LOOP` / `TURN_YOUR_TURN` as consts. `your turn` is painted in `AMBER`
and is the only colour the line carries.

### §2.3 — the pulse strip

A dedicated painter, `render_pulse`, sharing nothing with the M13 sparkline.
Ten 2px bars, 1px between them, 7px tall: `PULSE_STRIP_WIDTH` is derived from
that geometry rather than written down beside it, and comes to 29px. Height per
bar is `count / max(pulse)` of the full height, `ceil`-ed so any non-zero bucket
is at least a pixel. **Scaled row-relative**, with the reason in the doc comment.
Ink is the row's own state colour at `PULSE_ALPHA` (140). A pulse with no beats
paints nothing and allocates nothing — not even the strip's width.

### §2.4 — the demo

Six rows, as a `DemoRow` table rather than a nine-argument closure: rising
pulse + `in the loop`; decaying pulse + `your turn`; a subagent, busy over only
the span it has existed for; the honest Codex row with no turn phrase and no
strip; a finished Codex row with no branch, one line despite having a duration
and a version it could have said; and one Claude session past the two-hour line,
so the pane opens showing `// 1 older today` and the toggle has something to do.
The function's doc comment was rewritten — the old one enumerated four rows and
would have become false.

Every row's numbers agree with each other, and a test holds that: no bucket
claims a beat from before the session started, and the newest bucket with a beat
agrees with the row's own age.

### §2.5 — UI tests

Sixteen new tests, plus the M15 UI sentinel test extended. See the test table
below.

### §2.6 — README

Two sentences added to the agents paragraph, one on the two-hour default and one
on the second line and turn state. Plus one truth fix — see D3.

## Deviations

**D1 — `--agents-demo` now opens the demo-sized window, and `track_height`
refuses to save a height under either demo flag.** Not in the spec, and the
largest deviation here.

§2.4's six rows do not fit the 240px window `--agents-demo` opened at. Measured
through the shipped layout harness:

| theme | collapsed | expanded | window |
|---|---|---|---|
| Cipher Pine | 289px | 310px | 240px |
| Plain | 292px | 313px | 240px |

The accepted test `the_agents_demo_fits_the_window_it_opens` failed on the first
run, which is how this surfaced. Three ways out: trim the demo (violates §2.4),
let the `ScrollArea` take it (the foot line — the one thing the reviewer has to
click — opens below the fold, so the review would be of a feature with its
subject hidden), or give `--agents-demo` the demo window M13 already gives
`--pace-demo`. I took the third, for M13's own stated reason: reviewing a fixture
through a scroll bar is reviewing it in pieces.

The second half is **not optional and is the part worth checking**. `track_height`
observes the window each frame and writes a settled height to `config.cfg`;
under `--pace-demo` it returns early, under `--agents-demo` it did not. Opening
the taller agents demo without extending that guard would have persisted 330 as
the user's window height — a real bug, introduced by the fix. Both halves now
name the same pair of flags, and a test asserts they do.

`DEMO_WINDOW_HEIGHT` is reused rather than a second const added: 330 covers the
agents fixture in both states with room, and the constant's derivation doc says
so explicitly rather than leaving a reader to wonder which scenario it is from.

This changes a shipped, visually-accepted behaviour (`--agents-demo` opened the
ordinary window). **It is the owner's to overrule.**

**D2 — the demo's `your turn` row tails to nothing in the last *one* bucket, not
two.** §2.4 asks for "Claude, **working**, your turn, pulse tailing off to
nothing in the last two buckets". Those cannot all be true at once: two empty
one-minute buckets means nothing was written for ≥120s, and `state_for_age`
reads ≥120s as `Idle`, not `Working`. Since `pulse_from` buckets by
`PULSE_BUCKETS - 1 - age/60`, a `Working` row can have at most its newest bucket
empty.

I kept `working` and made the fixture honest: age 115s, pulse
`[8, 11, 9, 12, 7, 6, 4, 3, 1, 0]` — a decay to a single beat and then silence.
The visible shape is the one the spec asked for; the row no longer contradicts
itself. A test now holds every demo row's pulse against its age and duration, so
this cannot quietly rot.

**D3 — the README's allowlist sentence was corrected as well.** §2.6 asks for two
sentences, and I wrote two. But the paragraph below them listed the allowlist as
"ids, timestamps, record types, the working directory, the git branch" — which
**Phase 1 made false** (it added the CLI's version string, and `SECURITY.md` was
updated to say so in the same commit), and which my new sentence flatly
contradicts by naming the CLI version on screen. Fixed to match `SECURITY.md`'s
wording, plus one clause noting the strip counts timestamps and the turn phrase
reads a record's type. `README.md` is not §4.1-protected and doc-truth fixes
outside SECURITY/THREAT_MODEL/ARCHITECTURE are pre-approved (`DECISIONS.md` §3).
Recorded because it is a doc change §2.6 did not ask for.

**D4 — the second line is one `LayoutJob`, not one `RichText`.** §2.2 requires
both "the whole line is one `Label::truncate()`" and "`your turn` in AMBER while
the rest is dim". egui colours a `RichText` as a single unit, so a layout job
with two sections is the only construction that satisfies both. Consequence
worth knowing: `your turn` is a *section* of a galley, not a galley of its own,
so the test finds its colour through `job.sections` rather than
`galley.job.sections[0]` the way the M15 dot test does.

**D5 — `format_uptime` spells hours.** The spec only shows `up 12m`. A session
left open across a working day is routine and `up 431m` is a number nobody
reads, so ≥1h renders `up 3h20m`, minutes render `up 12m`, seconds render
`up 42s`. Pinned by a table.

**D6 — `format_cli_version` guards against a doubled `v`.** The `v` is the
window's, not the log's — both CLIs write a bare `2.0.14` — so a build that ever
starts writing its own is not handed `vv1.2.3`. One branch, one assertion.

**D7 — mechanical shape changes.** `AGENT_ROW_GAP` extracted from a literal
`4.0` because the second line's inset is the same number; `render_agent_row`
split into `render_agent_identity_line` + `render_agent_detail_line`, with a
one-line row drawn without the wrapping `ui.vertical` at all so every finished
row keeps exactly the geometry M15 was accepted at; `initial_inner_height`'s
first parameter renamed `pace_demo` → `demo`; `main.rs`'s module doc gained an
M16 paragraph.

**D8 — eleven tests beyond §2.5's list.** §2.5 names six; five of those are new
tests and one is the extension of the existing UI sentinel test. The other
eleven are below, and four of them exist because a mutation would otherwise have
survived.

**D9 — the mutation script named in the goal prompt does not exist on disk.**
The prompt says the Phase-1 session "already wrote Phase 2's four mutations into
the mutation script. Run them." There is no such file: not in the repo, not in
`_to_delete/`, not in any session scratchpad under this project's temp root
(searched by name and by glob). The Phase-1 session ran headless and its
scratchpad did not survive.

I did **not** treat this as a §4.7 stop, and the reasoning is worth stating: the
four mutations are enumerated verbatim in the goal prompt itself, so the
*substance* was fully specified and only the file was missing. Stale state is
not truth, but a missing convenience is not a conflicting claim either. I wrote
a fresh harness carrying the prompt's four plus thirteen of my own. It lives in
this session's scratchpad, not the repo — nothing about it was committed.

**D10 — the orphaned run `31121996517` could not be cancelled.** Attempted
exactly as instructed, three ways, all refused:

| Attempt | Result |
|---|---|
| `gh run cancel 31121996517` | `Cannot cancel a workflow run that is completed` |
| `POST /actions/runs/31121996517/cancel` | HTTP 409 — `Cannot cancel a workflow re-run that has not yet queued` |
| `POST /actions/runs/31121996517/force-cancel` | HTTP 409 — same message |

It is `run_attempt: 4`, `status: queued`, `conclusion: null`, last updated
`2026-08-06T19:13:41Z` — a re-run attempt that never queued, which GitHub's API
will neither run nor cancel. **Left exactly as found.** Not re-run. `.github/**`
untouched.

I pushed anyway rather than stopping, and checked the one thing that mattered
first: `ci.yml` declares **no `concurrency` group** (read-only inspection of the
workflow), so a stuck queued run cannot block or delay another. Confirmed
empirically — this session's run was created 2 seconds after the push and
started its first job 4 seconds later.

## Mutation pass

Three batches, all applied to the committed tree by script, each reverted with a
byte-for-byte equality assertion before the next, and the working tree verified
clean afterwards.

### The goal prompt's four — all caught

| Mutation | Result | Caught by |
|---|---|---|
| the two-hour split flipped to the wrong side (`>` → `>=`) | caught | `the_two_hour_boundary_belongs_to_the_recent_side`, `an_older_row_arrives_dimmed_and_keeps_its_state_dot` |
| the second line drawn for `Recent` rows | caught | `the_second_line_is_for_a_session_that_is_still_going`, `the_agents_demo_shows_the_whole_of_what_m16_added`, `the_agents_demo_fits_the_window_it_opens` |
| a plural invented on `N older today` | caught | `the_foot_line_counts_without_inventing_a_plural`, `the_pane_opens_on_the_last_two_hours_and_keeps_the_rest_one_click_away` |
| the pulse strip scaled against `PULSE_CAP`, not the row's busiest minute | caught | `the_pulse_scales_against_the_rows_own_busiest_minute` |

### Thirteen more of my own

| Mutation | Result | Caught by |
|---|---|---|
| `AGENTS_RECENT_WINDOW` widened back to the 24 h lookback | caught | `the_pane_opens_on_the_last_two_hours…`, `the_foot_line_counts…`, `clicking_the_foot_line…`, `the_agents_demo_shows_the_whole_of_what_m16_added` |
| `your turn` painted in the line's dim ink, not `AMBER` | caught | `your_turn_is_the_only_colour_the_second_line_carries` |
| the older rows arrive undimmed | caught | `an_older_row_arrives_dimmed_and_keeps_its_state_dot` |
| the second line reordered — version before the turn phrase | caught | `the_second_line_says_uptime_and_version_the_way_the_row_reads_them`, `your_turn_is_the_only_colour…` |
| a silent pulse still reserves its strip | caught | `a_silent_pulse_paints_nothing_and_costs_no_room` |
| **the second line loses its indent and sits under the dot** | **SURVIVED**, then caught | — → `the_second_line_starts_under_the_identity_and_not_under_the_dot` |
| the 24 h empty line reused where nothing is recent | caught | `the_two_hour_boundary_belongs_to_the_recent_side` |
| the foot line's click stops toggling | caught | `clicking_the_foot_line_folds_the_older_rows_in_and_out` |
| a live row with nothing to say still draws an empty second line | caught | `a_row_with_nothing_to_add_draws_exactly_one_line` |
| `format_cli_version` stops guarding against a doubled `v` | caught | `the_second_line_says_uptime_and_version_the_way_the_row_reads_them` |
| a non-zero bucket may round down to nothing (`ceil` → `floor`) | caught | `the_pulse_scales_against_the_rows_own_busiest_minute` |
| the older rows get their own header block instead of joining one | caught¹ | `the_pane_opens_on_the_last_two_hours…`, `the_agents_demo_fits_the_window_it_opens` |
| `--agents-demo` goes back to writing the user's saved height | caught | `neither_demo_reads_or_writes_the_saved_window_height` |

¹ **My first attempt at this one was an equivalent mutant and proved nothing.**
I wrote `showing && is_older(s) || !is_older(s)`, which is the same predicate as
`showing || !is_older(s)` for every input — so it "survived" because there was
nothing to catch. Rebuilt as a real structural change (a second pair of provider
passes, so the older rows get their own headers) and re-run: caught. Recorded
because a survivor that is really an equivalent mutant is exactly the kind of
thing that gets written into a report as a finding when it is not one.

### The real survivor, and what was done about it

**Dropping `ui.add_space(indent)` from the second line broke no test at all.**
§2.2 states the inset explicitly — "indented to sit under the identity rather
than under the dot" — and nothing held it. The row still fit the window, still
painted every string, still drew the right number of lines; it just sat in the
wrong column, which is a thing only the owner's eyes would have caught, at
§4.5, after the work was declared done.

Rather than footnote it I made it testable.
`the_second_line_starts_under_the_identity_and_not_under_the_dot` finds the dot,
the identity and the second line's first mark in the painted shapes and asserts
the second line's left edge lands within half a pixel of the identity's — for a
row that leads with its pulse strip *and* for a row whose second line is words
only, so the assertion is about the indent and not about the strip. Both themes.
Re-run: caught. The fix is in the same commit as the code it guards.

## Local §3 bar — green

Run on `b4ed4c7`, the pushed tree:

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test --workspace --locked` | **473 passed, 0 failed** (cli 64 + cli-integration 13 + core 156 + ui 240 + 0 doc-tests) |
| `python tools/check-invariants.py` | `OK: 8 invariants, 30 test bindings, tags and manifest set-equal, SECURITY.md id set matches` |

`usage-ui` went from 224 to 240: sixteen new tests, and the M15 UI sentinel test
extended rather than replaced. `usage-core` is unchanged at 156 — Phase 2 touched
no core code.

### The sixteen

§2.5's six (five new, one an extension):

- `the_pane_opens_on_the_last_two_hours_and_keeps_the_rest_one_click_away` — the split
- `the_foot_line_counts_without_inventing_a_plural` — `1 older today` / `2 older today`
- `the_second_line_is_for_a_session_that_is_still_going` — `Working`/`Idle` yes, `Recent` never
- `a_row_with_nothing_to_add_draws_exactly_one_line` — `Unknown` + `None` + `None` + zeros
- `your_turn_is_the_only_colour_the_second_line_carries` — `AMBER`, both themes
- `the_agents_pane_never_renders_conversation_content` — **extended**: the fixtures now
  carry `version`, `cli_version`, a `payload.git.commit_message` sentinel and a
  `message.model` sentinel, and the test asserts the turn phrase, the uptime and
  *both* CLIs' version strings reached the screen. Without that the no-sentinel
  assertion would pass over a second line nobody drew. `pulse` is a `[u32; 10]`
  of counts that paints rectangles and has no text to leak; the test says so
  rather than pretending to cover it.

Eleven beyond:

- `the_two_hour_boundary_belongs_to_the_recent_side` — the boundary row, and the second empty state
- `clicking_the_foot_line_folds_the_older_rows_in_and_out` — a real pointer gesture, `gesture_on_switcher`'s harness
- `the_pulse_scales_against_the_rows_own_busiest_minute`
- `a_silent_pulse_paints_nothing_and_costs_no_room` — including that the strip's width is not reserved
- `the_second_line_starts_under_the_identity_and_not_under_the_dot` — the survivor's fix
- `an_older_row_arrives_dimmed_and_keeps_its_state_dot`
- `the_second_line_says_uptime_and_version_the_way_the_row_reads_them`
- `the_second_line_elides_rather_than_overflowing_the_window` — every part at its longest at once
- `the_agents_demo_shows_the_whole_of_what_m16_added` — six rows, both phrases, and the internal consistency check
- `the_older_toggle_is_runtime_state_the_way_the_view_is` — the single config writer does not name it
- `neither_demo_reads_or_writes_the_saved_window_height` — D1's second half

## CI — 8/8 green, and it covers Phase 1 too

Run **[31231812550](https://github.com/cipherpine/quotapane/actions/runs/31231812550)**
on `b4ed4c7`, watched in the **foreground** with
`gh run watch 31231812550 --exit-status --interval 20` (exit 0). No background
watcher was started at any point.

| Time (UTC) | Observation |
|---|---|
| 2026-08-08 01:05:18 | `git push` accepted: `93c1e70..b4ed4c7 main -> main`; remote reports the ruleset bypass and "8 of 8 required status checks are expected" |
| 01:05:21 | run `31231812550` **created** — the outage is over |
| 01:05:25 | first jobs start |
| 01:08:25 | run completed, **conclusion `success`** |
| 01:08:41 | `gh run watch` exits 0 |

| Required check | Conclusion | Started → completed (UTC) |
|---|---|---|
| build & test (windows-latest) | success | 01:05:25 → 01:08:19 |
| build & test (ubuntu-latest) | success | 01:05:32 → 01:07:21 |
| build & test (macos-latest) | success | 01:05:26 → 01:07:06 |
| cargo-deny (licenses, bans, advisories, sources) | success | 01:05:25 → 01:05:58 |
| cargo-audit (RustSec advisories) | success | 01:05:31 → 01:08:24 |
| gitleaks — full-history secret scan | success | 01:05:31 → 01:05:36 |
| invariants — manifest, docs, and tests agree | success | 01:05:31 → 01:05:35 |
| invariant 4 — no telemetry | success | 01:05:25 → 01:05:28 |

**This closes the Phase-1 gap.** No run object was ever created for `eb5ae3e` or
`93c1e70`, and GitHub does not backfill; `b4ed4c7` is a descendant of both, so
this is the first run that has built and tested Phase 1's `usage_core::agents`
changes — the depth-3 lookup, `FORBIDDEN_KEYS`, `turn_for`, `epoch_secs`, the
pulse — on Windows, macOS and Linux, and the first time the `invariants` job has
checked Phase 1's `SECURITY.md` / `THREAT_MODEL.md` / `invariants.manifest`
patches against the live tests. All green. §4.6 did not trigger.

The orphaned run `31121996517` is still `queued` with zero jobs, exactly as
found — see D10.

## Things I was unsure of

- **Whether D1 was mine to make.** It changes a shipped, visually-accepted
  behaviour (`--agents-demo` opened the ordinary window) and it changes which
  flags gate a config write. I judged that a spec asking for six rows and a
  clickable foot line is implicitly asking for a window they fit in, and that
  the M13 precedent is exactly on point — but the honest description is that
  §2.4 forced a choice the spec did not anticipate, and I picked one. If the
  owner would rather the demo scrolled, the change is one const argument and one
  guard.
- **The subagent row's second line sits under `· sub`, not under the identity.**
  The inset is the dot's width plus the gap, and on a subagent row the `· sub`
  mark occupies the column between. I read §2.2's "under the identity rather
  than under the dot" as contrasting with the dot, and both readings are
  defensible; measuring past `· sub` too would be a few more lines. This is a
  §4.5 call and I would rather be told than guess.
- **Whether `up 3h00m` is the right shape** — always two minute digits, even at
  the hour. It reads like a clock and keeps a constant width, but `up 3h` is
  also defensible and I chose without a strong reason.
- **The foot line's drag-versus-click has no test of its own.** I used the view
  switcher's exact construction (`.selectable(false).sense(Sense::click())`),
  which the switcher's own test proves lets a window drag through to the handle
  behind it — so the reasoning transfers. But I did not replicate that harness
  for the pane, which would need a `CentralPanel` replica with the background
  drag handle. If a drag started on the foot line turns out to toggle the pane
  instead of moving the window, that is the gap and I put it there.
- **`PULSE_ALPHA = 140` is a guess** at what "reduced alpha" should be. It is a
  look, not a fact, and it is the owner's.
- **The demo's Codex `recent` row moved from 3h10m to 95m.** It had to come
  inside the two-hour window so exactly one row would be older; its age now
  reads `95m ago` rather than `190m ago`. Nothing depends on it, but it is a
  visible change to a fixture that has been reviewed before.
- **Whether extending the UI sentinel test was enough.** The scan-based fixture
  cannot produce a non-zero pulse without a live RFC 3339 timestamp, and
  building one in the UI crate would mean hand-rolling the civil-date
  arithmetic `usage_core` keeps private. I covered the three text-bearing fields
  through the real scanner and said plainly in the test why `pulse` is not among
  them, rather than reaching for a trick (a far-future stamp reads as "just now"
  and would have worked) that a later reader would have to decode.

## What the owner must do next

1. **Review the visuals (§4.5).** `cargo run -p usage-ui -- --agents-demo`. The
   window now opens at the demo height. Six rows, four of them two lines, one
   `your turn` in amber, and `// 1 older today` at the foot — click it.
2. **Rule on D1** — the demo window and the `track_height` guard. It is the one
   deviation that changes previously-accepted behaviour.
3. **Rule on D2** — the `your turn` row's pulse tailing to one bucket instead of
   two, because the spec's version could not be honest.
4. **Decide about the orphaned run `31121996517`** (D10). GitHub will not cancel
   it through the API. It is harmless — no concurrency group — but it will sit
   in the run list until GitHub reaps it.
5. **M16 is complete.** §2.1–§2.6 are shipped, Phase 1 is CI-verified, and
   acceptance is yours.
