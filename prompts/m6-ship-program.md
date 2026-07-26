# M6 SHIP PROGRAM — placeholder-name → public v1.0

Authored at the top tier (Cowork bridge), 2026-07-25. Companion to `DECISIONS.md`
(charter) and `ARCHITECTURE.md` §7 (supply-chain hygiene) / §9 (roadmap).

This is **not** a goal prompt. It is the map: the remaining gates, which of them
are yours alone, which land in §4.1 and therefore must be authored at the top
tier, and the goal prompts that drive each one. Run the prompts in order; the
owner decisions in §3 block the prompts that follow them.

---

## 0. Why this is a program and not a single prompt

`DECISIONS.md` §2 already calls M6 "decision-dense, interactive." Three
constraints make a single mega-prompt impossible without breaking the charter:

- **§4.8** reserves milestone plans and milestone acceptance to the owner. A
  prompt that carried itself from here to "repo is public" would be making
  milestone decisions on your behalf at every step.
- **§4.1** covers `.github/**`, `SECURITY.md`, `THREAT_MODEL.md`, `deny.toml`,
  `.cargo/**`, `.claude/**`. The release workflow, the secret-scanning job, and
  every doc correction in §2 below land inside that list. A floor session may
  **verify and commit** those bytes under §4a but may not author them.
- **§6** requires each goal prompt to state its tier and to declare any §4.1
  phase up front, choosing route (a) escalate-to-author or (b) carry
  pre-authored bytes. A prompt spanning all gates could not honestly do either.

So: gates, decisions, and one prompt each.

---

## 1. The gate map

- **G0 — M5a fix + acceptance.** *In flight.* The two-line per-model row fix,
  then your visual acceptance of M5a. Nothing below starts until M5a is ✅.
- **G1 — OWNER DECISION: freeze M5 scope for v1.** See D1.
- **G2 — OWNER DECISION: the name.** See D2. Blocks G3–G6 entirely.
- **G3 — Doc truth pass.** Rename fallout plus reconciling every aspirational
  claim with reality. Mixed tier: README/CONTRIBUTING are floor-authorable;
  `SECURITY.md`/`THREAT_MODEL.md`/`deny.toml` are §4.1, top tier authors.
- **G4 — Supply-chain CI.** gitleaks job (full history), release workflow with
  cosign signing + GitHub attestations + checksums. Entirely `.github/**` →
  §4.1, top tier authors, floor verifies under §4a.
- **G5 — v1.0.0 release.** Version bump, CHANGELOG, tag, dry-run the release
  workflow on a pre-release tag, then cut the real one and verify the published
  artifacts from a clean machine.
- **G6 — Public flip.** History secret scan, repo hygiene files, GitHub settings
  (private vulnerability reporting, branch protection), then flip visibility.
- **G7 — Packaging.** WinGet / Homebrew / AUR. **Explicitly post-1.0.** Do not
  let packaging gate the public launch; every packaging channel wants a
  published release to point at anyway.

Critical ordering note: **G2 before G3.** Renaming after the docs are rewritten
means rewriting them twice. The name is the long pole — start thinking about it
now, not at G2.

---

## 2. Blocking defects found 2026-07-25 (must close before public)

These are pre-existing, not M5a fallout. Both are false statements in a §4.1
security document, which is the highest-severity doc class this project has.

**F1 — `SECURITY.md` claims secret scanning that does not exist.**
Under *Supply-chain policy*: "Secret scanning (`gitleaks`) runs in CI and
pre-commit; test fixtures use synthetic tokens only." The second clause is true.
The first is false — `.github/workflows/ci.yml` has four jobs (build & test
matrix, cargo-deny, cargo-audit, invariant-4 no-telemetry) and none of them is
gitleaks. `ARCHITECTURE.md` §7 repeats the claim as a bullet.
*Resolution:* G4 makes it true (preferred), **or** G3 rewrites it to
future tense. Do not flip the repo public with it as-is.

**F2 — `SECURITY.md` claims signed, attested releases and verification
instructions that do not exist.**
Under *Build & release integrity*: artifacts "are **signed** (e.g. `cosign`) and
published with **build provenance / attestations**; checksums accompany every
release," and "**Verify before you run:** instructions for checking the
signature/provenance and checksum are in `README.md`." There is no release
workflow, no signing, no attestation, no checksum publication, and no such
README section. A user who reads this and downloads a binary is relying on a
control that isn't there.
*Resolution:* G4 + G5 make it true, and G3 adds the README verification section
the doc points at. These must land in the **same release** as the first
published binary — a signed-releases claim shipping one release ahead of actual
signing is the worst possible ordering.

