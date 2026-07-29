# Goal prompt spec: M7A — per-model truth (v1.1.0)

Authored at the top tier (Cowork bridge) 2026-07-29, from the owner's
--debug-raw evidence captured the same day, and the owner's decisions:
UI hides untouched buckets while CLI/JSON stay truthful; v1.1.0 contains
exactly this slice. REVISED 2026-07-29: M6-CLOSE has since landed
(843a09a), so Phase 0's patch now ONLY opens M7a.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Phase 0 carries an exact DECISIONS.md patch (§4a — that
file is amended at the top tier only; apply verbatim, byte-match gate).
Phases 1–3 are YOURS: `providers/`, `usage-ui`, `usage-cli` are ordinary
paths. HARD RULE restated: no `user_id` / `account_id` / `email` / `id`
field may be added to ANY usage-response struct — the evidence body
carries them; the parser must not. Parse ONLY the fields used.

## THE EVIDENCE (2026-07-29, owner's account; values here are the facts,
## fixtures below must be synthetic)

Claude `/api/oauth/usage` grew a generalized `limits` array; the legacy
`seven_day_opus`/`seven_day_sonnet` keys are now permanently null:

    "limits":[
      {"kind":"session","group":"session","percent":15,...,"scope":null},
      {"kind":"weekly_all","group":"weekly","percent":42,...,"scope":null},
      {"kind":"weekly_scoped","group":"weekly","percent":40,
       "resets_at":"...","scope":{"model":{"id":null,
       "display_name":"Fable"},"surface":null},"is_active":false}]

Per-model data = entries whose `scope.model.display_name` is set. The
headline numbers (session/weekly_all) duplicate the still-present legacy
top-level `five_hour`/`seven_day`, which remain the headline source.

Codex: `additional_rate_limits` includes an untouched bucket
(`GPT-5.3-Codex-Spark`, 0% used, reset_after == window) the owner never
uses. The parser is correct; the display is noise.

## PRECONDITIONS (mismatch = STOP and report)

P0 The working tree may be in DETACHED HEAD at the v1.0.0 tag (the owner
   ran the release from source). If so, your first act is
   `git checkout main` — the untracked spec file and the launcher edit
   survive the checkout. Then verify the rest.
P1 `main` tip is 843a09a (the M6 acceptance stamp), only tag v1.0.0, the
   v1.0.0 release is PUBLISHED, CI green on 843a09a.
P2 Tree clean EXCEPT:
   ```
    M prompts/m6-launchers.md
   ?? prompts/m7a-per-model-truth.md
   ```
P3 DECISIONS.md contains the M6 ✅ segment ("owner-accepted 2026-07-28")
   and contains NO occurrence of "M7a". If M7a already appears, STOP —
   this patch would double-apply.

## PHASE 0 — two commits

0a. This spec + the launcher index:
      docs(prompts): add M7A spec and launcher (per-model truth, v1.1.0)
0b. §4a — DECISIONS.md §2 roadmap (single long line; the M6 ✅ segment
    landed at 843a09a). Replace exactly this substring, once:

    OLD:
    before the owner published. Post-1.0 backlog:

    NEW:
    before the owner published. · **M7a per-model truth (v1.1.0) —
    underway 2026-07-29**: Claude per-model via the endpoint's
    generalized `limits` array (surfaces the Fable weekly-scoped
    quota); UI hides untouched buckets while CLI/JSON stay truthful
    (owner decisions 2026-07-29). Post-1.0 backlog:

    (Insert unwrapped, matching the file's single-line style — the line
    breaks above are prompt wrapping. Byte-match the OLD substring
    first; exactly one occurrence, LF file, em dash U+2014.)

    Commit: docs: open M7a (v1.1.0) — per-model truth

## PHASE 1 — Claude per-model via `limits` (one commit)

`crates/usage-core/src/providers/claude_subscription.rs`:
- Extend `RawUsage` with `limits: Option<Vec<RawLimit>>`. New structs
  parse ONLY what is used:
      RawLimit { percent: Option<f64>, resets_at: Option<String>,
                 scope: Option<RawScope> }
      RawScope { model: Option<RawScopeModel> }
      RawScopeModel { display_name: Option<String> }
  Deliberately NOT parsed: `kind`, `group`, `severity`, `is_active`,
  `scope.model.id`, `scope.surface` — the field-ignoring defense works
  by the field not existing. Document that in a comment.
- `per_model`: entries with a `display_name` → QuotaWindow { label:
  display_name verbatim, used_fraction: (percent/100).clamp(0,1),
  resets_in_secs: from resets_at via the existing helper }.
- Fallback preserved: if `limits` is absent or yields no model-scoped
  entries, the legacy `seven_day_opus`/`seven_day_sonnet` path still
  populates per_model as today. Headline (5h/7d) unchanged.
- Tests, synthetic values throughout: (a) a fixture mirroring the
  2026-07-29 shape — legacy per-model keys null, noise keys present,
  one model-scoped limit ("TestModel", 40%) → per_model has exactly
  that entry, 0.40, parsed reset; (b) limits absent → legacy fallback
  still works (keep the old fixtures green); (c) limits present but no
  model-scoped entries → per_model empty; (d) assert no PII-named field
  exists in the new structs (extend the existing guard pattern).

## PHASE 2 — UI hides untouched buckets; CLI stays truthful (one commit)

`crates/usage-ui/src/main.rs`:
- A per-model row is HIDDEN when `used_fraction` is None or == 0.0.
  The disclosure toggle renders only when ≥1 row is visible; a snapshot
  whose per-model buckets are all untouched shows no toggle at all.
- Tests via the existing layout harness: mixed buckets show only the
  used ones; all-untouched shows no toggle; the tooltip/representative
  logic is unaffected.
`crates/usage-cli`: one new test pinning that a zero-usage bucket IS
present in `--json` output — the truth-first half of the owner's
decision, guarded so a future "cleanup" can't quietly extend the UI
filter into the data.

## PHASE 3 — verify + ship

cargo clean -p usage-core, then fmt-check → build --workspace --locked →
clippy -D warnings → test --workspace. Push all commits. CI 7/7 green.

## END GATE — STOP (owner's eyes required, §4.5)

Report SHAs, CI, test count. Then STOP for the owner's visual check:
Claude pane gains the per-model toggle with a Fable row near 40%; the
Codex pane's Spark row is GONE from the window; `quotapane-cli --once
--json --provider all` still lists the Spark bucket. Do NOT bump the
version or tag anything — the v1.1.0 release (bump, CHANGELOG, rc dry
run per the m6-release.md pattern) is a separate prompt after the owner
accepts the visuals.
