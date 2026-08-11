# M18b end-gate — the update check, on invariant 5's own terms

**Session:** attended CLI session (Opus 5 — top tier per `CLAUDE.md`'s routing
table), 2026-08-11. Hand-carried paste; the M11d dispatcher remains paused.
**Spec:** `prompts/m18b-update-check.md`. §4 stops applied throughout.
**Tree footprint:** `crates/usage-core/src/{lib.rs, update.rs, egress/mod.rs}`,
`crates/usage-cli/{src/main.rs, tests/cli.rs}`,
`crates/usage-ui/src/{main.rs, config.rs}`, `SECURITY.md`, `THREAT_MODEL.md`,
`invariants.manifest`, `README.md`, `docs/cli-json.md`, and this report.
**Host toolchain:** rustc/cargo as committed in `Cargo.lock`, Python 3.14.4,
gh 2.92.0, host MINGW64_NT-10.0-26200.

> **Verdict: the milestone's pre-approved scope is complete and locally green;
> it is not accepted.** Three commits on `main`. **502 → 540 tests**
> (cli 101 + cli-integration 19 + core 170 + ui 250). Invariants **8**, test
> bindings **30 → 33**. Ten mutations run, **ten caught, no survivors**, every
> revert byte-identical. Zero new dependencies, no version bump, no CHANGELOG
> entry, nothing cached or written at runtime, no click handler on the notify
> line, no identifier in the request beyond the static User-Agent, and
> `~/.claude/**` and `~/.codex/**` never read. **The first-run ask and the
> notify line are §4.5 — the owner's eyes, never self-accepted.**

---

## 1. Preconditions

