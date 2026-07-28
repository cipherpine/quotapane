# QuotaPane — Architecture & Security Specification

> Project: **QuotaPane** — the name was adopted permanently by owner decision D2 (2026-07-26).
> Status: the shipped architecture as of v1.0. This document describes what **is built**; where a design was specified but not implemented, it is marked **Future (not implemented)** rather than left in the present tense.
> A live, always-on-top desktop window that shows your AI usage across providers (Anthropic + OpenAI), read locally from your own credentials.

---

## 1. Purpose & scope

### Goal
A single, standalone, always-on-top desktop window that shows **live usage/quota across Anthropic and OpenAI** at a glance, built so that the security-sensitive surface is *tiny and fully auditable*. This is a deliberate rebuild rather than a fork, so that the credential-touching code is small enough for any reviewer (including you) to read end to end.

### Design priorities, in order
1. **Auditable trust boundary.** The only sensitive operations — reading credential files and sending tokens to provider hosts — live in one small crate a reviewer can read in minutes.
2. **Minimal dependency surface.** All-Rust, no web/JS layer in the primary build, every dependency justified.
3. **Fit.** Standalone floating window, both providers, live, visual — the thing that off-the-shelf tools don't quite do.
4. **Good open-source citizenship.** Clear security docs, signed CI-built releases, honest disclaimers about undocumented endpoints.

### Non-goals
- Not a proxy, not a scraper, not an auth bypass. It only ever uses **your own** credentials, locally.
- Not a team/enterprise cost-attribution platform (that's Anthropic's Enterprise Analytics API territory).
- Not a replacement for provider dashboards for finance-grade billing reconciliation.

---

## 2. The core design fork (read this first)

"Usage" splits into two fundamentally different data sources, and the whole architecture flows from keeping them separate:

| | Subscription / quota | API billing / spend |
|---|---|---|
| **What it shows** | Claude Code 5h + weekly limits; OpenAI Codex session/weekly limits | Tokens + dollars against pay-as-you-go API keys |
| **Anthropic source** | Undocumented OAuth usage endpoint (same one Claude Code uses), read via `~/.claude/.credentials.json`. (Messages-API rate-limit headers were considered as a fallback; deferred, not shipped.) | **Official** Usage & Cost Admin API: `/v1/organizations/usage_report/messages` + `/v1/organizations/cost_report` |
| **OpenAI source** | Undocumented Codex usage endpoint, read via `~/.codex/auth.json` | Official OpenAI usage/costs API (org key) |
| **Credential** | Local OAuth token (bearer) | Admin/org API key (`sk-ant-admin-…` / OpenAI org key) |
| **Availability** | Anyone signed into the CLI | **Anthropic Admin API is unavailable for individual accounts** — requires an Organization on a Platform plan. Not available on Bedrock. |
| **Stability** | Fragile — undocumented, may change without notice; ToS gray area | Stable, documented; cost endpoints are **beta** (schema may shift) |

**Implication for v1:** the subscription/quota view is what people actually want on their desk, but it depends on undocumented endpoints. Ship the subscription providers behind a clear "uses undocumented endpoints" disclaimer. The API-billing column above was originally planned as an opt-in advanced mode (M4) — **that is now withdrawn; see ADR-002 below.** Keep the provider layer as one trait regardless, so a future token-free cost source can slot in without touching the trust boundary.

**Documented decision (ADR-002, 2026-07-23): the official Admin/billing APIs are out of scope.** Researching M4 established that both vendors' usage/cost endpoints require an **organization Admin API key** (`sk-ant-admin01-…` / an OpenAI admin key) and are **unavailable to individual Pro/Max/Plus/Codex subscribers** — Anthropic's docs state the Admin API is unavailable for individual accounts outright. So the billing view (a) serves a different audience (API-billed orgs) with **zero overlap** with QuotaPane's subscription users, (b) measures a different thing — metered API dollar-spend, not subscription rate-limit consumption (a flat-fee Max/Codex user has no per-token bill for that usage), and (c) most decisively, would force the trust boundary to ingest and hold an **org-admin credential**, the highest-blast-radius secret in either ecosystem — directly contradicting the "tiny, auditable, read-only" thesis that is this product's headline. The org-cost space is also already well served (first-party consoles; Vantage/Finout/Datadog). If cost visibility is ever wanted, the compatible path is the **token-free `OtelSource`** (M5) — never an admin key. Consequence: `AnthropicAdmin`/`OpenAiUsage` are withdrawn, and `api.openai.com` has been dropped from the egress allowlist — the trust boundary now reaches exactly **two** hosts, `api.anthropic.com` and `chatgpt.com`. (`api.github.com` was removed as well, 2026-07-27: it had been listed for an update check that was never implemented.)

