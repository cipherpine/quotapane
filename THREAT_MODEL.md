# Threat Model

> Project: **QuotaPane**. Companion to `ARCHITECTURE.md` (design) and `SECURITY.md` (policy & disclosure).
> Method: asset-centric analysis + STRIDE enumeration over the system's data flows, with explicit residual-risk and non-goal sections.
> Audience: reviewers deciding whether to trust this tool with their provider credentials.

---

## 1. Purpose

QuotaPane reads a user's **own** AI-provider credentials locally and displays their usage. The security value proposition is a **minimal, auditable trust boundary**. This document states what is being protected, from whom, what we mitigate, and — just as importantly — what we explicitly do **not** defend against.

---

## 2. System overview & data flow

```
credential files (read-only) ─┐
                              ▼
                   [TB1] credentials::Secret<T>        (secrets enter the process)
                              ▼
   poller ──► provider.poll() ──► [TB2] egress (allowlist) ──► provider API   (secrets leave the process)
                              ▲                                        │
                              └────────── ProviderSnapshot ◄───────────┘
                                              │
                                    channel (no secrets)
                                              ▼
                                   usage-ui / usage-cli  (pure render)
```

- **TB1 — Ingress boundary:** the filesystem → process. Secrets are read here and immediately wrapped.
- **TB2 — Egress boundary:** process → network. The only place a secret leaves the process, and only to an allowlisted host.
- Everything past the channel handles **non-secret** normalized data only.

---

## 3. Assets

| # | Asset | Why it matters |
|---|---|---|
| A1 | **Provider OAuth bearer tokens** (`~/.claude/.credentials.json`, `~/.codex/auth.json`) | Act-as-user credentials against the provider account/subscription. Primary asset. |
| A2 | ~~Admin/org API keys~~ — **withdrawn (ADR-002)** | An official-billing mode would have held an org Admin key (org-wide usage/cost, higher-privilege than A1). Evaluated and **rejected**; the app never ingests such a key, so this asset does not exist in the system. Row retained to record the decision. |
| A3 | **Integrity of the user's credential files** | Corruption or unintended writes could lock the user out or leak secrets. |
| A4 | **Release artifact integrity** | A tampered binary could exfiltrate A1. |
| A5 | **User trust / reputation of the project** | A public security-focused tool lives or dies on it. |

---

## 4. Trust boundaries

- **TB1 (filesystem → process):** Everything the process reads is assumed to be as trustworthy as the user's own account. We do not read anything we weren't pointed at, and we open credential files read-only.
- **TB2 (process → network):** Deny-by-default. A secret may cross only to a host on the allowlist, only over TLS.
- **Process → UI/CLI (channel):** No secret ever crosses. Enforced by the type of the message (`ProviderSnapshot`), not by convention.

---

## 5. Adversaries

| Actor | Capability | In our scope? |
|---|---|---|
| **Auditing reviewer** | Reads all source; runs the binary under instrumentation | Yes — we want them to succeed and come away satisfied. |
| **Network attacker / MITM** | On-path between the app and provider | Yes — mandatory TLS; no certificate pinning (see R1). |
| **TLS-inspecting proxy** | Terminates TLS at a corporate gateway | Partial — surfaced + opt-in; see residual risk R3. |
| **Malicious contributor** | Submits a PR | Yes — review + CI invariant tests + small surface. |
| **Compromised dependency** | Ships malicious code in a crate we depend on | Yes — pinning, `cargo-deny`/`cargo-audit`, minimal deps. |
| **Compromised maintainer account** | Publishes a malicious release | Partial — CI-only signed builds + provenance; see R2. |
| **Local malware / another local user** | Already executing as, or alongside, the user | **No** — out of scope; see §7. |
| **Provider-side change** | Alters or removes an undocumented endpoint | Not a security threat — a stability concern; graceful degradation. |

---

## 6. Threat enumeration (STRIDE)

Scoped to the two trust boundaries and the release pipeline.

### Spoofing
- **T-S1 — Impersonated provider endpoint / DNS or MITM redirection.** *Mitigation:* TLS verification is mandatory (`rustls`, platform trust anchors); redirects are never followed, and the egress allowlist means a redirect cannot carry a request to an off-list host anyway. **Certificate pinning is not implemented** — it was considered and has not been built, so it appears under residual risk, not here. *Residual:* R1.

