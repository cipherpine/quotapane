# Changelog

All notable changes to QuotaPane are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.1] - 2026-08-01

A small patch: the expired-token experience now explains itself.

### Changed

- **The expired-token message names the exact refresh action.** Each pane
  says what to do — start any `claude` session (even `claude -p hi`), or run
  `codex login` — and that QuotaPane recovers on its own within ~3 minutes of
  the refresh: no restart, no clicks. The CLI prints the same hint. QuotaPane
  itself has no login and never writes credential files; refresh always
  happens in the provider's official CLI.
- **The at-risk pace line goes quiet when data is stale.** A burn-rate
  forecast extrapolated from stale data is misinformation; once a pane
  crosses the 10-minute staleness threshold the "at this pace: …" line is
  suppressed until fresh data arrives. The pace tick on each bar still
  draws — elapsed time is a fact of the clock, not of the data.

### Added

- **README FAQ** — "Why does it say my token expired?": what the message
  means, why QuotaPane cannot refresh the token itself, and how recovery
  works.

No JSON key changed in this release. Zero new dependencies.

## [1.4.0] - 2026-07-31

The security-review release: an owner-commissioned adversarial review of the
codebase and its claims produced eleven findings — most of them places where the
documentation said more than the code did. Every finding was independently
re-verified and every fix ships here, each behavior change landing in the same
commit as the claim it affects. The review process itself is committed under
`prompts/`.

### Security

- **`--debug-raw` now redacts by default.** Identifier fields (`email`,
  `user_id`, `account_id`, `id`) are replaced with `«redacted»` at any depth
  before printing; bodies that are not valid JSON are withheld. New
  `--debug-raw-unsafe` restores the byte-exact dump after one warning. Existing
  invocations keep working — safer.
- **Proxy support is now reachable, CLI-only, and fail-closed.** New
  `quotapane-cli --allow-proxy` opts a single run into the proxy environment
  after a printed warning that a TLS-inspecting proxy can observe the bearer
  token. Without it, behavior is unchanged: a set proxy variable (either casing)
  means nothing is sent and the run exits with an error naming the variable —
  now followed by a hint pointing at the flag. The window has no opt-in surface
  at all.
- **A hostile `Retry-After` can no longer stall polling.** The provider's hint
  is honored only up to the 30-minute backoff cap.
- **Expired tokens recover fast.** An auth error retries at the polling floor
  instead of escalating backoff, so a token refreshed via the official CLI is
  noticed within minutes, not up to thirty.
- **Provider timestamps are validated against the real calendar** (month
  lengths, leap years) and must carry a UTC offset; anything else is rejected
  and renders as unknown rather than as a wrong reset time.
- **Every CI action is now pinned to a full commit SHA**, matching the release
  workflow, and cargo-audit installs at a pinned version.

### Documentation truth

The review's larger half: claims aligned with shipped reality. The persistence
invariant now names `theme.cfg` (one word, preferences only) instead of claiming
"no files at all"; TLS validation is correctly described as the bundled WebPKI
(Mozilla) root set — the OS trust store is not consulted, so an OS-installed
interception CA is rejected rather than trusted, and the threat model's
mitigation advice was rewritten to match; zeroization claims are scoped to
buffers QuotaPane owns; the auditable surface honestly includes the provider
parsers and the CLI raw-debug path; and text-mode CLI output is documented as a
summary (per-model rows and reset credits appear in `--json` and the window). No
JSON key changed in this release.

## [1.3.0] - 2026-07-30

Pace: know whether you will make it to reset, not just where you are.

### Added

- **Pace ticks.** Every quota bar carries a thin tick at the elapsed share of
  its window. Fill short of the tick means you are under budget; fill past it
  means you are consuming faster than time is passing. Shown in both themes — it
  is information, not decoration.
