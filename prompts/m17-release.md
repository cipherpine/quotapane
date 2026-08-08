# Goal prompt spec: M17-RELEASE — v1.7.0 (the visibility release)

Authored at the standing top tier 2026-08-08. The owner's acceptance of
M14, M15 and M16 was given in this session on 2026-08-08 (§4.8), verified
at the top tier against the device and the GitHub check-runs API the same
day.

Model: attended CLI session (the dispatcher is paused at the owner's
request — the legs are hand-carried pastes, one per leg).
Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
Exact prior-release discipline, TWO HARD STOPS, both mandatory:
Phase 2 ends in a report and a WAIT; Phase 3 runs only on the top
tier's explicit written go-ahead, which arrives as the Leg B paste.

Verification is tools/release-verify.sh — the six-step outsider
standard plus six negative controls with rules R1-R4 built in. Run it
verbatim and paste its full output; if the script itself fails to run
(not a verification failure — a tooling failure), STOP and report
rather than improvising the manual standard.

**Wait in the FOREGROUND.** `gh run watch <run-id> --exit-status`, or a
foreground poll loop of `gh run list`. Never a background watcher, never
a notification, never a poll-and-forget — this rule was written after a
headless session died with its watcher and it holds for attended sessions
too, because an unattended terminal is the same thing.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "prompts: the v1.7.0 release spec — M14+M15+M16"; parent
   `83cc5d6` "prompts: the M16 Phase 2 goal prompt, on the record".
   Tree clean. Version `1.6.0` in the workspace Cargo.toml. 473 tests
   (cli 64 + cli-integration 13 + core 156 + ui 240).

P2 **`83cc5d6` and the tip are local-only and unpushed.** `origin/main`
   is at `5aff938` "reports: M16b end-gate", whose CI run
   `31232299097` is green — verified at the top tier via the check-runs
   API. Phase 1's push carries all three commits at once; that is
   expected and is not a §4.7 conflict. There is no CI run for
   `83cc5d6` or for the tip and there will not be one: they are
   prompts-only commits that never reached the remote on their own.

P3 Tags exactly `v1.3.0`, `v1.4.0`, `v1.4.1`, `v1.5.0`, `v1.6.0`;
   `v1.6.0` is Latest. No M17 stamp exists in DECISIONS.md yet — the
   stamp is created in Phase 4.

P4 The orphaned run `31121996517` on `b6fac53` will not cancel and is
   left exactly as found. Its run header reads `queued` while its
   attempt 4 reads `completed`/`failure` — an outage-corrupted record
   GitHub's API will neither run nor reap. `ci.yml` declares no
   `concurrency` group, so it blocks nothing. **Do not touch it. Do not
   re-run it. Do not treat it as a §4.6 red.**

## PHASE 1 — version + CHANGELOG (one commit)

