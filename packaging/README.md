# WinGet manifests for `CipherPine.QuotaPane`

**This file lives one level above `winget/` on purpose.** WinGet parses
*every* file in a manifest directory as YAML, so a markdown README inside
`packaging/winget/` breaks `winget validate`, `winget install --manifest`,
and `wingetcreate submit` alike (found the hard way — reports/m18a-winget.md
F1; the failing token was this file's own first backtick). The manifest
directory must contain the three YAML files and nothing else, ever.

The three-file manifest set for QuotaPane 1.7.0: version, installer, and
`en-US` defaultLocale. `InstallerType: zip` with `NestedInstallerType:
portable`, which puts both `quotapane` and `quotapane-cli` on PATH — the
honest shape for a package whose archive holds two self-contained binaries
and has nothing to uninstall.

These files live here, in this repo. **Nothing here has been submitted to
`microsoft/winget-pkgs`** — see below.

## How these were validated

1. **The archive's hash was cross-checked, not trusted.** The release's own
   `SHA256SUMS` was downloaded alongside the Windows asset, an independent
   `sha256sum` was computed over the downloaded archive, and the two were
   compared (`sha256sum -c SHA256SUMS --ignore-missing` → `OK`) before the
   value was copied into `InstallerSha256`. Computing a hash from a download
   and pasting it in would prove only that the download was internally
   consistent with itself.

   ```
   5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f
     quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
   ```

   The stronger check is available to anyone: `SHA256SUMS` also ships a
   `cosign` keyless signature (`SHA256SUMS.sigstore.json`) and each archive
   carries a build-provenance attestation. See "Verify a release" in the
   root `README.md`.

2. **The nested paths were read out of the archive, not guessed.** The zip
   unpacks into a single versioned directory, so both `RelativeFilePath`
   entries are prefixed with `quotapane-v1.7.0-x86_64-pc-windows-msvc\`.
   That prefix carries the version and therefore changes every release.

3. **The manifest set was validated locally** with WinGet v1.29.280:

   ```powershell
   winget validate --manifest packaging\winget
   ```

   → `Manifest validation succeeded.`

   Validation checks schema and internal consistency. It does **not**
   download the installer or exercise the install, so the first real proof
   that the portable aliases land correctly is an actual install — either
   locally with `winget install --manifest packaging\winget`, or in the
   `winget-pkgs` CI that runs on a submission.

## Submitting these upstream — the owner's act

A first submission to `microsoft/winget-pkgs` publishes under a GitHub
identity and agrees to that repo's contribution terms on behalf of the
publisher. That is a person's decision, not an automated one, so this
session deliberately stopped at validated files on disk. Either route
below works:

**Fork and PR.** Fork `microsoft/winget-pkgs`, copy these three files to
`manifests/c/CipherPine/QuotaPane/1.7.0/`, and open a PR. The path is
derived from the package identifier and must match it exactly. The repo's
CI validates the manifest, downloads the installer, and installs it in a
sandbox; a moderator reviews from there.

**`wingetcreate`.** `winget install wingetcreate`, then:

```powershell
wingetcreate submit --prtitle "New package: CipherPine.QuotaPane version 1.7.0" packaging\winget
# First run: GitHub device-flow auth, creates your winget-pkgs fork, and the
# PR body carries the repo's contribution terms. Do NOT pass --token on the
# command line — wingetcreate's own help warns it may be logged. wingetcreate
# also has telemetry on by default; `wingetcreate settings` can turn it off.
```

which forks, commits, and opens the PR in one step. A token with `public_repo`
scope is enough.

### Before either

- The publisher identity in `CipherPine.QuotaPane.locale.en-US.yaml` —
  `Publisher: Cipher Pine` — is what appears in `winget show`. Change it
  there if it should read otherwise.
- `winget-pkgs` expects the package identifier to reflect the publisher.
  `CipherPine.QuotaPane` matches the `cipherpine` GitHub org that owns the
  release URLs, which is the association a moderator will look for.
- Consider installing from the local manifest once first — it is the only
  check here that actually runs the thing.

### For the next release

Three values change, and all three are version-derived: `PackageVersion`
(all three files), the `InstallerUrl` and `InstallerSha256`, and the version
prefix inside both `RelativeFilePath` entries. `ReleaseDate` should be
updated too. Automating this from the release workflow would mean giving CI
a token that can push to a fork of `winget-pkgs`, which is a decision for
whoever owns that account, not a mechanical follow-up.
