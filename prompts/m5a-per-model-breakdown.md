# M5a — per-model breakdown (with dead-variant cleanup as step 0)

Session tier: **top (Opus 5)** — authoring mandate granted.
Base: `main` at `d6f1fa3` (ADR-002 + egress tightening, CI 6/6 green).
Landed: `1aa812a` (step 0) + the M5a commit that follows it.

---

## 1. The prompt, as run

> Repo: `C:\dev\QuotaPane\QuotaPane` — note the doubled directory. `C:\dev\QuotaPane`
> has no `.git`, and the stray empty `C:\dev\.git` above it does not resolve. Confirm
> `git rev-parse --show-toplevel` points at the doubled path before touching anything.
>
> Start from main at `d6f1fa3739811f669fa6a03bf6c96197529eea56`. Read `DECISIONS.md` §4
> in full before starting; the governance rules below are quoted from it and you are
> bound by them.
>
> **Mandate for this run:** you may author. Step 0 and steps 1–5 touch no §4.1 protected
> path — `crates/usage-core/src/egress/mod.rs`, `SECURITY.md`, and `THREAT_MODEL.md` are
> all out of scope for this work and must come back unmodified in `git status`. If you
> find yourself needing to edit any of the three, stop and hand back a spec instead
> (§4a.3).
>
> **Step 0 — chore: remove dead M4 `ProviderId` variants** (its own commit, before any
> M5a work). ADR-002 withdrew the official-billing mode, leaving two `ProviderId`
> variants that no code path can ever construct. Remove them:
> `crates/usage-core/src/model/mod.rs` — delete `AnthropicAdmin` and `OpenAiUsage` from
> `enum ProviderId`. `crates/usage-cli/src/main.rs` — delete their match arms. Line 66
> has `ProviderId::OpenAiUsage => "openai",`; there is a second arm near line 134 that
> the prior verification pass noted resolves to `None`. Find both by compiling, not by
> trusting these line numbers — they will have shifted. Let the compiler find any
> others. **Do not add a `_ =>` catch-all arm** to silence a non-exhaustive match;
> exhaustive matching over `ProviderId` is the thing that will tell us about the next
> variant change. If a match genuinely needs a fallback, say so in the commit message
> rather than papering over it. Commit step 0 on its own. `cargo test --workspace` and
> `clippy -D warnings` must be clean at that commit before you move on.
>
> **Step 1 — model: add the per-model channel.** In `crates/usage-core/src/model/mod.rs`,
> add to `ProviderSnapshot`: `pub per_model: Vec<QuotaWindow>,`. Reuse `QuotaWindow`
> as-is (`label`, `used_fraction`, `resets_in_secs`) — do not introduce a parallel
> per-model struct. Empty vec is the normal case for a provider with no per-model data;
> there is no `Option` wrapper. Update every construction site the compiler flags.
>
> **Step 2 — Claude: split headline from per-model.**
> `crates/usage-core/src/providers/claude_subscription.rs`, in `build_snapshot`: the
> existing loop flattens four windows into `windows`. Change it so `windows` carries only
> 5h and 7d, and `7d-opus` / `7d-sonnet` move into `per_model`. Labels stay exactly as
> they are — `"7d-opus"`, `"7d-sonnet"` — so the UI change is presentational only and no
> label parsing is introduced anywhere. Existing tests that assert four windows will
> fail. Update them to assert the new split (two headline, two per-model) rather than
> loosening the assertion to a count-agnostic check.
>
> **Step 3 — Codex: parse `additional_rate_limits`, ingest zero PII.**
> `crates/usage-core/src/providers/codex_subscription.rs`. The response body carries
> `user_id`, `account_id`, and `email`. Those must remain absent from every struct in
> this file. Adding a field for any of them — even `#[serde(skip)]`, even unused, even
> for a test — is a hard no; serde field-ignoring is the defense and it works by the
> field not existing. Parse `additional_rate_limits` into `per_model`. Each entry has a
> `limit_name` and a nested `rate_limit.primary_window` with `used_percent` and
> `limit_window_seconds`; the existing fixture at roughly line 333 shows the shape
> (`"GPT-5.3-Codex-Spark"`). Map `limit_name` → `QuotaWindow::label` verbatim (no
> normalizing, no prettifying — the provider's name is the honest one), `used_percent` →
> `used_fraction` applying the same `/100` conversion the headline windows already use,
> and `limit_window_seconds` → whatever the headline path does for its window duration;
> match it rather than inventing a second convention. Update the doc comment at the top
> of the file: it currently says the per-model breakdown "is deferred to M5 depth". It
> isn't any more. The PII sentence stays and stays accurate. Tests: keep the existing
> PII-ignore regression test unchanged; add a test that parses a fixture containing both
> `additional_rate_limits` and the PII fields, asserting per-model windows come through
> with the expected labels and fractions and that no PII value appears anywhere in the
> resulting snapshot — assert on absence positively (e.g. format the snapshot with
> `Debug` and assert the fixture's email/user_id string literals do not appear) so the
> test would fail if someone later added a field; add a test for
> `additional_rate_limits` absent from the response (older/other accounts): `per_model`
> should be empty, not an error.
>
> **Step 4 — UI: collapsed-by-default toggle.** `crates/usage-ui/src/main.rs`, single
> file, currently no expand/collapse affordance anywhere. `render_windows` (around line
> 776) loops `snapshot.windows` flat. Add a per-provider disclosure toggle: ▸ collapsed,
> ▾ expanded, collapsed by default, state held per provider (not one global flag —
> Claude and Codex expand independently). When collapsed the pane looks exactly as it
> does today. When expanded, render the `per_model` rows below the headline rows using
> the same row renderer, indented. Suppress the toggle entirely when `per_model` is empty
> — no dead affordance that opens onto nothing. **Do not take screenshots or make any
> claim about how it looks.** §4.5 makes visual acceptance the owner's eyes only. Say
> "ready for visual check" and stop there.
>
> **Step 5 — CLI surface.** Confirm `usage-cli --json` emits `per_model`. If
> `ProviderSnapshot` serializes wholesale it comes for free; if the CLI hand-builds its
> JSON, add the field. Either way, note in the commit which it was.
>
> **Step 6 — verify, then commit.** Before trusting any test result: `cargo clean -p
> usage-core`, then rebuild. On the last run a fully-cached `cargo test` pass masked the
> fact that a stale binary was being checked against a stale test list. Cached green is
> not green on this repo. Then, in order: `cargo fmt --check`, `cargo build --locked`,
> `cargo clippy --all-targets -D warnings`, `cargo test --workspace`. All four zero-exit
> before you commit. Also check: `git status` shows no modification to
> `crates/usage-core/src/egress/mod.rs`, `SECURITY.md`, or `THREAT_MODEL.md`.
> `Cargo.lock` unchanged (no new dependency belongs in this slice). No CR bytes —
> `git diff --check` clean.
>
> Do not mark M5a complete, accepted, or ✅ in `DECISIONS.md`. §4.8 reserves milestone
> acceptance to the owner and §4.5 reserves visual acceptance to the owner's eyes.
> Recording "M5a implemented, awaiting visual acceptance" is fine; a ✅ is not, and never
> delete an existing acceptance record.
>
> Write `prompts/m5a-per-model-breakdown.md` capturing this prompt as-run plus what
> actually changed. Commit the M5a work (steps 1–5) as one commit or a small focused
> series, push, and report back with: the commit SHAs, CI status, what `git status`
> showed for the three protected paths, and confirmation that the test pass was
> post-`cargo clean`.

---

## 2. What actually changed

### Step 0 — `1aa812a` `chore: remove dead M4 ProviderId variants (ADR-002 cleanup)`

| File | Change |
|---|---|
| `crates/usage-core/src/model/mod.rs` | Deleted `AnthropicAdmin` and `OpenAiUsage` from `enum ProviderId`. |
| `crates/usage-cli/src/main.rs` | Deleted the `provider_cli_name` arms (`"anthropic-admin"`, `"openai"`) and the `AnthropicAdmin \| OpenAiUsage => None` arm in `build_provider`. |
| `crates/usage-ui/src/main.rs` | Deleted the `provider_label` and `not_signed_in_line` arms, plus the test assertions covering them. |
| `ARCHITECTURE.md` | The two withdrawn-provider bullets said the variants were "slated for removal"; now record that they were removed 2026-07-25. |

**No `_ =>` catch-all was added anywhere, and none was needed.** Every match over
`ProviderId` remains exhaustive over the two real variants, so the next variant change
still breaks the build rather than silently falling through.

The prompt's line numbers had indeed shifted; both CLI arms were located by compiling.
The compiler surfaced two further sites the prompt did not name — `provider_label` and
`not_signed_in_line` in `usage-ui` — plus their tests.

### Steps 1–5 — the M5a commit

**Step 1 — `crates/usage-core/src/model/mod.rs`.** Added `pub per_model: Vec<QuotaWindow>`
to `ProviderSnapshot`, between `windows` and `source`. `QuotaWindow` reused unchanged; no
parallel struct, no `Option` wrapper. Documented that "none reported" and "none exist" are
the same thing to every consumer, and that `windows` now means *headline* windows
specifically. Construction sites the compiler flagged: `claude_subscription::build_snapshot`,
`codex_subscription::build_snapshot`, the `FakeProvider` in `poller`'s tests, and the
`snap()` helper in `usage-ui`'s tray tests.

**Step 2 — `claude_subscription.rs`.** `build_snapshot` now fills `windows` from
`five_hour`/`seven_day` only, and `per_model` from `seven_day_opus`/`seven_day_sonnet`.
The per-window normalization was lifted into one `to_quota_window` helper so both paths
share it. Labels are untouched: `"7d-opus"`/`"7d-sonnet"` are still emitted verbatim.

`builds_snapshot_from_usage_json` previously asserted `windows.len() == 3` (opus was null
in its fixture). Rewritten to assert the split positively — headline `["5h", "7d"]`,
per-model `["7d-sonnet"]` — plus explicit assertions that no opus/sonnet row leaks into
the headline list. Not loosened to a count-agnostic check. Two new tests:
`both_per_model_windows_are_split_out` (all four present → 2 + 2) and
`no_per_model_windows_yields_an_empty_vec`.

**Step 3 — `codex_subscription.rs`.** Added `RawAdditionalRateLimit { limit_name,
rate_limit }` and `additional_rate_limits: Option<Vec<..>>` on `RawUsage`. **No struct in
this file gained a field for `user_id`, `account_id`, or `email`** — the PII defense
remains structural (nowhere for serde to put them).

A shared `to_quota_window` helper is now the single place `used_percent` becomes a
fraction (`/100`, clamped) and `reset_after_seconds`/`reset_at` become a countdown, so the
headline and per-model paths cannot drift into two conventions.

Module doc updated: the "deferred to M5 depth" sentence is gone; the PII sentence stays
and was strengthened to spell out *why* it holds (no field exists) and that adding one —
even `#[serde(skip)]`, even for a test — is not allowed.

The existing PII regression test `builds_snapshot_from_verified_wire_shape` keeps its
assertions **byte-for-byte unchanged**. Two comment lines inside it were corrected, since
they asserted in prose that `additional_rate_limits` "are intentionally not emitted",
which stopped being true (see §3, deviation 1).

New tests: `per_model_windows_parse_and_carry_no_pii` (both per-model rows + all three PII
fields; asserts labels/fractions/resets, then asserts positively that no PII literal
appears in the snapshot's `Debug` **or** in its serialized JSON — and first asserts the
fixture really contains the PII, so the test cannot go vacuous);
`additional_rate_limits_absent_yields_empty_per_model`;
`additional_rate_limits_empty_array_yields_empty_per_model`;
`per_model_entry_without_a_window_is_skipped_not_fatal`;
`unnamed_per_model_entry_falls_back_to_the_duration_label`;
`per_model_used_percent_is_clamped_like_the_headline`.

**Step 4 — `crates/usage-ui/src/main.rs`.** `ProviderPane` gained `expanded: bool`,
default `false` — per pane, so Claude and Codex expand independently; there is no global
flag. `render_windows` takes the current `expanded` and returns whether the toggle was
clicked; `render_pane` owns the flip, so nothing is mutably borrowed across an egui
closure (the same "record intent, act after" idiom the titlebar buttons already use).
The toggle is rendered only when `per_model` is non-empty, and sits between the headline
rows and the age footer. Expanded rows go through `render_window_row` — the same renderer
— inside `ui.indent(...)`, salted with the provider id so the two panes get distinct
region ids.

The triangle is **painted**, not a `▸`/`▾` text glyph, matching this file's existing
`draw_close_glyph`/`draw_minimize_glyph` convention and its stated rationale ("never
depends on font coverage — no tofu risk"). Orientation is as specified: right when
collapsed, down when expanded. Triangle and caption are both clickable so the target
isn't a 9px shape.

New tests: `panes_start_collapsed`, `panes_expand_independently`, and two tray regressions
(`per_model_windows_do_not_feed_the_tooltip`,
`per_model_only_snapshot_has_no_representative_window`) pinning that the tray tooltip
still summarizes headline windows only and a per-model row cannot hijack it.

**No screenshots were taken and no claim is made about appearance.** Ready for visual
check.

**Step 5 — CLI surface.** **It came for free.** `ProviderSnapshot` derives `Serialize` and
`usage-cli` serializes the struct (or a `Vec` of them for `--provider all`) wholesale via
`serde_json::to_string_pretty` — it does not hand-build its JSON, so no CLI change was
needed. Two tests were added to pin it: `json_output_includes_per_model` (object and array
forms) and `json_output_keeps_per_model_present_when_empty` (no `skip_serializing_if`, so
consumers can rely on the key existing). No new dependency: `serde_json` was already a
`usage-cli` dependency.

---

## 3. Deviations from the prompt

1. **Two comment lines changed inside the "unchanged" PII regression test.** The prompt
   said to keep `builds_snapshot_from_verified_wire_shape` unchanged. Its *assertions* are
   byte-for-byte unchanged and its fixture is untouched. But two of its comments stated
   that `additional_rate_limits` "are intentionally not emitted in M3" — prose that step 3
   makes false. They now say those entries go to `per_model`, never to `windows`. Nothing
   the test checks was weakened or removed.

2. **`limit_window_seconds` on per-model entries had no destination, so it became the
   label fallback.** The prompt said to map `limit_name` → `label` verbatim *and*
   `limit_window_seconds` → "whatever the headline path does for its window duration".
   In the headline path that field's only job is to *become* the label via
   `seconds_label`; on a per-model row the label is already taken by `limit_name`, so
   there is no second destination for it in `QuotaWindow`. It is therefore used only when
   `limit_name` is absent, via the same `seconds_label` helper — so an unnamed entry
   renders `"7d"` rather than a blank row. Reset handling matches the headline exactly
   (shared helper). Notably, `limit_window_seconds` is *not* written to `resets_in_secs`:
   a window's duration is not its time-to-reset, and conflating them would have been the
   second convention the prompt warned against.

3. **A caption was added beside the triangle.** The prompt specified ▸/▾ but no text. A
   bare 9px triangle with no context reads as mysterious, so the control is
   `▸ per-model` / `▾ per-model` in the weak text color at `small()` size.

4. **`ARCHITECTURE.md` was edited in step 0** (not named in the prompt). Its two
   withdrawn-provider bullets explicitly said "The `ProviderId` variant is slated for
   removal" — left alone, step 0 would have made the file wrong. It is not a §4.1
   protected path. Factual sync only; no architecture decision was made or revisited.

5. **Step 0 also touched `usage-ui`** (the prompt named only `model/mod.rs` and
   `usage-cli`). `provider_label` and `not_signed_in_line` had arms for both dead
   variants. Found by compiling, exactly as instructed.

## 4. Hard stops hit

None. No §4.1 protected path was touched, so §4a never came into play.

## 5. Verification

`cargo clean -p usage-core` (removed 718 files / 196 MiB) **first**, then, in order and
all zero-exit:

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo build --locked` | clean (usage-core, usage-ui, usage-cli all recompiled) |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test --workspace --locked` | **130 passed, 0 failed** (usage-cli 20, usage-core 53, usage-ui 57) |

The 14 new tests were confirmed by name in the runner output, not inferred from the total.

Protected paths — `crates/usage-core/src/egress/mod.rs`, `SECURITY.md`,
`THREAT_MODEL.md` — absent from `git status` throughout. `Cargo.lock` unmodified; no
dependency added. `git diff --check` clean; index blobs verified byte-exactly CR-free.

## 6. What the owner must do next

- **Visual check of the disclosure toggle** (§4.5 — the owner's eyes only). Collapsed by
  default; Claude should show `5h` + `7d` with a `per-model` toggle revealing
  `7d-opus`/`7d-sonnet`; Codex shows a toggle only if the account's response carries
  `additional_rate_limits`.
- **Milestone acceptance** (§4.8). `DECISIONS.md` records M5a as implemented and awaiting
  visual acceptance — deliberately not ✅.

---

# M5a-fix — two-line per-model rows (2026-07-25)

Follow-up to the above, same day. Base: `main` at `34e6441`. Non-§4.1 throughout;
`crates/usage-ui/src/main.rs` was the only production file changed.

## 7. The defect

The owner's visual check found the per-model row **clipped at the right edge**: the reset
countdown ran off the fixed 320px window.

`render_window_row` lays out `label → ProgressBar(desired_width 120) → "resets in …"` on
one line. That fits the two-to-seven-character headline labels it was written for (`5h`,
`7d`), but not an indented `GPT-5.3-Codex-Spark`.

**Measured after the fact: the single-line row wanted 358.9px inside the 304px the panel
actually offers** — an overflow of ~55px, matching the owner's estimate of "roughly 355px
inside 320".

### Why every test passed anyway

This is the part worth remembering. M5a shipped with 130 green tests, and **not one of
them could have caught this**: every M5a test asserted on *parsed fixtures*, and a fixture
is never laid out. The provider tests proved `"GPT-5.3-Codex-Spark"` arrives in
`per_model` with the right fraction; the UI tests proved the pane starts collapsed and the
panes toggle independently. Nothing measured whether a row *fits*. The fixture passed
precisely because it never rendered.

## 8. The fix

**Per-model rows are now two lines** — the model label on its own line, then the bar and
reset countdown inset beneath it — via a new `render_per_model_row`.
`render_window_row` was **not modified**: the headline rows are visually accepted and
still render byte-identically. (For the same reason the shared bar width was left as a
literal `120.0` in both functions rather than extracted into a constant, which would have
required editing the accepted function. A comment in each notes they must stay in step.)

Stacking is **length-independent**, which is the point: it holds for any model name.
Widening the label column would only have moved the cliff, and model names trend longer.

Kept per spec: `desired_width(120.0)`, `fraction_color`, `format_percent`, and the
`"resets in {}"` phrasing, so a per-model gauge stays comparable with a headline gauge.
The label renders `.small()` in the weak text color so the two lines read as one
subordinate row rather than two unrelated ones.

### Vertical budget

The central panel's content is now wrapped in an `egui::ScrollArea::vertical()`. Two-line
rows double the per-model height cost (~34px each, measured) against a fixed,
non-resizable 240px window with no scroll: enough models — or Claude also reporting
per-model windows — would push the age footer out of the window with no way to reach it.

It is a pure safety net: egui shows no scroll bar while content fits, so **nothing changes
in any state the owner has already accepted**. One deviation from a naive reading of the
instruction, made deliberately: `ScrollSource`'s `drag` is set to `DragScroll::Never`.
egui 0.35 defaults it to `OnTouch`, which on a touch-capable Windows machine would turn a
drag on the pane background into a scroll and steal the only gesture that moves this
decoration-less window. Wheel and scroll bar stay enabled.

## 9. Tests — the real change

The prompt expected little to be assertable ("that `render_per_model_row` exists and is
what the expanded branch calls"). That turned out to be too pessimistic: **egui can be
laid out headlessly**, so the layout is now measured directly.

A `lay_out` helper renders into a headless replica of the real window — same fixed size,
same `CentralPanel`, same `ScrollArea` — and reports the width the content occupied
against the width the window actually offers (measured, not hard-coded, so the assertions
self-calibrate if a margin changes).

Deliberately **not** `egui::__run_test_ui`: that helper installs `FontDefinitions::empty()`
to save CPU, so every string measures ~0 wide and a width assertion made through it would
pass no matter how far a row overflowed — it would have recreated the exact blind spot
that let this ship. A default `Context` keeps egui's real fonts.

| Test | What it pins |
|---|---|
| `per_model_row_fits_the_window` | the clipping label from the fixture now fits |
| `per_model_row_fits_for_any_label_length` | a 47-char name still fits — length-independence |
| `single_line_layout_would_not_fit_which_is_why_rows_stack` | the counterfactual: the one-line layout *does* overflow. Fails if anyone "simplifies" the two-line row back |
| `headline_rows_still_fit` | the untouched accepted rows stay fitting |
| `expanded_pane_fits_the_window_width` | integration — and the strongest available proof the expanded branch calls the two-line renderer, since calling the old one would overflow |
| `several_expanded_models_outgrow_the_window_height` | why the `ScrollArea` exists, measured: six models exceed the height the panel can ever have |
| `per_model_rows_use_the_dedicated_two_line_renderer` | the signature the expanded branch depends on |

No existing assertion was weakened. `panes_start_collapsed`,
`panes_expand_independently`, and both tray tests stay green.

## 10. Verification

`cargo clean -p usage-core` (610 files / 148 MiB) first, then all zero-exit:
`fmt --all --check` → `build --locked` → `clippy --workspace --all-targets --locked -D
warnings` → `test --workspace`. **137 tests pass** (cli 20, core 53, ui 64).

`git status` showed `crates/usage-ui/src/main.rs` as the only modified file — §4.1 paths
(`egress/**`, `credentials/**`, `SECURITY.md`, `THREAT_MODEL.md`, `deny.toml`, `.github/**`,
`.cargo/**`, `.claude/**`) and `Cargo.lock` all untouched.

No screenshots; no claim about appearance. M5a remains "awaiting visual acceptance" in
`DECISIONS.md` — still no ✅.