- **Forecast-to-limit.** QuotaPane estimates your burn rate from recent polls;
  when a window's projected exhaustion lands before its reset, one line appears
  under that provider: "at this pace: <window> full in ~…" — amber, turning
  cardinal inside six hours. When you are safe it shows nothing at all.
  Forecasts wait for roughly fifteen minutes of evidence before speaking, and a
  window reset clears the history. Note that any 5h-window warning is
  necessarily inside the six-hour band, so session-window warnings are always
  cardinal.
- **`--pace-demo`** renders a fixed synthetic scenario — no polling, no
  credentials read, no network at all — to see the feature without waiting for a
  bad week.
- `quotapane-cli --json`: every window object (headline and per-model) gains
  `duration_secs` — always present, null when the provider neither stated nor
  implied a window length. This is the only JSON surface change in this release.

## [1.2.0] - 2026-07-29

The Cipher Pine visual pass.

### Added

- **Cipher Pine theme (the default).** Near-black ground, a faint blueprint
  grid, monospace type, "//" section headers, and a "> quotapane" titlebar with
  a block status cursor — solid when idle and fresh, blinking only while the
  first poll is in flight or any provider's data is stale.
- **A theme toggle.** A tray-menu item switches live between Cipher Pine and a
  plain look and remembers the choice as a single word (`plain` or `cipherpine`)
  in `theme.cfg` under the platform config directory. `--plain` / `--themed`
  choose per run without writing the file. The file stores nothing but that
  word.
- **Window and tray icons, painted in code.** The window icon is the QuotaPane
  mark; the tray icon is a live miniature of it whose two bars track your Claude
  and Codex headline usage.
- README: brand banner and window screenshots.

### Changed

- **Bar colors are semantic in both themes**: pine below 50%, amber from 50%,
  cardinal from 80% (previously green until 80%, red at 95%). Bars read amber
  earlier by design.
- **Staleness threshold 15 min → 10 min.** The window flags stale data sooner.
- `quotapane-cli --json` output is unchanged in this release: no key was added,
  removed, or renamed.

## [1.1.0] - 2026-07-29

Per-model quota truth, and Codex reset credits.

Anthropic changed the shape of its usage response: the per-model numbers moved
into a general `limits` array, and the two older per-model fields they replaced
now come back empty. QuotaPane read only those older fields, so the Claude
per-model breakdown had quietly gone blank. This release reads the new shape,
so model-scoped quotas appear again.

### Added

- **Codex reset credits.** The Codex pane shows `resets available: N` when the
  provider reports rate-limit reset credits — the allowance for clearing a rate
  limit early. `--json` carries the same data as `reset_credits`: `available`
  is how many you own, `applicable_now` is how many you could spend at this
  moment, which is normally `0` unless you are actually rate-limited. A
  provider with no such concept — Claude — reports `null` and shows no line.

### Fixed

- **Claude per-model quotas are visible again.** They are now read from the
  provider's generalized `limits` array: any entry scoped to a model becomes a
  per-model row, labelled with the provider's own name for that model. The
  older `seven_day_opus` / `seven_day_sonnet` fields are still read as a
  fallback for accounts that continue to send them, so nothing is lost either
  way.

### Changed

- **The window hides per-model buckets you have never touched.** Providers list
  every model on your plan, not only the ones you use, so an unused bucket
  spent two lines saying `0%` in a window that cannot be resized. Those rows
  are now hidden, and the per-model toggle disappears entirely when hiding them
  leaves nothing to show. The hiding itself is a display change only —
  `quotapane-cli --json` still reports every bucket the provider sent, zeroes
  included. The JSON surface does change elsewhere in this release:
  `reset_credits` is a new key on every snapshot, and Claude's per-model entries
  are named by the provider's display names rather than the old fixed
  opus/sonnet labels — both changes documented above.

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

[1.2.0]: https://github.com/cipherpine/quotapane/releases/tag/v1.2.0
[1.1.0]: https://github.com/cipherpine/quotapane/releases/tag/v1.1.0
[1.0.0]: https://github.com/cipherpine/quotapane/releases/tag/v1.0.0
