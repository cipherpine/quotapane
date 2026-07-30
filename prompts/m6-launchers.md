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
G, one per session, never two in one session. Order note (owner decision
2026-07-27): the public flip now precedes the v1.0.0 tag — D → E → G's
history scan + flip → F.

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

## D — supply-chain CI (after C; bytes pre-authored, already on disk)

Authored 2026-07-27 from C's pin report. The workflow files and the two §4.1
code corrections are already written into the working tree by the top tier;
this session verifies and commits, per §4a. Gate order changed by owner
decision 2026-07-27: the public flip now precedes the v1.0.0 tag, so the
order is D → E → G(scan+flip) → F.

```
# Goal prompt: M6-CI — land the supply-chain workflows and §4.1 fixes

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-supply-chain.md IN FULL and execute it as this
session's goal prompt, verbatim. It is a §4a verify-and-commit: every
byte landing in a §4.1 path was authored at the top tier — the two Rust
files are already on disk; the two workflow files are embedded verbatim
in the spec's §W and your first act is transcribing them to disk
byte-exactly. You verify (md5 table in the spec), build,
test, commit with the given messages, push once — you author nothing and
fix nothing. If any hash mismatches, any test fails, or fmt/clippy would
change a byte of a pre-authored file: STOP and hand back. If the new
gitleaks job flags anything on its first full-history run: STOP and
report each finding as file + commit + rule id, redacted, never the
candidate value (§4.4). End gate: STOP; report the five SHAs, all six CI
job results, the gitleaks outcome, and the test count. Do not start
Prompt E.
```

---

## E — doc truth pass (after D; D5 resolved: no e-mail contact)

No fill-in values — the owner resolved D5 on 2026-07-28: GitHub private
vulnerability reporting is the only channel, and the pre-authored
SECURITY.md already reflects it. The two §4.1 docs are already on disk.

```
# Goal prompt: M6-DOCS — make every claim in this repo true

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-doc-truth.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher.

Shape (the spec governs): commit 0 lands the spec files; commit 1 is
YOURS — a real README rewrite (status, roadmap with M4 withdrawn and M5
frozen at M5a, Rust 1.92, install + "Verify a release" with the exact
release.yml-matching commands, disclaimer substance intact), CONTRIBUTING
and ARCHITECTURE corrections per the gap-report checklist. Commits 2 and
3 are §4a — SECURITY.md and THREAT_MODEL.md were fully re-authored at the
top tier and sit in your working tree; verify the md5 table, read every
hunk, commit verbatim, author nothing. End gate: STOP; report the four
SHAs, CI, and the closed-vs-deferred table covering every G## finding in
the gap report. Do not start Prompt F — the reordered gates put the
history scan and public flip (Prompt G phase 1, owner-gated) first.
```

---

## F — cut v1.0.0 (LAST; after the flip — repo must be public)

The spec at prompts/m6-release.md supersedes the B–G file's Prompt F
section (pre-reorder, pre-rewrite). It also carries the corrected SHA
transposition rule: resolve old pins through m6-sha-map-2.txt ALONE.

```
# Goal prompt: M6-RELEASE — cut v1.0.0 (final form)

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-release.md IN FULL and execute it as this session's
goal prompt, verbatim. It SUPERSEDES the Prompt F section of
prompts/m6-prompts-b-to-g.md; the new file governs, including the
corrected rule that old SHA pins resolve through m6-sha-map-2.txt alone.

Shape (the spec governs): P1 requires the repo to already be PUBLIC —
if it is private, STOP; the flip is the owner's act. Phase 1: version
0.1.0 → 1.0.0 + CHANGELOG, one commit; if any third-party Cargo.lock
entry moves, STOP. Phase 2: tag v1.0.0-rc.1 and verify the draft AS AN
OUTSIDER — fresh downloads, sha256sum -c, cosign verify-blob with
explicit identity flags, gh attestation verify with commit-SHA match,
archive contents, run --help from the extracted binary — recording every
command verbatim, then correct README's verify section from the
transcript if reality differs. HARD STOP: tagging v1.0.0 is the owner's
call. Phase 3 only on explicit owner go-ahead: tag, re-verify fully,
delete the rc, hand over the draft URL — YOU DO NOT PUBLISH. If
release.yml needs any fix, that is a STOP and a top-tier authoring pass,
never an inline edit mid-release.
```