### Tampering
- **T-T1 — Tampered release binary exfiltrates tokens.** *Mitigation:* releases are built only by the tag-triggered CI workflow (`release.yml`): `--locked` builds, published `SHA256SUMS` signed with cosign keyless signing, and build provenance attestations on every archive; per-release toolchain recorded (`TOOLCHAIN.txt`); "build from source" documented as the maximum-assurance path. *Residual:* R2.
- **T-T2 — Malicious dependency introduces a covert egress or a redaction bypass.** *Mitigation:* committed `Cargo.lock`, minimal justified deps, `cargo-deny` + `cargo-audit` in CI, small enough surface that a covert egress path would have to route around the single chokepoint (which tests guard).
- **T-T3 — App corrupts the user's `auth.json`.** *Mitigation:* credential files opened read-only; refresh delegated to official CLIs; the app never writes them. (Addresses A3.)

### Repudiation
- Low relevance for a local, single-user read-only tool. The app writes **no logs at all**: no logging backend is linked (`deny.toml` bans logger-backend crates, so every `log` macro in the dependency tree is a no-op), and no first-party telemetry exists.

### Information disclosure  *(primary risk category)*
- **T-I1 — Token written to disk.** *Mitigation:* invariant 1 (no persistence) + test.
- **T-I2 — Token in logs / crash dump / `Debug` output.** *Mitigation:* invariant 2 — `Secret<T>` with redacted formatting + `zeroize`; test asserts redaction.
- **T-I3 — Token sent to a non-provider host (accidental or malicious).** *Mitigation:* invariant 3 — deny-by-default egress allowlist + test asserting rejection of a non-listed host.
- **T-I4 — First-party telemetry ships usage data off-box.** *Mitigation:* invariant 4 — no telemetry exists in the codebase.
- **T-I5 — Token observed by a TLS-inspecting proxy.** *Mitigation:* invariant 7 — proxy off by default, explicit warning + opt-in. *Residual:* R3.
- **T-I6 — Conversation content surfaced or persisted by the agents pane.** *Mitigation:* invariant 8 — allowlisted metadata keys only, with a forbidden-key list that a nested lookup cannot be widened past; the sentinel-content test proves the content payload cannot reach any output type; turn state is read from a record's type and never its payload; nothing is written to disk or sent anywhere.

### Denial of service
- **T-D1 — Provider rate-limits the app (`429`).** *Mitigation:* a hard ≥180 s floor between polls, exponential backoff capped at 30 min, and `retry-after` honored when longer — all in one pure, tested function (`next_delay`). Impact is limited to stale display, never a crash.

### Elevation of privilege
- **T-E1 — App requests or requires elevated privileges.** *Mitigation:* runs as a normal user; no elevation requested. Autostart (opt-in) registers only a user-scope login entry.
- **T-E2 — Silent auto-update escalates into arbitrary code execution as the user.** *Mitigation:* invariant 5 — **no updater code path exists**: nothing is ever downloaded or executed. The update *check* is notify-only, off by default, and structurally incapable of escalating: it can carry no credential, reads only a version tag, renders one line of text, and is the sole caller of the one non-provider host on the egress allowlist — so a covert updater still has nowhere to call and nothing to call with.

---

## 7. Explicit non-goals (out-of-scope threats)

Stating these plainly is part of being trustworthy:

- **Compromised OS or user account.** If an adversary already executes as the user (malware, another admin, physical access to an unlocked session), they can read the same credential files directly. QuotaPane cannot and does not claim to defend against this. The token's security ceiling is the account's security.
- **Provider ToS / endpoint stability.** The subscription providers use undocumented endpoints. They may change or break. That is a functionality risk, not a security vulnerability, and is disclosed to users.
- **A proxy the user knowingly opted into.** Once a user accepts the TLS-inspection warning and opts in, observation by that proxy is a consented condition, not a defect.
- **Denial of service against the provider.** Out of scope by design; we rate-limit ourselves to be a good client, not to resist a hostile operator.

---

## 8. Residual risks

