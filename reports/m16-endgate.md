# M16 end-gate — the agents pane, second pass

Headless session under the M11d dispatcher. Spec: `prompts/m16-agents-refine.md`.
The spec asks for two phases, two commits, then this report. **One phase
shipped.** Phase 2 was gated on Phase 1's CI going green on all 8 required
checks, and GitHub Actions never created a run for the commit at all — §4.6,
described exactly as found in [CI](#ci) below.

Acceptance is the owner's (§4.8) and the visuals are the owner's eyes only
(§4.5): nothing here is self-accepted, no release is cut. Version stays
`1.6.0`, no CHANGELOG entry. Zero new dependencies. No `--json` change (the CLI
does not name `usage_core::agents` at all — checked, not assumed).

## Model tier — read this first

The dispatcher addresses this session as a *floor* session, but it ran on
**Opus 5**, which `CLAUDE.md`'s routing table places at the **top tier**. That
matters: Phase 1 **authored** a security surface — the depth-3 key lookup,
`FORBIDDEN_KEYS`, and eight new tests in `crates/usage-core/src/agents.rs`,
three of them touching the invariant-8 surface. Under `CLAUDE.md` that is
top-tier-only work and under §4a a floor session may only *verify and commit*
pre-authored bytes. The spec directed this session to author it and the tier it
actually ran at permits that, so the work is in bounds — but the owner should
record it as **top-tier-authored, not floor-verified**. The six §4a patches
were applied as pure verify-and-commit, extracted from the spec's own bytes and
never retyped.

## Commits

Base: `6c87626` — *reports: M15 end-gate addendum*. Working tree clean at
session start. The push carried the top tier's own spec commit `5dc246b` with
it, as the dispatcher said it would.

| Phase | SHA | Subject |
|---|---|---|
| P1 | `eb5ae3e603229a63b4dfae25ceb677ea3e147096` | `M16: whose move is it — turn state, pulse, and a fence for the allowlist` |
| P2 | — | **not started** (gated on P1's CI; see §4.6 below) |
| report | this commit | `reports: M16 end-gate` |

Remote `main` is at `eb5ae3e`.

## Phase 1 — what landed

`crates/usage-core/src/agents.rs`, and the protected files that carry its
claim, in one commit (the same-change rule; the checker's set-equality forces
it anyway).

- **`MAX_KEY_DEPTH = 3`** — the lookup walks the record, its children, and
  theirs, via a small recursive `search_levels` behind `read_allowlisted`. The
  membership check still happens once, before a single object is walked.
- **`FORBIDDEN_KEYS`** — the thirteen names from the spec, with the `model`
  paragraph written into the const's doc comment so the next reader does not
  "fix" it.
- **`ALLOWLISTED_KEYS`** gains `version`, `branch`, `cli_version`. `absorb`
  reads the Codex branch flat-first (`git_branch`, then `branch`), so both
  spellings produce the same row and no log on the owner's disk regresses.
- **`TurnState` + `turn_for`** — Claude's `user`/`assistant` alternation, Codex
  always `Unknown`, `Recent` always `Unknown`. Only the **tail** record feeds
  it; a test proves the head's type is not what was read.
- **`epoch_secs`** — a strict hand-rolled RFC 3339 reader (no new dependency),
  with `days_from_civil` for the calendar. Rejects a two-digit year, a missing
  zone, a month of 13, a day February does not have, an hour of 25, a leap
  second, an offset without its colon, and anything before 1970.
- **`duration`** (head stamp → `last_write`, `None` and never zero when the
  clock ran backwards) and **`cli_version`** (capped at 16 *characters*).
- **`pulse`** — ten one-minute buckets of record counts, computed only when the
  session is not `Recent`. `read_head_and_tail` now returns every complete line
  of the tail chunk; `absorb` still sees only the last of them.

Protected files, applied as §4a byte patches: `SECURITY.md` invariant 8,
`THREAT_MODEL.md` T-I6 and its traceability row, `invariants.manifest`, and the
two agents.rs test patches (E and F).

## §4a — how the patches were applied

A script extracted all six OLD/NEW blocks from `prompts/m16-agents-refine.md`'s
bytes by parsing its `### Patch` sections; nothing was retyped. Every OLD was
proven to appear **exactly once** before its patch, and every NEW **exactly
once** after:

| Patch | Target | OLD before | NEW after |
|---|---|---|---|
| A | `SECURITY.md` | 1 | 1 |
| B | `invariants.manifest` | 1 | 1 |
| C | `THREAT_MODEL.md` | 1 | 1 |
| D | `THREAT_MODEL.md` | 1 | 1 |
| E | `crates/usage-core/src/agents.rs` | 1 | 1 |
| F | `crates/usage-core/src/agents.rs` | 1 | 1 |

The protected-doc footprint is exactly the patches and nothing else:
`SECURITY.md` 1 line changed, `THREAT_MODEL.md` 2, `invariants.manifest` 2
added. `tools/check-invariants.py`: **OK: 8 invariants, 30 test bindings, tags
and manifest set-equal, SECURITY.md id set matches.**

One patch did not survive the mandated formatter byte-for-byte — see D2.

## Deviations

**D1 — `MAX_KEY_DEPTH` and `FORBIDDEN_KEYS` are `pub`; the spec wrote them
private.** Forced, not chosen. `FORBIDDEN_KEYS` is referenced only from
`#[cfg(test)]` code, so as a private const it is dead code in the lib build and
`cargo clippy --workspace --all-targets --locked -- -D warnings` — the §3 bar
*and* a required check — fails on it. The alternatives were worse: an
`#[allow(dead_code)]` that hides real dead code, or inventing a runtime use for
the list, which would have turned a fence on *what may be added* into a second
filter nobody asked for and would have made "delete an entry" a weaker
mutation. `MAX_KEY_DEPTH` followed so the intra-doc links from the two public
consts do not point at a private item. Both are facts `SECURITY.md` now claims
in prose, so publishing them makes the claim auditable from the API. No
behaviour change.

**D2 — `cargo fmt` reflowed four lines inside Patch F's NEW block.** The
patch applied verbatim and was verified; the reflow happened when the §3 bar's
formatter ran. The spec line

```rust
            turn_for(ProviderId::CodexSubscription, Some("response_item"), Working),
```

is a 71-character call, past rustfmt's default `fn_call_width` of 60 (the repo
has no `rustfmt.toml`), so `cargo fmt --all` breaks it vertically:

```rust
            turn_for(
                ProviderId::CodexSubscription,
                Some("response_item"),
                Working
            ),
```

**Not one token changed** — same call, same arguments, same assertion, and the
test passes. This is the one place two of the spec's own rules collide: §4a
wants the supplied bytes exactly, and §3/CI want `cargo fmt --all --check`
clean, and the supplied bytes are not rustfmt-stable. Keeping them would have
put a required check red on purpose. I took the formatter's normalisation and
am flagging it rather than hiding it; **if the top tier wants Patch F byte-
stable on replay, re-issue it pre-formatted.** This is the only §4a byte that
differs from the spec, and it is whitespace.

**D3 — the doc sentence §1.1 names lives elsewhere.** §1.1 says to update
`ALLOWLISTED_KEYS`'s doc comment "including its sentence about the two-level
search". That sentence is on `read_allowlisted`, not on the const. I updated
both, so nothing in the module still says "two levels".

**D4 — `turn` was added to `AgentSession`.** §1.4 and §1.5 enumerate
`duration`, `cli_version` and `pulse`; no section says "add `turn`" in so many
words, but §1.6's `every_output` list and §2.2's row both require it. Recorded
so it is not read as scope creep.

**D5 — five tests beyond §1.6's list.** Three of them close holes the spec's
own mutation list would otherwise have found:
- `the_lookup_reaches_exactly_three_levels_and_no_further` — **nothing else in
  the suite catches `MAX_KEY_DEPTH` widened to 4**, which the spec requires be
  caught. It pins depth 1/2/3 reachable and depth 4 not.
- `the_forbidden_list_is_pinned_the_way_the_allowlist_is` — Patch F's protected
  test names only 6 of the 13 forbidden entries, so deleting e.g. `stdout` was
  caught by nothing. This pins the whole list, the way the allowlist already is.
- `a_tail_chunk_that_starts_mid_line_hands_back_no_fragment` — written to close
  the pass's one survivor (see the mutation table).
- Plus `the_turn_is_read_from_the_tail_line_and_never_the_head` and
  `the_cli_version_comes_from_whichever_key_the_provider_writes`.

**D6 — the UI got a four-field fill-in inside Phase 1's commit.**
`AgentSession` grew four fields, so `demo_agents` and the `agent_row` test
helper in `crates/usage-ui/src/main.rs` had to name them or the workspace would
not compile and Phase 1's own §3 bar could not be green. They are set to the
honest nothing (`TurnState::Unknown`, `None`, `None`, zeros) and commented as
M16's to fill. Phase 2 would have replaced them.

**D7 — Phase 2 was not started.** The dispatcher gates it on Phase 1's CI being
green on all 8 required checks. That gate never opened. Nothing of Phase 2 is
in the tree: no `AGENTS_RECENT_WINDOW`, no toggle, no second line, no pulse
painter, no six-row demo, no README sentence. §2.1–§2.6 are **entirely
outstanding** and are the whole of what a follow-up session must do.

## Mutation pass

Every mutation was applied to the committed tree, tested, and reverted by
script; the working tree was verified clean afterwards. Phase 1's thirteen:

| Mutation | Result | Caught by |
|---|---|---|
| `MAX_KEY_DEPTH` 3 → 2 | caught | `a_codex_branch_reads_the_same_flat_or_nested`, `extraction_is_welded_to_the_allowlist_const`, `the_lookup_reaches_exactly_three_levels_and_no_further` |
| `MAX_KEY_DEPTH` 3 → 4 | caught | `the_lookup_reaches_exactly_three_levels_and_no_further` |
| `FORBIDDEN_KEYS` loses `model` | caught | `no_allowlisted_key_can_ever_name_message_content`, `the_forbidden_list_is_pinned_the_way_the_allowlist_is` |
| `FORBIDDEN_KEYS` loses `stdout` (not named by the protected test) | caught | `the_forbidden_list_is_pinned_the_way_the_allowlist_is` |
| `turn_for`: `user`/`assistant` arms swapped | caught | `the_turn_is_read_from_the_tail_line_and_never_the_head`, `turn_state_is_read_from_the_record_type_alone` |
| `turn_for`: the `Recent` guard removed | caught | `a_codex_row_and_a_finished_row_claim_no_turn`, `turn_state_is_read_from_the_record_type_alone` |
| `epoch_secs`: the offset sign ignored | caught | `epoch_secs_reads_the_stamp_both_clis_write_and_refuses_everything_else` |
| `pulse`: bucket index off by one | caught | `the_pulse_counts_records_by_the_minute_over_the_last_ten`, `the_tail_read_widens_what_is_counted_and_not_what_is_believed` |
| `pulse`: the `Recent` skip removed | caught | `a_finished_session_has_no_pulse_at_all` |
| `duration`: saturates to zero instead of `None` | caught | `duration_runs_from_the_head_stamp_to_the_last_write` |
| `absorb`: the nested branch spelling dropped | caught | `a_codex_branch_reads_the_same_flat_or_nested` |
| `read_head_and_tail`: tail fragment no longer dropped | **SURVIVED**, then caught | — → `a_tail_chunk_that_starts_mid_line_hands_back_no_fragment` |
| `cli_version` cap 16 → unbounded | caught | `the_cli_version_comes_from_whichever_key_the_provider_writes` |

**The survivor, and what was done about it.** Deleting the `.skip(1)` that
drops the tail chunk's leading fragment passed every test. Blast radius is
small — a mid-line fragment does not parse, so it contributes no beat, and
anything that somehow did parse would still have to clear `is_record` and the
allowlist before it could be read — but the line has a documented contract and
nothing held it. Rather than footnote it I made it testable: a fixture with one
padding line exactly `TAIL_CAP` long puts the tail read's start deterministically
inside that line, and the test asserts every line handed back is a whole line of
the file. Re-run: caught. The commit was amended (it had not been pushed) so
the fix ships with the code it guards, not as an afterthought.

**Four of the spec's mutations were not run**, because they are Phase 2's and
Phase 2 does not exist: the two-hour split flipped to the wrong side, the second
line drawn for `Recent` rows, the plural on `N older today`, and the pulse strip
scaled against the cap instead of the row's own busiest minute. They are already
written into the mutation script for whoever picks Phase 2 up.

## Local §3 bar — green

Run on `eb5ae3e`, the pushed tree:

| Gate | Result |
|---|---|
| `cargo test --workspace --locked` | **457 passed, 0 failed** (cli 64 + cli-integration 13 + core 156 + ui 224 + 0 doc-tests) |
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `python tools/check-invariants.py` | OK: 8 invariants, 30 test bindings, set-equal |

`usage-core` went from 155 to 156 tests in the amend; the agents module carries
29 of them, 9 of which are new in M16.

## CI — §4.6, and it is not this session's

**No workflow run was ever created for `eb5ae3e`.** Not queued, not failed —
absent.

| Time (UTC) | Observation |
|---|---|
| 21:15:30 | `git push` accepted: `6c87626..eb5ae3e main -> main`, remote reports "8 of 8 required status checks are expected" |
| 21:18:05 → 21:31:52 | `gh api .../actions/runs?head_sha=eb5ae3e` → `total_count: 0`, polled every ~110 s in the foreground for 16 minutes |
| 21:34:38 | still `0`. `gh api .../commits/eb5ae3e/check-runs` → `total_count: 0` |

The repo-wide picture says the same thing, and says it is not about this
change:

| Run | SHA | Created | State |
|---|---|---|---|
| [31121996517](https://github.com/cipherpine/quotapane/actions/runs/31121996517) | `b6fac53` (M15 report commit) | 2026-08-06 17:04:16Z | **queued, 0 jobs, 4 h 30 m and counting** |
| [31117902909](https://github.com/cipherpine/quotapane/actions/runs/31117902909) | `f7adb72` | 15:54:55Z | success — the last run that ever started |

That matches the outage the dispatcher named (incident open 15:22 UTC; runs
accepted, no jobs created) and it is worse for `eb5ae3e`: no run object at all.

**Re-runs:** one attempt, and it is the only one available.
`gh run rerun 31121996517` → *"run 31121996517 cannot be rerun; This workflow
is already running"* — GitHub will not re-run a queued run, and there is no run
on my own commit to re-run. `ci.yml` triggers on `push` and `pull_request`
only, with no `workflow_dispatch`, so a run cannot be summoned; adding one would
be a byte changed in `.github/**`, which is both §4.1-protected and exactly the
"do not change a byte of the tree to chase it" the dispatcher forbids. **Not one
byte of the tree was changed to chase CI.**

Per §4.6 the session stopped here rather than starting Phase 2 on an unverified
Phase 1.

## What the owner must do next

1. **Watch for CI to drain.** When Actions recovers, the run for `eb5ae3e`
   should appear on push-replay or on the next push. All 8 required checks must
   be green before Phase 2 begins — that gate is unchanged, only deferred.
2. **Decide D1 and D2.** D1 (`pub` on the two consts) and D2 (the rustfmt
   reflow inside Patch F) are the two places this session's output differs from
   the spec's letter. Both are argued above; both are the top tier's call.
3. **Dispatch Phase 2.** §2.1–§2.6 are untouched and self-contained. The
   mutation script for them is already written; the Phase-1 API it needs
   (`turn`, `duration`, `cli_version`, `pulse`, `PULSE_BUCKETS`, `PULSE_CAP`,
   `TurnState`) is on `main` and tested.
4. **§4.5 stands.** Nothing visual was reviewed, because nothing visual was
   built.

## Things I was unsure of

- **Whether taking rustfmt's reflow over the spec's bytes was right** (D2). I
  judged that a required check going red on purpose is worse than a
  whitespace-only difference in a test body, and that §4a's real target is
  *authorship* of protected content, which this is not. I would rather be
  overruled on the record than have quietly `#[rustfmt::skip]`-ed it.
- **Whether `FORBIDDEN_KEYS` should have a runtime use.** It would make the
  const load-bearing and moot D1. I deliberately did not: the spec is explicit
  that the list governs what may be *added*, a second runtime filter would be
  dead weight behind the allowlist check, and it would weaken the
  "delete an entry" mutation from a failing test to a no-op.
- **`epoch_secs` rejects lowercase `t`/`z`**, which RFC 3339 permits. Both CLIs
  write uppercase and the spec said strict, so strict is what it is — but it is
  a choice, not a fact, and a CLI that changed its formatter would silently
  cost every row its pulse and duration.
