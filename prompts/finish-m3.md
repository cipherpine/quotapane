# Goal prompt: FINISH M3 (compact)

Model: Sonnet 5. Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — its §4 stop conditions override
everything below. Work the phases in order; stop only at charter stops or
the end gate.

PRECONDITIONS (any mismatch = STOP and report):
P1 crates/usage-core/src/providers/codex_subscription.rs exists.
P2 "chatgpt.com" is in ALLOWED_HOSTS (egress mod.rs) and named in
   ARCHITECTURE.md + SECURITY.md.
P3 git status shows changes ONLY in: crates/usage-core/src/providers/,
   the egress module, ARCHITECTURE.md, SECURITY.md, README.md,
   CONTRIBUTING.md, DECISIONS.md, prompts/, CLAUDE.md.
P4 cargo test --workspace --locked passes (~70 tests).

PHASE A — land the top-tier-authored work:
Commit everything above as "M3: chatgpt.com allowlist + Codex subscription
provider (verified endpoint)". Push. Record the Actions URL; all jobs must
pass (esp. windows-latest) — unexplained red = charter §4.6, STOP.

PHASE B — multi-provider window (edit ONLY crates/usage-ui/):
- Read codex_subscription.rs rustdoc FIRST — constructor and
  CODEX_DEFAULT_USER_AGENT (re-exported from usage_core::providers) are
  documented there; don't guess.
- Spawn one poller::spawn(...) per provider (Claude + Codex); drain both
  channels each frame; track staleness and last failure PER provider.
- Render one titled section per provider ("Claude", "Codex") with the
  existing bar/countdown/staleness treatment.
- If a provider's Failure message contains "not found" (absent credential
  file), render one quiet line ("Codex: not signed in — run codex login")
  instead of a red banner; the other provider is unaffected.
- Window ~320x240; keep drag-to-move and 1s repaint; on_exit stops ALL
  handles.
- Keep --client-version (Claude); add a Codex UA flag defaulting to
  CODEX_DEFAULT_USER_AGENT. Hand-rolled parsing; NO new dependencies.
- Unit-test every new pure helper (TDD encouraged). Run the DECISIONS §3
  verification bar; commit "M3: multi-provider window (Claude + Codex)";
  push; record CI.

PHASE C — CLI parity (edit ONLY crates/usage-cli/):
Add --provider claude|codex|all (default claude, backward compatible).
"all": JSON mode emits an array of snapshots; text mode prints both
summaries. Absent credentials for a selected provider → clean stderr +
non-zero exit, no panic. Tests for the parsing; verification bar; commit
"M3: usage-cli multi-provider (--provider)"; push; record CI.

END GATE — STOP HERE. Report: commits + CI runs (all jobs), test-count
delta, any deviations. Then the owner's checklist:
1. cargo run -p usage-ui -- --client-version <claude --version>
   → both sections live = M3 acceptance. (Empty Codex bars = response
   wrapper differs; report back, do not fix — trust-boundary path.)
2. Optional: cargo run -p usage-cli -- --once --json --provider all
Do NOT start tray work (M3.5). Never touch charter §4.1 paths. Never
capture the screen.
