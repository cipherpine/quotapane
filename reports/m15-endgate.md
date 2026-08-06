# M15 end-gate — the agents pane (`usage // agents`)

Headless session under the M11d dispatcher. Spec: `prompts/m15-agents.md`.
Two phases, two commits, in order, plus this report. **Acceptance is the
owner's (§4.8), and the visuals are the owner's eyes only (§4.5) — nothing
here is self-accepted, and no release is cut.** Version stays `1.6.0`; no
CHANGELOG entry (both belong to M15-RELEASE). Zero new dependencies; no
`--json` change.

## Model tier — read this first

The dispatcher prompt addresses this session as a *floor* session, but the
session ran on **Opus 5**, which `CLAUDE.md`'s routing table places at the
**top tier**. That matters because Phase 1 **authored four new
security-invariant tests** (`crates/usage-core/src/agents.rs`, tagged
`// INV:8`) and a new module. Under `CLAUDE.md` those are top-tier-only work,
and under §4a a floor session may only *verify and commit* pre-authored bytes.
The spec directed this session to author them, and the tier it actually ran at
permits that — so the work is in bounds, but the owner should record it as
top-tier-authored rather than floor-verified. The five §4a doc/manifest patches
were still applied as pure verify-and-commit, byte-exactly from the spec.

## Commits

Base: `921138a` — *reports: M14 end-gate — density pass shipped, three
deviations flagged*. Working tree clean at session start.

| Phase | SHA | Subject |
|---|---|---|
| P1 | `9947e86a8417ee55fe9973928882aa4e9452a98d` | `M15: who is working right now — the agents scanner, metadata only` |
| P2 | `f7adb725bbf5c2b02060e290edc2e19d53e1d75d` | `M15: usage // agents — a second view in the same 320px pane` |

Phase 2 was not started until Phase 1's commit was green on all 8 required
checks, waited for in the foreground (`gh run watch <id> --exit-status`). No
background watcher was ever *chosen*: the harness moved two 10-minute waits to
the background at its own timeout cap, and each time the wait was re-attached
in the foreground and the run's state re-read directly. Every conclusion in
this report was read from `gh run view` after the run completed, not from a
backgrounded process.

## CI

| Phase | Run | SHA | Conclusion |
|---|---|---|---|
| P1 | <https://github.com/cipherpine/quotapane/actions/runs/31116076559> | `9947e86` | success |
| P2 | <https://github.com/cipherpine/quotapane/actions/runs/31117902909> | `f7adb72` | success |

Per-check, run 31117902909 (the tip):

| Check | Conclusion |
|---|---|
| build & test (windows-latest) | success |
| build & test (ubuntu-latest) | success |
| build & test (macos-latest) | success |
| cargo-deny (licenses, bans, advisories, sources) | success |
| cargo-audit (RustSec advisories) | success |
| gitleaks — full-history secret scan | success |
| invariant 4 — no telemetry | success |
| invariants — manifest, docs, and tests agree | success |

### The three infrastructure re-runs (§4.6, honest look taken)

The P2 run failed three times before going green, and **not once on the
diff**. Every failure was the same GitHub Actions control-plane error, logged
*before any step of the job ran*:

```
Failed to resolve action download info. Error: Service Unavailable
##[error]Service Unavailable
##[error]Failed to resolve action download info.
```

Attempt 1: `invariants` failed this way; `cargo-deny` and `cargo-audit` were
cancelled by fail-fast. Attempt 2: `invariants`, `cargo-deny` and
`build & test (windows-latest)` failed this way. Attempt 3:
`build & test (windows-latest)` failed this way and cancelled `invariants`.
Attempt 4: all green. `gh run rerun --failed` was the only action taken; no
byte of the tree changed between attempts, and P1's identical pipeline had
been green on the same eight checks forty minutes earlier. §4.6 is a stop for
a failure the change does not explain — this one is fully explained by the
logs, and the explanation is GitHub's, not the diff's.

## The §3 bar

Run clean locally before every push, on Windows:

```
cargo fmt --all --check                                          clean
cargo clippy --workspace --all-targets --locked -- -D warnings   clean
cargo test --workspace --locked                                  442 passed, 0 failed
python tools/check-invariants.py                                 OK: 8 invariants, 28 test
                                                                 bindings, tags and manifest
                                                                 set-equal, SECURITY.md id
                                                                 set matches.
```

