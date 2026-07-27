# M6 LAUNCHERS — paste-able goal prompts, one per gate

Each block below is what you paste into the floor session (all are well under
4000 characters). The full specs live in the repo — `prompts/m6-prep-audit.md`
for A, `prompts/m6-prompts-b-to-g.md` for B–G — so the launcher's only job is
to point the session at the right spec and carry your fill-in values.

Prompt A's launcher works today because its spec file is already on disk in
`prompts/`. The B–G spec file and this launcher file are placed into
`prompts/` after A lands; Prompt B's Phase 0 commits them, so from B onward
every session reads its spec from the tree.

Rules that apply to every launcher: run them in order A → B → C → D → E → F →
G, one per session, never two in one session. D is not launchable yet — its
content is §4.1 workflow bytes I author only after C reports real SHAs.

---

## A — release-readiness audit (runnable now)

```
# Goal prompt: M6-PREP — release-readiness audit

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-prep-audit.md IN FULL and execute it as this
session's goal prompt, verbatim. It was authored at the top tier and is
authoritative — if anything in it conflicts with what you find on disk,
that is a §4.7 STOP, not a judgment call. Its preconditions include an
exact git-status match; on any mismatch, STOP and report what you see.

Summary of what it asks (the file governs, not this summary): Phase 0
commits the pending --debug-raw work after full re-verification from
cargo clean, plus the three top-tier prompt documents verbatim; Phase 1
is a read-only audit producing prompts/m6-gap-report.md. No §4.1 path
is touched in either phase. End gate: STOP, report, fix nothing.
```

---

## B — adopt the name (after A + M5a ✅; D2 resolved: QuotaPane)

No fill-in values — the owner decided 2026-07-26 that QuotaPane, the working
name, is the final name.

```
# Goal prompt: M6-NAME — QuotaPane adopted; rename the binaries

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-prompts-b-to-g.md, section "Prompt B — adopt the
name", and execute that spec verbatim as this session's goal prompt. The
spec is authoritative over this launcher.

Context (the spec governs): the owner resolved D2 — QuotaPane, the working
name, IS the product name, and accepted M5a on 2026-07-27 (the acceptance
amendment is already in the working tree, authored at the top tier). Phase
0 commits the modified DECISIONS.md and the two untracked spec files
verbatim under §4a — verify the diff, author nothing. Phase 1 is one commit: DECISIONS.md §1 records the naming decision
as resolved, and the binaries rename per D3 via [[bin]] — quotapane and
quotapane-cli — while crate names stay usage-*. Phase 2 is one commit
fixing the audit's --help finding: --help/-h and --version exit 0 with
real usage text; unknown flags still error. P2: the working-tree
DECISIONS.md diff touches only the §2 M5 roadmap line; anything more,
STOP. End gate: STOP and report the binary filenames and the shipped
--help text; no visual re-check needed — the window title already says
QuotaPane and did not change. Do not start Prompt C.
```

---

## C — pin resolution (read-only; after B)

```
# Goal prompt: M6-PINS — resolve release-workflow pins (read-only)

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-prompts-b-to-g.md, section "Prompt C — supply-chain
pin resolution", and execute that spec verbatim as this session's goal
prompt. The spec is authoritative over this launcher.

You are NOT writing any workflow and NOT editing .github/** — you are
resolving action tags to full commit SHAs with authenticated gh, answering
the gitleaks licensing question, and mapping the release surface, into one
new file: prompts/m6-pin-report.md. Every value must carry the command or
URL that produced it; anything unresolved goes in an UNRESOLVED section
rather than being guessed. The report becomes §4.1 workflow bytes at the
top tier, which is why a guess here is a supply-chain pin nobody can
verify. End gate: STOP, one commit, report.
```

---

## D — supply-chain CI (NOT LAUNCHABLE YET)

No launcher exists yet, deliberately. D's content is two §4.1 workflow files
(the gitleaks job and release.yml) that I author from C's report — real SHAs,
the gitleaks approach C recommends, the artifact paths C confirms. When C's
report lands, hand it back to me; I return D's spec plus its launcher, and the
floor's role in D is §4a diff-and-commit of pre-authored bytes, not authoring.

---

## E — doc truth pass (after D; needs the security contact)

Fill SECURITY_CONTACT with a real address, or the literal word NONE to delete
the backup-channel line and rely on GitHub private vulnerability reporting.

```
# Goal prompt: M6-DOCS — make every claim in this repo true

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

OWNER VALUE: SECURITY_CONTACT = FILL_ME

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-prompts-b-to-g.md, section "Prompt E — doc truth
pass", and execute that spec verbatim with the value above. The spec is
authoritative over this launcher.

Shape (the spec governs): commit 1 is yours — a real README rewrite
(current status, accurate roadmap with M4 withdrawn and M5 frozen at M5a,
Rust 1.92 floor, install + verify sections, disclaimer substance intact)
and CONTRIBUTING corrections. Commit 2 is §4a — SECURITY.md and
THREAT_MODEL.md replacement bytes carried in the spec's marked TOP TIER
block; if that block is still a bracketed placeholder, STOP: the top tier
has not filled it yet and this prompt is not ready to run. Precondition
P1 (gitleaks job + release.yml exist) is verified by reading the
workflows, not by trusting the launcher. End gate: STOP, report findings
closed vs deferred, and every claim you had to soften — the owner rules
on those.
```

---

## F — cut v1.0.0 (after B, D, E)

```
# Goal prompt: M6-RELEASE — cut v1.0.0

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-prompts-b-to-g.md, section "Prompt F — v1.0.0", and
execute that spec verbatim. The spec is authoritative over this launcher.

Three phases (the spec governs): 1 — version bump 0.1.0 → 1.0.0 +
CHANGELOG, one commit; if any third-party Cargo.lock entry moves, STOP.
2 — tag v1.0.0-rc.1 and verify the draft release AS AN OUTSIDER: fresh
downloads, SHA256SUMS, cosign verify-blob with explicit identity flags,
gh attestation verify, extract and run --help; record exact commands and
output — they become the README verify section. HARD STOP after phase 2:
tagging v1.0.0 is the owner's call (§4.8). Phase 3 only on my explicit
go-ahead in a later turn, and even then you hand me the draft — YOU DO
NOT PUBLISH. If release.yml needs any fix, that is a STOP and a top-tier
authoring pass, never an inline edit mid-release.
```

---

## G — pre-flip audit (after F phase 3; scan BEFORE deciding D7)

```
# Goal prompt: M6-PUBLIC — pre-flip history scan and hygiene

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-prompts-b-to-g.md, section "Prompt G — public flip"
INCLUDING the warning paragraph above its code block, and execute that
spec verbatim. The spec is authoritative over this launcher.

Phase 1 first and alone: full-history secret scan, every commit on every
ref including deleted files — gitleaks plus an independent pass for this
project's own credential shapes. §4.4 absolutely: report file + commit +
matched key NAME only, never a candidate value. Also enumerate every path
history exposes that no longer exists (prompts/, _claude_setup, anything
else). Write prompts/m6-history-scan.md, commit, then HARD STOP — that
report decides rename-in-place vs fresh repo (D7) and whether keeping
prompts/ private is even possible, and both are mine. Phase 2 (hygiene
files) only after I resolve D7 in a later turn. You do not flip
visibility or change any GitHub setting — that list is reported to me.
```
