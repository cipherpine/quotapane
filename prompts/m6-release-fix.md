# Goal prompt spec: M6-RELEASE-FIX — release.yml v2, then resume F (F½)

Authored at the top tier (Cowork bridge) 2026-07-28, from the rc.1 failure
evidence (run 30399454524) and the cosign v3 interface, verified against
sigstore's own docs. Two defects in the top-tier-authored release.yml,
both mine, both fixed here:

- **A — upload glob matched the staging directory.** `path: quotapane-v*`
  captured the staging DIR alongside the archive; download-artifact
  recreated it in dist/ and `sha256sum` failed with "Is a directory"
  (exactly what the rc.1 log shows). v2 removes the staging dir after
  archiving, uploads `quotapane-v*.${{ matrix.archive }}` only, and
  checksums explicit extensions.
- **B — cosign v3 removed the signing flags.** `--output-signature` /
  `--output-certificate` are gone in cosign 3.x (which the pinned
  installer installs); v2 signs with `--bundle SHA256SUMS.sigstore.json`,
  and the release ships that bundle instead of .sig/.pem. Keyless verify
  keeps the identity flags, now with `--bundle`.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): route (b). The complete release.yml v2 is embedded in §W
below — transcribe byte-exactly, verify the md5, commit under §4a. You
author nothing in `.github/**`. This spec also AUTHORIZES two things the
m6-release.md spec forbade, narrowly: deleting the rc.1 tag (it points at
a failed run) and re-tagging as rc.2.

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is c12c640 (`release: 1.0.0 — version bump and CHANGELOG`),
   the only tag is `v1.0.0-rc.1`, and the tree is clean EXCEPT:
   ```
    M prompts/m6-launchers.md
   ?? prompts/m6-release-fix.md
   ```
P2 (after transcription) md5 of `.github/workflows/release.yml` is
   `d22c95afa99f086036acfd125a3efe77`.

## EXECUTE

0. Commit this spec + the launcher index:
       docs(prompts): add M6-RELEASE-FIX spec (F½ — release.yml v2)
1. Transcribe §W over `.github/workflows/release.yml`, verify P2, read
   the diff (every hunk must be one of the two fixes or their comments),
   commit:
       ci: release workflow v2 — upload archives only; cosign v3 bundle signing
2. Push both commits. CI green (release.yml is tag-triggered; confirm no
   workflow-parse error in the Actions tab).
3. Clean up rc.1: `git push origin :refs/tags/v1.0.0-rc.1` and
   `git tag -d v1.0.0-rc.1`. Delete any draft release rc.1 left (the run
   failed before `gh release create`, so there should be none — verify
   with `gh release list` rather than assume). Delete the two stale
   artifacts of run 30399454524 if `gh api` permits; otherwise note they
   expire in 7 days by design.
4. **Resume m6-release.md PHASE 2 exactly as written**, with `v1.0.0-rc.2`
   substituted for rc.1 everywhere, and one addition to its README step:
   the verify commands change shape under cosign v3 — the release now
   ships `SHA256SUMS.sigstore.json` (no .sig/.pem), and verification is
   `cosign verify-blob --bundle SHA256SUMS.sigstore.json
   --certificate-identity-regexp 'github.com/cipherpine/quotapane'
   --certificate-oidc-issuer https://token.actions.githubusercontent.com
   SHA256SUMS`. README's "Verify a release" section MUST be corrected to
   the bundle form, from your actual transcript (this was always the F
   spec's rule; the shape change makes it certain to trigger).
   SECURITY.md names no signature filenames and needs no edit — if you
   believe it does, that is a §4.1 STOP, not an edit.

## DO NOT

Edit any workflow byte beyond transcribing §W; touch SECURITY.md or
THREAT_MODEL.md; tag v1.0.0 (phase 3 remains owner-gated in
m6-release.md); publish anything; delete any tag other than rc.1.

## END GATE — STOP (same as m6-release.md phase 2)

Report: the fix commits; the rc.2 run; the full six-step outsider
verification transcript (checksums, bundle verify with identity flags,
attestation with commit-SHA match, archive contents, --help run); the
README correction diff; and your confidence that a stranger following
README verbatim reproduces your result. Then wait for the owner's
phase-3 go-ahead.

## §W — release.yml v2 (5553 bytes, md5 d22c95afa99f086036acfd125a3efe77)

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
          # rc.1 lesson: the staging DIRECTORY must not survive to match any
          # later quotapane-v* glob (sha256sum: "Is a directory").
          rm -rf "$STAGE"
      - uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a # v7.0.1
        with:
          name: archive-${{ matrix.target }}
          path: quotapane-v*.${{ matrix.archive }}
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
          sha256sum quotapane-v*.zip quotapane-v*.tar.gz > SHA256SUMS
          cat SHA256SUMS
      - uses: sigstore/cosign-installer@6f9f17788090df1f26f669e9d70d6ae9567deba6 # v4.1.2 (no major tag exists — pin report §3)
      - name: Sign SHA256SUMS (cosign v3 keyless — one bundle covers every artifact)
        # cosign v3 removed --output-signature/--output-certificate; the
        # sigstore bundle (.sigstore.json) carries both signature and
        # certificate. Verify with:
        #   cosign verify-blob --bundle SHA256SUMS.sigstore.json \
        #     --certificate-identity-regexp 'github.com/cipherpine/quotapane' \
        #     --certificate-oidc-issuer https://token.actions.githubusercontent.com \
        #     SHA256SUMS
        run: |
          set -euo pipefail
          cd dist
          cosign sign-blob --yes \
            --bundle SHA256SUMS.sigstore.json \
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
            dist/SHA256SUMS dist/SHA256SUMS.sigstore.json
````