---

## 3. Recommended tech stack

**Primary: Rust + `egui`/`eframe`, single static binary.**

Rationale: one memory-safe language end to end; no system webview or JavaScript runtime to audit; small, flat dependency tree; trivial always-on-top frameless window; cross-platform (Windows/macOS/Linux) from one codebase. A usage meter is bars, numbers, and sparklines — `egui` renders that with almost no surface area.

**Documented alternative (ADR-001): Tauri.** Prettier HTML/CSS UI, but pulls in the system webview + a JS frontend, enlarging the audit surface. Rejected for the primary build *specifically because* the project's headline feature is a minimal, fully-owned trust boundary. Revisit only if UI richness becomes a hard requirement.

Either way, the security-critical core is **UI-agnostic** and isolated in its own crate, so the UI choice never touches the trust boundary.

---

## 4. Component architecture

Cargo workspace with three crates. The trust boundary is entirely inside `usage-core`; the other two depend only on its public, non-secret types.

```
quotapane/
├─ crates/
│  ├─ usage-core/        # THE TRUST BOUNDARY. Audit target.
│  │  ├─ credentials/    # read-only credential loaders (Claude, Codex)
│  │  ├─ egress/         # single hardened HTTP chokepoint + host allowlist
│  │  ├─ providers/      # UsageProvider trait + one impl per source
│  │  ├─ poller/         # thread-based scheduler: adaptive intervals, backoff
│  │  └─ model/          # normalized types: QuotaWindow, UsageBucket, CostBucket, RateLimit
│  ├─ usage-ui/          # egui app: pure render, holds no credential logic
│  └─ usage-cli/         # headless --once / --json mode (scripting, tests, egress proof)
├─ Cargo.toml            # workspace, deps pinned
├─ Cargo.lock            # committed
├─ deny.toml             # cargo-deny policy
├─ SECURITY.md
├─ THREAT_MODEL.md
├─ ARCHITECTURE.md       # this file
├─ README.md
├─ CONTRIBUTING.md
├─ LICENSE-MIT
├─ LICENSE-APACHE
└─ .github/workflows/    # ci.yml: build/test, deny, audit, no-telemetry, gitleaks
                         # release.yml: tag-triggered build, checksum, sign, attest
```

### `usage-core` submodules

**`credentials`** — Read-only loaders. Resolves `~/.claude/.credentials.json` and `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`). Nothing else is read: WSL credential paths are **not implemented** (a possible post-1.0 addition). Returns tokens wrapped in a `Secret<T>` type that: zeroizes memory on drop, has a `Debug`/`Display` impl that prints `«redacted»`, and is never serialized. **Never writes** to credential files. Token *refresh* is delegated by spawning the official `claude` / `codex` CLI — the app itself never mints or rewrites `auth.json`, which eliminates a whole class of credential-corruption and leakage bugs.

**`egress`** — One HTTP client, one chokepoint, deny-by-default. A compile-time host **allowlist** is the only way a request leaves the process:
- `api.anthropic.com` — the Claude subscription usage endpoint (`GET /api/oauth/usage`). This is the only call made on this host; the Messages-API rate-limit-header fallback below is deferred, not shipped.
- `chatgpt.com` — Codex (ChatGPT-plan) subscription usage (`/backend-api/wham/usage`; verified in M3 against the open-source Codex CLI — the subscription endpoint is **not** on `api.openai.com`)

That is the whole list — exactly two hosts. Any attempt to dial a host not on it is a hard error. Proxy support is **off by default**; if `HTTPS_PROXY`/`ALL_PROXY` is set, the app surfaces a visible warning that a TLS-inspecting proxy (e.g. a corporate Zscaler-style gateway) can observe the bearer token at its decryption point, and requires explicit opt-in to proceed. TLS validation uses the platform root store via `rustls`; **certificate pinning is not implemented** (considered, not built — see `THREAT_MODEL.md` R1).

**`providers`** — A single trait, one implementation per source:

```rust
trait UsageProvider {
    fn id(&self) -> ProviderId;
    fn poll(&self, http: &Egress) -> Result<ProviderSnapshot>;
    fn cadence(&self) -> Cadence;      // adaptive interval hints
}
```

