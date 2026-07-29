# Goal prompt spec: M7-RELEASE — cut v1.1.0

Authored at the top tier (Cowork bridge) 2026-07-29. Runs ONLY after the
owner's visual acceptance of M7a + M7A2 — the owner pasting this
prompt's launcher IS that acceptance record. Follows the m6-release.md
pattern; where this file and that one differ, THIS file governs.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): floor tier; release.yml is NOT edited (any needed fix is
a STOP and a top-tier pass). Phase 4 carries a small pre-authored
DECISIONS.md patch (§4a).

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is f63b475 (M7A2 phase 2), tree clean EXCEPT:
   ```
    M prompts/m6-launchers.md
   ?? prompts/m7-release.md
   ```
   CI green; only tag v1.0.0; workspace version 1.0.0.
P2 The launcher's first line records the owner's visual acceptance of
   M7a + M7A2. If it does not, STOP — §4.5/§4.8.

## PHASE 0 — commit this spec + launcher index

    docs(prompts): add M7-RELEASE spec (v1.1.0)

## PHASE 1 — version + changelog (one commit)

- Cargo.toml `[workspace.package]` 1.0.0 → 1.1.0; `cargo build
  --locked` regenerates Cargo.lock for the three members ONLY (any
  third-party movement = STOP).
- CHANGELOG.md gains a `## [1.1.0]` entry above 1.0.0, written for a
  stranger: Claude per-model quotas via the provider's generalized
  limits array (model-scoped windows such as Fable now appear); the
  window hides untouched per-model buckets while `--json` keeps
  reporting them; the Codex pane shows owned rate-limit reset credits
  (`resets available: N`), also in the JSON as `reset_credits`.
- Commit: `release: 1.1.0 — per-model truth and Codex reset credits`
- Push; CI 7/7 green BEFORE tagging.

## PHASE 2 — rc dry run (THE REAL TEST), then HARD STOP

Tag `v1.1.0-rc.1` on the phase-1 commit; push; release.yml runs (it is
unchanged since the v1.0.0-verified v3, cosign pinned 3.0.6 — a failure
is a STOP, never an inline fix). Verify the draft AS AN OUTSIDER, all
six steps from m6-release.md phase 2, negative controls included, plus
one v1.1.0-specific check: extract an archive and run
`quotapane-cli --once --json --provider all` is NOT possible without
credentials on a clean box — instead run `--help` and `--version`
(expect `quotapane-cli 1.1.0`) and confirm the README inside the
archive is current. Record every command verbatim. Then STOP: tagging
v1.1.0 is the owner's call (§4.8).

## PHASE 3 — only on the owner's explicit go-ahead in a later turn

Tag `v1.1.0` on the same verified commit; re-verify all six steps
against the new draft; delete the rc tag and draft only after v1.1.0
verifies; hand the owner the draft URL. YOU DO NOT PUBLISH.

## PHASE 4 — after the owner confirms they published (later turn)

§4a — DECISIONS.md, replace exactly once:

OLD:
**M7a per-model truth (v1.1.0) — underway 2026-07-29**:

NEW:
**M7a per-model truth ✅ (v1.1.0 published — owner-accepted)**:

Commit: `docs: v1.1.0 published; M7a/M7A2 accepted (owner)` — push, CI
green, STOP. Nothing further queued; M7b (the visual pass) is a future
top-tier spec.

## DO NOT

Publish any release; edit any workflow; delete/rewrite any tag except
the rc you created; bump any dependency; touch any §4.1 path beyond the
Phase 4 patch.

## END GATE (phase 2) — STOP

Report: phase-1 SHA, CI, the rc run, the full verification transcript,
and confirmation the archived README needed no correction. Then wait.
