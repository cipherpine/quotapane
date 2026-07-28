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

Everything else — scheduling, normalization, rendering — never sees a raw secret. You can validate the project's security posture by auditing essentially these two modules. That is the design.

---

## Security invariants

Each invariant below is enforced in code. Where an invariant asserts a behavior, a test backs it; where it asserts an **absence** (1 and 5), it is enforced by there being no code path at all — the traceability table in `THREAT_MODEL.md` §9 records which is which, honestly. A change that weakens any invariant is a breaking security change and must be called out in review.

1. **No credential persistence.** The app never writes tokens to disk. Today the app writes **no files at all** — there is no config layer; the only settings are CLI flags, held in memory.
2. **No credential leakage.** Tokens are held in a `Secret<T>` wrapper that zeroizes on drop and redacts in all `Debug`/`Display`/serialization paths. Secrets never appear in logs, telemetry, or crash reports.
3. **Deny-by-default egress.** All outbound requests pass through a single client with a host **allowlist**. A request to any other host is a hard error. A test asserts a non-listed host is rejected.
4. **No first-party telemetry.** The app collects and transmits **no** analytics of any kind, to anyone.
5. **No self-update — no update mechanism exists at all.** The app never downloads or executes code, and contains no updater and no update check of any kind. Updating is always a manual act: your package manager, or a verified download (see below). If an update *check* is ever added it will be notify-only and off by default, and this document changes in the same PR.
6. **Read-only credentials.** Credential files are opened read-only. Token refresh is delegated to the official `claude` / `codex` CLIs; the app never writes `auth.json`.
7. **Proxy is opt-in.** If `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` is set, the app warns that a TLS-inspecting proxy can observe the bearer token, and requires explicit opt-in before sending anything through it.

---

## Credential handling

- Sources are read-only: `~/.claude/.credentials.json`, and `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`). Nothing else is read — WSL credential sources are **not** implemented (a possible post-1.0 addition, which would be called out here).
- Tokens live only in memory, wrapped in `Secret<T>`, zeroized after use.
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
- TLS is required — the chokepoint cannot construct a non-HTTPS URL, and redirects are never followed. **Certificate pinning is not implemented**; TLS validation trusts the platform root store (`rustls`). See `THREAT_MODEL.md` R1 for what that leaves open and what you can do about it.
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
