# Security Policy

> Project: **QuotaPane**. See `ARCHITECTURE.md` for the full design and `THREAT_MODEL.md` for the adversary analysis. This file is the authoritative statement of the project's security posture and disclosure process.

QuotaPane reads **your own** local AI-provider credentials and shows your usage in a desktop window. Because it touches bearer credentials, its entire reason for existing is a **small, auditable trust boundary**. This document tells you exactly what that boundary is, the invariants we hold inside it, and how to report a problem.

---

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

The channel: **GitHub private vulnerability reporting** (this repo → *Security* tab → *Report a vulnerability*). There is deliberately no e-mail channel; the GitHub flow is monitored and keeps reports private by default.

When reporting, please include:
- Affected version / commit.
- A description of the issue and its security impact.
- Reproduction steps or a proof of concept.
- Any suggested remediation.

**Our commitments:**
- Acknowledge within **72 hours**.
- Provide an initial assessment within **7 days**.
- Coordinate a fix and disclosure timeline with you; default embargo target is **90 days** or until a fix ships, whichever is sooner.
- Credit you in the advisory and release notes unless you prefer to remain anonymous.

This is a volunteer open-source project; there is no bug bounty. We're grateful for responsible disclosure regardless.

---

## Scope

**In scope** (please report):
- Any path that causes a credential/token to be **persisted, logged, transmitted to a non-provider host, or otherwise leaked**.
- Any way to make `egress` contact a host **outside the allowlist**.
- Any way to make the app **write to** a credential file.
- Weaknesses in **release integrity** (signing, provenance, reproducibility).
- Dependency vulnerabilities with a plausible exploitation path in this app.
- Redaction failures (a secret appearing in `Debug`/logs/crash output).

**Out of scope** (see `THREAT_MODEL.md` for why):
- Attacks that presuppose a **compromised OS or user account** — if an attacker already controls the account, they already have the tokens.
- The provider's own undocumented endpoints changing or breaking (that's a stability issue, not a vulnerability).
- A **TLS-inspecting proxy** the user has explicitly opted into observing traffic — this is warned about and consented to (see below).
- Social-engineering a user into disabling the built-in safeguards.

---

## The trust boundary

The **entire** security-sensitive surface of this application is two operations, both contained in the `usage-core` crate:

1. **Read** local credential files (`usage-core::credentials`).
2. **Send** a token to an allowlisted provider host (`usage-core::egress`).

Everything else — scheduling, normalization, rendering — never sees a raw secret. Validating the security posture means reading those two modules plus the thin layer that consumes their output — the two provider parsers and the CLI's raw-debug path handle provider responses (never tokens). That is the design, and `DECISIONS.md` §4.1 records the full protected-path list.

---

## Security invariants