**F3 (minor) — README is stale to the point of being wrong.**
Status line still reads "**M0 — trust boundary & scaffolding.** Not yet useful on
a desk" — three milestones out of date. The roadmap line still advertises "**M4**
opt-in official billing APIs," withdrawn by ADR-002. It says "Requires Rust
1.85+" while `Cargo.toml` pins `rust-version = "1.92"`. Not a security claim, but
it is the first file every visitor reads.

---

## 3. Owner decisions (each blocks the prompt named)

**D1 — What is the v1 scope of M5? (blocks G3)**
M5 is open-ended depth: history/sparklines, forecast-to-limit, thresholds/alerts,
the Codex User-Agent CLI flag, the token-free `OtelSource`. None of it is
required for a useful public v1.0 — `ARCHITECTURE.md` §9 already calls M3 "the
minimum shippable product," and M5a adds per-model on top of that. History and
alerts each drag in a new decision (on-disk storage format; a notification
dependency needing top-tier dependency review, as the tray icon did), so each is
a multi-session slice, not a finishing touch.
*Recommendation:* declare **M5a is all of M5 for v1.0**; everything else moves to
a post-1.0 milestone. This is the single biggest schedule lever you hold. The
counter-case is that sparklines are the feature that makes people share a
screenshot — if launch impact matters more than launch date, history is the one
slice worth the delay.

**D2 — The name. (blocks G3, G4, G5, G6)**
`DECISIONS.md` §1: "Placeholder name **QuotaPane** stays until the pre-release
naming decision (owner-only, M6)." Owner-only, and everything downstream embeds
it. Scope of the change, measured today: 9 hits in `usage-ui/src/main.rs`, 5 each
in `SECURITY.md` and `ARCHITECTURE.md`, 3 in `THREAT_MODEL.md`, 2 each in
`README.md`/`DECISIONS.md`, 1 each across the remaining crates, `LICENSE-MIT`
(copyright line), `deny.toml`, `CLAUDE.md`, and the `.claude/agents/*` files.
Good news: **`.github/workflows/ci.yml` contains no product or crate name**, so
the workflow survives a rename untouched. Also needed: the GitHub repo URL in
`Cargo.toml` `[workspace.package]`, and the window title + tray tooltip strings.
*Check before committing to a name:* crates.io availability (even if you never
publish, squatting the name is cheap insurance), GitHub org/repo availability, a
domain if you want the `security@` contact in D5 to be real, and a trademark
sanity check given the app sits adjacent to two large vendors' brands.

**D3 — Do the crate names change with the product name? (blocks G3)**
`usage-core` / `usage-ui` / `usage-cli` are descriptive and product-neutral.
*Recommendation:* leave crate names alone; rename only the **binary** users
actually invoke, via `[[bin]] name = "<product>"` in `usage-ui/Cargo.toml` (and
`<product>-cli` for the CLI). Minimal churn, and the internal names stay honest
about what each crate does. The counter-case is that a public repo whose binary
and crates disagree looks slightly unfinished to a browsing contributor.

**D4 — Does the agent-workflow material go public? (blocks G6)**
The repo carries `DECISIONS.md` (an AI autonomy charter), `CLAUDE.md`,
`.claude/agents/*`, `prompts/` (9 goal prompts including the 17KB M5a record),
and a `_claude_setup` directory. Publishing them is a real choice with a real
case on both sides. For: this is a credential-touching security tool, and the
single most persuasive thing about it is the discipline behind it — a published
charter with hard-stop conditions and an author≠verifier rule is unusually strong
evidence of that, and `THREAT_MODEL.md` + `DECISIONS.md` together are better
trust signals than any README paragraph. Against: it foregrounds "AI-built
security tool," which some of your audience will read as a negative regardless of
the artifact quality, and `prompts/` are working notes rather than documentation.
*Recommendation:* publish `ARCHITECTURE.md`, `DECISIONS.md`, `THREAT_MODEL.md`,
`SECURITY.md`, `CONTRIBUTING.md`, `CLAUDE.md`. Move `prompts/` and
`_claude_setup` out of the public tree (a `docs/history/` subfolder if you want
them preserved, or simply out of the repo). Keep `.claude/` — it is small,
useful to contributors running the same tooling, and already §4.1-protected.

