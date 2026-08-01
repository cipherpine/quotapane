# Goal prompt spec: M10-RELEASE — v1.4.1 (the expired-token UX patch)

Authored at the standing top tier 2026-08-01. The launcher paste is
the owner's acceptance of M10 (§4.8): three commits, verified at the
top tier against the device on 2026-08-01.

Model: floor. Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
Exact prior-release discipline, TWO HARD STOPS, both mandatory:
Phase 2 ends in a report and a WAIT; Phase 3 runs only on the top
tier's explicit written go-ahead in this session.

The release-verify standard, all four rules, applies in full:
R1 digest-match BOTH attestation subjects;
R2 every negative control asserted on its SPECIFIC error, artifacts
   restored between controls, controls never stacked;
R3 tamper controls must PROVE the bytes changed — record the
   artifact's digest before and after the tamper and show they differ;
R4 the wrong-repo control must target a repo confirmed to exist, so
   its failure cannot be a "no such repo" false pass.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "prompts: M10-RELEASE spec + launcher — v1.4.1";
   parent 33749eb "docs: FAQ — why does it say my token expired".
   Tree clean. CI green on 33749eb (7/7); the spec commit is
   prompts-only. Version 1.4.0 in the workspace Cargo.toml. 292 tests.
P2 Tags exactly v1.0.0–v1.4.0; v1.4.0 is Latest. No M10 entry
   exists in DECISIONS.md yet — the stamp is created in Phase 4.

## PHASE 1 — version + CHANGELOG (one commit)

Workspace version 1.4.0 → 1.4.1. Cargo.lock may move ONLY the three
workspace members. Insert into CHANGELOG.md, immediately above the
`## [1.4.0]` heading, this entry VERBATIM (date literal as written;
do not add a link-reference line at the file's foot — 1.3.0 and
1.4.0 set that precedent):

## [1.4.1] - 2026-08-01

A small patch: the expired-token experience now explains itself.

### Changed

- **The expired-token message names the exact refresh action.** Each pane
  says what to do — start any `claude` session (even `claude -p hi`), or run
  `codex login` — and that QuotaPane recovers on its own within ~3 minutes of
  the refresh: no restart, no clicks. The CLI prints the same hint. QuotaPane
  itself has no login and never writes credential files; refresh always
  happens in the provider's official CLI.
- **The at-risk pace line goes quiet when data is stale.** A burn-rate
  forecast extrapolated from stale data is misinformation; once a pane
  crosses the 10-minute staleness threshold the "at this pace: …" line is
  suppressed until fresh data arrives. The pace tick on each bar still
  draws — elapsed time is a fact of the clock, not of the data.

### Added

- **README FAQ** — "Why does it say my token expired?": what the message
  means, why QuotaPane cannot refresh the token itself, and how recovery
  works.

No JSON key changed in this release. Zero new dependencies.

§3 bar, commit ("release: v1.4.1"), push, CI 7/7 before any tag.

## PHASE 2 — rc dry run, then HARD STOP

Tag v1.4.1-rc.1 on the Phase 1 commit. Release run 3/3 green,
release.yml untouched. Six-step outsider verification in a clean
directory + six negative controls per the four-rule standard above.
Then HARD STOP: report and WAIT. No v1.4.1 tag, nothing published.

## PHASE 3 — on the top tier's explicit go-ahead ONLY

Tag v1.4.1 on the verified commit. Re-run all six steps fresh against
the v1.4.1 draft, negative controls per the standard. Only after
clean verification, prune the rc tag and draft. Hand back the draft
URL and STOP — the owner publishes.

## PHASE 4 — after the owner confirms publication (one commit)

Two §4a replacements, DECISIONS.md only, each OLD byte-matched at
exactly one occurrence before editing and NEW at exactly one after.

Patch A — the M10 stamp:
OLD: one top tier (owner decision 2026-07-30). Post-1.0 backlog:
NEW: one top tier (owner decision 2026-07-30). · **M10 expired-token UX ✅ (v1.4.1 published — owner-accepted 2026-08-01)**: per-provider expired-token copy naming the exact refresh action (start a `claude` session / `codex login`; auto-recovery within ~3 min), with a pin test welding the core Display marker to the UI matcher; the at-risk pace line suppressed once data is stale (rendering only — pace math and tick untouched); README FAQ. Zero new dependencies, no JSON change (owner decisions 2026-08-01, prompted by the owner hitting the opaque recovery path himself). Post-1.0 backlog:
Patch B — retire the stale backlog item (the ruleset tightening
shipped 2026-07-29 and is already recorded earlier in the same line):
OLD: CLI User-Agent parity), ruleset tightening (require PRs + status checks now that direct-push sessions are done), dead
NEW: CLI User-Agent parity), dead

Commit: docs: v1.4.1 published; M10 accepted (owner)
Push, CI 7/7 green, STOP. Nothing further queued.

## DO NOT

Publish anything. Touch .github/, assets/, README.md, or any §4.1
path (Phase 4's DECISIONS patches are the sole exception, verbatim).
Change code. Add any dependency. Skip either stop.
