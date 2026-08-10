# M18a end-gate — reach: statusline, gating recipes, WinGet manifests

**Session:** attended CLI session (Opus 5 — top tier per `CLAUDE.md`'s routing
table), 2026-08-09/10. Hand-carried paste; the M11d dispatcher remains paused.
**Spec:** `prompts/m18a-reach.md`. §4 stops applied throughout.
**Tree footprint:** `crates/usage-cli/{src/main.rs, src/statusline.rs,
tests/cli.rs}`, `README.md`, `docs/cli-json.md`, `docs/gating.md`,
`packaging/winget/**`, and this report. **No §4.1 path touched.**
**Host toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0
(c980f4866 2026-06-30), Python 3.14.4, gh 2.92.0 (2026-04-28), WinGet
v1.29.280, host MINGW64_NT-10.0-26200.

> **Verdict: the milestone's pre-approved scope is complete and green; it is
> not accepted.** Four commits on `main`, CI run **31350410742 — 8/8 success**.
> 473 → 502 tests. Invariants unchanged at 8 / 30 bindings. Zero new
> dependencies, no version bump, no CHANGELOG entry, nothing submitted to
> `microsoft/winget-pkgs`, and `~/.claude/**` never read or written. Five
> mutations run, five caught, no survivors. **Acceptance is the owner's
> (§4.8), and the statusline's look in a live Claude Code session is the
> owner's eyes only (§4.5-adjacent — it renders in his terminal, not mine).**

---

## 1. Preconditions

**P1 — reconciled, as the paste directed.** The spec names tip
`0323e1c` "docs: screenshots from this decade"; the actual tip was `f19822a`
"prompts: M18a — reach (…)", one prompts-only commit ahead of `origin/main`,
whose parent is `0323e1c`. The paste stated this explicitly and pre-cleared it
as expected rather than a §4.7 conflict, so it was not treated as one. The
first push carried that prompts commit, as the paste said it would.

Everything else in P1 held exactly: tree clean, `HEAD == origin/main` at
`0323e1c`, version `1.7.0`, and **473 tests** — cli 64 + cli-integration 13 +
core 156 + ui 240, measured, not assumed.

**P2 — held.** No `--statusline` anywhere in the workspace (the only two hits
were the spec itself and one forward-looking README Roadmap sentence, neither
a flag). No `packaging/`. No `docs/gating.md`.

`tools/check-invariants.py` at baseline: `OK: 8 invariants, 30 test bindings,
tags and manifest set-equal, SECURITY.md id set matches.`

## 2. The pinned stdin schema, and where it came from

Pinned from the **official Claude Code statusline documentation**, fetched
2026-08-09 from `https://docs.claude.com/en/docs/claude-code/statusline.md`
(the `.md` mirror of the docs page; HTTP 200, 64 387 bytes). WebFetch is not
available in this session, so the page was fetched with `curl` into the
scratchpad and read from disk — no subagent, no browser, and **nothing read
from the owner's live config**, per the spec's §4.4 fence.

The quota-bearing part of the payload:

```json
"rate_limits": {
  "five_hour": { "used_percentage": 23.5, "resets_at": 1738425600 },
  "seven_day": { "used_percentage": 41.2, "resets_at": 1738857600 }
}
```

Four facts that shaped the parser, each quoted or derived from that page:

1. **`used_percentage` is 0–100, not a fraction** — "Percentage of the 5-hour
   or 7-day rate limit consumed, from 0 to 100". This is the one number in the
   codebase that is not a fraction; `worst_at_or_over` and the window both take
   fractions. The module has its own `percent()` for that reason, rounding and
   clamping the same way so the printed number agrees with the window's.
2. **`resets_at` is absolute Unix epoch seconds**, not a countdown — "Unix
   epoch seconds when the 5-hour or 7-day rate limit window resets". Snapshot
   windows carry `resets_in_secs`; this one needs the clock subtracted, which
   is why `line()` takes `now_unix_secs` as an argument.
