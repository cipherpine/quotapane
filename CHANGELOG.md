# Changelog

All notable changes to QuotaPane are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-07-28

First stable release.

QuotaPane is a small, always-on-top desktop window that shows how much of your
Claude and Codex **subscription** quota is left. It reads the credential files
the official `claude` / `codex` CLIs already wrote on your own machine, asks the
providers what your remaining quota is, and draws the answer. There is no
account to create, no server in the middle, and nothing reported back to anyone.

The point of the project is the size of its trust boundary: exactly two modules
ever touch a credential or the network, and they are small enough to read end to
end in one sitting. Everything else is scheduling and rendering.

Scope for 1.0 is subscription quota only. Official admin/billing usage APIs are
deliberately out of scope — they require an organization admin key, which
individual subscribers do not have and which would undermine the trust boundary
this project exists to keep small.

### Added

- **Always-on-top window.** A slim titlebar with minimize and close, drag
  anywhere to position it, and scrolling when the content is taller than the
  window.
- **Claude (Anthropic).** The 5-hour and 7-day subscription windows: percent
  used, and how long until each one resets.
- **Codex (OpenAI).** The rate-limit windows the Codex endpoint reports,
  labelled by their duration — typically a short rolling window plus a 7-day
  one — with the same percent and reset countdown.
- **Per-model breakdown.** Where a provider reports per-model limits, each
  provider pane has a collapsed-by-default toggle that expands them into their
  own rows (M5a).
- **Staleness reporting.** When the data is older than it should be, the window
  says so instead of quietly showing you a stale number.
- **System tray icon** rendering current usage, with a tooltip and a
  Show/Hide/Quit menu, on Windows and macOS. `--no-tray` starts without it if
  tray creation fails (M3.5).
- **Headless CLI.** `quotapane-cli --once` prints the same normalized snapshot
  as text, or as JSON with `--json` — for scripts, for cron, and for proving to
  yourself exactly what the app talks to. Also accepts
  `--provider claude|codex|all`, `--client-version <VER>`, `--debug-raw`,
  `--help`, and `--version`.
- **Two binaries** in every archive: `quotapane` (the window) and
  `quotapane-cli` (headless).
- **Poll discipline.** A hard floor of 180 seconds between polls per provider,
  exponential backoff capped at 30 minutes, and `retry-after` is honored.
- **Platform support.** Windows x86-64 and Linux x86-64 (glibc) ship as
  prebuilt archives; macOS is supported by building from source.

### Security

- **Egress is deny-by-default** through a single chokepoint with a
  compile-time allowlist of exactly two hosts, `api.anthropic.com` and
  `chatgpt.com`. Any other host is a hard error, and tests prove it.
- **Tokens are never persisted, logged, or serialized.** They exist only in
  memory inside a `Secret<T>` that zeroizes on drop and prints `«redacted»` in
  `Debug` output. On-disk config holds preferences only. The redaction is
  tested on failure paths, not just happy ones.
- **No first-party telemetry**, of any kind, to anyone. CI enforces its absence
  on every push.
- **No auto-update and no update check.** There is no updater in the codebase
  at all; updating is always something you do deliberately.
- **Credential files are opened read-only.** QuotaPane never writes them. When
  a token has expired it tells you to run `claude` or `codex` to refresh —
  refresh is delegated to the official CLIs, always.
- **Proxy support is opt-in**, behind an explicit warning that a TLS-inspecting
  proxy can observe your bearer token.
- The subscription endpoints used are **undocumented**, which the app discloses
  at runtime. It fails closed on schema drift rather than guessing, and sends
  official-client `User-Agent` strings — a deliberate, documented choice.

### Release integrity

- Release archives are **built only in CI** from a version tag, never on a
  maintainer's machine.
- Every release ships **`SHA256SUMS`** covering all archives, a **cosign
  keyless signature** over that file as a Sigstore bundle
  (`SHA256SUMS.sigstore.json`, carrying both the signature and the signing
  certificate) whose identity is this repository's release workflow rather than
  a long-lived key, and a **GitHub build-provenance attestation** on each
  archive.
- Each archive contains a **`TOOLCHAIN.txt`** recording the exact `rustc` and
  `cargo` versions that built it.
- Every GitHub Action used is **pinned by full commit SHA**.
- Releases are created as **drafts** and are never auto-published.

A passing signature proves an artifact came from this repository's CI at a
specific commit. It does not prove that commit was benign — see residual risk
R2 in `THREAT_MODEL.md`. For maximum assurance, build from source.

### Changed

- **Repository history was rewritten on 2026-07-28**, before the repository
  went public, to correct author and committer identity metadata across every
  commit — first the email address, then the display name. Only metadata
  changed: every tree hash in the history is byte-for-byte identical before and
  after, which is the invariant the rewrite was checked against. Because the
  commit SHAs did move, any SHA cited in a document written before that date
  resolves through the cumulative old→new map in
  `prompts/m6-sha-map-2.txt`; `prompts/m6-sha-map.txt` is retained only as the
  record of the intermediate state.

[1.0.0]: https://github.com/cipherpine/quotapane/releases/tag/v1.0.0