---

## F½ — release.yml v2 fix + resume rc (after F's phase-2 STOP on rc.1)

The rc.1 dry run caught two defects in the top-tier release.yml (upload
glob matched the staging dir; cosign v3 removed the old signing flags).
Fixed bytes are embedded in the spec; rc.1 is authorized for deletion.

```
# Goal prompt: M6-RELEASE-FIX — release.yml v2, then resume F (F½)

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-release-fix.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher.

Shape (the spec governs): transcribe the embedded release.yml v2
byte-exactly (§4a, md5-gated) — it fixes the staging-dir glob and moves
signing to cosign v3's --bundle SHA256SUMS.sigstore.json; commit, push;
delete the rc.1 tag (authorized by this spec, narrowly); then resume
m6-release.md PHASE 2 verbatim with v1.0.0-rc.2 — tag, let the workflow
run (any new failure = STOP, no inline fixes), verify the draft AS AN
OUTSIDER including the new bundle-form cosign verify, and correct
README's "Verify a release" to the bundle commands from your transcript.
SECURITY.md needs no edit; if you think it does, that is a §4.1 STOP.
HARD STOP after verification: tagging v1.0.0 stays the owner's call.
```

---

## G½ — history identity rewrite (between G phase 1 and phase 2)

Owner decisions 2026-07-28 (after the clean phase-1 scan): rewrite the
commit email to justin.parsons@cipherpine.com, strip Claude-Session lines,
keep Co-Authored-By; publish everything, prompts/ stays in place.

```
# Goal prompt: M6-REWRITE — history identity rewrite (G½)

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-history-rewrite.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher.

Shape (the spec governs): resolve the dependabot branch per §3 first, so
no unrewritten ref survives; mirror-backup and fsck BEFORE anything;
git filter-repo maps justin.parsons919@gmail.com →
justin.parsons@cipherpine.com (author + committer) and strips
Claude-Session: lines, keeping Co-Authored-By; the R5 invariants must ALL
hold before the force-push — same commit count, same Co-Authored-By
count, zero session lines, zero gmail hits, and a BYTE-IDENTICAL ordered
tree list proving no file content changed anywhere in history; then
commit the filter-repo commit-map as prompts/m6-sha-map.txt (all older
SHA pins transpose through it — a standing rule the spec authorizes),
re-add origin, force-push main, CI green. Any invariant failure = STOP
before pushing; the backup stays private and is never pushed. End gate:
STOP with the invariant table and the new tip. Do not start
m6-public-flip phase 2.
```

---

## G½b — author display name rewrite (after G½; before phase 2)

Owner decision 2026-07-28: public history reads "Justin Parsons
<justin.parsons@cipherpine.com>".

```
# Goal prompt: M6-REWRITE-2 — author display name (G½b)

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-history-rewrite-2.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher.

Shape (the spec governs): same machinery as G½, one variable — a second
mirror backup + fsck first; git filter-repo --name-callback maps
justinparsons919 → Justin Parsons (author + committer), touching nothing
else; the R3 invariants must ALL hold before the force-push — zero
justinparsons919 hits on any ref, unchanged commit count, BYTE-IDENTICAL
ordered tree list, unchanged Co-Authored-By count, and message bodies
hash-equal before/after (this pass must not touch them); then commit the
commit-map as prompts/m6-sha-map-2.txt (pre-G½ pins now chain through
BOTH maps in order — the spec extends G½'s standing rule), set repo-local
git config user.name, re-add origin, force-push, CI green. Any invariant
failure = STOP before pushing. End gate: STOP with the invariant table
and new tip. Do not start m6-public-flip phase 2.
```

---

## G — history scan + hygiene + flip (after E; BEFORE F under the new order)

The spec at prompts/m6-public-flip.md supersedes the B–G file's Prompt G
section, whose preconditions predate the gate reorder.

