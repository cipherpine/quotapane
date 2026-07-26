# Goal prompt: M6-PREP — release-readiness audit (read-only)

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Two phases, both floor tier.

- **Phase 0** commits work that already exists and is already verified. Two of
  the three files were authored by you in a prior session; the third
  (`prompts/m6-ship-program.md`) was authored at the top tier and you commit it
  **verbatim** under §4a — you do not edit it, reformat it, reflow it, or
  "fix" its wording. `prompts/**` is not a §4.1 path, so this is a normal
  commit of bytes you did not write.
- **Phase 1** is READ-ONLY except for one new file under `prompts/`. It touches
  NO §4.1 path. You are not authorizing, fixing, or rewriting anything — you are
  producing an inventory. If you find yourself wanting to fix something, write
  it down instead. A fix attempt in Phase 1 is a §4.7 stop.

Neither phase touches `.github/**`, `SECURITY.md`, `THREAT_MODEL.md`,
`deny.toml`, `.cargo/**`, `.claude/**`, `crates/usage-core/src/egress/**`, or
`crates/usage-core/src/credentials/**`. If a step seems to require one of those,
that is the stop, not a judgment call.

## PRECONDITIONS (mismatch = STOP and report)

P1 `git log --oneline -1` on `main` is **7e72282** — `fix(ui): two-line
   per-model rows so long model labels don't clip`. Any other tip: STOP.

P2 `git status --porcelain` shows **exactly** these four entries and nothing
   else:

   ```
    M crates/usage-cli/src/main.rs
    M crates/usage-core/src/providers/claude_subscription.rs
   ?? prompts/m6-prep-audit.md
   ?? prompts/m6-ship-program.md
   ```

   The two untracked files are this prompt and its companion program document,
   both authored at the top tier and written into the tree ahead of you. If
   there is a fifth entry, or one of these is missing, enumerate what you
   actually see and STOP. Do not guess which extra file is safe to include.

P3 `DECISIONS.md` records M5a as **awaiting visual acceptance**, with no ✅.
   That is correct and must stay that way — M5a acceptance is the owner's
   (§4.8, §4.5). If it already carries a ✅ you did not put there, STOP.

P4 `.github/workflows/ci.yml` has exactly four jobs. Confirm this by reading it
   and name them in your report. You need this fact for Phase 1 anyway.

## PHASE 0 — land the pending work (three commits)

**0a.** The `--debug-raw` extension: `crates/usage-cli/src/main.rs` and
`crates/usage-core/src/providers/claude_subscription.rs`. Re-verify before
committing rather than trusting the earlier run — `cargo clean -p usage-core`,
then `cargo fmt --check` → `cargo build --locked` → `cargo clippy --workspace
--all-targets -- -D warnings` → `cargo test --workspace`. All zero-exit, 138
tests. Commit:

    feat(cli): extend --debug-raw to the Claude provider

    Both providers now route their usage request through a shared fetch, so
    the raw dump provably reflects the request normal polling sends. Passing
    --debug-raw with --json now emits a stderr note instead of silently
    dropping --json.

**0b.** `prompts/m6-ship-program.md` and `prompts/m6-prep-audit.md` — this
file — together, unmodified. Before committing, confirm both are LF-only with
no CR bytes; if either has CRLF, that is the one permitted change and you note
it in your end-gate report. Commit:

    docs(prompts): add M6 ship program and release-readiness audit prompt

    Gate map from M5a to a public v1.0 plus the first prompt against it, both
    authored at the top tier. Recorded here so floor sessions can read the
    gate order and the owner decisions that block each one.

Yes, this includes committing the prompt you are currently executing. That is
intentional — the record of what was asked should land with the work, not after
it.

**0c.** is Phase 1's report. Push once, after all three.

If Phase 0's re-verification fails on any step, STOP there and report the
failure. Do not proceed to Phase 1 on a red tree.

## PHASE 1 — produce `prompts/m6-gap-report.md`

One commit, that file only.

Inventory every gap between what this repo **claims** and what it **does**. For
each finding record: file + line, the claim verbatim, the reality, severity
(`blocks-public` / `should-fix` / `cosmetic`), and which §4.1 path (if any) a
fix would have to touch. Do not fix. Do not rewrite. Report.

Cover at minimum:

- **Doc-vs-reality.** Every present-tense claim in `SECURITY.md`,
  `THREAT_MODEL.md`, `ARCHITECTURE.md`, `README.md`, `CONTRIBUTING.md`,
  `CLAUDE.md` that asserts a control, a gate, a CI job, a test, or a file that
  exists. Verify each against the tree. Two are already known and **must**
  appear in your report or your method is wrong: `SECURITY.md` claims `gitleaks`
  runs in CI (P4 tells you it does not), and claims signed/attested releases
  with checksums plus verification instructions in `README.md` (there is no
  release workflow, no signing, no attestation, and no such README section).
  `ARCHITECTURE.md` §7 repeats the gitleaks claim — find it and every other
  place a false claim is echoed. A claim repeated in three files is three
  findings, because three files need editing.