**D5 — The security contact. (blocks G3)**
`SECURITY.md` currently reads `security@<DOMAIN>` *(fill in before first
release)*. GitHub private vulnerability reporting covers the primary channel, so
the backup address is optional — but the placeholder cannot ship. Either supply a
real address or delete the backup-channel line and rely on GitHub's form.

**D6 — Release artifact matrix. (blocks G4)**
CI builds and tests on windows/macOS/ubuntu today. `DECISIONS.md` §1 says Windows
is primary, macOS/Linux best-effort. Do releases publish all three, or Windows
only with the others built-from-source? Signing and notarization cost differs
sharply: an unsigned macOS binary triggers Gatekeeper and generates support
questions, and proper notarization needs a paid Apple Developer account.
*Recommendation:* publish Windows and Linux binaries at v1.0; document macOS as
build-from-source until you decide whether notarization is worth the cost.

**D7 — Repo identity. (blocks G6)**
`Cargo.toml` points at `https://github.com/cipherpine/quotapane` (private).
Renaming the repo in place preserves history and redirects old URLs; a fresh repo
gives a clean history but loses everything. If any real token was ever committed
in this repo's history, the fresh-repo option becomes much more attractive — G6's
history scan answers that, so **run G6's scan before committing to D7.**

---

## 4. The prompts

### Prompt A — release-readiness audit (runnable now, after G0)

Runnable immediately after the M5a fix lands, independent of every decision
above, and it sharpens all of them. Read-only, so it cannot break anything.

---

```
# Goal prompt: M6-PREP — release-readiness audit (read-only)

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): This phase is READ-ONLY except for one new file under
prompts/. It touches NO §4.1 path. You are not authorizing, fixing, or
rewriting anything — you are producing an inventory. If you find yourself
wanting to fix something, write it down instead. A fix attempt here is a
§4.7 stop.

PRECONDITIONS (mismatch = STOP and report):
P1 `main` tip is the M5a fix commit, CI green, working tree clean.
P2 M5a is marked "awaiting visual acceptance" or accepted in DECISIONS.md.

SCOPE — produce `prompts/m6-gap-report.md`. One commit, that file only.

Inventory every gap between what this repo CLAIMS and what it DOES. For
each finding record: file + line, the claim, the reality, severity
(blocks-public / should-fix / cosmetic), and which §4.1 path (if any) a fix
would touch. Do not fix. Do not rewrite. Report.

Cover at minimum:
- **Doc-vs-reality.** Every present-tense claim in SECURITY.md,
  THREAT_MODEL.md, ARCHITECTURE.md, README.md, CONTRIBUTING.md that
  asserts a control, a gate, a CI job, or a file that exists. Verify each
  against the tree. Two are already known and must appear in your report or
  your method is wrong: SECURITY.md claims gitleaks runs in CI (no such
  job), and claims signed/attested releases plus README verify instructions
  (no release workflow, no such README section).
- **Placeholders.** Every `<DOMAIN>`, `TODO`, `TBD`, `fill in`, `working
  name`, or bracketed stub across all docs.
- **Stale roadmap/status.** Any milestone state contradicting DECISIONS.md
  §2, and any surviving reference to withdrawn M4 as live scope.
- **Version claims.** Every stated toolchain/version requirement vs
  Cargo.toml `rust-version` and the actual workspace version.
- **Naming surface.** Full list of files and line numbers containing the
  placeholder product name, split into (a) §4.1 paths and (b) everything
  else, so the rename can be planned as two commits. Note separately every
  USER-VISIBLE string carrying the name: window title, tray tooltip, CLI
  output, binary filenames.
- **Release surface.** What a release workflow would need that does not
  exist yet: artifact naming, per-OS build steps, checksum generation,
  signing, attestation, CHANGELOG, version bump points.
- **Public-repo hygiene.** Which of these are missing: CHANGELOG.md,
  CODE_OF_CONDUCT.md, issue templates, PR template. Note their absence;
  do not create them.
- **Files that should not ship.** Anything in the tree that is working
  material rather than product — enumerate it, recommend nothing.

METHOD NOTE: grep is your inventory tool, but a claim is only verified when
you have checked the thing it asserts. "SECURITY.md says X" is not a
finding; "SECURITY.md:112 says X, and <specific check> shows not-X" is.

DO NOT: run the app against real credentials; print, log, or persist any
token material; touch any §4.1 path; create any file other than the report;
open a PR.

VERIFY + SHIP: report is markdown, LF-only, no CR bytes. `git status` shows
exactly one new file. Commit "M6-prep: release-readiness gap report"; push;
CI green (the report changes no code, so any failure is §4.6).

END GATE — STOP. Report the commit + CI, the finding count by severity, and
the three findings you judge most likely to embarrass the project on day
one of being public. Do not fix anything. Do not start the rename. Do not
touch .github/.
```

