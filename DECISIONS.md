# DECISIONS.md — Standing Decision Record & Autonomy Charter

> Read this together with `CLAUDE.md` at the start of every agent session.
> Purpose: let sessions run long, milestone-sized tasks without stalling on
> questions that are already answered — while making the hard stops explicit.
> This file is authored and amended at the top model tier only, with the
> owner's approval. If work-in-flight conflicts with this file, this file wins;
> if this file conflicts with SECURITY.md/THREAT_MODEL.md, those win — stop
> and report.

## 1. Settled decisions (do not relitigate)

- License: dual **MIT OR Apache-2.0**. Placeholder name **QuotaPane** stays until the pre-release naming decision (owner-only, M6).
- v1 scope: **Claude + Codex subscription quota only.** Official Admin/usage billing APIs are opt-in advanced mode, deferred to M4.
- Undocumented endpoints are **in**, gated behind the runtime disclaimer, failing closed on schema drift. QuotaPane sends official-client User-Agents (`claude-code/<ver>`, and the Codex equivalent) — a deliberate, disclosed choice (README + SECURITY.md).
- Stack: Rust + egui/eframe (glow, no wgpu; ADR-001 rejected Tauri). `ureq` + `rustls`, synchronous, thread-based poller (no tokio). Windows is the primary target; macOS/Linux best-effort.
- Expired tokens: **error + instruct** ("run `claude`/`codex` to refresh"). The app never writes credential files; refresh is delegated to official CLIs.
- Verified hosts: Claude subscription = `api.anthropic.com` `/api/oauth/usage`; Codex (ChatGPT-plan) subscription = `chatgpt.com` `/backend-api/wham/usage` (**not** `api.openai.com`; verified against the open-source Codex CLI).
- Poll discipline: ≥180 s hard floor between polls per provider; exponential backoff capped at 30 min; honor `retry-after`.
- System tray icon is approved scope, sequenced **after** M3 (as M3.5). Windows DeskBand is rejected (deprecated).

## 2. Roadmap state

M0 (skeleton + trust boundary) ✅ · M1 (Claude provider, headless) ✅ · M2 (live window, visually confirmed) ✅ · M3 (Codex provider + multi-provider window) ✅ **(visually accepted 2026-07-20 — Codex section renders)** · **M3.5 tray icon — next (top-tier review of the tray-icon dependency BEFORE any code)** · M4 opt-in official billing · M5 depth (history, forecast, thresholds; also the deferred `additional_rate_limits` per-model breakdown and a Codex User-Agent flag in the CLI) · M6 ship (naming, packaging, signed releases, repo public — decision-dense, stays interactive).

> Queued design item (owner request, 2026-07-20): after M3.5, revisit the Usage window's **look** — likely add percentage labels and/or other detail to make it more visually useful and appealing. This is a `usage-ui`-only polish pass (non-boundary); scope it as its own goal prompt when M3.5 lands.

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