```
# Goal prompt: M6-PUBLIC — history scan, hygiene, and the flip

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m6-public-flip.md IN FULL and execute it as this
session's goal prompt, verbatim. It SUPERSEDES the Prompt G section of
prompts/m6-prompts-b-to-g.md (old gate order); the new file governs.

Shape (the spec governs): Phase 0 commits the spec files. Phase 1 is the
full-history secret scan — the pinned gitleaks binary locally PLUS an
independent pass for this project's own credential shapes, every commit
on every ref, deleted files included, never printing a candidate value
(§4.4) — plus the exposure inventory of every path history reveals, into
prompts/m6-history-scan.md. Then HARD STOP: the owner decides D7
(rename-in-place vs fresh repo) and whether prompts/ publishes, on your
report. Phase 2 runs only on the owner's explicit go-ahead in a later
turn: transcribe the pre-authored .github templates from the spec's §W
(md5-gated, §4a), author CODE_OF_CONDUCT.md, push. You never flip
visibility or change a GitHub setting — that list is reported to the
owner, verbatim. End gate: STOP; scan verdict first and most prominent.
```

---


## M7a — per-model truth, v1.1.0 (after v1.0.0 published)

Owner decisions 2026-07-29: Claude per-model via the endpoint's new
`limits` array (Fable); UI hides untouched buckets, CLI/JSON stay
truthful; v1.1.0 is exactly this slice. M6-CLOSE landed at 843a09a;
this spec's patch now only opens M7a.

```
# Goal prompt: M7A — per-model truth (v1.1.0)

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m7a-per-model-truth.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher. Note P0: the working tree may be detached at the v1.0.0 tag —
`git checkout main` first; the spec files survive the checkout.

Shape (the spec governs): Phase 0 commits the spec files, then applies
the exact DECISIONS.md patch (§4a, byte-match gate — inserts the M7a
segment after the M6 ✅ stamp that landed at 843a09a). Phase 1 is yours: parse the Claude limits
array (only percent/resets_at/scope.model.display_name — no id, no PII
field ever), per_model from model-scoped entries with the legacy keys as
fallback, synthetic fixtures for the new shape. Phase 2: UI hides
per-model rows at 0%/unknown usage and drops the toggle when none are
visible; a new CLI test pins that zero-usage buckets STAY in --json.
Full bar from cargo clean, push, CI 7/7. End gate: STOP for the owner's
visual check (Fable row appears, Spark row gone, CLI still truthful).
Do not bump the version or tag — the v1.1.0 release is a later prompt.
```

## M7a2 — Codex reset credits (after M7a; same v1.1.0 slice)

Owner addition 2026-07-29: surface rate_limit_reset_credits from the
Codex response. Runs after M7a landed (196fd56); no DECISIONS.md edit.

```
# Goal prompt: M7A2 — Codex rate-limit reset credits (v1.1.0 slice)

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m7a2-codex-reset-credits.md IN FULL and execute it as
this session's goal prompt, verbatim. The spec is authoritative over
this launcher.

Shape (the spec governs): all floor-authorable, no §4.1 phase. Phase 1:
ProviderSnapshot gains reset_credits: Option<ResetCredits> (available +
applicable_now); the Codex parser reads ONLY available_count and
applicable_available_count from rate_limit_reset_credits — the raw body
carries PII fields and none may enter any struct; Claude sets None.
Phase 2: the Codex pane renders one small line "resets available: N"
(absent when None, so Claude is untouched), layout-harness tested; the
CLI JSON-pinning tests extend to reset_credits always present (null for
Claude). Full bar from cargo clean, push, CI 7/7. End gate: STOP — the
owner's visual check covers M7a + this together (Fable row, no Spark,
resets available: 1). Do not bump the version or tag.
```

## M7-RELEASE — v1.1.0 (paste ONLY after your visual acceptance)

