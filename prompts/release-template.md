# TEMPLATE — release spec (instantiate; do not execute this file)
#
# Top tier: copy to prompts/m<NN>-release.md, fill every {{SLOT}}, delete
# this header block. Frozen rules below are the accumulated standard —
# edit them only by top-tier decision recorded in DECISIONS.md.
# Rule learned the hard way: name the tip by SUBJECT + parent SHA, never
# by its own SHA — a spec cannot cite the commit that carries it.

# Goal prompt spec: M{{NN}}-RELEASE — v{{VERSION}} ({{NICKNAME}})

Authored at the standing top tier {{DATE}}. The launcher paste is the
owner's acceptance of M{{NN}} (§4.8), verified at the top tier against
the device on {{DATE}}.

Model: floor. Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
Exact prior-release discipline, TWO HARD STOPS, both mandatory:
Phase 2 ends in a report and a WAIT; Phase 3 runs only on the top
tier's explicit written go-ahead in this session.

Verification is tools/release-verify.sh — the six-step outsider
standard plus six negative controls with rules R1-R4 built in. Run it
verbatim and paste its full output; if the script itself fails to run
(not a verification failure — a tooling failure), STOP and report
rather than improvising the manual standard.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "{{TIP_SUBJECT}}"; parent {{PARENT_SHA}}
   "{{PARENT_SUBJECT}}". Tree clean. CI green on all required checks.
   Version {{PREV_VERSION}} in the workspace Cargo.toml.
   {{TEST_COUNT}} tests.
P2 Tags exactly {{TAG_LIST}}; v{{PREV_VERSION}} is Latest. No M{{NN}}
   stamp exists in DECISIONS.md yet — the stamp is created in Phase 4.

## PHASE 1 — version + CHANGELOG (one commit)

Workspace version {{PREV_VERSION}} -> {{VERSION}}. Cargo.lock may move
ONLY the three workspace members. Insert into CHANGELOG.md, immediately
above the previous release heading, this entry VERBATIM (date literal
as written; no link-reference line at the file's foot):

{{CHANGELOG_ENTRY}}

§3 bar, commit ("release: v{{VERSION}}"), push, CI green on all
required checks before any tag.

## PHASE 2 — rc dry run, then HARD STOP

Tag v{{VERSION}}-rc.1 on the Phase 1 commit. Release run 3/3 green,
release.yml untouched. Then, in Git Bash:

    tools/release-verify.sh v{{VERSION}}-rc.1

Paste its complete output. Add the content spot-check: the release
payload's user-visible strings appear in the shipped artifacts
({{CONTENT_SPOT_CHECK}}). Then HARD STOP: report and WAIT. No
v{{VERSION}} tag, nothing published.

## PHASE 3 — on the top tier's explicit go-ahead ONLY

Tag v{{VERSION}} on the verified commit. Run tools/release-verify.sh
v{{VERSION}} fresh against the draft. Only after RESULT: PASS, prune
the rc tag and rc draft. Hand back the draft URL and STOP — the owner
publishes (pasting the release body from the CHANGELOG entry before
clicking publish).

## PHASE 4 — after the owner confirms publication (one commit)

Write the end-gate report to reports/m{{NN}}-release-endgate.md and
include it in this commit (the reports convention, M11d). Then the
§4a replacement(s), DECISIONS.md only, each OLD byte-matched at
exactly one occurrence before editing and NEW at exactly one after —
extract OLD/NEW programmatically from this spec's bytes, never retype:

{{PHASE4_PATCHES}}

Commit: docs: v{{VERSION}} published; M{{NN}} accepted (owner)
Push, CI green on all required checks, STOP. Nothing further queued.

## DO NOT

Publish anything. Touch .github/, assets/, README.md, or any §4.1
path (Phase 4's DECISIONS patches are the sole exception, verbatim).
Change code. Add any dependency. Skip either stop.