3. **`rate_limits` is frequently absent.** Verbatim: "appears only for
   Claude.ai subscribers (Pro/Max) after the first API response in the session.
   Each window (`five_hour`, `seven_day`) may be independently absent." The
   docs' own recommended handling is `jq -r '… // empty'`.
4. **The rest of the payload is large and sensitive-adjacent** — `cwd`,
   `session_id`, `session_name`, `prompt_id`, `transcript_path`, `model.id`,
   `workspace.*` (including `repo.owner`/`repo.name`), `version`,
   `output_style`, `cost.*`, `context_window.*`, `vim.mode`, `agent.name`,
   `pr.url`, `worktree.*`. None of it is parsed.

The spec also named two third-party writeups
(`nyosegawa.com/en/posts/claude-code-statusline-rate-limits/`,
`mareksuppa.com/til/claude-code-rate-limits-ccstatusline/`). **Deviation D1 —
they were not fetched.** The official docs page turned out to carry the full
schema *and* the absence semantics *and* three worked examples (bash/jq,
Python, Node), which is strictly more authoritative than either writeup and
covers everything the spec wanted them for. Fetching community posts to
corroborate a first-party schema would add sources, not confidence. The
#40094 caveat the spec asked to be recorded is stated in `docs/gating.md`, in
the module's doc comment, and below.

Also pinned from the same page and used in §2's recipe 5: the settings shape
(`statusLine.type: "command"`, `.command`, optional `.padding`,
optional `.refreshInterval` with a minimum of 1), and the fact that the
statusline command runs only after the workspace trust dialog is accepted.

## 3. What landed

### §1 — `quotapane-cli --statusline` (commit `2ccf2ff`)

A third mode beside `--once` and `--watch`, in its own module
(`crates/usage-cli/src/statusline.rs`, 520 lines with tests) so "sends
nothing" can be pinned by scanning that file.

- **Output.** `5h 12% · 7d 48%` — one segment per window present, session
  first, from a fixed table rather than the payload's key order. A segment at
  or over 80% gains a bang. The most-used window's reset, when it reported
  one, becomes a trailing `· resets 2h10m`.
- **Reuse.** The countdown uses the CLI's existing `format_reset`, per the
  spec's "reuse if reusable, do not move code out of usage-core".
- **Zero egress.** No credential path, no `Egress`, no request. Two structural
  pins: the module's source is scanned for the names it must never contain
  (`Egress`, `egress`, `UsageProvider`, `build_provider`, `with_default_path`,
  the two provider types, `credential`, `Credential`, `ureq`, `reqwest`,
  `TcpStream`, `http`), and `main`'s arm is pinned to appear **and return**
  ahead of the `Egress::new(` line — so the claim does not rest on that
  constructor happening to be harmless to call.
- **Defensive by rule.** Invalid JSON, absent/empty/wrong-typed `rate_limits`,
  a missing or non-numeric or non-finite `used_percentage`, a reset already in
  the past — each degrades to "print nothing" or "skip that segment", never to
  an error. Exit is **always 0** in this mode.
- **The aperture test.** The full documented payload with a sentinel planted
  in every non-quota string field, asserting the real line still renders **and**
  that no sentinel reached it. `context_window.used_percentage` carries a
  distinctive `77` — the one number in the payload shaped exactly like the two
  that are read — and is asserted absent.
- **Conflicts.** `--once`, `--watch`, `--json`, `--provider`, `--fail-at`,
  `--debug-raw-unsafe`, `--debug-raw`, `--allow-proxy` → exit 2 naming the
  first, in that fixed order, checked in both argument orders by test.
- **Docs in the same commit:** `--help` (its own synopsis line plus an entry),
  README's Usage table row, and the `docs/cli-json.md` line stating the
  statusline output is a human-format surface outside the JSON contract.

### §2 — `docs/gating.md` (commit `085a396`)

