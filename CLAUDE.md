# QuotaPane

Always-on-top desktop window showing live AI usage/quota across Anthropic + OpenAI, read locally from the user's own credentials. Rust cargo workspace, three crates: `usage-core` (the trust boundary), `usage-ui` (egui, pure render), `usage-cli` (headless `--json`). **The trust boundary IS the product** — a tiny, fully auditable credential-touching surface is the headline feature, not a nice-to-have.

Before non-trivial work, read `ARCHITECTURE.md` (design, crate layout, roadmap), `SECURITY.md` (invariants, disclosure policy), and `THREAT_MODEL.md`.

## Security-critical paths — top-tier models only

Changes to any of the following must be **authored or reviewed at the top model tier** (Fable/Opus-class). No exceptions, including "trivial" edits:

- `crates/usage-core/src/egress/**` — the single HTTP chokepoint and host allowlist
- `crates/usage-core/src/credentials/**` — credential loaders, `Secret<T>` (zeroize, redaction)
- Any test enforcing a security invariant (deny-by-default egress, no-persistence, redaction/zeroize)
- `SECURITY.md`, `THREAT_MODEL.md`, and any architecture/threat-model/security-review decision
- `deny.toml`, dependency additions, and release-signing/provenance workflow files

If you are a lower-tier session or subagent and a task turns out to touch these paths: **stop, do not improvise a fix**, and either hand the change to the `security-reviewer` agent or flag it for top-tier review in the handoff notes.

## Model routing policy

The orchestrating session (top tier — Fable 5 in Cowork, Opus in Claude Code; the default is pinned in `.claude/settings.json`) owns model selection and delegates downward. Follow this table by default; justify any departure in your response:

| Tier | Models | Work |
|---|---|---|
| Top | Fable 5 / Mythos 5 / Opus (latest) | All security-critical paths above; architecture, threat-model, and security-review decisions; planning and orchestration |
| High | Opus 4.8 | Complex builds, substantial refactors, hard debugging |
| Mid | Sonnet 5 | Everyday implementation: well-specified modules, tests, CI config, packaging, docs. **Also the floor for any standalone Claude Code session/handoff, even purely mechanical tasks** — Haiku does not support Auto-Mode, so pointing a session at it forces manual model switching. |
| Low | Haiku 4.5 | Mechanical work (small edits, formatting, renames) — **in-session subagent delegation only** (the `mechanical` agent via the Task tool), never as a standalone session's model |

Delegation mechanics inside a session: use the Task tool with the project subagents in `.claude/agents/` — `implementer` (sonnet), `mechanical` (haiku), `security-reviewer` (opus, read-only). Batch routine implementation into `implementer`; never delegate trust-boundary authoring below the top tier.

## Handoff format

When routing work to a separate Claude Code session, output a labeled, self-contained prompt the user can paste, stating: (1) the model to set, (2) the crate/files it applies to, (3) the full task spec including relevant invariants, (4) what must come back for top-tier review. State the routing decision and reasoning at each milestone so the user can override it.

## Non-negotiable invariants (never weaken; each is test-backed)

1. Tokens are never persisted; config stores preferences only.
2. Tokens never appear in logs, `Debug` output, telemetry, or crash reports (`Secret<T>` + zeroize + redaction).
3. Egress is deny-by-default through the single chokepoint; non-allowlisted hosts are a hard error.
4. No first-party telemetry. No silent auto-update (check-and-notify only, off by default).
5. Credential files are opened read-only; token refresh is delegated to the official `claude`/`codex` CLIs.
6. Proxy support is opt-in with an explicit TLS-inspection warning.

A change that weakens any invariant is a breaking security change: call it out explicitly, route it to the top tier, and update `SECURITY.md`/`THREAT_MODEL.md` in the same change.

## Build & conventions

- Rust stable; `Cargo.lock` committed; every new dependency justified in `CONTRIBUTING.md` and vetted against `deny.toml`.
- CI must stay green on `cargo test`, `cargo-deny`, `cargo-audit` across Windows/macOS/Linux. Windows is the primary release target.
- Test fixtures use synthetic tokens only — never real credentials, even in local scratch files.
- Roadmap order matters: trust boundary first, headless proof second, UI third (see ARCHITECTURE.md §9).
