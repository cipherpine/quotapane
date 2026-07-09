# Security Policy

> Project: **QuotaPane** (working name). See `ARCHITECTURE.md` for the full design and `THREAT_MODEL.md` for the adversary analysis. This file is the authoritative statement of the project's security posture and disclosure process.

QuotaPane reads **your own** local AI-provider credentials and shows your usage in a desktop window. Because it touches bearer credentials, its entire reason for existing is a **small, auditable trust boundary**. This document tells you exactly what that boundary is, the invariants we hold inside it, and how to report a problem.

---

## Reporting a vulnerability

**Please do not open a public issue for security problems.**

Preferred channel: **GitHub private vulnerability reporting** (this repo → *Security* tab → *Report a vulnerability*).
Backup channel: `security@<DOMAIN>` *(fill in before first release; PGP key fingerprint here if used)*.

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

Each invariant below is enforced in code and backed by a test. A change that weakens any of them is a breaking security change and must be called out in review.

1. **No credential persistence.** The app never writes tokens to disk. Config files store preferences only.
2. **No credential leakage.** Tokens are held in a `Secret<T>` wrapper that zeroizes on drop and redacts in all `Debug`/`Display`/serialization paths. Secrets never appear in logs, telemetry, or crash reports.
3. **Deny-by-default egress.** All outbound requests pass through a single client with a host **allowlist**. A request to any other host is a hard error. A test asserts a non-listed host is rejected.
4. **No first-party telemetry.** The app collects and transmits **no** analytics of any kind, to anyone.
5. **No silent auto-update.** The app never downloads and executes new code on its own. The optional update *check* only notifies; the user updates via their package manager or a manual, verifiable download. The update check is **off by default**.
6. **Read-only credentials.** Credential files are opened read-only. Token refresh is delegated to the official `claude` / `codex` CLIs; the app never writes `auth.json`.
7. **Proxy is opt-in.** If `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` is set, the app warns that a TLS-inspecting proxy can observe the bearer token, and requires explicit opt-in before sending anything through it.

---

## Credential handling

- Sources are read-only: `~/.claude/.credentials.json`, `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`), and optionally a credential file inside a named WSL distro.
- Tokens live only in memory, wrapped in `Secret<T>`, zeroized after use.
- Optional official-billing mode uses an Admin/org API key (`sk-ant-admin-…` / OpenAI org key) supplied by the user via environment variable or the OS keychain — **never** stored by the app in its config.
- The app treats these tokens as bearer credentials that can act as you against the provider. Anything reading them is inside your trust boundary; the project's job is to keep that boundary small and honest.

---

## Network / egress policy

- Single HTTP chokepoint, deny-by-default allowlist:
  - `api.anthropic.com` — subscription usage + fallback; official Admin API.
  - OpenAI usage host(s) — `api.openai.com` and the Codex usage host.
  - `api.github.com` — update **check only**, and only if the user enables it.
- TLS required; optional certificate pinning for provider hosts.
- Proxy off by default (see invariant 7).

---

## Build & release integrity

- Release artifacts are built **only in CI**, never on a maintainer's machine.
- Artifacts are **signed** (e.g. `cosign`) and published with **build provenance / attestations**; checksums accompany every release.
- Reproducible builds are pursued where the toolchain allows; the exact toolchain version is documented per release.
- **Verify before you run:** instructions for checking the signature/provenance and checksum are in `README.md`. If you want maximum assurance, build from source (see below).

---

## Supply-chain policy

- `Cargo.lock` is committed; the dependency tree is intentionally small and each dependency is justified in `CONTRIBUTING.md`.
- CI gates on `cargo-deny` (licenses, bans, advisories) and `cargo-audit`.
- `dependabot` tracks dependency and GitHub Action updates.
- Secret scanning (`gitleaks`) runs in CI and pre-commit; test fixtures use synthetic tokens only — no real credentials ever enter the repo.

---

## Hardening guidance for security-conscious users

You do not have to take our word for any of the above:

1. **Build from source.** Audit `usage-core/credentials` and `usage-core/egress` (that's the whole sensitive surface), skim `build.rs`, then `cargo build --release`. This eliminates any binary-provenance question entirely.
2. **Pin and freeze updates.** Check out a reviewed tag/commit and keep the update check disabled; update manually after diffing releases.
3. **Verify egress once.** Run `usage-cli --json` behind a packet capture or host-firewall allowlist and confirm the only destinations are the provider hosts.
4. **Run as a normal user.** No elevation is needed or requested.
5. **Mind TLS-inspecting proxies.** On a corporate-managed device, an inspecting gateway may already see your provider traffic; keep proxy support off unless you understand and accept that exposure.

---

## Disclaimer

QuotaPane is an independent, community project. It is **not affiliated with, endorsed, or supported by Anthropic or OpenAI.** The subscription/quota providers rely on **undocumented endpoints that may change or break at any time**, and use **your own** local credentials only. No authentication is bypassed and nothing is scraped. Use at your own risk.