| # | Risk | Why it remains | User mitigation |
|---|---|---|---|
| **R1** | On-path attacker who can mint a certificate chaining to a WebPKI (Mozilla) root could MITM the provider connection | **Certificate pinning is not implemented**; TLS trusts the WebPKI root set bundled into the binary (`webpki-roots`) — the OS trust store is **not** consulted, so an OS-installed interception CA is rejected rather than trusted | Note this is stricter than OS-store validation: enterprise TLS gateways and captive portals hard-fail by design, and cleaning your OS trust store changes nothing here. For maximum assurance build from source and verify egress with a packet capture (`SECURITY.md`, hardening §3). |
| **R2** | A compromised maintainer account could publish a malicious release — signed, because the signing identity is the CI workflow itself | Keyless signing + provenance prove an artifact came from this repo's CI at a given commit; they cannot prove the commit was benign | Build from source; pin to a reviewed commit; diff releases; check the provenance's commit SHA against the audited source. |
| **R3** | TLS-inspecting corporate proxy can see the bearer token | Inherent to TLS interception; can't be prevented once opted in | Keep proxy off; understand your managed-device posture. |
| **R4** | Undocumented endpoint change could alter behavior unexpectedly | We don't control the provider | Graceful degradation; the app fails closed (shows stale/error, never leaks). |

---

## 9. Invariant → control → test traceability

This table is honest about its two kinds of rows: invariants that assert a
**behavior** are backed by named tests; invariants that assert an **absence**
(1, 5) are enforced by there being no code path — the control is the empty
grep, re-checked at every review touching the trust boundary (§11). The
test column below is machine-checked: `invariants.manifest` binds each
invariant to its tests and CI's `invariants` job fails on drift.

| Invariant (`SECURITY.md`) | Enforcing control | Test / check |
|---|---|---|
| 1. No credential persistence | absence of any credential write path in the workspace | enforced by absence; the read path's invariant-6 test (`loads_credential_readonly_and_redacted`) pins credential file access as read-only |
| 2. No credential leakage | `credentials::Secret<T>` | redaction + zeroize tests (`secret.rs`); `Debug` scrub test (`credentials/mod.rs`); end-to-end failure-path redaction test (`poller::tests::failures_are_forwarded_as_non_secret_messages` — a provider that formats its `Secret` into an error provably cannot leak it to the UI channel) |
| 3. Deny-by-default egress | `egress` (`ALLOWED_HOSTS`, exactly three hosts) | `non_allowlisted_host_is_rejected` (incl. the removed host `api.openai.com` and smuggling variants of all three allowed hosts), `get_refuses_non_allowlisted_host`, `authority_smuggling_paths_are_rejected` |
| 4. No first-party telemetry | (absence) | CI `no-telemetry` job: greps deps and sources for analytics |
| 5. No self-update; the check is notify-only, off by default | no updater code path (still enforced by absence) + `update` (gate, single call site, anonymous request) | `the_update_check_sends_nothing_unless_asked`, `the_update_request_cannot_carry_a_credential`, `update_is_the_only_caller_of_the_github_host` |
| 6. Read-only credentials | `credentials` | `loads_credential_readonly_and_redacted`: file bytes identical after load; no write handle exists |
| 7. Proxy opt-in (CLI-only, fail-closed) | `egress` proxy gate; `quotapane-cli --allow-proxy` is the only opt-in surface — the window has none | `proxy_env_without_opt_in_fails_closed` (either casing) + opt-in and empty-var tests; CLI tests pin the hint line, the per-run warning, and the absence of a window opt-in |
| 8. Agent visibility is metadata-only | `usage_core::agents` — allowlisted key extraction fenced by a forbidden-key list; content payloads never deserialized | `sentinel_content_never_reaches_any_output`, `extraction_is_welded_to_the_allowlist_const`, `unparseable_file_still_reports_liveness_from_mtime`, `scanner_opens_only_jsonl_under_the_session_roots`, `no_allowlisted_key_can_ever_name_message_content`, `turn_state_is_read_from_the_record_type_alone` |

---

## 10. Assumptions

- The user's account and OS are not already compromised (see §7).
- The official `claude` / `codex` CLIs, when invoked for token refresh, behave as documented.
- TLS and the system trust store are intact.
- Provider hosts on the allowlist are the legitimate providers.

---

## 11. Review cadence

Revisit this model whenever: a new provider or data source is added, the egress allowlist changes, the update mechanism changes, or a dependency with network or serialization capability is introduced. Any such change requires re-checking §6 and §9 in the same PR.
