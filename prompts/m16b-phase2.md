You are a QuotaPane implementation session working in `C:\dev\QuotaPane\QuotaPane`.
Read `CLAUDE.md` and `DECISIONS.md` first and treat them as binding. You are
running interactively under the owner's eye, not headless.

# Task — M16 Phase 2, and only Phase 2

The spec is `prompts/m16-agents-refine.md`. Implement **§2.1 through §2.6**. Phase 1
is already done and on `main`; do not redo, revisit, or revert any of it.

Read `reports/m16-endgate.md` before you start — especially its Deviations section.
That is the previous session's account of Phase 1 and it tells you what state you
are inheriting.

## State you are inheriting

- `main` is at `93c1e70`, working tree clean, `origin/main` in sync.
- Phase 1 shipped as `eb5ae3e`. Its API is on `main` and tested: `TurnState`,
  `turn_for`, `AgentSession::turn`, `::duration`, `::cli_version`, `::pulse`,
  `PULSE_BUCKETS`, `PULSE_CAP`.
- **D1 and D2 from the Phase-1 report are ACCEPTED by the top tier.** `MAX_KEY_DEPTH`
  and `FORBIDDEN_KEYS` stay `pub`. The rustfmt reflow inside Patch F stays. No rework,
  no reverting, no re-issuing.
- `crates/usage-ui/src/main.rs` carries four placeholder field fills in `demo_agents`
  and the `agent_row` test helper (`TurnState::Unknown`, `None`, `None`, zeros),
  commented as M16's to fill. Phase 2 replaces them — that is §2.4.
- Version stays `1.6.0`. No CHANGELOG entry. No release is being cut.

## CI — read this carefully, it is not the normal situation

**No workflow run exists for `eb5ae3e` or `93c1e70`, and none will ever be created.**
GitHub Actions was in a major outage from 2026-08-06 15:22Z until roughly
2026-08-07 00:05Z. Pushes during it were accepted but no run objects were created, and
GitHub does not backfill them. Actions is operational again now. `ci.yml` triggers on
push-to-main and pull_request only, with no `workflow_dispatch` — and `.github/**` is
§4.1-protected, so **do not add one**.

Consequences for you:

1. **Your push is the first CI run that covers Phase 1 as well as Phase 2.** All 8
   required checks must be green before you write your end-gate report.
2. **Wait in the FOREGROUND**: `gh run watch <run-id> --exit-status`. Never start a
   background watcher, never poll-and-forget.
3. **Before you push, cancel the orphaned run `31121996517`** (`gh run cancel
   31121996517`). It is on `b6fac53`, has been `queued` with zero jobs since
   2026-08-06 17:04Z, and is outage debris. Cancel only — do not re-run it, do not
   touch `.github/**`. This is safe: `b6fac53` is a reports-only commit and its code
   tree is identical to `f7adb72`, which is already 8/8 green.
4. **If CI goes red, §4.6 applies: stop and report.** Phase 1's local §3 bar was fully
   green (457 tests), so a CI-only red on Phase-1 code is a real finding, not noise.
   Report it plainly. Do not paper over it and do not change a byte of the tree to
   chase a red the change does not explain.

## The §3 bar — all four, green, before every commit

```
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
python tools/check-invariants.py
```

## Standing rules

- **Zero new dependencies.** A needed crate is a STOP and an ADR, not a decision you
  make.
- **§4.1 protected paths are top-tier only**: `crates/usage-core/src/egress/**`,
  `crates/usage-core/src/credentials/**`, any security-invariant test, `deny.toml`,
  `SECURITY.md`, `THREAT_MODEL.md`, `.github/**`, `.cargo/**`, `.claude/**`,
  `invariants.manifest`, `tools/check-invariants.py`. Phase 2 should need none of them.
  `README.md` is not protected; `.github/**` is.
- **§4.4**: never read `~/.claude/**` or `~/.codex/**` yourself — the product reads
  them read-only, sessions never do. Never print, log, or persist token material; key
  names only, never values.
- **§4.5**: UI acceptance is the owner's eyes only. Never capture his screen. Never
  self-accept a visual.
- **§4.7 stop-on-conflict**: a resent instruction is not authorization; stale state is
  not truth.
- **§4.8**: acceptance and roadmap are the owner's, not yours.
- **Same-change rule**: behavior and the claim it affects go in ONE commit.
- Commit identity is the repo-local noreply `282068396+cipherpine@users.noreply.github.com`.
  It is already configured. Do not change it.

## Commits

1. **§2.1–§2.6 in one commit** — the two-hour window and the `// N older today` /
   `// hide older` toggle, the second row line, the pulse painter, the six-row demo,
   the UI tests, and the README sentence together.
2. **The end-gate report** as its own commit.

## Mutation pass — required

The Phase-1 session already wrote Phase 2's four mutations into the mutation script.
Run them. Each must be caught by a named test, then reverted, and the working tree
verified clean afterwards:

- the two-hour split flipped to the wrong side
- the second line drawn for `Recent` rows
- the plural on `N older today`
- the pulse strip scaled against `PULSE_CAP` instead of the row's own busiest minute

If a mutation survives, do not footnote it — make it testable, fix it, and say so.

## Report

Write `reports/m16b-endgate.md`:

- commits with full SHAs and subjects
- what landed, section by section
- every deviation from the spec, with your reasoning, numbered
- the mutation table: mutation, caught/survived, which test caught it
- the local §3 bar, all four gates, with the test count
- the CI table with real UTC timestamps and the run id
- a "things I was unsure of" section — write it honestly, it is the most useful part

Do not self-accept. Acceptance is the owner's.

## Housekeeping

The repo lives on a mount that refuses `unlink`. After every git operation, sweep any
`.git/*.lock` and `.git/tmp_obj_*` that will not delete into `_to_delete/git-stale/`
with `mv`, then verify `.git` is clean.

## One difference from the headless runs

You are attended, so you may ask the owner a question instead of stopping cold. But the
stop conditions still bind: do not proceed past a §4.6 CI red or a §4.7 conflict without
his answer.