Each invariant below is enforced in code. Where an invariant asserts a behavior, a test backs it; where it asserts an **absence** (1's no-token-write and 5), it is enforced by there being no such code path at all — the traceability table in `THREAT_MODEL.md` §9 records which is which, honestly. The mapping itself is machine-checked: `invariants.manifest` registers every invariant and the exact tests that prove it, and CI's `invariants` job fails on any drift between that file, this list, and the tests in the tree. A change that weakens any invariant is a breaking security change and must be called out in review.

1. **No credential persistence.** The app never writes tokens to disk — no code path serializes a token. Since v1.6.0 the app writes at most two non-credential files under the platform config directory: `config.cfg`, a handful of key=value preference lines (theme, history, alerts — see the README), and, only when `history=on`, `history.jsonl` — timestamps, window labels, and usage percentages, nothing else. The legacy one-word `theme.cfg` is still read as a fallback but never written. Preferences and percentages only, never secrets; every other setting is a CLI flag held in memory.
2. **No credential leakage.** Tokens are held in a `Secret<T>` wrapper that zeroizes on drop and redacts in `Debug`/`Display`/serialization. Zeroization is best-effort and covers buffers QuotaPane owns — a transient copy necessarily exists inside the HTTP/TLS stack while a request is in flight (the egress module documents this boundary). QuotaPane itself never writes a secret to logs, telemetry, or crash reports.
3. **Deny-by-default egress.** All outbound requests pass through a single client with a host **allowlist**. A request to any other host is a hard error. A test asserts a non-listed host is rejected.
4. **No first-party telemetry.** The app collects and transmits **no** analytics of any kind, to anyone.
5. **No self-update — no update mechanism exists at all.** The app never downloads or executes code, and contains no updater and no update check of any kind. Updating is always a manual act: your package manager, or a verified download (see below). If an update *check* is ever added it will be notify-only and off by default, and this document changes in the same PR.
6. **Read-only credentials.** Credential files are opened read-only. Token refresh is delegated to the official `claude` / `codex` CLIs; the app never writes `auth.json`.
7. **Proxy is opt-in, CLI-only, and fail-closed.** If a proxy environment variable is set (`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`, upper- or lowercase), egress sends nothing and fails with an error naming the variable. `quotapane-cli --allow-proxy` opts in for that single run, after a printed warning that a TLS-inspecting proxy can observe the bearer token; the CLI's error output points at the flag. The window has no opt-in surface at all — its egress is constructed proxy-off unconditionally, so under a proxy environment it fails closed and shows the error rather than sending anything anywhere.
8. **Agent visibility is metadata-only.** The agents pane lists your local Claude Code / Codex CLI sessions by reading their session-log files (`~/.claude/projects/`, `~/.codex/sessions/`) read-only, extracting a fixed allowlist of metadata keys — ids, timestamps, record types, working directory, git branch, and the CLI's own version string — and nothing else. Key lookup searches three levels of nested objects, because the Codex CLI files a session's branch two levels down; the price of that reach is paid by a companion list of names that may never join the allowlist (`content`, `text`, `message`, `model`, `toolUseResult` and their kin), so a lookup can descend into a record without ever being able to return the words inside a message. Conversation content is never deserialized, rendered, persisted, or transmitted; a fixture test plants sentinel conversation text and asserts it cannot reach any output. Liveness is inferred from file modification times alone when parsing fails, so degradation is graceful and silent. Nothing about these sessions leaves the machine.

---

## Credential handling

- Sources are read-only: `~/.claude/.credentials.json`, and `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`). No other credential source is read — WSL credential sources are **not** implemented (a possible post-1.0 addition, which would be called out here). The only other provider files the app opens are the session-log files named by invariant 8: read-only, metadata-only, never a token-bearing path.
- Tokens live only in memory, wrapped in `Secret<T>`, zeroized after use (best-effort — invariant 2 states the exact scope).
- Diagnostic output is redacted by default: `quotapane-cli --debug-raw` replaces identifier fields (`email`, `user_id`, `account_id`, `id`) with `«redacted»` at any depth before printing, and withholds bodies that are not valid JSON. The byte-exact escape hatch (`--debug-raw-unsafe`) prints one warning first.
- QuotaPane **never** ingests an organization Admin/org API key. An official-billing mode was evaluated and **rejected** (see `ARCHITECTURE.md`, ADR-002): holding that higher-privilege key would enlarge this trust boundary — the opposite of the project's purpose. The only credentials read are your own local subscription tokens, above.
- The app treats these tokens as bearer credentials that can act as you against the provider. Anything reading them is inside your trust boundary; the project's job is to keep that boundary small and honest.

### Presenting as the official client (subscription mode)

The Claude subscription usage endpoint (`GET /api/oauth/usage`) rate-limits requests that lack a `User-Agent: claude-code/<version>` header, so QuotaPane sends that header. This means its subscription requests **present as the official Claude Code client**. This is a deliberate, disclosed choice: QuotaPane calls only the endpoint that client already calls, with your own OAuth token, read-only, and to a single allowlisted host. It is not an authentication bypass and nothing is scraped, but you should understand that the request is indistinguishable at the wire from the official client's own usage check. The endpoint is undocumented and may change or be withdrawn without notice; QuotaPane fails closed (shows stale/error, never leaks) when it does.

The same applies to the Codex provider: its requests to `chatgpt.com` send the Codex CLI's default `User-Agent` (`codex-cli`) and the `ChatGPT-Account-Id` header, so they present as the official Codex client. Same posture, same disclosure, same fail-closed behavior; token refresh is delegated to `codex login` and the app never writes `auth.json`.