Five recipes in the README's voice, each with a PowerShell variant where it
differs: the pre-flight one-liner explained, a CI stage, a `--watch` NDJSON
heartbeat, a warn-never-block `pre-push` hook, and the statusline settings
snippet. Exit codes are **linked** to `cli-json.md#exit-codes`, not
duplicated. README's Usage section gains the "Gating" pointer.

Two caveats stated rather than glossed, both discovered while writing:

- **The CI recipe only works on a self-hosted runner.** A hosted runner is a
  fresh VM with nobody signed in, so QuotaPane has no credential files to read
  and the step can only ever exit 1 there. A recipe that omitted this would be
  a recipe that never works.
- **Exit 1 ≠ "you have room."** The page leads with this, because
  `|| exit 1` collapses "over threshold" and "could not tell" into one
  outcome and a reader should choose that deliberately.

### §3 — `packaging/winget/` (commit `af12f13`)

Three manifests for `CipherPine.QuotaPane` 1.7.0 (version, installer,
`en-US` defaultLocale), `InstallerType: zip` + `NestedInstallerType: portable`
with both `quotapane` and `quotapane-cli` as portable commands.

**The hash was cross-checked, not trusted**, exactly as the spec required.
The release's own `SHA256SUMS` and the Windows asset were both downloaded;
an independent `sha256sum` was computed over the download and compared:

```
$ sha256sum -c SHA256SUMS --ignore-missing
quotapane-v1.7.0-x86_64-pc-windows-msvc.zip: OK
```

`InstallerSha256:
5B465E544DFAF62C80BFBA6F6093C2A4C674BBCE94D414747A9A3B95BE86B64F`.