```
Owner acceptance: I have visually accepted M7a + M7A2 (Fable row, no
Codex toggle, resets available: 1). Proceed.

# Goal prompt: M7-RELEASE — cut v1.1.0

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m7-release.md IN FULL and execute it as this session's
goal prompt, verbatim. The spec is authoritative over this launcher and
governs over m6-release.md where they differ.

Shape (the spec governs): Phase 1 bumps 1.0.0 → 1.1.0 + CHANGELOG entry,
one commit, STOP if any third-party Cargo.lock entry moves. Phase 2 tags
v1.1.0-rc.1 and verifies the draft AS AN OUTSIDER — all six steps with
negative controls; release.yml is unchanged since v1.0.0 verified it and
is never edited inline. HARD STOP after verification: tagging v1.1.0 is
the owner's call. Phase 3 on explicit go-ahead: tag, re-verify, delete
the rc, hand over the draft URL — you do not publish. Phase 4, after the
owner confirms publication: apply the spec's exact DECISIONS.md patch
(§4a) stamping M7a/M7A2 accepted and v1.1.0 shipped.
```

## M7b — Cipher Pine visual pass (after v1.1.0 closes)

Direction B locked 2026-07-29; marks 1b/1c adopted; live tray miniature.
Pre-steps DONE: brand kit + README banner committed at the top tier;
avatar and social preview set in the GitHub UI. Tree is clean at launch.

```
# Goal prompt: M7B — Cipher Pine visual pass

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m7b-visual-pass.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher.

Shape (the spec governs): Phase 0 applies the exact DECISIONS.md patch
(§4a, byte-match) opening M7b — spec, launcher index, brand assets, and
README banner are already committed. Phase 1 is the
theme — exact Color32 palette in the spec, full built-in-mono type,
blueprint grid, "// PROVIDER" headers, cardinal "> quotapane" titlebar,
semantic pine/amber/cardinal bar fills, stale lines go cardinal — with
the layout harness as arbiter: shrink type before widening layout.
Phase 2: the block cursor as a STATUS indicator — solid when idle and
fresh, blinking only while polling or stale, zero idle repaints,
unit-tested. Phase 3: dependency-free icon.rs painting mark 1c as RGBA;
window icon at startup, live tray icon re-rendered per poll from real
fractions, set only when bytes change; pure-function pixel tests.
Phase 4 is VERIFY-ONLY — brand assets and README banner already landed.
ZERO new dependencies anywhere — needing one is a STOP. Full bar from
cargo clean, push, CI 7/7. End gate: STOP for the owner's visual pass;
expect iteration rounds; version stays 1.1.0.
```

## M7B-R1 — visual iteration 1 (owner round-1 feedback, 2026-07-29)

Grid→noise, type weight, eye comfort, plain-vs-themed toggle. Owner
decisions locked: fainter+wider grid; brighten+enlarge (font embed =
round-2 escalation); tray toggle persisted; Plain = pre-M7b look.

```
# Goal prompt: M7B-R1 — visual pass iteration 1

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m7b-r1-visual-iteration.md IN FULL and execute it as
this session's goal prompt, verbatim. The spec is authoritative over
this launcher.

Shape (the spec governs): the owner's round-1 feedback, applied.
Phase 1 calms the theme — grid 64px at alpha 6, type up (16/13/11.5),
ink brightened; the layout harness still arbitrates at 320px (shrink
type, never widen). Phase 2 adds Theme::{CipherPine, Plain} with a
one-word persisted config (std-only; garbage/absent → CipherPine) —
Plain is the pre-M7b look; the semantic bar mapping stays in BOTH
themes. Phase 3 wires the tray-menu toggle (live switch + persist),
--plain/--themed run-only flags, and the spec's verbatim README
"Theming" section. usage-core untouched; ZERO new dependencies — a
config/TOML/dirs crate is a STOP. Full bar from cargo clean, push,
CI 7/7. End gate: STOP for the owner's round-2 visual pass; no
DECISIONS change, no tag, version stays 1.1.0.
```

## M7B-RELEASE — v1.2.0 (paste ONLY after your visual acceptance)

Pasting this launcher records the owner's §4.5 acceptance of the M7b
look (round 2). Spec: prompts/m7b-release.md — the proven release
pipeline, verbatim.