Workspace version `1.6.0` -> `1.7.0`. Cargo.lock may move ONLY the three
workspace members. Insert into CHANGELOG.md, immediately above the
`## [1.6.0] - 2026-08-05` heading, this entry VERBATIM (date literal as
written; no link-reference line at the file's foot):

## [1.7.0] - 2026-08-08

The visibility release: the window makes room, and learns to say who is
working.

### Added

- **A height of your own.** The window has been hard-fixed at 320x240
  since the first build; the height is now yours. Drag the 8px grip at
  the bottom edge, or double-click it to snap the window to exactly fit
  what it is showing. The width stays 320 forever — even a borderless
  edge-drag can only move the bottom edge. A new `height` key in
  `config.cfg` remembers where you left it, written once a resize has
  settled rather than on every frame of an OS-driven drag.
- **`usage // agents` — a second view in the same pane.** The titlebar
  gains a switcher; click either word to move between them. The agents
  view names the Claude Code and Codex CLI sessions running on this
  machine, read from their own session logs on your disk: a state dot,
  `project · branch · id8`, a subagent marked `· sub`, and how long ago
  it last wrote. A window left on `usage` does not read a session log,
  does not list a session directory, and does not stat one.
- **What each session is doing, without reading a word of it.** A
  session that is still going carries a second line: a ten-bar pulse of
  how busy the last ten minutes were, whether the CLI is working or
  waiting on you (`in the loop` / `your turn`, in amber), how long it
  has been up, and which CLI version wrote the log. All of it derived
  from record types, timestamps and counts — invariant 8 still forbids
  conversation content from reaching any output, and a new
  `FORBIDDEN_KEYS` list machine-checks that the metadata allowlist can
  never name a content field.
- **The pane opens on what is alive.** Sessions from the last two hours
  show by default; everything older sits behind one clickable
  `// N older today` line and comes back with a click. The 24-hour
  lookback is unchanged — it is what the scanner reads, not what the
  pane shows.

### Changed

- **The freshness dot replaces the freshness row.** The per-provider
  `• updated Ns ago` text line paid a full row to say what a coloured
  dot says at a glance. The dot moves to the right edge of the provider
  header — green, amber past five minutes, cardinal once stale — and the
  exact seconds move to hover (`updated 5s ago at 09:14:22 UTC`).
  Identical in both themes, because how old the data is, is data rather
  than decoration.
- SECURITY.md invariant 8 and THREAT_MODEL.md's T-I6 now describe the
  agents scanner's aperture — a bounded key-depth search fenced by an
  explicit forbidden list — with the same machine-checked traceability
  as every other claim.

§3 bar, commit ("release: v1.7.0"), push, CI green on all 8 required
checks before any tag. Wait in the FOREGROUND.

## PHASE 2 — rc dry run, then HARD STOP

Tag `v1.7.0-rc.1` on the Phase 1 commit. Release run 3/3 green,
release.yml untouched. Then, in Git Bash:

    tools/release-verify.sh v1.7.0-rc.1

Paste its complete output.

**Content spot-check, and read this before running it.** Confirm the
release payload's user-visible strings appear in the shipped artifacts:
the two GUI binaries should contain `your turn`, `in the loop` and
`// hide older`, and the shipped README should carry "agents".

The spot-check is evidence, not a gate. A release build is LTO'd and a
string const that is only ever formatted into a larger literal can be
folded away — a previous release burned a session chasing exactly that.
So: report which strings were found and which were not, and treat a
partial miss as a limitation of the spot-check. Only *all four* absent
is a signal worth stopping on, and even then report rather than
improvise.

Then HARD STOP: write the report, commit it, push, CI green, and WAIT.
No `v1.7.0` tag, nothing published, no Phase 3.

## PHASE 3 — on the top tier's explicit go-ahead ONLY

The Leg B paste **is** that go-ahead. Absent it, Phase 3 does not exist.

Tag `v1.7.0` on the verified commit. Run `tools/release-verify.sh
v1.7.0` fresh against the draft. Only after `RESULT: PASS`, prune the rc
tag and the rc draft. Hand back the draft URL and STOP — the owner
publishes, pasting the release body from the CHANGELOG entry before
clicking publish.

## PHASE 4 — after the owner confirms publication (one commit)

Write the end-gate report to `reports/m17-release-endgate.md` and
include it in this commit (the reports convention, M11d). Then the §4a
replacement, DECISIONS.md only, with OLD byte-matched at exactly one
occurrence before editing and NEW at exactly one after — extract OLD and
NEW programmatically from this spec's bytes, never retype.

Note before you check it: NEW *begins with* OLD — the stamp is appended
to the end of the ledger paragraph. So after applying, a search for OLD
still finds one occurrence (inside NEW). That is correct. The two
assertions that matter are OLD exactly once **before** and NEW exactly
once **after**. Pre-flighted at the top tier on 2026-08-08 against the
live file: OLD 216 bytes ×1, NEW 2456 bytes ×0 before / ×1 after.

### Patch A -> DECISIONS.md

OLD:
```
Post-1.0 backlog: packaging (WinGet/Homebrew/AUR), deferred M5 features (history/sparklines, forecast, thresholds/alerts, OtelSource, CLI User-Agent parity), dead `RateLimitHeaders` cleanup, dormant-cadence decision.
```

NEW:
```
Post-1.0 backlog: packaging (WinGet/Homebrew/AUR), deferred M5 features (history/sparklines, forecast, thresholds/alerts, OtelSource, CLI User-Agent parity), dead `RateLimitHeaders` cleanup, dormant-cadence decision. · **M14–M16 the visibility arc ✅ (v1.7.0 published — owner-accepted 2026-08-08)**: M14 density pass — the window's height becomes the user's (8px grip, double-click snap-to-fit, `height` in config.cfg, width frozen at 320 in both directions) and the per-provider freshness row collapses to a dot on the header with the seconds on hover (UTC clock, flagged in the end-gate: std exposes no timezone and the milestone's limit is zero new dependencies, so a confidently-wrong local clock was refused). M15 agent visibility — `usage_core::agents` reads the local Claude Code and Codex session logs read-only through ALLOWLISTED_KEYS, candidates stat'ed before opening and at most two 16 KiB reads per file, liveness from mtime alone so an unparseable log still reports a state; the titlebar gains a `usage // agents` switcher over the same 320px pane, and a window left on `usage` touches no session root at all. Invariant 8 landed in the same commit as the behaviour. M16 second pass, on the owner's ask that the tab was full of old jobs with no sense of what they were doing — turn state from the record type alone, a ten-bar pulse from counted timestamps, uptime and CLI version, and a two-hour default with everything older one click away; the depth-3 key lookup Codex's nested branch needed ships fenced by FORBIDDEN_KEYS and a welded test, and the model slug stays unreachable by design (the owner chose model/CLI provenance; the top tier narrowed it to the CLI version because Claude Code writes the slug at `message.model`, inside the object invariant 8 exists to keep shut). Process: M14–M16 Phase 1 ran under the M11d dispatcher; from M16 Phase 2 the owner paused the dispatcher and hand-carried prompts to an attended CLI session, which is now the working model. A GitHub Actions outage (incident open 2026-08-06 15:22Z) accepted the pushes for eb5ae3e and 93c1e70 but created no run object for either and does not backfill, so M16 Phase 2's own run 31231812550 is the first that ever built and tested Phase 1 on all three OSes — 8/8 green. The orphaned run 31121996517 is outage debris whose header and attempt disagree; GitHub will neither run nor cancel it and it is left as found (owner decisions 2026-08-07/08).
```

Commit: `docs: v1.7.0 published; M14-M16 accepted (owner)`
Push, CI green on all required checks, STOP. Nothing further queued.

## DO NOT

Publish anything. Touch `.github/`, `assets/`, `README.md`, or any §4.1
path (Phase 4's DECISIONS patch is the sole exception, verbatim). Change
code. Add any dependency. Skip either stop. Use
`--dangerously-skip-permissions`. Read `~/.claude/**` or `~/.codex/**`.
Capture the owner's screen.

## Housekeeping

The repo lives on a mount that refuses `unlink`. After every git
operation, sweep any `.git/*.lock`, `.git/objects/maintenance.lock` and
`.git/objects/*/tmp_obj_*` that will not delete into
`_to_delete/git-stale/` with `mv`, then verify `.git` is clean.
