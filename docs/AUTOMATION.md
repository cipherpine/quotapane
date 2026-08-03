# Development & release automation (M11)

QuotaPane is built by a two-tier agent workflow under a charter
(`DECISIONS.md`): a standing **top tier** authors specs and verifies
every result independently against the repository; **floor** sessions
execute specs and stop on any conflict; the **owner** makes all
acceptance, roadmap, and publish decisions. M11 turned the repeatable
parts of that discipline into code. What is automated is the courier
work — never the judgment.

## The pieces

**invariants.manifest + tools/check-invariants.py** (CI job:
`invariants`) — every SECURITY.md invariant is registered with the
exact tests that prove it; tests carry `// INV:<n>` tags; the checker
fails CI when manifest, SECURITY.md, and tags drift in any direction.
Both files are §4.1 protected paths.

**tools/release-verify.sh** — the six-step outsider verification and
six negative controls (rules R1–R4) as one deterministic run. Release
specs invoke it verbatim; a tooling failure is a STOP, not a license
to improvise.

**prompts/release-template.md** — the release spec as a template with
slots, so each release's spec authoring is reduced to the per-release
facts and the frozen rules cannot be mistyped.

**reports/** — floor end-gate reports are committed in-tree (see
`reports/README.md`); the in-tree bytes are the report of record.

**tools/dispatch.ps1** — the owner-side dispatcher: watches
`prompts/queue/` (local, gitignored) and runs each launcher file as a
headless Claude Code floor session, one at a time, logging to
`reports/dispatch/` (local, gitignored). The top tier hands work over
by writing a queue file through the device bridge; no human copy-paste
remains in the loop.

## What stays human, permanently

- The owner's publish click (releases are draft-only from CI).
- The owner's acceptance of milestones, roadmap, and patch decisions.
- Visual/UI acceptance (charter §4.5 — the owner's eyes only).
- The top tier's written go-ahead between a release's rc dry run
  (Phase 2) and the real tag (Phase 3).

## Permission posture for headless floors

Floor sessions run with the repository's checked-in Claude Code
permission settings. `--dangerously-skip-permissions` is never used —
an unattended session with unbounded permissions has nobody watching
it. The specs' STOP discipline (§4.7) is the in-band brake and has
held in every run since it was written; headless operation makes it
more load-bearing, not less.