Implementations:
- `ClaudeSubscription` — OAuth usage endpoint. *(Future, not implemented: a Messages-API rate-limit-header fallback. It needs a second verified endpoint and is deferred; `SnapshotSource::RateLimitHeaders` exists as its placeholder variant and is never constructed today.)*
- `CodexSubscription` — OpenAI Codex usage endpoint.
- ~~`AnthropicAdmin`~~ *(withdrawn — ADR-002)* — would have needed an org admin key; out of scope. The `ProviderId` variant was removed 2026-07-25.
- ~~`OpenAiUsage`~~ *(withdrawn — ADR-002)* — would have needed an org admin key; out of scope. The `ProviderId` variant was removed 2026-07-25.
- `OtelSource` *(opt-in, advanced; M5)* — reads from your **existing** local OTEL/Prometheus endpoint instead of calling providers directly (keeps tokens out of this tool entirely). Per ADR-002 this is the **only** acceptable path to any cost/spend view, since it needs no admin key.

**`poller`** — Thread-based scheduler (one lightweight thread per provider; amended from async/`tokio` in M1 — the `ureq`/`rustls` sync stack was chosen to keep the trust boundary's dependency tree minimal, and 2–4 providers don't need an async runtime). Per-provider, staggered. Interval mechanism: `Cadence::Fast` (~5 min), `Normal` (~7 min), `Slow` (~20 min), under a hard ≥180 s floor, with exponential backoff on `429` capped at 30 min and `retry-after` honored when longer. Two caveats, both deliberate: **both shipped providers return `Cadence::Normal` unconditionally**, so Fast and Slow are implemented and tested but never selected in production (polite, predictable polling beats adaptive polling against an undocumented endpoint); and **snapping to imminent quota resets is not implemented** — `next_delay` takes only cadence, consecutive failures, and `retry-after`. Emits normalized snapshots over a channel to the UI. Tokens are loaded lazily, held only in memory, zeroized after use.

### Data flow

```
credential files (read-only) ─┐
                              ▼
                        credentials::Secret
                              ▼
   poller ──► provider.poll() ──► egress (allowlisted host) ──► provider API
                              ▲                                        │
                              └────────── ProviderSnapshot ◄───────────┘
                                              │
                                    channel (no secrets)
                                              ▼
                                   usage-ui  /  usage-cli  (pure render)
```

Secrets never cross the channel. The UI and CLI receive only normalized, non-sensitive `ProviderSnapshot` values.

---

## 5. Security model (the centerpiece)

This is a public repo whose selling point *is* its trust boundary, so the security model is a first-class artifact (`SECURITY.md` + `THREAT_MODEL.md`), not an afterthought.

### The entire sensitive surface
Two operations, both inside `usage-core`:
1. **Read** local credential files (`credentials`).
2. **Send** the token to an allowlisted provider host (`egress`).
A reviewer validates the security posture by reading essentially two files. Everything else is rendering and scheduling.

### Enforced invariants

`SECURITY.md` carries the authoritative wording. Invariants that assert a **behavior** are backed by named tests; the two that assert an **absence** are enforced by there being no code path at all — `THREAT_MODEL.md` §9 records which is which, row by row.

- Tokens are **never persisted**. The app writes **no files at all**: there is no config layer (see §6), so the only settings are CLI flags held in memory.
- Tokens **never** appear in logs, telemetry, crash reports, or `Debug` output (redaction + `zeroize`).
- Egress is **deny-by-default**; a unit test asserts that no host outside the two-host allowlist is reachable.
- **No first-party telemetry/analytics.** The app phones home to nobody.
- **No self-update.** There is no updater, and no update check, anywhere in the codebase. Updating is always a manual act. (This closes the "trusted updater becomes the attack vector" hole by not having an updater at all.)
- Credential files are opened **read-only**; the app never writes `auth.json`.
- Proxy is **opt-in** with a loud warning (Zscaler/MITM caveat).

### Threat model summary

| Threat | Posture |
|---|---|
| Curious/hostile reviewer auditing the repo | **Defended by design** — tiny, isolated trust boundary; deny-by-default egress; no telemetry. |
| Accidental token leakage into logs/crash dumps | **Defended** — `Secret<T>`, redacted `Debug`, `zeroize`. |
| Our own dependency supply chain | **Mitigated** — pinned `Cargo.lock`, `cargo-deny` + `cargo-audit` in CI, minimal deps, each justified. |
| Malicious release binary | **Mitigated** — tag-triggered CI-only builds (`release.yml`), `SHA256SUMS` signed with `cosign` keyless signing, a build-provenance attestation per archive, and the exact toolchain recorded in each archive (see §7). Bit-for-bit reproducibility is **not** claimed. |
| Corrupting/leaking the user's credentials | **Defended** — read-only; refresh delegated to official CLIs. |
| TLS-inspecting corporate proxy sees the token | **Surfaced, not silently accepted** — proxy off by default, explicit warning + opt-in. |
| **Compromised OS / user account** | **Explicit non-goal.** If an attacker already has the user's account, they already have the token. This tool cannot and does not claim to defend against that. |

