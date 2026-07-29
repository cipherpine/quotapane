# Goal prompt spec: M7B-RELEASE — v1.2.0 (Cipher Pine visual pass)

Authored at the top tier 2026-07-29. The owner visually accepted the
M7b look (round 2: calmed grid, brightened type, theme toggle) on
2026-07-29 — the paste of this spec's launcher IS the acceptance
record (§4.5, §4.8). Font embedding and further polish are explicitly
deferred to a later slice; they are not in this release.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
This spec runs the exact v1.0.0/v1.1.0 release discipline. `.github/`
is never touched. Phase 4 carries an exact §4a DECISIONS.md patch.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject is "docs(prompts): add M7B-RELEASE spec (v1.2.0)"; its
   parent is e550ef5 (the screenshots commit). Tree clean. CI 7/7
   green on the tip. Workspace version 1.1.0.
P2 DECISIONS.md contains "**M7b Cipher Pine visual pass — underway
   2026-07-29**" and no M7b acceptance stamp.
P3 Tags are exactly v1.0.0 and v1.1.0. v1.1.0 is the Latest release.

## PHASE 1 — version + CHANGELOG (one commit)

Bump the workspace version 1.1.0 → 1.2.0 in the root Cargo.toml.
Cargo.lock may move ONLY the three workspace members. Insert this
CHANGELOG.md entry above [1.1.0], verbatim, re-wrapped to the file's
80-column width, LF only:

## [1.2.0] - 2026-07-29

The Cipher Pine visual pass.

### Added

- **Cipher Pine theme (the default).** Near-black ground, a faint
  blueprint grid, monospace type, "//" section headers, and a
  "> quotapane" titlebar with a block status cursor — solid when idle
  and fresh, blinking only while the first poll is in flight or any
  provider's data is stale.
- **A theme toggle.** A tray-menu item switches live between Cipher
  Pine and a plain look and remembers the choice as a single word
  (`plain` or `cipherpine`) in `theme.cfg` under the platform config
  directory. `--plain` / `--themed` choose per run without writing
  the file. The file stores nothing but that word.
- **Window and tray icons, painted in code.** The window icon is the
  QuotaPane mark; the tray icon is a live miniature of it whose two
  bars track your Claude and Codex headline usage.
- README: brand banner and window screenshots.

### Changed

- **Bar colors are semantic in both themes**: pine below 50%, amber
  from 50%, cardinal from 80% (previously green until 80%, red at
  95%). Bars read amber earlier by design.
- **Staleness threshold 15 min → 10 min.** The window flags stale
  data sooner.
- `quotapane-cli --json` output is unchanged in this release: no key
  was added, removed, or renamed.

Commit: release: 1.2.0 — Cipher Pine visual pass and theme toggle

Full bar first: cargo clean -p usage-core, fmt-check, build --locked,
clippy -D warnings, test (expect the M7B-R1 count). Push; CI 7/7
green BEFORE any tag.

## PHASE 2 — rc dry run, then HARD STOP

Tag v1.2.0-rc.1 on the Phase 1 commit. Release run 3/3 green,
release.yml untouched. Then the six-step outsider verification
against the rc draft in a clean directory (download, sha256sum -c,
cosign verify-blob --bundle with the README command verbatim,
gh attestation verify, extract + inventory both archives, run the
shipped Windows CLI --once-less: --help and --version equivalents),
plus the six negative controls (one-byte corruption, wrong identity,
wrong issuer, tampered SHA256SUMS, wrong repo, unrelated file), each
restored and re-verified clean. HARD STOP: report everything and
wait. You do not publish; you do not tag v1.2.0.

## PHASE 3 — on the top tier's explicit go-ahead only

Tag v1.2.0 on the same verified commit. Re-run all six steps fresh
against the v1.2.0 draft. Only after it verifies clean, delete the rc
tag and rc draft. Hand back the draft URL and STOP — the owner
publishes.

## PHASE 4 — after the owner confirms publication (one commit)

§4a byte-match patch, replace exactly once:
OLD: **M7b Cipher Pine visual pass — underway 2026-07-29**:
NEW: **M7b Cipher Pine visual pass ✅ (v1.2.0 published — owner-accepted 2026-07-29)**:
DECISIONS.md is the only file in the commit.
Commit: docs: v1.2.0 published; M7b accepted (owner)
Push, CI 7/7 green, then STOP. Nothing further is queued.

## DO NOT

Publish anything. Touch .github/, assets/, README.md, or any §4.1
path (Phase 4's DECISIONS patch is the sole exception, verbatim).
Change any code — this is a release of what is already accepted. Add
any dependency. Print or persist token material (§4.4).
