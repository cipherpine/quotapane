# Goal prompt spec: M9b — hardening (v1.4.0 scope)

> **CORRECTION (top tier, 2026-07-30):** Phase 4 as written below was
> amended before execution — its §4a invariant-7 replacement wrongly
> described the no-flag path as "ignored" when the shipped egress gate
> fails closed (the floor caught this as a §4.7 stop; the spec author's
> error). The corrected patch — fail-closed, either casing,
> `--allow-proxy` as the CLI-only opt-in with a hint line on the error
> path — is what landed in 2e5586c. The original text is preserved
> below for the record.

Authored at the standing top tier 2026-07-30, reconciling the
owner-commissioned adversarial review (all findings independently
re-verified; see DECISIONS.md M9 entry). M9a (2672ace) landed the
doc-truth set. M9b is the behavior set, and its rule IS the slice:
every phase changes behavior AND lands the claim that behavior
affects in the SAME commit.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
ZERO new dependencies (`serde_json` is already a usage-cli
dependency; use it). Version stays 1.3.0 — the bump is M9-RELEASE's.

**§4.1 boundary:** you may not edit `crates/usage-core/src/egress/**`,
`crates/usage-core/src/credentials/**`, any existing security-invariant
test, `deny.toml`, `THREAT_MODEL.md`, or `.github/**`. Phase 4 carries
ONE pre-authored SECURITY.md patch (§4a byte-match) — that patch is
the only permitted touch on any §4.1 file, applied verbatim in the
same commit as Phase 4's code. Existing PII-guard and redaction tests
are read-only: extend coverage with NEW tests, never edit those.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "docs(prompts): add M9b hardening spec"; parent
   2672ace. Tree clean. CI green on tip. Version 1.3.0.
P2 DECISIONS.md contains "**M9 security-review remediation — underway
   2026-07-30 (v1.4.0 scope)**".
P3 Commit identity: repo-local user.email is the GitHub noreply
   address (set 2026-07-30 after a GH007 push block). Do not change
   it; do not "fix" it back to a real address.

## PHASE 1 — poller: Retry-After cap + auth-error retry floor (one commit)

In `crates/usage-core/src/poller/mod.rs` (ordinary path; its
redaction test is read-only):
- **Cap the hint.** `next_delay` currently applies `delay.max(ra)`
  AFTER the MAX_BACKOFF cap, so a hostile or malformed `Retry-After`
  induces an unbounded stall. New rule: the hint is honored up to
  MAX_BACKOFF — `delay.max(ra.min(MAX_BACKOFF))` — and the module doc
  states the cap and why (a read-only usage poll never owes a
  provider more than 30 minutes of silence).
- **Auth errors retry at the floor.** `is_auth_error(err)` = true for
  `ProviderError::TokenExpired` ONLY. An auth failure retries at
  MIN_INTERVAL with no escalation: the condition clears only when the
  user refreshes via the official CLI (invariant 6), so backoff
  postpones noticing the refresh by up to 30 minutes while the error
  text says "retry". A Retry-After hint still outranks the floor
  (politeness wins), capped as above. Document the carve-out in the
  module doc's scheduling section.
- Tests (new): hostile hint (u64::MAX secs) → exactly MAX_BACKOFF;
  hint below cap honored exactly; auth error → MIN_INTERVAL across
  all cadences and failure counts incl. u32::MAX; auth error + hint →
  hint (capped); every other ProviderError variant is NOT an auth
  error (enumerate them).

## PHASE 2 — time.rs: real calendar validation (one commit)

`crates/usage-core/src/providers/time.rs`:
- Day validation becomes calendar-true: `days_in_month(year, month)`
  with leap years (divisible by 4, except centuries unless /400);
  2026-02-31 and 2026-04-31 are rejected, 2024-02-29 accepted,
  2026-02-29 rejected.
- A timestamp with NO UTC offset is rejected (None): RFC3339 requires
  an offset, and guessing UTC is the kind of silent assumption this
  codebase doesn't make. Fail closed — a rejected timestamp renders
  as unknown, never as a wrong reset time.
