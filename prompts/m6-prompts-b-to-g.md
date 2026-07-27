# M6 GOAL PROMPTS B–G — M5a to public v1.0

Authored at the top tier (Cowork bridge), 2026-07-26. Companion to
`prompts/m6-ship-program.md` (the gate map) and `prompts/m6-prep-audit.md`
(Prompt A). Owner decisions resolved 2026-07-26:

- **D1 — M5 scope: FROZEN AT M5a.** M5a per-model breakdown is all of M5 for
  v1.0. History/sparklines, forecast-to-limit, thresholds/alerts, the Codex
  User-Agent CLI flag, and the token-free `OtelSource` all move to a post-1.0
  milestone. No new features between here and the public flip.
- **D2 — The name: QUOTAPANE.** Resolved 2026-07-26 — the working name is
  the product name. The rename gate collapses: no prose changes, no §4.1
  substitution pass, only the binary rename below and a DECISIONS.md record.
- **D3 — Crate names stay.** `usage-core` / `usage-ui` / `usage-cli` are
  product-neutral and unchanged. Only the **binaries** rename, via `[[bin]]`.
- **D4 — Charter public, prompts not.** Publish `ARCHITECTURE.md`,
  `DECISIONS.md`, `THREAT_MODEL.md`, `SECURITY.md`, `CONTRIBUTING.md`,
  `CLAUDE.md`, `.claude/`. Move `prompts/` and `_claude_setup` out of the
  public tree. **See the warning in Prompt G — this only holds if D7 goes
  fresh-repo.**
- **D6 — Artifacts: Windows + Linux.** Signed binaries for
  `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`. macOS is documented
  build-from-source; CI keeps testing it.

