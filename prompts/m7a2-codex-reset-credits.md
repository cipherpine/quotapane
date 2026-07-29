# Goal prompt spec: M7A2 — Codex rate-limit reset credits (v1.1.0 slice)

Authored at the top tier (Cowork bridge) 2026-07-29, on the owner's
addition to the v1.1.0 scope: surface Codex's rate-limit reset credits.
Runs AFTER M7a (which landed at 196fd56); ships in the same v1.1.0.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): floor-authorable throughout — `model/`, `providers/`,
`usage-ui`, `usage-cli` are ordinary paths; no §4.1 phase exists. HARD
RULE restated: the raw body carries `user_id`/`account_id`/`email`; no
PII-named field may be added to any usage-response struct. Parse ONLY
the two count fields below.

## THE EVIDENCE (owner's --debug-raw, 2026-07-29)

The Codex wham/usage response carries:

    "rate_limit_reset_credits": {
      "available_count": 1,
      "applicable_available_count": 0
    }

Semantics: `available_count` = reset credits the account owns;
`applicable_available_count` = how many are usable right now (0 unless
currently rate-limited). Claude has no equivalent.

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is 196fd56 (M7a phase 2), tree clean EXCEPT:
   ```
    M prompts/m6-launchers.md
   ?? prompts/m7a2-codex-reset-credits.md
   ```
   CI green on the tip.
P2 DECISIONS.md §2 contains the M7a segment (M7a phase 0 applied). This
   prompt does NOT touch DECISIONS.md — it is in-scope of the already-
   recorded v1.1.0 slice per the owner's 2026-07-29 addition.

## PHASE 0 — commit this spec + the launcher index

    docs(prompts): add M7A2 spec (Codex reset credits)

## PHASE 1 — model + parser (one commit)

`crates/usage-core/src/model/mod.rs`:
- New plain DTO (non-secret, serde derive per house rules):
      pub struct ResetCredits {
          pub available: u32,
          pub applicable_now: Option<u32>,
      }
- `ProviderSnapshot` gains `pub reset_credits: Option<ResetCredits>` —
  no `skip_serializing_if`; it serializes as `null` when absent, same
  contract as the rest of the snapshot.

`crates/usage-core/src/providers/codex_subscription.rs`:
- Raw struct parses ONLY:
      RawResetCredits { available_count: Option<u32>,
                        applicable_available_count: Option<u32> }
  wired as `rate_limit_reset_credits: Option<RawResetCredits>` on the
  existing raw usage struct. No other new fields — the field-ignoring
  defense works by the field not existing.
- Map to `Some(ResetCredits { available, applicable_now })` when
  `available_count` is Some; else None.

`claude_subscription.rs`: sets `reset_credits: None` (one line per
construction site). Fix all struct-literal sites the new field breaks
(poller/UI/CLI test fixtures) — mechanical, `None` everywhere except
Codex fixtures.

Tests (synthetic values): Codex fixture with the evidence shape →
available 1, applicable_now Some(0); fixture WITHOUT the key → None
(degradation); Claude snapshot → None; extend the PII-guard pattern
over the new raw struct.

## PHASE 2 — surface it (one commit)

`usage-ui`: in a provider pane whose snapshot has `Some(reset_credits)`,
render one small dim mono line after the windows (above the per-model
toggle): `resets available: N` — nothing when None, so Claude is
untouched. Layout-harness tests: line present for Codex-style snapshot,
absent for None, no width overflow.

`usage-cli`: extend the JSON-pinning tests — `"reset_credits"` key is
ALWAYS present (null for Claude, object with both fields for the Codex
fixture), including in `--provider all` array form.

## PHASE 3 — verify + ship

cargo clean -p usage-core, then the full §3 bar. Push. CI 7/7 green.

## END GATE — STOP (owner's eyes, §4.5)

Report SHAs, CI, test count. The owner's visual check now covers M7a +
this together: Claude pane shows the Fable row (~40%) with no Spark row
under Codex, and the Codex pane shows `resets available: 1`. Do NOT
bump the version or tag — the v1.1.0 release prompt follows the owner's
acceptance of both.