---

## Network / egress policy

- Single HTTP chokepoint, compile-time deny-by-default allowlist of exactly **two hosts** (`crates/usage-core/src/egress/mod.rs`, `ALLOWED_HOSTS`):
  - `api.anthropic.com` — the Claude subscription usage endpoint (`GET /api/oauth/usage`). Nothing else is called on this host today; a Messages-API rate-limit-header fallback is a deferred idea, not shipped code.
  - `chatgpt.com` — Codex (ChatGPT-plan) subscription usage (verified against the open-source Codex CLI; **not** `api.openai.com`).
- `api.github.com` was removed from the allowlist 2026-07-27: it existed for an optional update check that was never implemented, and an allowlist should be exactly as wide as the code behind it.
- TLS is required — the chokepoint cannot construct a non-HTTPS URL, and redirects are never followed. **Certificate pinning is not implemented**; TLS validation uses the WebPKI root set bundled into the binary (`webpki-roots`, the Mozilla CA program) — the operating system's trust store is **not** consulted. A CA added to your OS, including a corporate interception CA, is therefore rejected rather than silently trusted; the flip side is that enterprise-TLS gateways and captive portals hard-fail. See `THREAT_MODEL.md` R1.
- Proxy off by default (see invariant 7).

---

## Build & release integrity

- Release artifacts are built **only in CI**, never on a maintainer's machine: `.github/workflows/release.yml` runs only on version tags, builds each target with `--locked`, and uploads to a **draft** release — publishing is always a human act.
- Every release ships `SHA256SUMS` covering all archives; that file is signed with **`cosign` keyless signing** (GitHub OIDC identity), and each archive carries a **build provenance attestation** (`actions/attest-build-provenance`), verifiable with `gh attestation verify`.
- The exact toolchain is recorded per release: every archive contains a `TOOLCHAIN.txt` with the `rustc -V` / `cargo -V` that built it. Full bit-for-bit reproducibility is **not yet claimed**.
- **Verify before you run:** the exact commands for checking the checksum, signature, and provenance are in `README.md` ("Verify a release"). If you want maximum assurance, build from source (see below).

---

## Supply-chain policy

- `Cargo.lock` is committed; the dependency tree is intentionally small and each dependency is justified in `CONTRIBUTING.md`.
- CI gates on `cargo-deny` (licenses, bans, advisories) and `cargo-audit`.
- `dependabot` tracks dependency and GitHub Action updates.
- Secret scanning (`gitleaks`) runs in CI on every push and pull request, over the **full git history** (`fetch-depth: 0`), using a checksum-pinned release binary (see `ci.yml`). There is deliberately no claim of pre-commit scanning — a hook cannot be enforced on contributors, and CI can. Test fixtures use synthetic tokens only; no real credentials ever enter the repo.

---

## Hardening guidance for security-conscious users

You do not have to take our word for any of the above:

1. **Build from source.** Audit `crates/usage-core/src/credentials/` and `crates/usage-core/src/egress/` — that is the whole sensitive surface, and there are no build scripts (`build.rs` does not exist in this workspace) — then `cargo build --release --locked`. This eliminates any binary-provenance question entirely.
2. **Pin and freeze.** Check out a reviewed tag/commit and build that. There is no auto-update to disable — the app cannot update itself — so staying pinned is the default, not an option you must set.
3. **Verify egress once.** Run `quotapane-cli --once --json` behind a packet capture or host-firewall allowlist and confirm the only destinations are `api.anthropic.com` and `chatgpt.com`. (Yes, that exact command works — it is tested; `--once` is required because one-shot polling is the CLI's only mode.)
4. **Run as a normal user.** No elevation is needed or requested.
5. **Mind TLS-inspecting proxies.** On a corporate-managed device, an inspecting gateway may already see your provider traffic; keep proxy support off unless you understand and accept that exposure.

---

## Disclaimer

QuotaPane is an independent, community project. It is **not affiliated with, endorsed, or supported by Anthropic or OpenAI.** The subscription/quota providers rely on **undocumented endpoints that may change or break at any time**, and use **your own** local credentials only. No authentication is bypassed and nothing is scraped. Use at your own risk.
