# Goal prompt spec: M6-CI — supply-chain workflows + §4.1 code corrections

Authored at the top tier (Cowork bridge) 2026-07-27, from
`prompts/m6-pin-report.md` (59bb302) and the gap report (76b6421). This is
**Prompt D** of the M6 program; it supersedes the "not written yet"
placeholder section in `prompts/m6-prompts-b-to-g.md`.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): route (b) throughout. Every byte that lands in a §4.1 path
was authored at the top tier. The two Rust files are ALREADY ON DISK in
your working tree, placed over the Cowork bridge. The two WORKFLOW files
could not be placed remotely (the bridge protects .github/workflows — a
good guard), so their COMPLETE contents are embedded verbatim in §W below;
your first act is to write them to disk byte-exactly, then verify the md5
table. That is §4a.1's "supplied to the session verbatim — as full file
contents": transcribing them is landing pre-authored bytes, not authoring.
Then: verify all four hashes, confirm no OTHER protected-path bytes
changed, build and test, commit each file with the message given, push
once, watch CI. You author nothing. If fmt or clippy would change even one
byte of a pre-authored file, that is a MISMATCH: STOP and hand back — do
not "fix" the top tier's bytes.

CONTEXT (owner decisions 2026-07-27, folded in): the public flip moves
BEFORE the v1.0.0 tag (attestations are public-repo-only on this plan), so
release.yml carries no visibility conditionals — by tag time the repo is
public. Gitleaks runs as the pinned MIT release BINARY verified by SHA256,
not the proprietary-EULA action. New gate order: D → E → G(scan+flip) → F.

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is 59bb302, CI green, and `git status --porcelain` shows
   EXACTLY these four entries (the workflow files appear only after you
   write them in §W):

   ```
    M crates/usage-core/src/egress/mod.rs
    M crates/usage-core/src/poller/mod.rs
    M prompts/m6-launchers.md
   ?? prompts/m6-supply-chain.md
   ```

P2 (checked AFTER §W) md5 of each pre-authored file matches this table
   exactly:

   | file | md5 |
   |---|---|
   | `.github/workflows/ci.yml` | `f7458a016596f26865cc951ffcff5def` |
   | `.github/workflows/release.yml` | `c13fc7117c5dd0ad17f0186fc122cb20` |
   | `crates/usage-core/src/egress/mod.rs` | `e2de23067cc4f359f30471b74fdf69aa` |
   | `crates/usage-core/src/poller/mod.rs` | `48d733c93f9de870221f94c28ad1bae4` |

   (`prompts/m6-launchers.md` and this file are non-§4.1; no hash gate,
   commit verbatim.)

## §W — MATERIALIZE THE WORKFLOW BYTES (do this first)

Write the following two files exactly as given — LF line endings, one
trailing newline, no BOM, not one byte of adjustment. The md5 table in P2
is the arbiter; if your written file hashes differently, re-transcribe
rather than diagnose.

`.github/workflows/ci.yml` (full replacement — 4135 bytes):

````yaml
# CI — every gate here is part of the security posture (SECURITY.md).
# Weakening or removing a job is a security-relevant change and must be
# called out in review.
#
# Supply-chain note: actions are pinned by major tag and kept current by
# Dependabot (.github/dependabot.yml). For release workflows (M6), pin by
# full commit SHA.
name: CI

on:
  push:
    branches: [main]
  pull_request:

permissions:
  contents: read