Still open, and each is named as a precondition where it bites: **D5** the
security contact, **D7** rename-in-place vs fresh repo (decide only after
Prompt G's history scan).

Run order: **A → B → C → D → E → F → G.** Each prompt is independently
runnable and ends in a STOP. Do not chain two in one session.

**How these are launched.** The floor's input box caps a goal prompt at 4000
characters, so the pasted prompt is a short LAUNCHER naming the spec section in
this file; the session's first act is to read that section here and execute it
as the goal prompt, verbatim. The spec in this file is authoritative — if the
launcher and this file ever disagree, that is a §4.7 stop, not a judgment call.
Owner-supplied values (`FILL_ME` fields) arrive in the launcher, not by editing
this file.

---

## Prompt B — adopt the name (D2 resolved: QuotaPane)

D2 resolved 2026-07-26: **QuotaPane — the working name — is the product name.**
That dissolves the rename gate almost entirely. There is no prose rename: the
docs, the protected files, the window title, and the repo URL
(`github.com/cipherpine/quotapane`) already carry the right name, so the
mechanical-substitution commit this section used to describe no longer exists,
and NO §4.1 path is touched. What survives: record the decision, rename the
binaries per D3, and fix the --help defect the gap report found (audit finding:
--help and -h exit 2 as unrecognized arguments).

Worth doing once, outside the repo: register `quotapane` on crates.io even
though nothing publishes there (squatting your own name is cheap insurance),
and a trademark sanity check now that the name is permanent.

---

```
# Goal prompt: M6-NAME — QuotaPane adopted; rename the binaries

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Floor tier throughout. Phase 0 commits top-tier-authored
bytes verbatim under §4a — including the DECISIONS.md acceptance amendment,
which was authored at the top tier on the owner's explicit acceptance
(2026-07-27, recorded in the Cowork session). You verify and commit those
bytes; you do not author or adjust them. Phases 1–2 are yours to author and
touch NO §4.1 path.

PRECONDITIONS (mismatch = STOP and report):
P1 `main` tip is the M6-PREP gap-report commit, CI green. Tree clean EXCEPT
   exactly: modified DECISIONS.md, plus untracked
   prompts/m6-prompts-b-to-g.md and prompts/m6-launchers.md. Anything else
   untracked or modified: STOP.
P2 DECISIONS.md §2 (working tree) marks M5a ✅ visually accepted 2026-07-27
   and M5 frozen at M5a (D1). The diff vs HEAD touches ONLY the §2 roadmap
   M5 segment — one line. If the diff reaches any other line, STOP.

PHASE 0 — commit the top-tier bytes, verbatim, two commits:
  0a. DECISIONS.md alone. Read the diff first: every hunk must be inside
      the §2 roadmap line (M5a acceptance + D1 freeze + deferred-list
      correction). §4a applies — no edit, no reflow. Commit:
          docs: record M5a visual acceptance (owner, 2026-07-27); freeze
          M5 at M5a per D1
  0b. The two untracked prompt files, LF-only check as usual. Commit:
          docs(prompts): add M6 goal-prompt specs B–G and launchers
If any of the three files is absent or its content mismatches this
description, STOP and report — do not reconstruct it.

PHASE 1 — one commit:
- DECISIONS.md §1: the line "Placeholder name QuotaPane stays until the
  pre-release naming decision (owner-only, M6)" becomes a record of the
  resolved decision: QuotaPane is the product name, decided by the owner
  2026-07-26. Rewrite that line only; if the §2 roadmap lists the naming
  decision as an open M6 item, mark it resolved there too.
- Binary rename per D3 — crate names DO NOT CHANGE:
  crates/usage-ui/Cargo.toml gains
      [[bin]]
      name = "quotapane"
      path = "src/main.rs"
  crates/usage-cli/Cargo.toml gains
      [[bin]]
      name = "quotapane-cli"
      path = "src/main.rs"
- Update anything that names the old BINARY files as executables (README
  build/run commands, test helpers, prompts are historical records and stay).
  Crate names in Cargo.toml and imports stay. README's full rewrite is
  Prompt E's job; here you touch only binary-name accuracy, plus you may
  delete the "Working name — will be renamed" banner line since it is now
  false. Nothing else in README changes.

Commit: "name: adopt QuotaPane as the product name (D2); rename binaries
per D3"

PHASE 2 — one commit, still no §4.1 path:
- usage-cli: implement --help / -h (print usage covering every flag
  parse_args actually accepts — enumerate them from the parser, not from
  any document — then exit 0) and --version (workspace version, exit 0).
  The gap report proved --help currently exits 2 as "unrecognized
  argument"; the first command a stranger types must not fail. This is a
  defect fix, not new feature scope, so it does not violate the D1 freeze.
- Tests: --help exits 0 and its text names every accepted flag (a test
  that enumerates the parser's accepted set and asserts each appears in
  the help text, so a future flag cannot ship undocumented); --version
  exits 0; a genuinely unknown flag still errors as before.
Commit: "cli: add --help and --version; unknown flags still error"

TESTS: cargo clean -p usage-core first, then fmt-check → build --workspace
--locked → clippy --workspace --all-targets --locked -- -D warnings →
test --workspace. Then prove the rename took: the build output contains
quotapane(.exe) and quotapane-cli(.exe), the old binary names are gone, and
`quotapane-cli --help` prints usage and exits 0. A [[bin]] block that
silently did nothing is the most likely quiet failure here.

VERIFY + SHIP: the phase 1 and 2 diffs show no §4.1 path. Cargo.lock should
not change — package names did not move; if it did, explain exactly why
before committing it. Push all four commits together. CI green.

END GATE — STOP. Report the SHAs, CI, the exact binary filenames
produced, and the full --help text as shipped. No owner visual re-check is
needed — the window title already reads QuotaPane and did not change. Do
not start Prompt C.
```

---

## Prompt C — supply-chain pin resolution (read-only, no decisions needed)

This exists because `ci.yml`'s own header says: *"For release workflows (M6), pin
by full commit SHA."* I cannot author those pins from here — an unauthenticated
API lookup is refused, and a fabricated or stale SHA in a `.github/**` file is
worse than no pin at all. You have authenticated `gh`. Resolve, report, and I
author the workflows from what you report.

---

```
# Goal prompt: M6-PINS — resolve release-workflow action pins (read-only)

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): READ-ONLY except one new file under prompts/. You are NOT
writing a workflow. You are NOT editing .github/**. The workflows are §4.1
and will be authored at the top tier from your report. If you find yourself
drafting YAML, stop — that is the wrong session.

PRECONDITIONS: `main` tip is Prompt B's M6-NAME commit, tree clean, CI
green.

SCOPE — produce `prompts/m6-pin-report.md`. One commit, that file only.

For each action below, report: the newest release tag, the FULL 40-character
commit SHA that tag currently points at, whether that tag is a mutable
lightweight ref the maintainer moves (most GitHub actions move vN), the
license, and the publisher's account type (github-owned vs third-party).
Use `gh api repos/<owner>/<repo>/git/ref/tags/<tag>` — record the resolved
object sha, and if the ref is an annotated tag, dereference to the commit.

  actions/checkout
  actions/upload-artifact
  actions/download-artifact
  actions/attest-build-provenance
  sigstore/cosign-installer
  dtolnay/rust-toolchain

Cross-check: ci.yml currently uses actions/checkout@v7 and
dtolnay/rust-toolchain@stable. Report whether v7 is really the current major
for checkout, and what commit `stable` resolves to for rust-toolchain today —
`stable` being a moving branch ref is exactly the kind of pin a release
workflow should not inherit uncritically, and I want your read on it.

SECRET SCANNING — answer these as questions, do not implement:
- Does `gitleaks/gitleaks-action` require a paid GITLEAKS_LICENSE for this
  repo's owner type? Report the current licensing terms as stated by the
  project, and where you read it.
- What is the alternative: the release-binary-plus-checksum approach
  (download the gitleaks release for linux-x64, verify a pinned SHA256, run
  it). Report the current release tag, the asset filename, and the published
  SHA256 for that asset.
- Which approach gives fewer moving parts for a repo that must scan FULL
  history (`fetch-depth: 0`), not just the tip? Recommend one, with reasons.
  Your recommendation is input, not a decision.

RELEASE SURFACE — report, do not build:
- Exact `cargo build --release --locked` output paths for both binaries on
  windows-latest and ubuntu-latest runners.
- What `rustc -V` and `cargo -V` print on each runner today. SECURITY.md:105
  promises "the exact toolchain version is documented per release," so the
  release workflow has to capture this and I need to know its shape.
- Complete list of files a 0.1.0 → 1.0.0 version bump must touch. Start from
  Cargo.toml `[workspace.package] version` and prove whether anything else
  hardcodes a version — grep for "0.1.0" across the tree.
- Whether `strip = true` in [profile.release] interferes with build
  provenance attestation of the resulting binary. If you cannot establish
  this, say so plainly rather than guessing.

METHOD NOTE: every value in this report must come from a command you ran or
a page you read, with the command or URL recorded next to it. This report
becomes §4.1 workflow bytes — a value you half-remembered becomes a
supply-chain pin nobody can verify. Anything you could not resolve goes in an
explicit "UNRESOLVED" section. An honest unresolved beats a confident guess,
and I will ask you to re-run rather than shipping a guess.

DO NOT: edit .github/**; write any YAML; add any dependency; run the app
against real credentials; open a PR.

VERIFY + SHIP: report is markdown, LF-only. One new file. Commit
"M6-prep: release-workflow pin and surface report"; push; CI green.

END GATE — STOP. Report the commit, the UNRESOLVED count, your gitleaks
recommendation with its reasoning, and any place where what you found
contradicts a comment already in ci.yml.
```

---

## Prompt D — supply-chain CI (§4.1 throughout; I author, floor verifies)

**Not written yet, deliberately.** Its entire content is the two workflow files,
and those are §4.1 bytes I author from Prompt C's report — real SHAs, the
gitleaks approach C recommends, the artifact paths C confirms. Writing it now
would mean inventing the values C exists to establish.

What it will contain, so you can see the shape:

- **`gitleaks` job added to `ci.yml`**, with `fetch-depth: 0` so it scans full
  history rather than the tip. This is the job whose absence makes
  `SECURITY.md:115` a false statement today. Note that the line also claims
  pre-commit scanning — a CI job alone does not make the whole sentence true,
  so Prompt E either adds a hook or drops that clause. I lean drop: a
  pre-commit hook is unenforceable on contributors and claiming it is a control
  you don't have is how this class of drift started.
- **`release.yml`**, triggered on `v*` tags: per-OS `--locked` release builds for
  the two D6 targets, archives named
  `quotapane-vX.Y.Z-<target>.{zip,tar.gz}` containing both binaries plus both
  licenses and the README, a `SHA256SUMS` covering every archive, `cosign`
  keyless signing of `SHA256SUMS` (one signature covering everything, rather
  than one per artifact), `actions/attest-build-provenance` on the archives, a
  recorded toolchain version to satisfy `SECURITY.md:105`, and upload to a
  **draft** release — never auto-published.
- **Permissions**, called out rather than pasted in: `contents: write`,
  `id-token: write`, `attestations: write`. Widening workflow permissions is
  itself a §4.1-grade decision, not boilerplate, and `id-token: write` is the
  one that lets a compromised workflow mint OIDC identities. It is scoped to the
  release job only, never at the workflow top level.
- Third-party actions pinned by full SHA per `ci.yml`'s own rule, with the tag
  in a trailing comment so a human can re-verify the pin later. The floor's job
  is to confirm each SHA still resolves to the named tag with `gh api` and STOP
  on any mismatch — verification, not authoring.

---

## Prompt E — doc truth pass (needs D5; closes F1–F3)

Runs **after** D so the security claims become true rather than being
future-tensed and then remembered later. Future-tensing a security document and
reverting it at release is exactly the two-step that gets half-done.

---

```
# Goal prompt: M6-DOCS — make every claim in this repo true

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

OWNER-SUPPLIED VALUE (if still FILL_ME, STOP):
  SECURITY_CONTACT = FILL_ME   # a real address, or the literal string
                               # NONE to delete the backup-channel line and
                               # rely on GitHub private vulnerability reporting

TIER NOTE (§6): Mixed, route (b). README.md and CONTRIBUTING.md are yours to
author. SECURITY.md and THREAT_MODEL.md are §4.1 — this prompt will carry
their replacement bytes verbatim, authored at the top tier, and you diff and
commit them without authoring. Two commits, in that order, so the
non-protected work is reviewable separately from the protected work.

PRECONDITIONS (mismatch = STOP):
P1 Prompt D has landed: ci.yml has a gitleaks job scanning full history, and
   release.yml exists. Verify by reading both, not by trusting this line.
P2 prompts/m6-gap-report.md exists. Every finding in it is either closed by
   this prompt or explicitly deferred with a reason in your end-gate report.
   A finding that is neither is a miss.
P3 Prompt B (M6-NAME) landed: the binaries are quotapane / quotapane-cli.

SCOPE — COMMIT 1: README.md and CONTRIBUTING.md (yours to author).

README.md needs a real rewrite, not patches. It is the first file every
visitor reads and it is currently three milestones stale. Required:
- Delete the "Working name" banner — the name is decided.
- Status: describe what the app DOES today, per-model breakdown included.
  Not a milestone code. A visitor does not know what M5a is.
- Roadmap: accurate. M4 was withdrawn by ADR-002 and must not appear as
  live scope. M5 is frozen at M5a per D1; everything else is post-1.0. Say
  so plainly.
- "Requires Rust 1.85+" is wrong — Cargo.toml pins rust-version = "1.92".
  State the real floor and where it comes from (eframe 0.35).
- An **install** section: download the release, and a **Verify before you
  run** section with the exact commands. SECURITY.md:106 points readers at
  this section and it does not exist — that is half of blocking defect F2.
  Write the commands from what Prompt F actually ran, not from what
  release.yml was supposed to do. If Prompt F has not run yet, this section
  is the one thing you may leave marked TODO — flag it loudly in your end
  gate, because shipping without it re-opens F2.
- Keep the disclaimer paragraph's substance intact. It is load-bearing:
  undocumented endpoints, own credentials only, bypasses no authentication,
  presents as the official client via User-Agent. You may re-word for the
  new name and for flow; you may not soften any of those claims.
- Note macOS as build-from-source per D6.

CONTRIBUTING.md: correct any version/toolchain claim, confirm the
dependency-justification table still matches the actual dependency set
(tray-icon's row included), and make sure it describes the real CI gates
now that gitleaks exists.

Commit 1 message:
    docs: rewrite README for v1.0 reality; correct CONTRIBUTING

SCOPE — COMMIT 2: SECURITY.md and THREAT_MODEL.md. §4a — the bytes are
carried in this prompt, pre-authored. [TOP TIER: bytes inserted here once
Prompt D has landed and the claims are actually true. The three lines they
replace are SECURITY.md:104 (signed/attested/checksums), :106 (verify
instructions in README), and :115 (gitleaks in CI *and pre-commit*), plus
the ARCHITECTURE.md §7 echo of the gitleaks claim and the D5 contact line.]

Diff before committing. Every hunk must be one of: a claim becoming
accurate, the pre-commit clause resolved, or the D5 contact filled or
removed. Anything else, revert and report.

Commit 2 message:
    docs(security): reconcile SECURITY.md and THREAT_MODEL.md with reality

    Closes the false gitleaks-in-CI claim and the signed-releases claim,
    both now backed by actual workflows. Fills the security contact.

TESTS: docs only, so the gate is: `cargo test --workspace` still green (a
doc test or a test asserting on README content may exist — if one breaks,
that is information, not noise), and every relative link in every edited
document resolves to a file that exists. Check the links mechanically, not
by eye.

VERIFY + SHIP: LF-only, no CR bytes. `git diff --check` clean. Two commits.
Push, CI green including the new gitleaks job on full history — if gitleaks
fires on a historical commit, STOP and report it as a finding rather than
touching history. That result is also an input to D7 and the owner needs it.

END GATE — STOP. Report: both SHAs, CI, gap-report findings closed vs
deferred-with-reason, whether the README verify section is real or TODO, and
any claim you could not make true and had to soften instead. That last list
is the important one — a claim softened is a feature the project no longer
promises, and the owner decides that, not you.
```

---

## Prompt F — v1.0.0 (after D1 ✓, Prompt D)

```
# Goal prompt: M6-RELEASE — cut v1.0.0

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Floor tier. Touches no §4.1 path — release.yml already
exists and you do not edit it. If the release workflow needs a fix, that is
a STOP and a top-tier authoring pass, not an inline edit. Editing a workflow
mid-release to make a release succeed is how unreviewed bytes reach a
signing job.

PRECONDITIONS (mismatch = STOP):
P1 Prompts B, D, E all landed. Tree clean, CI green including gitleaks.
P2 DECISIONS.md marks M5a accepted, and M5 frozen at M5a for v1.0 per D1.
P3 No feature work is pending. If any exists uncommitted, STOP — D1 froze
   scope and a release is not the place to discover a stray change.

SCOPE — three phases. STOP between phase 2 and 3 for the owner.

PHASE 1 — version + changelog. One commit.
- Cargo.toml [workspace.package] version 0.1.0 → 1.0.0. Update Cargo.lock
  via `cargo build --locked` (the workspace members' versions move; no
  dependency should change — if any third-party entry moves, STOP).
- Write CHANGELOG.md, Keep-a-Changelog format, covering M0 → M5a as the
  1.0.0 entry. Write it for a stranger: what the app does, what each
  provider reads, what the security posture guarantees. Milestone codes may
  appear in parentheses at most.
- Any other file the pin report identified as hardcoding a version.
Commit: "release: 1.0.0 — version bump and CHANGELOG"

PHASE 2 — release-candidate dry run. THIS IS THE REAL TEST.
- Tag v1.0.0-rc.1, push the tag, let release.yml run end to end.
- Then verify the draft release AS AN OUTSIDER WOULD, and be pedantic:
  download every artifact fresh; check every SHA256 against SHA256SUMS;
  `cosign verify-blob` the SHA256SUMS signature with the explicit
  --certificate-identity-regexp and --certificate-oidc-issuer for this repo;
  `gh attestation verify` each archive against the repo; extract each
  archive and confirm it contains both binaries, both licenses, and the
  README; run the CLI binary's --help from the extracted archive on this
  machine.
- Record the exact commands and their exact output. These commands become
  the README "Verify before you run" section, so they must be the commands
  that actually worked, copied from your terminal, not reconstructed.
- Verification that passes because you skipped a step is worse than a
  failed release. If any step cannot be run here, say which and why.

STOP AFTER PHASE 2. Report everything and wait. Tagging v1.0.0 is the
owner's call (§4.8) and it is irreversible in the way that matters — a
published 1.0.0 with a broken signature is a permanent artifact.

PHASE 3 — only on explicit owner go-ahead in a later turn.
- Tag v1.0.0, push, let the workflow run, verify the draft exactly as in
  phase 2, then hand the draft to the owner to publish. YOU DO NOT PUBLISH.
- Delete the rc tag and its draft release only after v1.0.0 verifies.
- If the README verify section was left TODO by Prompt E, fill it now from
  phase 2's recorded commands. One commit: "docs: verification instructions
  from the v1.0.0 release".

DO NOT: publish any release; edit release.yml or any workflow; run the app
against real credentials; print/log/persist token material; force-push;
delete or rewrite any tag other than the rc you created.

END GATE — STOP. Report: the version commit, the rc tag, the full
verification transcript, every artifact with its checksum, and your
confidence that an outsider following your recorded commands gets the same
result. Then wait for the owner.
```

---

## Prompt G — public flip (needs D7; run the scan FIRST)

**Read this before running anything.** D4 chose "publish the charter, not the
prompts." Moving `prompts/` and `_claude_setup` out of the tree does **not**
remove them from git history — every prior commit still contains them, and on a
public repo anyone can retrieve them in one command. So D4 as chosen only holds
under the fresh-repo option in D7. If you rename in place and keep history, the
honest position is "the prompts are working notes, not maintained
documentation," and you should just publish them rather than pretend.

That interacts with the other reason D7 might go fresh-repo: if any real token
ever entered this history. Phase 1 below answers both questions, and it is the
only part of this gate that must run before you decide anything.

---

```
# Goal prompt: M6-PUBLIC — pre-flip audit and hygiene

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Phase 1 is read-only and produces a report. Phase 2 is
hygiene files in non-protected paths. The GitHub settings changes are the
OWNER'S, in the web UI — you do not flip visibility, you do not change
branch protection, you do not enable anything. §4.8.

PRECONDITIONS: v1.0.0 tagged and verified (Prompt F phase 3 complete).
Tree clean, CI green.

PHASE 1 — FULL-HISTORY SECRET SCAN. Read-only. Do this first.
- Scan EVERY commit on EVERY ref, not the working tree and not just main.
  `git rev-list --all` is the input set. Use the gitleaks configuration
  Prompt D installed, plus an independent pass for the credential shapes
  this project actually handles: `sk-ant-`, `sk-`, OAuth refresh-token
  shapes, `Bearer ` followed by a long token, and the exact key NAMES from
  the credential structs (names only — §4.4: never print a candidate value,
  report file+commit+line and the matched key name only).
- Deleted files count. A token committed and later removed is still in
  history and is exactly what this scan is for.
- Every hit gets: commit, file, line, matched key name, and whether the
  value is a known synthetic test fixture. Fixtures are expected — the
  point is to confirm every hit IS one.
- Also enumerate what history exposes beyond secrets: every path that ever
  existed and no longer does, so the owner knows what a public history
  reveals. prompts/ and _claude_setup are the ones D4 cares about; report
  anything else surprising.
- Write prompts/m6-history-scan.md. Commit it. This report may itself be one
  of the files D4 excludes from the public tree — note that in it.

STOP AFTER PHASE 1. This report decides D7 (rename in place vs fresh repo)
and it decides whether D4 is achievable at all. Do not proceed to phase 2
until the owner has resolved D7, because a fresh repo makes half of phase 2
moot.

PHASE 2 — only after the owner resolves D7.
- Move prompts/ and _claude_setup per D4 and D7's outcome. Note: on this
  machine `rm` may be unavailable through the bridge — if a delete fails,
  move to a _to_delete/ folder and tell the owner rather than forcing it.
  There is already an empty _to_delete/ at the repo root from a prior
  session; remove it if you can, and confirm it is not tracked.
- Add CODE_OF_CONDUCT.md (Contributor Covenant 2.1, with the D5 contact),
  .github/ISSUE_TEMPLATE/bug_report.yml and feature_request.yml, and
  .github/PULL_REQUEST_TEMPLATE.md. The bug template must NOT ask for
  anything that would invite a user to paste credential material — no "paste
  your config," no "attach your credentials file," no raw --debug-raw output
  without an explicit redaction warning. Ask for version, OS, provider, and
  the normalized CLI output only. Write that warning into the template.
- The PR template asks contributors to confirm they have not touched a §4.1
  path without maintainer review, and to state which security invariant
  their change could plausibly affect.
- Note: .github/** is §4.1. The issue and PR templates land there, so this
  phase carries pre-authored bytes for those files [TOP TIER: authored once
  D5 and D7 are resolved] and you diff-and-commit without authoring.

OWNER'S LIST — report it, do not do it:
Enable private vulnerability reporting; enable branch protection on main
(require CI, require PR review); confirm Actions permissions allow the
release workflow's id-token and attestations scopes; then flip visibility.
Finally, DECISIONS.md gets its final roadmap line and the owner's
acceptance stamp — theirs alone, §4.8.

END GATE — STOP. Report the scan result first and most prominently: hit
count, how many are confirmed synthetic fixtures, and any hit that is not.
Then the hygiene commits. Then your honest read on whether this repo's
history is safe to make public, argued rather than asserted. If the answer is
"I am not certain," say that — a fresh repo is cheap and an exposed token in
public history is not.
```

---

## What remains after G

Packaging: WinGet, Homebrew, AUR. Explicitly post-1.0 — every channel wants a
published GitHub release to point at, so all three are strictly cheaper after
v1.0.0 exists than before. And the post-1.0 feature milestone that D1 deferred:
history/sparklines, forecast-to-limit, thresholds/alerts, the Codex User-Agent
flag, `OtelSource`.