- Module doc updated in the same commit to state both rules.
- Tests: the five cases above, plus a positive-offset (+05:30) and
  negative-offset round-trip, and hour/minute/second bounds.

## PHASE 3 — --debug-raw redacts by default (one commit)

`crates/usage-cli` + provider doc comment:
- Default `--debug-raw` output parses the body as JSON (serde_json)
  and replaces the VALUE of every key named `email`, `user_id`,
  `account_id`, or `id` — at any nesting depth, in arrays too — with
  the string `«redacted»`, then pretty-prints. If the body does not
  parse as JSON, print `(body withheld: not valid JSON — use
  --debug-raw-unsafe for exact bytes)` and no body: fail closed.
- New flag `--debug-raw-unsafe`: byte-exact dump (today's behavior),
  preceded by one stderr warning that the output may contain account
  identifiers and email. Implies --debug-raw. Not a rename: existing
  `--debug-raw` invocations keep working, safer.
- Same commit, fix the self-contradiction: the `debug_raw_body` doc
  comment in `codex_subscription.rs` (~:120) currently calls the body
  "non-secret" while the module header documents PII in it. Reword to
  name the PII and point at the CLI's default redaction. README's
  CLI flags line documents both flags and the default.
- Tests (new): synthetic body with nested/array PII → output contains
  «redacted» and NONE of the sentinel values; unsafe flag → bytes
  exact; non-JSON body → withheld notice, no body; help text lists
  both flags. Existing PII-guard tests untouched.

## PHASE 4 — CLI --allow-proxy; GUI stays hard-off (one commit)

- `quotapane-cli` gains `--allow-proxy`: constructs
  `Egress::new(true)` for that run, after printing a warning that a
  TLS-inspecting proxy can observe the bearer token at its decryption
  point. Without the flag: `Egress::new(false)` as today, and if any
  of `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` is set, print one notice
  that the proxy environment is being ignored (mention the flag).
  You are CALLING the egress constructor with a different argument —
  the egress module itself is not edited; if a change there seems
  needed, §4.7 STOP.
- The GUI keeps `Egress::new(false)` unconditionally — add a code
  comment at its call site: deliberate, documented in SECURITY.md
  invariant 7.
- SAME COMMIT, §4a byte-match on SECURITY.md, replace exactly once:
OLD: 7. **Proxy is opt-in.** If `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` is set, the app warns that a TLS-inspecting proxy can observe the bearer token, and requires explicit opt-in before sending anything through it.
NEW: 7. **Proxy is opt-in, and CLI-only.** The window never uses a proxy — its egress is constructed proxy-off unconditionally, as a deliberate choice. `quotapane-cli --allow-proxy` enables proxy support for that single run after a printed warning that a TLS-inspecting proxy can observe the bearer token; without the flag, `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY` are ignored and a notice says so.
  (Byte-match OLD first; nothing else in SECURITY.md changes.)
- Same commit: ARCHITECTURE.md's proxy sentence (~:95) updated to
  match (ordinary file, your words, consistent with the invariant);
  README flags line gains --allow-proxy.
- Tests (new): flag → Egress::new(true) reached (observable via a
  seam in usage-cli, not by editing egress); no flag + proxy env →
  ignored + notice; warning text present with flag; GUI source
  contains no --allow-proxy surface (grep-style test or review note).

## VERIFY + SHIP

Full bar from `cargo clean -p usage-core`. Push. CI 7/7 green. No
Cargo.lock movement beyond nothing (no dep changes at all). Version
still 1.3.0.

## END GATE — STOP

Report per phase: SHA, what the doc line in that commit was. Plus CI
run, test delta, the complete CLI surface change list for the v1.4.0
CHANGELOG (--allow-proxy, --debug-raw default change,
--debug-raw-unsafe), and any place a spec number or rule proved wrong
— flag, don't retune. M9c (top-tier §4.1 coherence pass + ci.yml
pinning) and M9-RELEASE follow; neither is yours. STOP.