---

### Prompt B — the rename (after D2, D3)

Floor tier, **two commits**, because the name crosses the §4.1 line.

- Commit 1 (floor authors): every non-protected occurrence — crate sources,
  `README.md`, `CONTRIBUTING.md`, `ARCHITECTURE.md`, `DECISIONS.md`,
  `Cargo.toml` repo URL, `LICENSE-MIT` copyright, the `[[bin]]` rename per D3,
  window title and tray tooltip strings, and the `provider_label`-adjacent user
  strings. Tests that assert on the old name update with it.
- Commit 2 (§4a verify-and-commit): `SECURITY.md`, `THREAT_MODEL.md`,
  `deny.toml`, `.claude/agents/*` — I author those bytes, the prompt carries
  them verbatim, the floor diffs and commits without authoring.

Prompt must state: `.github/workflows/ci.yml` contains no product or crate name
and must come back unmodified. Owner visual re-check afterward, since the window
title and tray tooltip both change (§4.5).

### Prompt C — doc truth pass (after Prompt A, B; closes F1–F3)

Consumes the gap report. README gets a real rewrite: current status, accurate
roadmap, correct Rust version, install/verify instructions, and a screenshot.
`SECURITY.md`'s two false claims get resolved — the decision is whether G4 lands
first (make them true) or the docs go future-tense now and revert to present
tense at G5. I author every §4.1 byte; the floor verifies.

*Sequencing recommendation:* run G4 first and make the claims true. Future-tensing
a security doc and then remembering to change it back at release is exactly the
kind of two-step that gets half-done.

### Prompt D — supply-chain CI (§4.1 throughout; I author, floor verifies)

Two workflow changes: a `gitleaks` job scanning **full history**, not just the
tip; and `release.yml` triggered on `v*` tags doing per-OS `--locked` release
builds, SHA256SUMS, `cosign` keyless signing, `actions/attest-build-provenance`,
and draft-release upload. Requires D6 for the matrix. Needs `id-token: write` and
`attestations: write` permissions — worth noting because widening workflow
permissions is itself a §4.1-grade decision, not boilerplate.

### Prompt E — v1.0.0 (after D1, Prompt D)

Version bump `0.1.0` → `1.0.0`, write `CHANGELOG.md` covering M0–M5a, tag a
`v1.0.0-rc.1` **first** and let the release workflow run end to end on it. Then
verify the artifacts as an outsider would: download from the draft release on a
clean machine, check the checksum, verify the cosign signature and the provenance
attestation, and run the binary. Only after that passes does `v1.0.0` get tagged.
The README verify instructions get written from what you actually ran, not from
what the workflow was supposed to do.

### Prompt F — public flip (after D4, D7, everything above)

Mostly your hands in the GitHub UI, but one hard prerequisite the floor can run:
a **full-history secret scan**, every ref and every commit, not the working tree.
That result feeds D7. Then: move or delete the files D4 excludes, add
`CHANGELOG.md` / `CODE_OF_CONDUCT.md` / issue + PR templates, fill or remove the
D5 contact, enable private vulnerability reporting and branch protection on
`main`, and flip visibility. `DECISIONS.md` §2 gets its final roadmap line and
your acceptance stamp — yours alone, per §4.8.

---

## 5. Shortest honest path

If D1 resolves to "M5a is all of M5," the critical path from the M5a fix to a
public v1.0 is: **A → D2 → B → D → C → E → F.** Six floor sessions and three
top-tier authoring passes (the rename's protected half, the doc corrections, and
both workflows). The two owner decisions that gate everything are the **name**
and the **M5 scope freeze** — the rest is execution against a known list.

Packaging (G7) stays out. Every channel wants a published GitHub release to point
at, so it is strictly cheaper after v1.0 than before.
