# DECISIONS.md — Standing Decision Record & Autonomy Charter

> Read this together with `CLAUDE.md` at the start of every agent session.
> Purpose: let sessions run long, milestone-sized tasks without stalling on
> questions that are already answered — while making the hard stops explicit.
> This file is authored and amended at the top model tier only, with the
> owner's approval. If work-in-flight conflicts with this file, this file wins;
> if this file conflicts with SECURITY.md/THREAT_MODEL.md, those win — stop
> and report.

## 1. Settled decisions (do not relitigate)

- License: dual **MIT OR Apache-2.0**. Name: **QuotaPane** is the product name — the working name was adopted permanently by owner decision **D2, 2026-07-26**. The pre-release naming gate is closed; there is no rename pending. Binaries are `quotapane` (window) and `quotapane-cli` (headless) per **D3**; the crate names `usage-core` / `usage-ui` / `usage-cli` are product-neutral and deliberately unchanged.
- v1 scope: **Claude + Codex subscription quota only.** Official Admin/usage billing APIs are **out of scope (ADR-002, 2026-07-23):** admin-key-only, unavailable to individual subscribers, no audience overlap, and holding an org-admin key would break the trust-boundary thesis. Any future cost view is token-free via `OtelSource` (M5), never an admin key.
- Undocumented endpoints are **in**, disclosed in the docs (README's Disclaimer and SECURITY.md's presenting-as-the-official-client section — there is deliberately no runtime nag), failing closed on schema drift. QuotaPane sends official-client User-Agents (`claude-code/<ver>`, and the Codex equivalent) — a deliberate, disclosed choice (README + SECURITY.md).
- Stack: Rust + egui/eframe (glow, no wgpu; ADR-001 rejected Tauri). `ureq` + `rustls`, synchronous, thread-based poller (no tokio). Windows is the primary target; macOS/Linux best-effort.
- Expired tokens: **error + instruct** ("run `claude`/`codex` to refresh"). The app never writes credential files; refresh is delegated to official CLIs.
- Verified hosts: Claude subscription = `api.anthropic.com` `/api/oauth/usage`; Codex (ChatGPT-plan) subscription = `chatgpt.com` `/backend-api/wham/usage` (**not** `api.openai.com`; verified against the open-source Codex CLI).
- Poll discipline: ≥180 s hard floor between polls per provider; exponential backoff capped at 30 min; honor `retry-after`.
- System tray icon is approved scope, sequenced **after** M3 (as M3.5). Windows DeskBand is rejected (deprecated).

## 2. Roadmap state

M0 (skeleton + trust boundary) ✅ · M1 (Claude provider, headless) ✅ · M2 (live window) ✅ · M3 (Codex + multi-provider window) ✅ **(visually accepted 2026-07-20 — Codex section renders)** · M3.5 (system tray) ✅ **(visually accepted 2026-07-24)** · window look pass (percent + reset countdown, slim titlebar) ✅ **(visually accepted 2026-07-24)** · **M4 (official billing) — WITHDRAWN (ADR-002).** · **M5 — frozen at M5a for v1.0 (owner decision D1, 2026-07-26)**: M5a per-model breakdown via a collapsible toggle, speced 2026-07-24, implemented 2026-07-25, ✅ **(visually accepted 2026-07-27 — two-line per-model rows render un-clipped; the expanded-state bottom cutoff is accepted, with polish queued post-1.0)**. Deferred to a post-1.0 milestone: history/sparklines, forecast-to-limit, thresholds/alerts, the token-free `OtelSource`, and the expanded-window bottom polish. (The Codex User-Agent flag already ships in the GUI — gap report §E — so it is v1.0 scope as built; CLI parity deferred.) · **M6 ship ✅ (owner-accepted 2026-07-28 — v1.0.0 published)**: QuotaPane adopted as the name; full-history secret scanning and a tag-triggered release pipeline (cosign v3.0.6-pinned keyless bundle signing, provenance attestations, SHA256SUMS, draft-only) in CI; every doc claim reconciled with shipped reality; history identity rewrite; repo public; v1.0.0 cut from c363b56 and six-step outsider-verified with negative controls before the owner published. · **M7a per-model truth ✅ (v1.1.0 published — owner-accepted)**: Claude per-model via the endpoint's generalized `limits` array (surfaces the Fable weekly-scoped quota); UI hides untouched buckets while CLI/JSON stay truthful (owner decisions 2026-07-29). · **M7b Cipher Pine visual pass ✅ (v1.2.0 published — owner-accepted 2026-07-29)**: direction B (terminal: grid, mono, // labels, cardinal prompt, status cursor), live-miniature tray icon painting mark 1c from real usage, marks 1b/1c adopted, no vendor logos (owner decisions 2026-07-29). · **M8 pace ✅ (v1.3.0 published — owner-accepted 2026-07-29)**: elapsed-time pace markers on every bar; burn-rate forecast-to-limit from an in-memory snapshot ring, surfaced only when projected exhaustion precedes the reset; QuotaWindow gains duration_secs (new nullable JSON key); sparklines/persistence deferred to v1.4 (owner decisions 2026-07-29). · **Ruleset (owner decision 2026-07-29)**: protect-main requires PRs + the 7 status checks with Repository admin on the bypass list — admin sessions keep direct-push as the working model until a second contributor exists; then the bypass comes off and the specs are rewritten for PR flow as their own gate. Sessions must not re-flag admin bypass events as violations; they are the design. · **M9 security-review remediation ✅ (v1.4.0 published — owner-accepted 2026-07-30)**: an owner-commissioned adversarial review (advisory session, 2026-07-29/30) produced ten confirmed findings, each independently re-verified at the top tier before remediation (an eleventh — the §1 disclosure-claim drift — was found post-review by a push session and fixed in 847f334); M9a = doc truth (this commit), M9b = behavior with its doc line in the same commit (CLI --allow-proxy, --debug-raw redacted by default + --debug-raw-unsafe, Retry-After cap, timestamp validation, auth-error retry floor), M9c = top-tier §4.1 coherence pass incl. ci.yml SHA-pinning; ships as v1.4.0 (owner decisions 2026-07-30). Advisory sessions deliver reports, never files — one top tier (owner decision 2026-07-30). Post-1.0 backlog: packaging (WinGet/Homebrew/AUR), deferred M5 features (history/sparklines, forecast, thresholds/alerts, OtelSource, CLI User-Agent parity), ruleset tightening (require PRs + status checks now that direct-push sessions are done), dead `RateLimitHeaders` cleanup, dormant-cadence decision.