The two `RelativeFilePath` entries were read out of the archive rather than
guessed — it unpacks into a single versioned directory
(`quotapane-v1.7.0-x86_64-pc-windows-msvc\`), so both paths carry the version
and change every release. `packaging/winget/README.md` records the validation
and the submission path, and states plainly that the upstream PR is the
owner's act.

**WinGet validation ran** (the spec allowed for it being unavailable; it was
not). WinGet v1.29.280:

```
$ winget validate --manifest packaging\winget
Manifest validation succeeded.
```

Honest limit on that result: `winget validate` checks schema and internal
consistency only. It does not download the installer or perform an install,
so the portable aliases landing correctly on PATH is **not** yet proven — the
first real proof is `winget install --manifest packaging\winget` or the
`winget-pkgs` CI. That is called out in `packaging/winget/README.md` too, and
is listed under §7 below.

## 4. Mutation pass

Run after commit 1, before the push, on the committed tree; each mutation
applied, `cargo test -p usage-cli --locked --no-fail-fast` run, then reverted
with `git checkout --` and the tree confirmed clean.

| # | Mutation | Caught by |
|---|---|---|
| 1 | Bang threshold `>=` flipped to `>` (80% loses its bang) | `statusline::tests::the_bang_threshold_is_inclusive_at_eighty`, `…::the_bang_follows_the_rounded_number_the_reader_sees` |
| 2 | `cwd` / `session_id` / `model.display_name` routed into the output | `statusline::tests::no_field_other_than_the_quota_numbers_can_reach_the_line`, `cli::statusline_prints_one_line_from_a_real_payload_and_exits_zero` |
| 3 | An absent `rate_limits` made a non-zero exit | `cli::statusline_survives_garbage_and_quota_less_payloads_with_exit_zero` |
| 4 | `--statusline` allowed to combine with `--fail-at` | `tests::statusline_refuses_every_polling_flag_and_names_the_one_it_found`, `cli::statusline_combined_with_a_polling_flag_exits_two` |
| 5 | Session/weekly segment order swapped | 9 unit tests incl. `…::the_session_window_prints_first_whatever_order_the_payload_used`, plus `cli::statusline_prints_one_line_from_a_real_payload_and_exits_zero` |

**Five applied, five caught, no survivors.**

**A process finding worth recording (deviation D2).** The first attempt at
mutations 2 and 3 used a Python rewrite whose `open(p, "w")` translated the
files to CRLF on Windows. That flipped an *unrelated* test red
(`help_text_documents_every_flag_the_parser_accepts`, which does
`body.find("\n}\n")` over `include_str!` of its own source) and — because
`cargo test` stops after a failing target — hid the integration tests
entirely. Mutation 3's result was therefore **not a real measurement** and was
thrown away. Both were redone with `newline=""` preserved, CR count asserted
0, and `--no-fail-fast` so every target reports. The table above is the redone
run. The first attempt's numbers appear nowhere in it. Two lessons: a mutation
harness that can perturb the file *as a file* can produce a false catch, and
`--no-fail-fast` should be the default for this kind of pass.

## 5. The §3 verification bar

Run in full before each of the four commits. Final state on `af12f13`:

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test --workspace --locked` | **502 passed, 0 failed** |
| `tools/check-invariants.py` | `OK: 8 invariants, 30 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |

Test count 473 → **502**:

| Target | Before | After | Δ |
|---|---|---|---|
| usage-cli unit (`main.rs` + `statusline.rs`) | 64 | 89 | +25 |
| usage-cli integration (`tests/cli.rs`) | 13 | 17 | +4 |
| usage-core | 156 | 156 | — |
| usage-ui | 240 | 240 | — |
| **total** | **473** | **502** | **+29** |

Clippy caught one thing worth naming: `statusline_conflict` first took eight
positional `bool`s (`too_many_arguments`). Rather than `#[allow]` it, it now
takes a `PollingFlags` struct — eight same-typed positional booleans are a
transposition hazard that would compile silently and report the wrong flag.

## 6. CI

| | |
|---|---|
| Run | **31350410742** — <https://github.com/cipherpine/quotapane/actions/runs/31350410742> |
| Head sha | `af12f13` (commits 1–3 plus the carried `f19822a` prompts commit) |
| Started / finished | 2026-08-10T02:40:05Z / 2026-08-10T02:43:21Z (3m16s) |
| Result | **completed / success — 8 of 8** |

All eight required checks green: `build & test` on ubuntu-latest,
macos-latest and windows-latest; `invariants — manifest, docs, and tests
agree`; `invariant 4 — no telemetry`; `cargo-deny`; `cargo-audit`;
`gitleaks — full-history secret scan`.

This report is commit 4 and pushes after the above; its own run is
doc-only and is named in the handoff below rather than pre-judged here.

**Live smoke run** of the built binary, separate from the test suite —
`5h 12% · 7d 84%! · resets 2h10m` from a real-shaped payload (exit 0); no
output and exit 0 for an absent `rate_limits`, for garbage, and for empty
stdin; exit 2 with the naming diagnostic for `--statusline --fail-at 85`.

## 7. Deviations, numbered

**D1 — the two third-party writeups were not fetched.** §2 above gives the
reasoning: the official docs page carried the full schema, the absence
semantics, and three worked examples, and corroborating a first-party schema
against community posts adds sources rather than confidence. The #40094
caveat they were cited for is recorded in three places. *Impact: none on the
code; the spec's "record what you found" is discharged by §2.*

**D2 — the mutation pass was run twice for mutations 2 and 3.** §4 above. The
first run was invalidated by a CRLF-rewriting harness bug and is not reported
as a result. *Impact: none on the shipped tree; the tree was verified clean
after every revert.*

**D3 — `--client-version` is accepted, and inert, alongside `--statusline`.**
The spec's conflict list is a closed set of seven flags and does not include
it. It was left permitted rather than adding an eighth conflict, because the
list is the top tier's decision and widening it would be authoring past the
spec. It is worth an explicit ruling — see §8.

**D4 — `--help`'s statusline synopsis is a second line, not a widened first
line.** `usage: quotapane-cli (--once | --watch <SECS>) […]` is unchanged and
a `       quotapane-cli --statusline` line follows it. Correct on the merits
(the mode combines with no bracketed flag, so folding it in would claim the
opposite) and it also leaves the existing integration assertion on that line
intact. Flagged because "correct on the merits" and "does not disturb an
existing test" pointing the same way is exactly the coincidence worth
declaring rather than quietly enjoying.

**D5 — one pre-existing test string was *not* changed, and one guard list
was.** No existing assertion needed rewriting. The scanner guard inside
`help_text_documents_every_flag_the_parser_accepts` gained `"--statusline"`,
which strengthens it (it protects against the scanner silently matching
nothing) rather than relaxing it.

Nothing else deviates. No §4 condition was hit.

## 8. Things I was unsure of

1. **`--client-version` with `--statusline` (D3).** It is accepted and does
   nothing. This codebase elsewhere argues that a silently ignored flag "reads
   as 'the tool produced no output', not 'that flag does not apply'" — by that
   standard it should probably be an eighth conflict. I followed the spec's
   closed list. **A one-line ruling would close it.**
2. **The countdown format for weekly windows.** `format_reset` was reused as
   instructed, and it has no day unit — so a 7-day window three days out reads
   `resets 72h0m`. Consistent with what `--once` text mode already prints, and
   inventing a `3d0h` format would have been authoring past the spec, but it
   is long for a status bar. A day-aware format would be a follow-up decision,
   not mine.
3. **A reset already in the past is dropped, not shown as `0s`.** The spec did
   not say. I judged that a stale or just-turned-over window says nothing
   useful, and printing `resets 0s` would look like a bug. Tested both ways
   round (`NOW - 60` and exactly `NOW`).
4. **"Naming the first conflict" — first in which order?** Ambiguous between
   "first in a fixed list" and "first as typed on the command line". I chose a
   fixed list order (the spec's own, with `--debug-raw-unsafe` slotted ahead of
   `--debug-raw` so the flag the user actually typed is the one named), because
   a deterministic message is worth more to a script author than a positional
   one. Both argument orders are tested.
5. **The WinGet manifest schema version is 1.6.0.** It validates on WinGet
   1.29.280 and supports nested portable installers. A newer schema exists;
   1.6.0 was chosen for breadth of compatibility. If `winget-pkgs` prefers
   newer on submission, it is a three-line change in three files.
6. **`MinimumOSVersion: 10.0.0.0`** is a reasonable inference from "Windows is
   the primary target", not a measured floor. Nothing in the repo states a
   minimum Windows version.
7. **`Publisher: Cipher Pine`** was inferred from the `cipherpine` GitHub org
   and the spec-given `CipherPine.QuotaPane` identifier. It is what `winget
   show` will display, so it should be confirmed before submission.

## 9. What the owner must do next

Nothing is accepted; §4.8 leaves the gates with you. In rough order:

1. **Judge the statusline in your own terminal (§4.5-adjacent).** I have not
   read, written, or configured anything under `~/.claude/**`, so it has never
   run in a real session. The snippet is in `docs/gating.md` §5. What is
   yours to judge: whether `5h 12% · 7d 84%! · resets 2h10m` reads right at
   status-bar size, whether the `!` is the right flag, and whether an empty
   line for sessions with no `rate_limits` is acceptable in practice.
2. **Rule on D3 / open question 1** — should `--client-version` conflict with
   `--statusline`?
3. **Prove the WinGet manifests by installing them**
   (`winget install --manifest packaging\winget`) before any submission.
   Validation did not exercise it.
4. **Confirm `Publisher: Cipher Pine`** and, if you want it upstream, make the
   `winget-pkgs` submission yourself — it publishes under your GitHub identity
   and agrees to that repo's terms on your behalf.
5. **Decide release timing.** No version bump and no CHANGELOG entry were
   made, per the spec; this is sitting on `main` at 1.7.0.
6. **Check this report's own CI run** (the commit-4 push), which had not
   completed when this section was written.

M18b — the update check and the remaining packaging targets (Homebrew/AUR) —
was not started and does not exist as a spec yet.