**P1 — held, with one arithmetic caveat in the spec itself (deviation 1).**
Tip was `3132002` (the spec's own commit) directly on `f5ce9f6` "packaging: the
README moves out of the manifest directory it was breaking", one commit ahead
of `origin/main`, exactly as the paste pre-cleared. Tree clean. Version
`1.7.0`.

Tests measured, not assumed: **502**, as `89` (cli unit) + `17`
(cli-integration) + `156` (core) + `240` (ui). The spec's P1 states the same
total but breaks it down as "cli 106 + cli-integration 13 + core 156 + ui 240",
which sums to **515**. The total is what matched reality; `106` appears to be
the cli crate's combined figure (89 + 17) with the integration count then added
again. P1 offers the tiebreaker itself — "count `#[test]` across crates if
unsure" — so this was treated as a spec typo, not a §4.7 reality mismatch. Had
the *total* disagreed, this session would have stopped.

**P2 — held.** `update_check` appeared nowhere in the workspace (the only hit
in the repo was a `git grep` recipe quoted inside `prompts/m6-gap-report.md`),
and `crates/usage-core/src/update.rs` did not exist.

Baseline checker: `OK: 8 invariants, 30 test bindings, tags and manifest
set-equal, SECURITY.md id set matches.`

## 2. §0 — the two M18a rulings (commit `35fdfd7`)

**§0.1 — `--client-version` joins `--statusline`'s conflict list.** Added to
`PollingFlags` and to `statusline_conflict`'s fixed order, last, so the message
a script sees for every pre-existing combination is byte-for-byte what it was.
The M18a test that asserted the *opposite* — that `--statusline
--client-version 1.2.3` parses — was the one existing behaviour this ruling
reverses; it was replaced by
`statusline_refuses_client_version_rather_than_ignoring_it`, which checks both
argument orders.

**§0.2 — the countdown grows a day unit.** `format_reset` renders `3d0h` above
48 hours instead of `72h0m`. Pinned by a table test whose two load-bearing rows
are `172_800 → "48h0m"` and `172_801 → "2d0h"`: the ruling says *above* 48h, so
exactly two days is still hours. A second test asserts the property rather than
the samples (nothing past two days reports in bare hours), and a third drives
the real statusline end to end — `7d 41% · resets 3d0h`.

Two things worth the owner's eye here, neither of which the spec settled:

1. **The boundary is 48h; the window's own is 24h.** `usage-ui`'s
   `format_reset` switches to `Nd Nh` at 86_400. The spec said "above 48h" for
   this one, and that is what shipped. The two surfaces now agree on the
   *unit* and disagree on *when* it appears. Defensible — a status bar is read
   at a glance and `36h0m` still parses at a glance — but it is a divergence,
   and it was the spec's number rather than mine.
2. **`format_reset` is shared with the text summary** (deviation 10). It is one
   function, used by `--statusline` and by `print_summary`, so `--once` text
   output now prints `resets in 3d0h` where it printed `resets in 72h0m`. The
   ruling names the function, and M18a §8.2's own reasoning was that the two
   should agree, so this reads as intended rather than as spillage. It changes
   no `--json` key and is not covered by the stability contract.

## 3. §1 — `usage_core::update` (commit `276a89e`)

One module, 705 lines including tests. `GET api.github.com
/repos/cipherpine/quotapane/releases/latest` through the existing chokepoint,
`User-Agent: quotapane-update-check` and nothing else, at most 64 KiB of body
read, `tag_name` and only `tag_name` lifted out of a generic
`serde_json::Value` — there is no struct here for a field to be added to by
accident. Strictly newer → `Some`; everything else → `None`. No cache, no
state, no file write, no timer: restarting is the re-check, recorded here as a
deliberate simplicity, because a timer would need state and state is the first
thing to grow a cache file.

**The three registered tests** (`// INV:5`, in `invariants.manifest` via Patch
I):

- `the_update_check_sends_nothing_unless_asked` — behavioural *and*
  structural. Both un-asked states return nothing through both entry points,
  the gate's truth table is asserted directly, and the module's source is
  scanned to prove the gate returns *before* the fetch rather than merely
  sitting above it.
- `the_update_request_cannot_carry_a_credential` — the statusline module's
  self-scan technique: real code only (test module sliced off, comment lines
  dropped), then ten forbidden names — `Authorization`, `Bearer`, `bearer`,
  `Secret`, `token`, `Token`, `credential`, `Credential`, `auth`, `Auth`. Plus
  the call site pinned literally, with `None` where a credential would go, and
  the running version proven absent from the request builder's scope.
- `update_is_the_only_caller_of_the_github_host` — the `refresh_agents`
  single-call-site idea widened from one file to the whole source tree, because
  the allowlist is a workspace-level promise. Walks `crates/**/*.rs`, skips
  `egress/mod.rs` (whose job is to name the host) and each file's
  `#[cfg(test)]` tail, and requires exactly one file with exactly one
  occurrence: `update.rs`'s `HOST` constant.

### Two spec clauses that could not both be honoured literally (deviations 2–3)

§1 says "**One public function**, roughly `check(egress) -> Option<UpdateNotice>`"
and "**No error type escapes this module** — a failed check is
indistinguishable from no-update, by design." §3 says the CLI, "on a failed
check[,] says `update check failed` and exits 1 (the CLI is allowed to be
honest about failure; only the window must be silent)."

A caller cannot report failure through a value that is by construction
indistinguishable from success. The resolution keeps both sentences' intent and
breaks the letter of one:

- `check(egress, setting) -> Option<UpdateNotice>` is unchanged in shape and is
  what the window calls. Failure is `None`, indistinguishable from "current".
- `check_outcome(egress, setting) -> CheckOutcome` is what the CLI calls.
  `CheckOutcome` is `Newer(UpdateNotice) | Current | Inconclusive`, and
  **`Inconclusive` carries no payload**. There is still no error type in this
  module — not a struct, not an enum implementing `std::error::Error`, no
  `Result` in the public API. The CLI can say *that* a check failed and can say
  nothing about *why*, which is also the better security posture: a failure
  detail is exactly where a proxy variable's name or a URL would reach a
  terminal.
- `opted_in(setting) -> bool` is public as the single gate both callers route
  through, which §1's own gate test asks for.

So three public functions rather than one. A test,
`no_error_type_escapes_this_module`, pins the absence: the module names no
`Error`, `Result<`, `std::error`, `thiserror`, `anyhow`, `panic!`, `unwrap()`
or `expect(` in shipped code, and the notice's only dynamic field is a version
string beside a `&'static str` URL.

**`Inconclusive` deliberately merges "not opted in" with "the check failed"**
(deviation 3, second half). They are the same answer — neither is a claim that
you are current — and the CLI, which always passes the literal `Some(true)`,
can only ever see the second.

### Deviation 4 — a fourth test left untagged on purpose

`no_error_type_escapes_this_module` is the kind of test that would ordinarily
carry `// INV:5`. It does not. `invariants.manifest` is a §4.1 protected path
whose M18b entry (Patch I) was authored at the top tier and lands verbatim, and
`tools/check-invariants.py` holds source tags and manifest entries **set-equal
in both directions** — so a fourth tag would fail CI, and adding the matching
manifest line would be this session authoring protected bytes. The test runs
and guards either way; the choice is recorded in its own doc comment so the
next reader does not "fix" it.

## 4. §2 — the window

**2.1 `update_check`, the only tri-state preference.** `Option<bool>` on
`Config`, defaulting to `None`. The load path is the existing closed-key
`match`; `tri_switch` accepts `on`/`off` and reads anything else as un-asked,
erring toward "ask again" rather than "assume yes".

**The part that is easy to get wrong, and the test that pins it:** `render`
**omits the key entirely** while it is `None`. The whole config is rewritten on
every save, so writing `update_check=off` for an unanswered question would mean
a user dragging the resize grip silently answers a question about the network
on their own behalf — and the ask would never return to correct it.
`an_unanswered_update_check_is_not_written_by_an_unrelated_save` renders a
theme-and-height change and asserts the key is absent from the bytes, then
asserts both real answers do survive.

**2.2 The first-run ask.** One line in the usage view's footer, under the panes
and above the grip, in the `// N older today` register and ink:

    // check github for new versions?  on · off

`on` and `off` are `selectable(false)` + `Sense::click()` labels, installed the
way the `usage // agents` switcher is and for the same reason — a
decoration-less window cannot afford to lose a drag to a footer. Clicking
writes the answer and the line is gone that frame; `update_footer` returns
`Ask` only while the setting is `None`, so it never returns. The agents view
gets neither line.

**2.3 The notify line.** `// v1.8.0 available`, faint, hover tooltip carrying
`github.com/cipherpine/quotapane/releases`. **No click handler** — pinned
structurally by `the_notify_line_has_no_click_handler`, which scans the
`Notice` arm for `clicked()`, `Sense::`, `sense(`, `CursorIcon` and `open`, and
also asserts it does not use `AMBER`, because amber is quota's colour in this
window and a release is not a quota event.

The check runs off the UI thread on its own `std::thread`, exactly as polls do,
with the result taken once through a channel. `start_update_check` is called
from exactly two places — startup, and the moment the user answers `on`, so a
user who just said yes does not have to restart — and refuses if either demo is
running, if the setting is not `on`, or if a check is already in flight or
already answered. "Once per launch" is therefore a property of the call sites,
not of a flag someone must remember to set.

**2.4 Demos.** `update_footer(_, _, demo: true)` is `Silent` for all six
combinations, asserted as a table, and `start_update_check` returns on the demo
check before the opt-in gate and long before the thread.

**2.5 The UI tests** reuse `gesture_on_switcher`'s harness shape: the panel's
whole-rect background drag handle installed first, the footer drawn on top,
label positions discovered from the shapes actually painted rather than from
coordinates written down and left to rot.

### Deviation 11 — one assertion I could not honestly make

The spec's §2.5 asks that "a drag starting on the ask's words reaches the
window handle". It does, and
`a_drag_starting_on_an_answer_moves_the_window_instead_of_answering` proves it.
What I could *not* assert is the mirror claim that a plain click does **not**
begin a window drag: the central panel's background senses `Sense::drag()`
only, so egui begins a drag on press anywhere in the pane — including on a word
— and delivers the click on release regardless. That is pre-existing behaviour
this footer shares with M16's `// N older today` toggle, which was accepted at
the time, and a press that never travels moves the window nowhere. The test
says so in a comment rather than quietly dropping the case.

### Deviation 5 — an invariant-7 test widened (flagged for top-tier review)

`the_window_exposes_no_proxy_opt_in` asserted `Egress::new(` occurs **exactly
once** in `usage-ui`. The window now constructs a second chokepoint — the
update check's, on its own thread — so that assertion had to change. It was
**widened, not relaxed**:

```rust
let constructions = code.matches("Egress::new(").count();
assert_eq!(constructions, code.matches("Egress::new(false)").count(), ...);
assert_eq!(constructions, 2, ...);
```

The claim that matters was never the count — it is that no egress in this crate
can be proxy-enabled — and equality between the two counts states that
directly, for any number of constructions. Pinning the total at 2 keeps a third
from appearing unnoticed. This test is not in `invariants.manifest`, but it
enforces invariant 7's window half, so it is called out here explicitly:
**this is an invariant-test edit the spec did not anticipate.**

The CLI's matching pin, `the_only_egress_constructor_call_is_fed_by_the_seam`,
was **not** touched — it still requires exactly one `Egress::new(` fed by
`egress_proxy_opt_in(&args)`. Keeping it intact is why `--check-update` is a
fourth `Mode` rather than a fourth `Invocation`: as a `Mode` it flows through
the same seam, so the update check is proxy-gated by the same expression every
poll is (deviation 6).

## 5. §3 — the CLI

`--check-update` refuses every flag `--statusline` refuses, plus `--statusline`
itself, sharing one conflict table so the two lists cannot drift. It carries
`Mode::CheckUpdate` with all polling configuration at defaults, dispatches
immediately after the single `Egress::new`, and returns before any provider is
built — pinned by `the_update_check_returns_before_the_poll_path_is_entered`.
`config.cfg` is never consulted: running the command *is* the opt-in, passed as
the literal `Some(true)` at the one call site.

The three outcomes are byte-exact and testable without a network, because the
formatting is a pure function (`check_update_report`) separate from the run:

```
quotapane 1.7.0 — v1.8.0 available: github.com/cipherpine/quotapane/releases
quotapane 1.7.0 — up to date
update check failed                                          (stderr, exit 1)
```

**Deviation 13 — the one untested hop**, stated plainly: no test performs a
real `--check-update` run, because that dials `api.github.com` and no test in
this repository is allowed to make a network request. Everything either side of
the dial is covered — the gate, the request's shape, the parse, the comparison,
the three output lines, the exit codes, every conflict through the real binary.
The dial itself is the owner's to try.

**Deviation 12:** `--check-update` also conflicts with `--client-version`. §3
defines its conflict set as "`--once`, `--watch`, `--statusline`, and the rest
of the statusline conflict set", and after §0.1 that set contains
`--client-version` — so this follows from the spec rather than extending it,
but it is an inference and is recorded as one.

## 6. §4a — the thirteen patches, byte-verified

Twelve `### Patch` blocks carrying **thirteen** OLD/NEW pairs (Patch I holds
two: invariants 3 and 5). Every pair was extracted programmatically from
`prompts/m18b-update-check.md`'s own bytes by a script that parses the headings
and fenced blocks — **nothing was retyped, and nothing was typed by hand into a
protected file.** The script requires each OLD to occur exactly once before
replacing and each NEW to occur exactly once after, and aborts before writing
anything if either fails.

| Patch | Target | OLD before | NEW after |
|---|---|---|---|
| A | `SECURITY.md` (invariant 1: the new config key) | ×1 | ×1 |
| B | `SECURITY.md` (invariant 5 rewritten) | ×1 | ×1 |
| C | `SECURITY.md` (network policy: host count) | ×1 | ×1 |
| D | `SECURITY.md` (the removed host returns) | ×1 | ×1 |
| E | `SECURITY.md` (verify-egress recipe) | ×1 | ×1 |
| F | `THREAT_MODEL.md` (T-E2) | ×1 | ×1 |
| G | `THREAT_MODEL.md` (§9 row 3) | ×1 | ×1 |
| H | `THREAT_MODEL.md` (§9 row 5) | ×1 | ×1 |
| I1 | `invariants.manifest` (invariant 3) | ×1 | ×1 |
| I2 | `invariants.manifest` (invariant 5) | ×1 | ×1 |
| J | `README.md` (security bullet) | ×1 | ×1 |
| K | `egress/mod.rs` (the host returns) | ×1 | ×1 |
| L | `egress/mod.rs` (rejection fixtures) | ×1 | ×1, then respaced — below |

### Deviation 7 — rustfmt reflowed Patch L, as the spec predicted

The spec warned that rustfmt may reflow Patch K/L comment lines and directed
M16's D2 precedent: take the formatter, flag the reflow, never
`#[rustfmt::skip]`. That is what happened, to Patch L only.

Patch L adds four rejection fixtures whose longest entry
(`"api.github.com.evil.com",`) is wider than the existing ones, so rustfmt
pushed the whole block's trailing-comment column right. Verified precisely
rather than assumed: under whitespace normalisation Patch L's NEW block occurs
**exactly once** in the file, and line by line, **all four lines Patch L
actually adds are byte-identical**; the two pre-existing lines it carried
through (`"openai.com",` and `"api.openai.com",`) are the only ones respaced.
Content intact, spacing only. Patches A–K remain byte-identical after `cargo
fmt`.

## 7. Deviations, numbered

1. **P1's test breakdown is internally inconsistent** — `106+13+156+240 = 515`,
   not the 502 it states. The total matched reality exactly; the breakdown did
   not. Treated as a spec typo under P1's own "count if unsure" clause. §1.
2. **Three public functions, not one** — `check`, `check_outcome`, `opted_in`.
   §3's requirement that the CLI report failure cannot be met through a value
   that is by design indistinguishable from success. §3 above.
3. **"No error type escapes" honoured as literally as possible** — there is no
   error type at all; failure is a payload-free `CheckOutcome::Inconclusive`,
   which also merges "not opted in" with "did not complete". §3 above.
4. **A fourth would-be `INV:5` test is deliberately untagged**, because
   `invariants.manifest` is protected, pre-authored, and set-equality-checked.
   §3 above.
5. **An invariant-7 test was widened** —
   `the_window_exposes_no_proxy_opt_in`, from "exactly one construction" to
   "every construction is proxy-off, and there are exactly two". Strengthened,
   not relaxed; **flagged for top-tier review.** §4 above.
6. **`--check-update` is a `Mode`, not an `Invocation`** — so it flows through
   the CLI's single `Egress::new` seam and leaves
   `the_only_egress_constructor_call_is_fed_by_the_seam` untouched. §4 above.
7. **rustfmt respaced two pre-existing lines inside Patch L.** Formatter taken,
   reflow characterised precisely, no `#[rustfmt::skip]`. §6 above.
8. **Two README claims outside the spec's enumerated §4 scope were false and
   were fixed.** §4 said the Security-posture sentence is "handled by Patch J",
   but Patch J rewrites the *egress* bullet only. The next bullet read "**No
   auto-update and no update check** — there is no updater in the codebase at
   all", and the Roadmap paragraph read "an update *check* that would be
   notify-only and off by default — today there is none of any kind." Both
   became false the moment `update.rs` landed, so both were rewritten in the
   same commit under the same-change rule. `README.md` is not a §4.1 path, so
   this session may author it — but it is beyond what the spec listed.
9. **The `exit codes:` block was reworded**, from "`1  a provider or credential
   error`" to "`…; with --check-update: the check failed`", in `--help`,
   `docs/cli-json.md`, and the two tests that pin it verbatim. No code's
   meaning changed; exit 1 still means "something went wrong".
10. **§0.2 changes the text summary too** — `format_reset` is shared, so
    `--once` prints `resets in 3d0h`. §2 above.
11. **One §2.5 assertion could not be made honestly** — a click on the ask does
    also begin a (zero-distance) window drag, pre-existing behaviour shared
    with M16's older-toggle line. §4 above.
12. **`--check-update` conflicts with `--client-version`**, inferred from "the
    rest of the statusline conflict set" after §0.1. §5 above.
13. **The successful `--check-update` network path is untested**, deliberately:
    no test here may dial. §5 above.

## 8. Mutation pass

Run after commit 2, before the push, on the committed tree. Each mutation was
applied by exact string replacement, the relevant suite run with
`--no-fail-fast`, then reverted with `git checkout --` and the file's **SHA-256
compared against its pre-mutation hash** — not `git status`, which on Windows
would forgive a CRLF flip (the M18a lesson). All ten reverts were
byte-identical and `git status` was empty at the end of the pass.

| # | Mutation | Result | Caught by |
|---|---|---|---|
| 1 | the gate inverted (checks when off) | caught | `the_update_check_sends_nothing_unless_asked` |
| 2 | the gate removed (checks when un-asked) | caught | `the_update_check_sends_nothing_unless_asked` |
| 3 | `tag_name` parse widened to read the release `body` | caught | `only_tag_name_is_read_out_of_a_release_document` |
| 4 | an `Authorization` header added to the request | caught | `the_update_request_cannot_carry_a_credential` (+ the gate test) |
| 5 | a second call site for `api.github.com` added elsewhere | caught | `update_is_the_only_caller_of_the_github_host` |
| 6 | the notify line rendered when there is nothing to report | caught | `the_notice_renders_for_some_and_nothing_at_all_for_none` |
| 7 | the first-run ask shown when the key is present | caught | `the_ask_is_shown_only_while_the_question_is_unanswered` (+ the click test) |
| 8 | `--check-update` combined with `--once` allowed | caught | `check_update_refuses_every_other_flag_…` + the integration test |
| 9 | version compare flipped (older reported as newer) | caught | `versions_compare_as_numbers_not_as_text` + four others |
| 10 | the ask rendered in a demo | caught | `the_ask_is_shown_only_while_the_question_is_unanswered` |

**10 / 10 caught, no survivors.**

## 9. The §3 verification bar

Run clean before each commit:

```
cargo fmt --all --check          clean
cargo clippy --workspace --all-targets --locked -- -D warnings    clean
cargo test --workspace --locked  540 passed, 0 failed
python tools/check-invariants.py OK: 8 invariants, 33 test bindings,
                                 tags and manifest set-equal,
                                 SECURITY.md id set matches.
```

| Suite | Before | After | Δ |
|---|---|---|---|
| `usage-cli` unit | 89 | 101 | +12 |
| `usage-cli` integration | 17 | 19 | +2 |
| `usage-core` | 156 | 170 | +14 |
| `usage-ui` | 240 | 250 | +10 |
| **total** | **502** | **540** | **+38** |

Invariants **8** (unchanged). Test bindings **30 → 33**: the three registered
`update.rs` tests joined invariant 5, whose entry also changed from
`kind: absence` to `kind: test-backed` (Patch I2).

## 10. CI

Commits `35fdfd7`, `276a89e`, and this report were pushed together. The run
this push created had not completed when this section was written — the
session waited on it in the foreground with `gh run watch <id> --exit-status`
and reports the id and result back to the owner directly, following the M18a
precedent for a report that is itself the last commit.

## 11. Things I was unsure of

1. **The 48h boundary.** The spec's number, and it disagrees with the window's
   own 24h switch. Both now use days; they start using them at different
   points. A one-line ruling would settle whether they should match.
2. **Requiring the `v` on release tags.** `tag_version` rejects `1.8.0`. Every
   release this project has cut is `vX.Y.Z`, and failing closed on drift is the
   house habit — but if a future release is ever tagged without the `v`, the
   check will go quiet rather than notice, and it will do so silently, because
   silence is the only failure mode it has.
3. **`Inconclusive` merging "not asked" with "failed".** Right for the CLI,
   which can never see the first. If a third caller ever appears that needs to
   tell them apart, the enum is where that goes.
4. **Whether the ask belongs in the agents view too.** The spec says the agents
   view never shows it, and that is what shipped. It does mean a user who lives
   on the agents tab is never asked.
5. **The tri-state's on-disk shape.** Omitting the key is the only way I found
   to keep "un-asked" durable across an unrelated save. It does mean
   `config.cfg` no longer always lists every key, which the file's own header
   does not promise but a reader might assume.
6. **`update_check=maybe` reads as un-asked**, so a typo re-raises the ask
   rather than being ignored. The alternative — treating anything unrecognised
   as `off` — would silently swallow a user's intent to opt in.

## 12. What the owner must do next

Nothing is accepted; §4.8 leaves the gates with you.

1. **Look at the two lines (§4.5).** The first-run ask
   (`// check github for new versions?  on · off`) and the notify line
   (`// v1.8.0 available`) have never been seen by anyone but a test. Delete
   `update_check` from `config.cfg` — or start with no config at all — to get
   the ask back.
2. **Try `quotapane-cli --check-update` yourself.** It is the one hop no test
   in this repo may exercise (deviation 13). It should print
   `quotapane 1.7.0 — up to date` against the current release.
3. **Rule on deviation 5** — the widened invariant-7 test in `usage-ui`. It is
   the only security-invariant test this session edited.
4. **Rule on deviation 2/3** — three public functions and the payload-free
   `CheckOutcome`, which is the only place this session departed from §1's
   letter.
5. **Rule on the 48h vs 24h split** (§11.1) if the two surfaces should agree.
6. **Decide release timing.** No version bump and no CHANGELOG entry were made,
   per the spec; this is sitting on `main` at 1.7.0. When it is cut, `v1.8.0`
   is the first tag this check will ever have something to say about.