442 = 64 (`usage-cli` unit) + 13 (`usage-cli/tests/cli.rs`) + 141
(`usage-core`) + 224 (`usage-ui`). Baseline at `921138a` was 411, so the
milestone added 31 tests: 14 in `usage-core::agents`, 17 in `usage-ui`.

## Phase 1 — `usage_core::agents` + invariant 8

One commit, exactly as the spec's same-change rule requires: the module, the
four `// INV:8` tags, `invariants.manifest`, `SECURITY.md` and
`THREAT_MODEL.md` landed together. The checker's tag↔manifest set-equality
makes any other sequencing red, and that was verified rather than assumed
(see the mutation table).

**Shape.** `AgentSession { provider, short_id, project, branch, state,
last_write, age, is_subagent }`; `AgentState { Working, Idle, Recent }`;
`SessionRoots { claude, codex }` dependency-injected, with `from_env()` for
production and a pure `roots_from(home, codex_home)` underneath it so the
resolution rules are testable without a real home. `ACTIVE_WITHIN` 120 s,
`IDLE_WITHIN` 1800 s, `LOOKBACK` 24 h, `TAIL_CAP` 16 KiB, each with the doc
comment naming its rationale, and the ladder asserted at **compile** time
(`const _: () = assert!(…)`) as well as in a boundary-table test.

**The aperture.** `ALLOWLISTED_KEYS` is the whole list, and
`read_allowlisted` is the only way a value leaves a parsed line — it checks
membership before returning, and refuses containers (objects/arrays) even
under an allowlisted name. Deleting an entry from the const removes a
capability, not a mention.

**Bounded reads.** `candidates()` does the whole traversal with `read_dir` +
`metadata` and opens nothing; `scan()` opens only what `candidates()` returns.
"Only files inside the lookback are ever opened" is therefore structural, and
the test asserts it against the candidate list rather than against a promise.
An opened file yields at most two `TAIL_CAP` reads, one from each end.

**§4a patch application.** All five OLD/NEW pairs were extracted
**programmatically from `prompts/m15-agents.md`'s own bytes** (regex over the
`**Patch X — …**` headers and the two fenced blocks under each), never
retyped. Pre-flight and post-apply proof:

| Patch | Target | OLD bytes | unique before | NEW bytes | absent before | unique after |
|---|---|---|---|---|---|---|
| A | `SECURITY.md` (invariant list) | 601 | yes | 1264 | yes | yes |
| B | `SECURITY.md` (credential handling) | 244 | yes | 404 | yes | yes |
| C | `invariants.manifest` | 104 | yes | 733 | yes | yes |
| D | `THREAT_MODEL.md` (§9 table) | 318 | yes | 647 | yes | yes |
| E | `THREAT_MODEL.md` (threat list) | 147 | yes | 411 | yes | yes |

`git diff` over the three protected files was reviewed line by line before the
commit: it contains those five hunks and nothing else. No other protected-path
byte changed in the commit (§4a.2). The spec's pre-flight note (unique at the
top tier on 2026-08-06 against `6f54ff4`) was re-proven against this session's
HEAD, as instructed.

## Phase 2 — the tab, `--agents-demo`, README

**Switcher.** `usage // agents` in the titlebar, current view `TEXT`, other
`TEXT_FAINT`, click to switch, default `usage`, not persisted. It coexists
with `StartDrag` two ways, and the second one was a real find: egui's `Label`
ORs `Sense::click_and_drag` into whatever sense it is given whenever
`selectable_labels` is on — which it is, for every label in this window. An
explicit `.sense(Sense::click())` was therefore doing nothing, and a drag
starting on a switcher word would have been swallowed as a text selection
instead of moving the window. `.selectable(false)` fixes it, and the code
comment now describes what actually happens. Found by mutation testing, not by
reading.

**Scanning.** `should_scan(view, agents_demo, since_last)` is the entire
policy as one pure function; `refresh_agents` is the only place `agents::scan`
is named in the crate and asks it first. A test pins the policy at every
input, and a source-scan test pins that there is exactly one call site and
that it is inside the guarded function. A window on `usage` does not
`read_dir` a session root, does not stat one, and does not open a log.

**Demo.** `--agents-demo` opens **on** the agents view (a fixture nobody is
looking at reviews nothing) with `SessionRoots::default()` — nowhere — so no
real log is reachable even if the gate were wrong. Four rows: Claude working,
Claude working + subagent, Codex idle, Codex recent with no branch.

