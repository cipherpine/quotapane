# QuotaPane — Architecture & Security Specification

> Working name: **QuotaPane** (rename before first public release — see Open Decisions).
> Status: draft spec, v0.1. Intended as the seed document for a public, open-source repository.
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
| **Anthropic source** | Undocumented OAuth usage endpoint (same one Claude Code uses), read via `~/.claude/.credentials.json`; Messages-API rate-limit headers as fallback | **Official** Usage & Cost Admin API: `/v1/organizations/usage_report/messages` + `/v1/organizations/cost_report` |
| **OpenAI source** | Undocumented Codex usage endpoint, read via `~/.codex/auth.json` | Official OpenAI usage/costs API (org key) |
| **Credential** | Local OAuth token (bearer) | Admin/org API key (`sk-ant-admin-…` / OpenAI org key) |
| **Availability** | Anyone signed into the CLI | **Anthropic Admin API is unavailable for individual accounts** — requires an Organization on a Platform plan. Not available on Bedrock. |
| **Stability** | Fragile — undocumented, may change without notice; ToS gray area | Stable, documented; cost endpoints are **beta** (schema may shift) |

**Implication for v1:** the subscription/quota view is what people actually want on their desk, but it depends on undocumented endpoints. The API-billing view is stable and official but needs an org/admin key that solo users may not have, and it does *not* cover Claude Code-on-subscription usage. Ship the subscription providers first (behind a clear "uses undocumented endpoints" disclaimer), and treat the official Admin/usage APIs as an **opt-in advanced mode**. Design the provider layer so both are just implementations of one trait.

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
│  │  ├─ credentials/    # read-only credential loaders (Claude, Codex, WSL paths)
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
├─ LICENSE
└─ .github/workflows/    # CI: build, test, audit, signed release
```

### `usage-core` submodules

**`credentials`** — Read-only loaders. Resolves `~/.claude/.credentials.json`, `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`), and optionally a credential file inside a named WSL distro. Returns tokens wrapped in a `Secret<T>` type that: zeroizes memory on drop, has a `Debug`/`Display` impl that prints `«redacted»`, and is never serialized. **Never writes** to credential files. Token *refresh* is delegated by spawning the official `claude` / `codex` CLI — the app itself never mints or rewrites `auth.json`, which eliminates a whole class of credential-corruption and leakage bugs.

**`egress`** — One HTTP client, one chokepoint, deny-by-default. A compile-time host **allowlist** is the only way a request leaves the process:
- `api.anthropic.com` (subscription usage + fallback; official Admin API)
- `chatgpt.com` — Codex (ChatGPT-plan) subscription usage (`/backend-api/wham/usage`; verified in M3 against the open-source Codex CLI — the subscription endpoint is **not** on `api.openai.com`)
- `api.openai.com` — official OpenAI usage/costs API (opt-in billing mode, M4)
- `api.github.com` — **update check only**, and only when the user enables it
Any attempt to dial a host not on the list is a hard error. Proxy support is **off by default**; if `HTTPS_PROXY`/`ALL_PROXY` is set, the app surfaces a visible warning that a TLS-inspecting proxy (e.g. a corporate Zscaler-style gateway) can observe the bearer token at its decryption point, and requires explicit opt-in to proceed. Optional certificate pinning for provider hosts.

**`providers`** — A single trait, one implementation per source:

```rust
trait UsageProvider {
    fn id(&self) -> ProviderId;
    fn poll(&self, http: &Egress) -> Result<ProviderSnapshot>;
    fn cadence(&self) -> Cadence;      // adaptive interval hints
}
```

Implementations:
- `ClaudeSubscription` — OAuth usage endpoint + Messages-API rate-limit-header fallback.
- `CodexSubscription` — OpenAI Codex usage endpoint.
- `AnthropicAdmin` *(opt-in)* — official Usage & Cost Admin API. Needs `sk-ant-admin-…`; unavailable to individual accounts.
- `OpenAiUsage` *(opt-in)* — official OpenAI usage/costs API. Needs an org key.
- `OtelSource` *(opt-in, advanced)* — reads from your **existing** local OTEL/Prometheus endpoint instead of calling providers directly (fits your current OTEL setup; keeps tokens out of this tool entirely for the billing view).

**`poller`** — Thread-based scheduler (one lightweight thread per provider; amended from async/`tokio` in M1 — the `ureq`/`rustls` sync stack was chosen to keep the trust boundary's dependency tree minimal, and 2–4 providers don't need an async runtime). Per-provider, staggered. Adaptive intervals: fast (~5 min) during active use, normal (~7 min), slow (~20 min) when idle, snap to imminent quota resets, exponential backoff on `429`. Emits normalized snapshots over a channel to the UI. Tokens are loaded lazily, held only in memory, zeroized after use.

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

### Enforced invariants (each backed by a test)
- Tokens are **never persisted** by the app; config stores preferences only.
- Tokens **never** appear in logs, telemetry, crash reports, or `Debug` output (redaction + `zeroize`).
- Egress is **deny-by-default**; a unit test asserts that no host outside the allowlist is reachable.
- **No first-party telemetry/analytics.** The app phones home to nobody.
- **No silent auto-update.** Update = check GitHub releases and *notify only*; the user updates via their package manager or a manual download. (This closes the "trusted updater becomes the attack vector" hole that the reference tool leaves open.)
- Credential files are opened **read-only**; the app never writes `auth.json`.
- Proxy is **opt-in** with a loud warning (Zscaler/MITM caveat).

### Threat model summary

| Threat | Posture |
|---|---|
| Curious/hostile reviewer auditing the repo | **Defended by design** — tiny, isolated trust boundary; deny-by-default egress; no telemetry. |
| Accidental token leakage into logs/crash dumps | **Defended** — `Secret<T>`, redacted `Debug`, `zeroize`. |
| Our own dependency supply chain | **Mitigated** — pinned `Cargo.lock`, `cargo-deny` + `cargo-audit` in CI, minimal deps, each justified. |
| Malicious release binary | **Mitigated** — CI-only builds, signed artifacts + build provenance (see §7); reproducible where feasible. |
| Corrupting/leaking the user's credentials | **Defended** — read-only; refresh delegated to official CLIs. |
| TLS-inspecting corporate proxy sees the token | **Surfaced, not silently accepted** — proxy off by default, explicit warning + opt-in. |
| **Compromised OS / user account** | **Explicit non-goal.** If an attacker already has the user's account, they already have the token. This tool cannot and does not claim to defend against that. |

### Responsible-use disclaimer (ship in README + at runtime)
Independent, community project; **not affiliated with, endorsed, or supported by Anthropic or OpenAI.** The subscription/quota providers use **undocumented endpoints that may change or break at any time**, and read **your own** local credentials only. No auth is bypassed and nothing is scraped. Use at your own risk.

---

## 6. Configuration & storage

Preferences only — **no secrets** ever written by the app. JSON in the platform config dir (`%APPDATA%\QuotaPane\` / `~/.config/quotapane/` / `~/Library/Application Support/QuotaPane/`):

- Window: position, size/zoom, compact vs expanded, always-on-top, monitor.
- Providers: which are enabled; per-provider poll cadence overrides.
- Display: theme, color thresholds (e.g. amber ≥ 80%, red ≥ 95%), which quota bars to show.
- Update: whether the GitHub update-check is enabled (default off).
- Proxy: explicit opt-in flag (default off).

Schema is versioned and documented in `README.md`.

---

## 7. Open-source & supply-chain hygiene

Because it's public and touches credentials, the repo itself must model good practice:

- **License:** MIT or Apache-2.0 (Open Decision).
- **`SECURITY.md`:** disclosure policy + contact; documents the trust boundary and invariants above.
- **Deps:** `Cargo.lock` committed; `cargo-deny` (licenses, bans, advisories) + `cargo-audit` gated in CI; a short "why each dependency exists" table in `CONTRIBUTING.md`.
- **Releases:** built **only** in CI (never a maintainer laptop); artifacts signed (e.g. `cosign`) with **GitHub build provenance / attestations**; checksums published. This is the concrete fix for the "is the binary really the source?" gap.
- **Reproducible builds** where the toolchain allows; document the exact toolchain version.
- **Secret hygiene:** `gitleaks` pre-commit + CI secret scanning; `.gitignore` covers any local credential/test fixtures; no real tokens in tests (use synthetic fixtures).
- **`dependabot`** for dependency and Action updates.
- **CI matrix:** build + test on Windows/macOS/Linux; run the egress-allowlist and redaction tests on every PR.

---

## 8. UI specification

Always-on-top, frameless, draggable floating window.
- **Per-provider row:** provider name/icon, one or more quota bars (e.g. Claude 5h + weekly; Codex session + weekly), each color-coded by threshold, with a reset countdown. Optional cost readout when an official billing provider is enabled.
- **Interactions:** drag to move; scroll to resize/zoom; click a row to expand a detail popover (sparkline history, per-model breakdown, forecast-to-limit, top projects if available); right-click for settings/position/theme/minimize.
- **Modes:** compact (thin strip) and expanded; multi-monitor aware; position persists.
- **Liveness:** staleness indicator when data is older than expected; a subtle pulse on refresh; never blocks input.
- **Headless parity:** `usage-cli --json` emits the same normalized snapshot for scripting and for verifying behavior without the GUI.

---

## 9. Phased roadmap

Build the **trust boundary first**, prove it headless, then add the window and more providers.

- **M0 — Skeleton + security scaffolding.** Workspace, CI, `SECURITY.md`/`THREAT_MODEL.md`, `egress` chokepoint that denies everything, one read-only credential loader with redaction + zeroize tests. *No provider calls yet.*
- **M1 — First provider, headless.** `ClaudeSubscription` (usage endpoint + header fallback), `usage-cli --json`, egress-allowlist test passing. Proves the whole pipeline with no UI.
- **M2 — The window.** `egui` always-on-top floating window rendering the Claude provider live.
- **M3 — Second provider.** `CodexSubscription`; multi-row UI; both providers live. *This is the minimum shippable "both providers, own window" product.*
- **M4 — Opt-in official billing.** `AnthropicAdmin` + `OpenAiUsage` behind an advanced-mode config gate (org/admin keys). Clearly labeled and separated from the subscription view.
- **M5 — Depth.** History/sparklines, forecast-to-limit, thresholds/alerts, optional `OtelSource` that reuses your existing OTEL pipeline.
- **M6 — Ship.** Packaging (WinGet/Homebrew/AUR), signed CI releases + provenance, docs, `v1.0`.

---

## 10. Open decisions (resolve in Cowork)

1. **UI:** `egui` (recommended) vs Tauri. Recommendation stands unless rich HTML UI becomes a requirement.
2. **v1 provider scope:** ship subscription-only (Claude + Codex) first; official billing in M4. Confirm.
3. **Undocumented endpoints — in or out?** They power the view users actually want but are fragile and a ToS gray area. Recommendation: include them, gated behind the runtime disclaimer, with graceful degradation when they change.
4. **Name & license:** pick a real name; MIT vs Apache-2.0.
5. **Platform priority:** you're on Windows — confirm Windows is the primary CI/release target, with macOS/Linux best-effort at first.
6. **OTEL role:** first-class billing source, or advanced/optional? (You already run OTEL, so this could become the *cleanest* token-free path to the billing view.)

---

## 11. First tasks for the Cowork/Claude Code build

1. Scaffold the workspace and the three crates; commit `Cargo.lock`.
2. Land CI with `cargo test`, `cargo-deny`, `cargo-audit`, and the cross-platform build matrix **before** any provider code.
3. Implement `egress` as deny-all with an allowlist and a test proving a non-listed host errors.
4. Implement `credentials::Secret<T>` with redaction + zeroize + tests (synthetic fixtures only).
5. Write `SECURITY.md` and `THREAT_MODEL.md` from §5 so the security posture is in the repo from commit one.
6. Only then implement `ClaudeSubscription` + `usage-cli --json` (M1).
