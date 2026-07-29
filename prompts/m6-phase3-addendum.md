# Owner go-ahead addendum: cosign pin (v3) + Phase 3 — authorized 2026-07-28

Authored at the top tier. This addendum rides with the owner's Phase 3
go-ahead for m6-release.md and adds ONE pre-step. Same session may
execute it.

PRE-STEP (§4a): the complete release.yml v3 differs from the verified v2
by exactly one block — pinning the cosign binary to v3.0.6, the version
the rc.2 run verified. Transcribe from §W below, verify md5
`d639f2dcb33a50878c37f13d382cfb52` (5936 bytes), read the diff (one added `with:` block on the
cosign-installer step, plus its comment; NOTHING else), commit:
    ci: pin cosign binary to v3.0.6 (the rc.2-verified version)
Push. CI green.

THEN PHASE 3 of m6-release.md, as written, with these bindings:
- Tag v1.0.0 on the pin commit (it contains 63b40d7, so the shipped
  archives carry the corrected README — the floor's own finding).
- Re-verify ALL SIX steps from scratch against the v1.0.0 draft,
  including negative controls; the rc.2 verification does not transfer.
- Delete the rc.2 tag and its draft only after v1.0.0 verifies.
- Hand the owner the draft URL. YOU DO NOT PUBLISH.

## §W — release.yml v3 (5936 bytes, md5 d639f2dcb33a50878c37f13d382cfb52)

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
        with:
          # Pin the cosign BINARY too, not just the installer action: the
          # default floats with upstream, and cosign 3.0 already broke the
          # v2-era sign-blob flags once (rc.1). v3.0.6 is the exact version
          # the v1.0.0-rc.2 run verified end to end. Bump deliberately, with
          # a fresh rc dry run.
          cosign-release: 'v3.0.6'
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