> Design note: the Usage-window look pass (per-bar percent + reset countdown, slim titlebar with minimize/close) shipped 2026-07-23 (commits b186446 / c62364a).

## 3. Pre-approved defaults (no need to ask)

- Commit style: imperative subject prefixed with the milestone (e.g. `M3: …`). Separate commits for separable concerns. Never commit secrets; never commit a container-regenerated `Cargo.lock` over the repo's.
- Dependabot PRs: **merge only if** the diff is a version bump of an already-present dependency or GitHub Action AND its CI run is green. Anything else (new crate, feature change, major bump of a trust-boundary dep): stop and report with the diff.
- `cargo fmt` and clippy-suggested fixes in **non-boundary** code: apply freely. Test additions in non-boundary code: apply freely.
- Doc typo/clarity fixes outside `SECURITY.md`/`THREAT_MODEL.md`/`ARCHITECTURE.md`: apply freely.
- Verification bar for every push: `cargo test --workspace --locked`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings` all clean locally; then confirm the Actions run (all jobs, especially `windows-latest`).

## 4. HARD STOP CONDITIONS — halt, report, never work around

1. Any edit to trust-boundary or protected paths: `crates/usage-core/src/egress/**`, `crates/usage-core/src/credentials/**`, any security-invariant test, `deny.toml`, `SECURITY.md`, `THREAT_MODEL.md`, `.github/**`, `.cargo/**`, `.claude/**`. These are authored/reviewed at the top tier only. If a task turns out to require touching them, stop. (The single, narrow exception is §4a below.)
2. Adding, removing, or version-pinning **any dependency** beyond §3's Dependabot rule.
3. Any change to the egress allowlist or anything that would weaken a SECURITY.md invariant.
4. Anything involving the owner's real credentials beyond running the project's own built binaries as intended. Never construct ad-hoc requests with real tokens. Never print/log/persist token material — key *names* only, never values.
5. UI acceptance: the owner's eyes only. **Never capture the owner's screen.** Report behavior programmatically and stop for visual confirmation.
6. CI failure your own change doesn't explain after one honest look at the logs.
7. A conflict between this file, the specs, or the on-disk code — including preconditions that don't match reality. (Precedent: a resent instruction is not authorization; stale state is not truth. Verify, then stop if it doesn't add up.)
8. The gates themselves: milestone plans and milestone acceptance belong to the owner. Finish the milestone's pre-approved scope, then stop.

## 4a. The one exception to §4.1 — verify, don't author

A session below the top tier MAY commit and push changes that land in the §4.1 protected paths **only** when **all** of the following hold:

1. **Pre-authored at the top tier.** The exact bytes were authored (or reviewed) at the top tier and supplied to the session verbatim — as full file contents or an exact patch embedded in the goal prompt, or already written to disk by a top-tier step named in the prompt.
2. **Verified byte-for-byte before committing.** The session confirms the working tree matches the supplied bytes exactly (e.g. `git diff` review, or a hash/`cmp` compare) **and** confirms no *other* protected-path bytes changed in the same commit. Restoring an already-committed file to its reviewed state (e.g. `git checkout --` to discard editor churn such as CRLF flips) also qualifies — that lands *reviewed* bytes, nothing new.
3. **Authors nothing itself.** The session writes no new content and makes no "fix-ups", reformatting, or opportunistic edits in those paths. If it needs to *change* even one byte, §4.1 reapplies: stop and hand back to the top tier.

Any mismatch, ambiguity, or temptation to edit → **stop and report.** Verifying and landing top-tier bytes is permitted; originating or modifying them is not. Doc-only, non-code protected files (e.g. this file, `SECURITY.md`) authored at the top tier are the typical case; a code path under `egress/`/`credentials/` should almost always come back to the top tier to *build and test*, not just to commit.

## 5. Model routing (summary — CLAUDE.md is authoritative)

Top tier (Cowork/Fable or Opus): everything in §4.1's path list, architecture/threat-model decisions, this file. Sonnet 5: the floor for every standalone Claude Code session, including goal-prompt sessions. Haiku: in-session Task-tool subagents only, never a session's model.

## 6. Session protocol for goal prompts

Read `CLAUDE.md`, then this file, then the goal prompt. Verify the prompt's preconditions against the working tree before acting. Work the phases in order; run the §3 verification bar before every push; keep a running log of what was done, what was skipped, and why. End with: commits + CI URLs/results, deviations from the prompt, anything hit from §4, and exactly what the owner must do next (approvals, visual checks, file moves into protected dirs).

**Every goal prompt states the session's model tier at the top.** If any phase is expected to land changes in a §4.1 protected path, the prompt MUST say so explicitly and take one of two routes: (a) escalate that phase to the top tier to author it, or (b) carry the pre-authored bytes and instruct a §4a verify-and-commit (which a floor-tier session may perform). A phase that *unexpectedly* reaches a protected path is a §4.7 conflict — stop, do not improvise.