### Responsible-use disclaimer (ship in README + at runtime)
Independent, community project; **not affiliated with, endorsed, or supported by Anthropic or OpenAI.** The subscription/quota providers use **undocumented endpoints that may change or break at any time**, and read **your own** local credentials only. No auth is bypassed and nothing is scraped. Use at your own risk.

---

## 6. Configuration & storage

**As shipped: there is none.** QuotaPane writes **no files at all** — no config file, no state file, no cache. The only settings are command-line flags (documented in `README.md`), held in memory for the life of the process. Nothing persists across runs, including window position. This is why invariant 1 is stronger than "no secrets on disk": there is no write path of any kind.

### Future (not implemented)

If a preferences layer is ever added it would be preferences only — **no secrets** — as JSON in the platform config dir (`%APPDATA%\QuotaPane\` / `~/.config/quotapane/` / `~/Library/Application Support/QuotaPane/`), plausibly covering:

- Window: position, size/zoom, always-on-top, monitor.
- Providers: which are enabled; per-provider poll cadence overrides.
- Display: theme, color thresholds (e.g. amber ≥ 80%, red ≥ 95%), which quota bars to show.
- Proxy: explicit opt-in flag (default off).

Adding it would mean introducing the app's first write path, so it is a trust-boundary change: it would need a versioned, documented schema and a `SECURITY.md`/`THREAT_MODEL.md` update in the same change.

---

## 7. Open-source & supply-chain hygiene

Because it's public and touches credentials, the repo itself must model good practice:

- **License:** dual **MIT OR Apache-2.0** (settled; `LICENSE-MIT` + `LICENSE-APACHE`, reflected in `Cargo.toml`).
- **`SECURITY.md`:** disclosure policy + contact; documents the trust boundary and invariants above.
- **Deps:** `Cargo.lock` committed; `cargo-deny` (licenses, bans, advisories) + `cargo-audit` gated in CI; a short "why each dependency exists" table in `CONTRIBUTING.md`.
- **Releases:** `.github/workflows/release.yml` triggers on `v*` tags and builds **only** in CI (never a maintainer laptop): `--locked` release builds for `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`, `SHA256SUMS` over every archive, `cosign` keyless signing of that file, and `actions/attest-build-provenance` on each archive. Artifacts upload to a **draft** release — publishing is always a human act. Third-party actions are pinned by full commit SHA. This is the concrete fix for the "is the binary really the source?" gap; the verification commands are in `README.md` ("Verify a release").
- **Toolchain provenance:** every archive carries a `TOOLCHAIN.txt` with the `rustc -V` / `cargo -V` that built it. Full bit-for-bit **reproducibility is not claimed** — it is a possible future goal, not a shipped property.
- **Secret hygiene:** `gitleaks` runs in CI over the **full git history** (`fetch-depth: 0`) on every push and pull request, as a checksum-pinned release binary invoked with `--redact`. There is deliberately no pre-commit hook — a hook cannot be enforced on contributors, and CI can. `.gitignore` covers local credential/test fixtures; no real tokens in tests (synthetic fixtures only).
- **`dependabot`** for dependency and Action updates.
- **CI matrix:** build + test on Windows/macOS/Linux; the egress-allowlist and redaction/zeroize invariant tests run on every push and PR, alongside `cargo-deny`, `cargo-audit`, the `no-telemetry` absence check, and the `gitleaks` history scan.

---

## 8. UI specification

Always-on-top, frameless, draggable floating window.
- **Per-provider row:** provider name/icon, one or more quota bars (e.g. Claude 5h + weekly; Codex session + weekly), each color-coded by threshold, with a reset countdown. (An optional cost readout is possible only via a future token-free `OtelSource` (M5); the official Admin/billing APIs are out of scope — ADR-002.)
- **Interactions (as shipped):** a slim custom titlebar carries the app name plus minimize and close buttons, and the strip itself is the window-drag handle. Scrolling **scrolls** the content (a `ScrollArea`); it does not resize or zoom. Each provider pane has an inline disclosure toggle that expands its per-model rows in place — not a popover. On Windows and macOS the tray icon's menu offers Show/Hide and Quit.
  *Future (not implemented):* a right-click settings menu, theme switching, sparkline history, forecast-to-limit, and per-project breakdown.
- **Modes:** one fixed, non-resizable size (`WINDOW_WIDTH` × `WINDOW_HEIGHT`, `.with_resizable(false)`), always-on-top. *Future (not implemented):* a compact thin-strip mode, multi-monitor awareness, and position persistence — position cannot persist while there is no config layer (§6).
- **Liveness:** a staleness indicator when data is older than expected; polling never blocks input. *Future (not implemented):* a refresh pulse animation.
- **Headless parity:** `quotapane-cli --once --json` emits the same normalized snapshot for scripting and for verifying behavior without the GUI.

---

## 9. Phased roadmap

Build the **trust boundary first**, prove it headless, then add the window and more providers.

- **M0 — Skeleton + security scaffolding.** Workspace, CI, `SECURITY.md`/`THREAT_MODEL.md`, `egress` chokepoint that denies everything, one read-only credential loader with redaction + zeroize tests. *No provider calls yet.*
- **M1 ✅ First provider, headless.** `ClaudeSubscription` (the OAuth usage endpoint; the header fallback was deferred), `quotapane-cli --once --json`, egress-allowlist test passing. Proves the whole pipeline with no UI.
- **M2 ✅ The window.** `egui` always-on-top floating window rendering the Claude provider live.
- **M3 ✅ Second provider.** `CodexSubscription`; multi-row UI; both providers live. *The minimum shippable "both providers, own window" product.*
- **M3.5 ✅ System tray.** Icon, tooltip, and a Show/Hide/Quit menu on Windows and macOS; Linux stays window-only.
- **M4 — ~~Opt-in official billing~~ WITHDRAWN (ADR-002).** The official Admin/billing APIs need org-admin keys, serve a different (API-billed) audience with no overlap with subscription users, and would break the read-only trust-boundary thesis. Any future cost view comes token-free via `OtelSource`, not admin keys.
- **M5 — Depth. Frozen at M5a for v1.0** (owner decision D1, 2026-07-26). Shipped: the per-model breakdown behind a collapsible toggle. Deferred to post-1.0: history/sparklines, forecast-to-limit, thresholds/alerts, and the optional token-free `OtelSource`.
- **M6 — Ship.** Signed, attested CI releases (`release.yml`), doc truth pass, public repo, `v1.0`. Package-manager distribution (WinGet/Homebrew/AUR) is post-1.0.

---

## 10. Decisions (all resolved)

Every item that was open here has been decided; `DECISIONS.md` is the standing record.

1. **UI:** ✅ Resolved — `egui`/`eframe` (ADR-001). Tauri rejected: a system webview plus a JS frontend would enlarge the audit surface, which is the one thing this project will not trade.
2. **v1 provider scope:** ✅ Resolved — subscription-only (Claude + Codex). Official billing (M4) is **withdrawn** (ADR-002).
3. **Undocumented endpoints — in or out?** ✅ Resolved — **in**, gated behind the runtime disclaimer, failing closed on schema drift.
4. **Name & license:** ✅ Resolved — **QuotaPane** (owner decision D2, 2026-07-26); dual **MIT OR Apache-2.0**. Binaries are `quotapane` and `quotapane-cli` (D3); the crate names `usage-core` / `usage-ui` / `usage-cli` stay product-neutral by design.
5. **Platform priority:** ✅ Resolved — Windows is the primary CI/release target; macOS and Linux are best-effort. Released binaries cover Windows and Linux; macOS is build-from-source (D6). Linux is window-only (no tray).
6. **OTEL role:** ✅ Resolved — advanced/optional, deferred to post-1.0. With the admin-key billing APIs out (ADR-002), a token-free `OtelSource` is the *only* acceptable path to any cost view — but it stays optional, not first-class.

---

## 11. Build order (historical — all complete)

Recorded because the *order* was the point: the trust boundary was built and tested before anything could call a provider. All six landed in M0–M1.

1. ✅ Scaffold the workspace and the three crates; commit `Cargo.lock`.
2. ✅ Land CI with `cargo test`, `cargo-deny`, `cargo-audit`, and the cross-platform build matrix **before** any provider code.
3. ✅ Implement `egress` as deny-all with an allowlist and a test proving a non-listed host errors.
4. ✅ Implement `credentials::Secret<T>` with redaction + zeroize + tests (synthetic fixtures only).
5. ✅ Write `SECURITY.md` and `THREAT_MODEL.md` from §5 so the security posture is in the repo from commit one.
6. ✅ Only then implement `ClaudeSubscription` + the headless CLI (M1).