jobs:
  test:
    name: build & test (${{ matrix.os }})
    strategy:
      fail-fast: false
      matrix:
        os: [windows-latest, macos-latest, ubuntu-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: Build (locked)
        run: cargo build --workspace --locked
      - name: Test (locked) — includes the egress-allowlist and redaction/zeroize invariant tests
        run: cargo test --workspace --locked
      - name: Format
        run: cargo fmt --all --check
      - name: Clippy
        run: cargo clippy --workspace --all-targets --locked -- -D warnings

  deny:
    name: cargo-deny (licenses, bans, advisories, sources)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: EmbarkStudios/cargo-deny-action@v2
        with:
          command: check

  audit:
    name: cargo-audit (RustSec advisories)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-audit
        run: cargo install cargo-audit --locked
      - name: Audit
        run: cargo audit

  no-telemetry:
    # SECURITY.md invariant 4 / THREAT_MODEL.md §9 row 4: no first-party
    # telemetry exists in the codebase. This job fails if an analytics
    # dependency or telemetry endpoint string appears anywhere.
    name: invariant 4 — no telemetry
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - name: No analytics/telemetry dependencies
        run: |
          set -euo pipefail
          PATTERN='sentry|posthog|segment|rudderanalytics|amplitude|mixpanel|datadog|newrelic|bugsnag'
          if grep -rInE "$PATTERN" --include='Cargo.toml' .; then
            echo '::error::telemetry/analytics dependency detected (SECURITY.md invariant 4)'
            exit 1
          fi
      - name: No telemetry endpoints in source
        run: |
          set -euo pipefail
          PATTERN='ingest\.sentry|posthog\.com|api\.segment|api\.amplitude|api\.mixpanel'
          if grep -rInE "$PATTERN" --include='*.rs' crates/; then
            echo '::error::telemetry endpoint detected in source (SECURITY.md invariant 4)'
            exit 1
          fi

  gitleaks:
    # SECURITY.md supply-chain policy: secret scanning runs in CI over the
    # FULL history, not just the tip — a secret deleted at the tip is still
    # a leak. Uses the MIT-licensed gitleaks release BINARY verified against
    # a pinned SHA256 (pin report §5: stronger than an action tag, no vendor
    # license logic running in CI). To bump: update GITLEAKS_VERSION and
    # GITLEAKS_SHA256 together, from the release's checksums file.
    name: gitleaks — full-history secret scan
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0 # full history, every ref reachable from this SHA
      - name: Download and verify gitleaks
        env:
          GITLEAKS_VERSION: "8.30.1"
          GITLEAKS_SHA256: "551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb"
        run: |
          set -euo pipefail
          curl -sSfL -o gitleaks.tar.gz "https://github.com/gitleaks/gitleaks/releases/download/v${GITLEAKS_VERSION}/gitleaks_${GITLEAKS_VERSION}_linux_x64.tar.gz"
          echo "${GITLEAKS_SHA256}  gitleaks.tar.gz" | sha256sum -c -
          tar -xzf gitleaks.tar.gz gitleaks
      - name: Scan full history
        # --redact: a finding must never print candidate secret bytes into
        # a CI log (the log would out-leak the leak).
        run: ./gitleaks git --no-banner --redact .
````

`.github/workflows/release.yml` (new file — 4946 bytes):

````yaml
# Release pipeline (M6) — tag-triggered, builds ONLY in CI (SECURITY.md:
# release artifacts are never built on a maintainer's machine), uploads to a
# DRAFT release. Publishing the draft is a human act, always.
#
# Supply-chain: actions are pinned by full commit SHA per the policy note in
# ci.yml's header; the tag each SHA matched at authoring time is recorded in
# a trailing comment. Verify any pin with:
#   gh api repos/<owner>/<repo>/git/ref/tags/<tag>
# dtolnay/rust-toolchain is pinned to the commit its `stable` branch pointed
# at on 2026-07-27 (pin report §3.2 — the only tag, v1, is ~11 months
# stale). Pinning the ACTION does not pin the TOOLCHAIN: the exact rustc and
# cargo versions used are captured into each archive's TOOLCHAIN.txt at
# build time (SECURITY.md — the exact toolchain is documented per release).
#
# This workflow assumes a PUBLIC repository at tag time (owner decision
# 2026-07-27: the public flip precedes the first v1.0.0 tag, because
# attestations are public-repo-only on this plan).
name: Release

on:
  push:
    tags: ["v*"]

# Deny-by-default at the workflow level; each job asks for exactly what it
# needs. id-token:write (OIDC for cosign keyless + provenance) and
# attestations:write are deliberately scoped to the release job only.
permissions: {}

jobs:
  build:
    name: build (${{ matrix.target }})
    permissions:
      contents: read
    strategy:
      fail-fast: true
      matrix:
        include:
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            archive: zip
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            archive: tar.gz
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1
      - uses: dtolnay/rust-toolchain@4cda84d5c5c54efe2404f9d843567869ab1699d4 # stable branch head, 2026-07-27
      - name: Build (locked, release)
        run: cargo build --workspace --release --locked
      - name: Record toolchain (SECURITY.md — exact toolchain per release)
        shell: bash
        run: |
          set -euo pipefail
          { rustc -V; cargo -V; } | tee TOOLCHAIN.txt
      - name: Package
        shell: bash
        run: |
          set -euo pipefail
          VERSION="${GITHUB_REF_NAME#v}"
          STAGE="quotapane-v${VERSION}-${{ matrix.target }}"
          mkdir "$STAGE"
          if [ "${{ matrix.archive }}" = "zip" ]; then
            cp target/release/quotapane.exe target/release/quotapane-cli.exe "$STAGE"/
          else
            cp target/release/quotapane target/release/quotapane-cli "$STAGE"/
          fi
          cp LICENSE-MIT LICENSE-APACHE README.md TOOLCHAIN.txt "$STAGE"/
          if [ "${{ matrix.archive }}" = "zip" ]; then
            7z a "${STAGE}.zip" "$STAGE"
          else
            tar -czf "${STAGE}.tar.gz" "$STAGE"
          fi
      - uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1
        with:
          name: archive-${{ matrix.target }}
          path: quotapane-v*
          if-no-files-found: error
          retention-days: 7

  release:
    name: checksum, sign, attest, draft
    needs: build
    runs-on: ubuntu-latest
    permissions:
      contents: write # create the DRAFT release and upload assets
      id-token: write # OIDC identity: cosign keyless signing + provenance
      attestations: write # actions/attest-build-provenance
    steps:
      - uses: actions/download-artifact@3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c # v8.0.1
        with:
          path: dist
          merge-multiple: true
      - name: SHA256SUMS
        run: |
          set -euo pipefail
          cd dist
          sha256sum quotapane-v* > SHA256SUMS
          cat SHA256SUMS
      - uses: sigstore/cosign-installer@6f9f17788090df1f26f669e9d70d6ae9567deba6 # v4.1.2 (no major tag exists — pin report §3)
      - name: Sign SHA256SUMS (cosign keyless — one signature covers every artifact)
        run: |
          set -euo pipefail
          cd dist
          cosign sign-blob --yes \
            --output-signature SHA256SUMS.sig \
            --output-certificate SHA256SUMS.pem \
            SHA256SUMS
      - name: Attest build provenance
        uses: actions/attest-build-provenance@0f67c3f4856b2e3261c31976d6725780e5e4c373 # v4.1.1
        with:
          subject-path: |
            dist/quotapane-v*.zip
            dist/quotapane-v*.tar.gz
      - name: Draft release (never auto-published)
        env:
          GH_TOKEN: ${{ github.token }}
        run: |
          set -euo pipefail
          gh release create "${GITHUB_REF_NAME}" \
            --repo "${GITHUB_REPOSITORY}" \
            --draft \
            --verify-tag \
            --title "QuotaPane ${GITHUB_REF_NAME}" \
            --generate-notes \
            dist/quotapane-v*.zip dist/quotapane-v*.tar.gz \
            dist/SHA256SUMS dist/SHA256SUMS.sig dist/SHA256SUMS.pem
````

## WHAT EACH CHANGE IS — so your diff review is informed, not ritual

1. **ci.yml** — adds a fifth job, `gitleaks`: full-history secret scan
   (`fetch-depth: 0`), gitleaks **v8.30.1** release binary downloaded and
   verified against its published SHA256 (both pinned in-file, from the pin
   report §5), run with `--redact` so a finding never prints candidate
   secret bytes into a CI log. The file also normalizes to LF on disk (it
   carried CRLF; `.gitattributes` already normalized the index, so the
   committed diff must be the job addition ONLY — confirm that in the diff).
2. **release.yml** — NEW. Triggers on `v*` tags. Two-target matrix
   (windows-msvc + linux-gnu, per D6), `--locked` release builds,
   `TOOLCHAIN.txt` (rustc -V + cargo -V) inside every archive
   (SECURITY.md's per-release toolchain promise), archives
   `quotapane-v<ver>-<target>.{zip,tar.gz}` carrying both binaries + both
   licenses + README, `SHA256SUMS` over all archives, cosign keyless
   `sign-blob` of SHA256SUMS (one signature covers everything),
   `attest-build-provenance` on the archives, upload to a **draft** release
   — publishing is a human act, always. Workflow-level `permissions: {}`;
   `contents: read` for build; `contents/id-token/attestations: write`
   scoped to the release job only. Third-party actions pinned by full
   commit SHA with the matching tag in a trailing comment (pin report §2);
   dtolnay/rust-toolchain pinned to the commit `stable` pointed at
   2026-07-27 (§3.2).
3. **egress/mod.rs** — removes `api.github.com` from `ALLOWED_HOSTS` (zero
   callers in the workspace — the update check it existed for is
   unimplemented; gap report A4) and adds `api.github.com` to the
   denial-test's host list so the removal is pinned by a test. The headline
   control is now exactly TWO hosts wide. This narrows egress; it weakens
   nothing.
4. **poller/mod.rs** — replaces the vacuous invariant-2 test the gap report
   flagged (test-harness blind spots): `FailingProvider` (unit-variant
   error that structurally could not carry a token) becomes
   `LeakyFailingProvider`, which holds a `Secret<String>` sentinel and
   deliberately interpolates it into its error. The test now proves the
   forwarded message contains the `«redacted»` marker and NOT the sentinel
   bytes, and is exactly the error's Display output — the redaction
   invariant, end to end through the poller.

## EXECUTE — five commits in this order, one push at the end

0. `prompts/m6-supply-chain.md` (this file) + `prompts/m6-launchers.md`:
       docs(prompts): add M6-CI spec and launcher (Prompt D)
1. `.github/workflows/ci.yml`:
       ci: add gitleaks full-history secret scan (pinned binary, SHA256-verified)
2. `.github/workflows/release.yml`:
       ci: add release workflow — build, checksum, cosign sign, attest, draft
3. `crates/usage-core/src/egress/mod.rs`:
       security: remove unused api.github.com from the egress allowlist
4. `crates/usage-core/src/poller/mod.rs`:
       test: prove failure-path redaction end to end (closes gap-report blind spot)

## TESTS (before the push)

`cargo clean -p usage-core`, then `cargo fmt --all --check` →
`cargo build --workspace --locked` → `cargo clippy --workspace
--all-targets --locked -- -D warnings` → `cargo test --workspace`.
Expect the SAME total as at 81bc17b (147): the poller test was replaced,
not added. If fmt-check or clippy objects to a pre-authored file: STOP
(that is a byte mismatch by definition), report, hand back.

## VERIFY + SHIP

- Push. CI green — now SIX jobs; name all six in your report.
- The gitleaks job's FIRST run scans all history. Three possible outcomes:
  **green** (report it — this is early input to owner decision D7);
  **findings** (STOP; report each as file + commit + rule id, REDACTED,
  never the candidate value — §4.4; do NOT add any allowlist or gitleaks
  config to appease it, that is top-tier work); **infrastructure failure**
  (§4.6 — one honest look at the logs, then stop and report).
- release.yml does not run on push (tag-triggered). Confirm the Actions tab
  shows no workflow-parse error for it. Its real test is Prompt F's rc tag.
- `git diff HEAD~5..HEAD -- crates/usage-core/src/credentials .cargo
  .claude deny.toml SECURITY.md THREAT_MODEL.md` → empty.

## DO NOT

Edit any pre-authored byte; add a gitleaks config; touch credentials/**;
run anything against real credentials; tag anything; open a PR.

## END GATE — STOP

Report: five SHAs; the CI run with all six job results; explicitly the
gitleaks first-run outcome; the workspace test count. Do not start Prompt
E — its SECURITY.md/THREAT_MODEL.md bytes are authored at the top tier
only after this lands, because E makes claims true that D just built.