## Mutation checks — every one caught

Each mutation was applied to a clean tree, the suite run, and the tree
restored byte-for-byte (verified). The two the spec names explicitly are M1
and M2/P9.

| # | Mutation | Killed by |
|---|---|---|
| M1 | delete a key (`cwd`) from `ALLOWLISTED_KEYS` | `extraction_is_welded_to_the_allowlist_const`, `a_codex_rollout_reads_as_one_row` |
| M2 | route a content field into a pub field | `sentinel_content_never_reaches_any_output` + 4 others |
| M3 | drop the membership check in `read_allowlisted` | `extraction_is_welded_to_the_allowlist_const` |
| M4 | open files past `LOOKBACK` | `a_file_outside_the_lookback_is_never_opened` |
| M5 | accept any file, not just `.jsonl` under the roots | `scanner_opens_only_jsonl_under_the_session_roots` |
| M6 | drop the mtime-only id fallback | `unparseable_file_still_reports_liveness_from_mtime` |
| M6b | drop the mtime-only project fallback | `unparseable_file_still_reports_liveness_from_mtime` |
| M7 | let a container through under an allowlisted key | `extraction_is_welded_to_the_allowlist_const` |
| M8 | stop honouring `isSidechain` | `is_sidechain_marks_a_subagent_and_its_absence_does_not` |
| M9 | comment out one `// INV:8` tag | `tools/check-invariants.py` (exit 1, names the exact binding) |
| P1 | switcher labels stop sensing clicks | `clicking_a_switcher_label_switches_the_view` |
| P1b | switcher labels become selectable again | `dragging_from_a_label_moves_the_window_and_switches_nothing` |
| P2 | a finished drag counts as a click | `dragging_from_a_label_moves_the_window_and_switches_nothing` |
| P3 | `should_scan` stops checking the view | `no_scan_is_reachable_while_the_usage_view_shows` |
| P4 | `refresh_agents` drops its guard | `the_scanner_has_exactly_one_call_site_and_it_is_the_guarded_one` |
| P5 | the identity label stops truncating | `an_agent_row_fits_the_window_for_any_branch_name` |
| P6 | the empty state goes away | `an_empty_scan_says_so_and_draws_no_provider_headers` |
| P7 | the subagent mark goes away | `the_agents_demo_shows_every_state_both_providers_and_a_subagent` |
| P8 | idle and recent share a colour | `the_row_dot_carries_the_state_and_nothing_else_does` + 1 |
| P9 | (core) content routed into a pub field | `the_agents_pane_never_renders_conversation_content` (cross-crate) |

**One survivor, disclosed:** the first form of P1 — removing
`.sense(Sense::click())` while leaving the label selectable — was **not**
caught, because with `selectable_labels` on the label senses clicks anyway.
That survivor is what exposed the `selectable(false)` defect above; after the
fix, P1 and P1b both bite. No mutation survives in the tree as committed.

## Deviations from the spec, and why

1. **`AgentSession` gained an `is_subagent` field.** The spec's field list
   does not name one, but it requires "when present-and-true the row is marked
   as a subagent" and a subagent row prefixed `· sub`. A bool is the only way
   to carry that across the crate boundary.
2. **`read_allowlisted` searches two levels, not one.** Top-level object
   first, then any directly-nested object. This is how the Codex
   `session_meta` fields are found without the wrapper key (`payload`) being
   named — the search never widens the allowlist, it only reaches the spec's
   own keys where the format actually puts them. Only scalars are ever
   returned.
3. **The Codex branch may be empty in production — please check.** The spec's
   allowlist names `git_branch` as a flat key on the `session_meta` payload,
   and that is what shipped. Recent Codex CLI builds may instead nest it as
   `git.branch`, in which case the Codex rows will show `project · id8` with
   no branch. §4.4 forbids this session from opening the real
   `~/.codex/sessions/**` to check, so this is flagged rather than fixed:
   **one line of the allowlist, top tier, if the owner's own window shows no
   Codex branch.** Everything else about those rows (state, project, id, age)
   is unaffected.
4. **`--agents-demo` opens on the agents view.** The spec says the default
   view is `usage`; that governs an ordinary launch. A demo flag that opened
   on the pane it is not demonstrating would review nothing.
