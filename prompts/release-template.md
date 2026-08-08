# TEMPLATE — release spec (instantiate; do not execute this file)
#
# Top tier: copy to prompts/m<NN>-release.md, fill every {{SLOT}}, delete
# this header block. Frozen rules below are the accumulated standard —
# edit them only by top-tier decision recorded in DECISIONS.md.
#
# Rules learned the hard way, each after it cost a release:
# - Name the tip by SUBJECT + parent SHA, never by its own SHA — a spec
#   cannot cite the commit that carries it.
# - State the MODEL TIER, not just the session mode. DECISIONS.md §6 and
#   CLAUDE.md's handoff format both require it, and Phase 4 lands bytes
#   in a §4.1 path, so "which tier is executing" is load-bearing.
# - Enumerate tags as a RANGE, never as a list. A list written from a
#   truncated `git tag` reading is wrong the moment a tag is added, and
#   it has produced a false precondition mismatch twice (M13, M17).
# - DERIVE the release date, never pin it. A pinned date is perishable
#   if the legs straddle midnight UTC.
# - WAIT IN THE FOREGROUND. `gh run watch <id> --exit-status`, or a
#   foreground poll loop. A background watcher dies with its session and
#   cost M13 a whole leg. This applies to attended sessions too: an
#   unattended terminal is the same thing.

# Goal prompt spec: M{{NN}}-RELEASE — v{{VERSION}} ({{NICKNAME}})

Authored at the standing top tier {{DATE}}. The launcher paste is the
owner's acceptance of M{{NN}} (§4.8), verified at the top tier against
the device and the GitHub check-runs API on {{DATE}}.

Model tier: {{MODEL_TIER}}. Session mode: {{SESSION_MODE}}.
Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
Exact prior-release discipline, TWO HARD STOPS, both mandatory:
Phase 2 ends in a report and a WAIT; Phase 3 runs only on the top
tier's explicit written go-ahead.

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
P2 Tags are exactly the contiguous range v1.0.0 through {{PREV_TAG}},
   and v{{PREV_VERSION}} is Latest. **The operative clause is that the
   v{{VERSION}} namespace is empty** — no v{{VERSION}} tag and no
   v{{VERSION}}-rc.* tag, local or remote. If the count differs from
   your expectation but the namespace is empty, that is an incomplete
   enumeration in this spec, not a precondition failure: report it and
   continue.
P3 No M{{NN}} stamp exists in DECISIONS.md yet — the stamp is created
   in Phase 4.

## PHASE 1 — version + CHANGELOG (one commit)

Workspace version {{PREV_VERSION}} -> {{VERSION}}. Cargo.lock may move
ONLY the three workspace members. Insert into CHANGELOG.md, immediately
above the previous release heading, this entry VERBATIM, **except that
the date in the heading is the UTC date on which you make this commit**
— derive it, do not copy a pinned one:

{{CHANGELOG_ENTRY}}

The entry must end with the consistency line the recent releases carry
("No JSON key changed in this release. Zero new dependencies.", or the
truthful variant). Verify both claims before asserting them.

§3 bar, commit ("release: v{{VERSION}}"), push, CI green on all
required checks before any tag. FOREGROUND.

## PHASE 2 — rc dry run, then HARD STOP

Tag v{{VERSION}}-rc.1 on the Phase 1 commit. Release run 3/3 green,
release.yml untouched. Then, in Git Bash:

    tools/release-verify.sh v{{VERSION}}-rc.1

Paste its complete output.

### Content spot-check — evidence, not a gate

Confirm the release payload's user-visible strings reach the shipped
artifacts: {{CONTENT_SPOT_CHECK}}.

**Read this before drawing any conclusion from it.** `[profile.release]`
sets `lto = true`, `codegen-units = 1` and `strip = true`. A string const
may legitimately not survive as a contiguous rodata literal, whether or
not it is formatted into something larger — this has produced a false
negative in three consecutive releases (`24h` in M13, `// hide older`
and the `format!`-built `// N older today` in M17). Therefore:

- Never choose a needle that is built by `format!` — it cannot exist as
  a contiguous literal by construction.
- Use raw-byte search, not `strings`, which may not be installed and
  can return empty silently. An empty result from a missing tool is
  vacuous, not a finding.
- A missing needle is a limitation of the spot-check. Report which were
  found and which were not. Only *all* needles absent is worth stopping
  on.
- The stronger evidence is always a passing behavioural test plus the
  owner's own eyes on the built binary. If a needle is missing, say so
  and propose the ten-second behavioural check rather than investigating
  the compiler.

Then HARD STOP: report and WAIT. No v{{VERSION}} tag, nothing published.

## PHASE 3 — on the top tier's explicit go-ahead ONLY

Tag v{{VERSION}} on the verified commit. Run tools/release-verify.sh
v{{VERSION}} fresh against the draft. Only after RESULT: PASS, prune the
rc tag and rc draft with `gh release delete v{{VERSION}}-rc.1
--cleanup-tag --yes`, which removes the local tag as well — **do not
follow it with `git tag -d`**, which will report "not found" and read as
a failure. Confirm the end state by listing tags rather than by the
delete command's exit code.

When reporting artifact verification: the release ships **one** SLSA
attestation covering **both** archives, so printing `subject[0]` names
the same archive regardless of which file was verified. Verify against
both subjects and say which.

Hand back the draft URL and STOP — the owner publishes, pasting the
release body from the CHANGELOG entry before clicking publish.

## PHASE 4 — after the owner confirms publication (one commit)

Gate on your own read, not on anyone's word:

    gh release view v{{VERSION}} --json tagName,isDraft,publishedAt,url

`publishedAt` non-null and `isDraft` false. **Also confirm the release
body is non-empty and matches the CHANGELOG entry** — publication and
completeness are different states, and a release can be published with
an auto-generated body while the owner is still pasting. If the body is
bare, wait and re-read rather than stamping it.

Write the end-gate report to reports/m{{NN}}-release-endgate.md and
include it in this commit (the reports convention, M11d). Then the
§4a replacement(s), DECISIONS.md only, each OLD byte-matched at
exactly one occurrence before editing and NEW at exactly one after —
extract OLD/NEW programmatically from this spec's bytes, never retype.
Where NEW begins with OLD (the usual shape for an appended stamp), OLD
still matches once afterward, inside NEW. That is correct.

{{PHASE4_PATCHES}}

Commit: docs: v{{VERSION}} published; M{{NN}} accepted (owner)
Push, CI green on all required checks, STOP. Nothing further queued.

## DO NOT

Publish anything. Touch .github/, assets/, README.md, or any §4.1
path (Phase 4's DECISIONS patches are the sole exception, verbatim).
Change code. Add any dependency. Skip either stop. Use
`--dangerously-skip-permissions`. Read `~/.claude/**` or `~/.codex/**`.
Capture the owner's screen. Change a byte of the tree to chase a CI or
spot-check result.