- **Placeholders.** Every `<DOMAIN>`, `TODO`, `TBD`, `fill in`, `placeholder`,
  `working name`, `XXX`, or bracketed stub across all docs and all code
  comments.

- **Stale roadmap / status.** Any milestone state contradicting `DECISIONS.md`
  §2. Any surviving reference to **M4 opt-in official billing APIs** as live
  scope — that was withdrawn by ADR-002, and `ProviderId` no longer has the
  variants. The README status line still reads "M0 — trust boundary &
  scaffolding," three milestones stale; find every sibling of that.

- **Version claims.** Every stated toolchain or version requirement, against
  `Cargo.toml` `rust-version` (`1.92`) and the actual workspace version. The
  README's "Rust 1.85+" is one instance; look for others in CONTRIBUTING and
  any CI-adjacent prose.

- **CLI surface.** Run the CLI's `--help` and compare every flag it exposes
  against every flag any document describes. Phase 0 just changed
  `--debug-raw` from Codex-only to both providers, so any doc describing it as
  Codex-specific is now stale — that is a finding, and it is the newest one in
  the tree, so it is the best test of whether your method catches recent drift
  rather than only old drift.

- **Naming surface.** Complete list of files **with line numbers** containing
  the placeholder product name, split into (a) §4.1 paths and (b) everything
  else, so the rename can be planned as two commits. Separately and explicitly:
  every **user-visible** string carrying the name — window title, tray tooltip,
  any CLI output, binary filenames, and the `Cargo.toml` repo URL. Confirm or
  refute that `.github/workflows/ci.yml` contains no product or crate name.

- **Release surface.** What a release workflow would need that does not exist
  yet: artifact naming convention, per-OS build steps, checksum generation,
  signing, attestation, CHANGELOG, and every file a version bump would have to
  touch. List the version-bump points precisely — that list becomes a later
  prompt's checklist.

- **Public-repo hygiene.** Which of these are absent: `CHANGELOG.md`,
  `CODE_OF_CONDUCT.md`, issue templates, PR template, `.gitattributes`. Note
  absence; **create nothing.**

- **Files that should not ship.** Anything in the tree that is working material
  rather than product — `prompts/`, `_claude_setup`, scratch files, anything
  generated. Enumerate with sizes. Recommend nothing; that call is the owner's.

- **Test-harness blind spots.** Your last session found that
  `egui::__run_test_ui` installs empty fonts, so any width assertion made
  through it passes regardless of overflow. Sweep the whole test suite for
  assertions that could pass vacuously for a comparable reason — a measurement
  taken against a null context, an assertion that only checks a function exists
  or a string is non-empty where the intent was to check a value, a test whose
  fixture cannot reach the branch it claims to cover. This is the one section
  where you are auditing your own prior work, so be harder on it than on the
  docs.

- **Known accepted drift.** Record the `120.0` bar-width literal duplicated
  across `render_window_row` and the two-line per-model renderer, as an
  accepted risk with the reason it was accepted, so it is in the record rather
  than only in a code comment. Add anything else of that class you know of.

METHOD NOTE: grep is your inventory tool, but a claim is only verified when you
have checked the thing it asserts. "SECURITY.md says X" is not a finding;
"SECURITY.md:112 says X, and `<the specific check I ran>` shows not-X" is. Every
finding carries the check that established it. A finding without a check is a
suspicion, and suspicions go in a separate clearly-labelled section at the end.

## DO NOT

- Run the app or CLI against real credentials. `--help` is fine; `--once` is
  not. Never print, log, or persist token material — key *names* only (§4.4).
- Capture the owner's screen (§4.5).
- Touch any §4.1 path, in either phase.
- Create any file other than the report.
- Start the rename, edit `.github/`, or resolve any finding.
- Open a PR.

## TESTS

Phase 0 re-verification is the test gate: `cargo clean -p usage-core`, then
fmt-check, `build --locked`, `clippy --workspace --all-targets -D warnings`,
`test --workspace` at 138 passing. Phase 1 adds no code and so needs no tests —
if you find yourself writing a test in Phase 1, you have started fixing
something.

## VERIFY + SHIP

- Report is markdown, LF-only, no CR bytes.
- After Phase 1's commit, `git status` is clean and `git log --oneline -3` shows
  your three commits with 7e72282 as their parent.
- `git diff --check` clean.
- Push. CI green — the report changes no code, so any CI failure is a §4.6
  infrastructure problem, not a code problem, and you report it as such rather
  than editing code to appease it.

## END GATE — STOP

Report:

1. The three commit SHAs and the CI run.
2. Finding count by severity, and the count of §4.1-touching fixes implied.
3. The three findings you judge most likely to embarrass this project on day
   one of being public — your judgment, argued, not just the three
   highest-severity ones.
4. Anything in the audit that surprised you, especially in the
   test-harness-blind-spots section.
5. Any place where the gap report and `prompts/m6-ship-program.md` disagree.
   The program document was authored without running this audit; if it asserts
   something your checks contradict, that disagreement is the single most
   valuable thing you can report.

Then STOP. Do not fix anything. Do not start the rename. Do not touch
`.github/`. M5a's visual acceptance and every decision in the program document
are the owner's.