5. **The `demo` prompt marker now covers both demo flags.** M13's stated
   reason for the marker is that a decoration-less window shows its OS title
   only in the taskbar, so a pane of invented data has to say so where the
   data is. Invented sessions are the same claim, so `--agents-demo` marks
   itself too, in the prompt and in the OS title.
6. **The empty state is whole-pane, not per-provider.** The spec gives one
   empty line; a provider with no sessions therefore gets no header at all
   rather than a header over nothing. Two empty headers would spend a third of
   a 240px window saying nothing twice.
7. **The agent-row identity elides.** A branch name is user text of no bounded
   length; `Label::truncate()` keeps the row inside 320px. Asserted for an
   absurd branch name in both themes.

## §4 conditions hit

- **§4.1 / §4a** — the five doc/manifest patches, applied as verify-and-commit
  from the spec's bytes (table above). Nothing else in a protected path
  changed; `.github/**`, `.cargo/**`, `.claude/**`, `deny.toml`,
  `egress/**` and `credentials/**` are untouched by both commits.
- **§4.4** — no real `~/.claude/**` or `~/.codex/**` path was read at any
  point. Every test uses a fresh temp directory it creates and removes;
  `SessionRoots::from_env()` is production wiring and is exercised only
  through the pure `roots_from` with synthetic inputs. No token material
  appears anywhere in this milestone.
- **§4.6** — three CI failures, all read and all explained by GitHub's own
  logs as a control-plane outage (see above). Not stopped on, because the
  logs explain them and the diff does not.
- **§4.5 / §4.8** — no visual accepted, no milestone accepted, no release cut.

## Addendum — the tip commit's own CI run (added after the report landed)

**Both phase commits are green on all 8 required checks; that is unchanged and
is what the end gate requires.** This addendum is about the run triggered by
the *report* commit, `b6fac53` — a commit that touches nothing but
`reports/m15-endgate.md`, a markdown file no CI job reads.

That run (<https://github.com/cipherpine/quotapane/actions/runs/31121996517>)
reached **7 of 8 green** and then stalled: `build & test (ubuntu-latest)`
could not be allocated a hosted runner, repeatedly, over roughly two hours and
eight `gh run rerun --failed` attempts, each waited for in the foreground. The
job never ran a step; the failures alternate between the same control-plane
error the phase-2 run hit —

```
Failed to resolve action download info. Error: Service Unavailable
```

— and

```
The job was not acquired by Runner of type hosted even after multiple attempts
```

Green on that run, per the last read: `invariants`, `cargo-deny`,
`cargo-audit`, `gitleaks`, `invariant 4 — no telemetry`,
`build & test (windows-latest)`, `build & test (macos-latest)`. Outstanding:
`build & test (ubuntu-latest)` only.

Nothing was changed to chase it, and nothing should be: the same job passed on
`f7adb72` — the identical tree plus one markdown file — 40 minutes earlier,
and on `9947e86` before that. `main` may therefore show a red X on the tip
until GitHub's incident clears; **one `gh run rerun --failed` on that run is
the whole remedy**, and it needs no code change. This session stops here
rather than pushing further commits, each of which would only queue another
run into the same outage.

## What the owner needs to do

1. **§4.5 visual pass.** Run `quotapane --agents-demo` (and
   `quotapane --plain --agents-demo`). What to look at:
   - the titlebar switcher: `usage // agents`, current bright / other faint,
     and that clicking each word moves between the views;
   - that dragging the bar — including dragging *from* one of the two words —
     still moves the window;
   - the four demo rows: green/amber/faint dots, `project · branch · id8`, the
     `· sub` row, the branch-less row, the right-aligned ages;
   - the empty state, which needs a real run on `usage`→`agents` with no
     sessions in the last 24 h (or simply a machine that has run neither CLI
     today): `// no agent sessions in the last 24h`.
2. **A live look at the real thing**, at your discretion: launch `quotapane`
   normally and switch to `agents` while a `claude` session is running. This
   is the only way to confirm deviation 3 — **if the Codex rows show no
   branch, say so and it comes back to the top tier as a one-line allowlist
   change.**
3. **Record the tier note** at the top of this report: the four new
   `// INV:8` tests were authored in-session at Opus (top tier), not
   floor-verified.
4. **Accept or reject M15** (§4.8). Nothing further is queued from this
   session's side; M15-RELEASE owns the version bump and CHANGELOG.