```
# Goal prompt: M7B-RELEASE — v1.2.0

Owner acceptance: I have visually accepted the M7b Cipher Pine pass
(round 2 — calmed grid, brightened type, theme toggle, screenshots).
Proceed.

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m7b-release.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher.

Shape (the spec governs): Phase 1 bumps 1.1.0 → 1.2.0 and inserts the
spec's verbatim CHANGELOG entry; full bar, push, CI 7/7 before any
tag. Phase 2 tags v1.2.0-rc.1, runs the release workflow, then the
six-step outsider verification with all six negative controls — and
HARD STOPS with a report. Phase 3 runs only on the top tier's
explicit go-ahead: tag v1.2.0 on the verified commit, re-verify
fresh, delete the rc only after v1.2.0 verifies, hand back the draft
URL — the owner publishes. Phase 4, only after the owner confirms
publication: the exact §4a DECISIONS stamp, push, CI green, STOP.
No code changes, no new dependencies, nothing published by you.
```

## M8 — pace slice (v1.3.0 scope; after v1.2.0 closed)

Roadmap research accepted 2026-07-29: pace markers + burn forecast;
sparklines/persistence deferred to v1.4. Zero new deps, no new
persistence, no new network behavior.

```
# Goal prompt: M8 — pace

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m8-pace.md IN FULL and execute it as this session's
goal prompt, verbatim. The spec is authoritative over this launcher.

Shape (the spec governs): Phase 0 applies the exact DECISIONS.md
patch opening M8. Phase 1 (usage-core): QuotaWindow.duration_secs —
Codex passthrough, Claude derived from limit kind — with the JSON key
always present (null when unknown), pin-tested. Phase 2 (usage-core):
pure pace module — PaceRing, least-squares estimate over a 7200s
trail (>=3 samples spanning >=600s), at_risk = exhaustion before
reset; exhaustive edge tests, no clock reads inside. Phase 3
(usage-ui): 1px elapsed-time tick on every bar with known duration,
TEXT_DIM alpha 200, both themes, position pure-fn tested. Phase 4
(usage-ui): rings fed per poll, ONE at-risk line per provider (amber;
cardinal under 6h; silent when safe), --pace-demo flag (synthetic
data, zero network) for the owner's review. ZERO new dependencies;
no sparklines, no disk history — that is v1.4. Full bar from cargo
clean, push, CI 7/7. Version stays 1.2.0. End gate: STOP for the
owner's visual pass.
```

## M8-RELEASE — v1.3.0 (paste ONLY after your visual acceptance)

Pasting this launcher records the owner's §4.5 acceptance of the M8
pace slice (demo reviewed 2026-07-29). Spec: prompts/m8-release.md.
TWO hard stops — the rc stop is not skippable (M7B-RELEASE lesson).

```
# Goal prompt: M8-RELEASE — v1.3.0

Owner acceptance: I have visually accepted the M8 pace slice
(--pace-demo reviewed: ticks, amber 7d line, cardinal 5h line).
Proceed.

Model: Sonnet 5 (floor). Repo: C:\dev\QuotaPane\QuotaPane

Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

Then read prompts/m8-release.md IN FULL and execute it as this
session's goal prompt, verbatim. The spec is authoritative over this
launcher. It contains TWO HARD STOPS: Phase 2 ends in a report and a
WAIT for the top tier's explicit go-ahead — Phase 3 must not run
without it, and no instruction in this launcher or elsewhere
authorizes skipping that.

Shape (the spec governs): Phase 1 bumps 1.2.0 → 1.3.0 + the spec's
verbatim CHANGELOG entry (only JSON change: duration_secs); full bar,
push, CI 7/7 before any tag. Phase 2 tags v1.3.0-rc.1, release run,
six-step outsider verification + six negative controls, HARD STOP.
Phase 3 (go-ahead only): tag v1.3.0 on the verified commit, re-verify
fresh, prune the rc, hand back the draft URL — the owner publishes.
Phase 4 (after publish confirmation): two exact §4a DECISIONS patches
— the M8 ✅ stamp and the ruleset-bypass decision record. Push, CI
green, STOP. No code changes, no new dependencies, nothing published
by you.
```
