# M17-RELEASE Leg A — rc verified

Attended CLI session on **Opus 5** (top tier per `CLAUDE.md`'s routing table).
Spec: `prompts/m17-release.md`. Scope executed: **preconditions P1–P4, Phase 1,
Phase 2 — and nothing else.**

> **Verdict: Phase 2 met its gate.** `tools/release-verify.sh v1.7.0-rc.1`
> returned `RESULT: PASS — v1.7.0-rc.1: six steps, six controls, R1-R4`, run
> verbatim, complete output in §5. Phase 1's CI is 8/8 green; the rc release run
> is 3/3 green; the draft is unpublished.
>
> **This session is now stopped at Phase 2's HARD STOP and is waiting.** No
> `v1.7.0` tag exists. Nothing is published. Phase 3 has not begun and will not
> begin without the top tier's Leg B paste.

**One finding needs a Leg B ruling before the v1.7.0 tag is cut** — the content
spot-check's `// hide older` needle is absent from both shipped GUI binaries.
It is characterised in §6 with a reproduction; it is **not** a verification
failure and the spec directs that it not be treated as a gate.

## 1. Preconditions

| | Clause | Observed | |
|---|---|---|---|
| P1 | tip subject `prompts: the v1.7.0 release spec — M14+M15+M16` | `e333a8f`, byte-identical (U+2014 em dash confirmed via `od -c`) | ✅ |
| P1 | parent `83cc5d6` `prompts: the M16 Phase 2 goal prompt, on the record` | `83cc5d64c97dae38d6e3593927781c5273a3422e` | ✅ |
| P1 | tree clean | `git status --porcelain` empty | ✅ |
| P1 | version `1.6.0` in workspace Cargo.toml | `Cargo.toml:10`, `[workspace.package]` | ✅ |
| P1 | 473 tests (cli 64 + cli-integration 13 + core 156 + ui 240) | exactly those four counts | ✅ |
| P2 | `origin/main` at `5aff938` | `5aff938334422cb1c1e9f9a5d8dbfdde46be6c44` | ✅ |
| P2 | its run `31232299097` green | `completed` / `success`, 8/8 jobs, on that exact `head_sha` | ✅ |
| P2 | `83cc5d6` and the tip local-only | remote `GET /commits/<sha>` → **HTTP 422 "No commit found"** for both; control on `5aff938` → 200 | ✅ |
| P2 | no CI run for either | `?head_sha=` → `total_count: 0` for both | ✅ |
| P3 | `v1.6.0` is Latest | `gh release list` + `releases/latest` | ✅ |
| P3 | no M17 stamp in DECISIONS.md | zero occurrences of `M17` or `1.7.0` | ✅ |
| P3 | tags **exactly** `v1.3.0 … v1.6.0` | **eight tags, not five** — see below | ⚠️ |
| P4 | orphan `31121996517` left as found | read-only: `queued` / `conclusion: null` / `run_attempt: 4`. Not touched, not re-run | ✅ |

### P3 — the one difference, and the owner's ruling

P3 says "Tags **exactly** `v1.3.0`, `v1.4.0`, `v1.4.1`, `v1.5.0`, `v1.6.0`". The
repo has **eight**: those five plus `v1.0.0`, `v1.1.0`, `v1.2.0`.

Nothing P3 lists is missing, and local and remote tag SHAs are byte-identical
for all eight. The three unlisted tags are the three **oldest**, a clean prefix
— a deletion or force-move would leave a hole in the middle or truncate the tip.
Each is recorded in `DECISIONS.md` §2 as an owner-accepted publication (M6 →
`v1.0.0` "cut from c363b56", which cross-checks exactly against that annotated
tag's target commit; M7a → `v1.1.0`; M7b → `v1.2.0`), and `gh release list`
shows a published non-draft release for each of the eight, one-to-one, with no
orphan in either direction. The one milestone §2 says shipped no release (M11,
"version stays 1.4.1") correctly has no tag.

**This was raised with the owner before any mutation and ruled an incomplete
enumeration in the spec — proceed, record the deviation.** An independent
read-only audit reached the same verdict separately (`DIFFERS`,
`stop_recommended: false`), and noted that the equivalent line in
`prompts/m13-release.md` reads "Tags exactly v1.0.0–v1.5.0" — the full range.
The M17 author truncated the list.

P3's operative content — `v1.6.0` is Latest, no M17 stamp, and (the clause P3
does not state but Phases 2–3 actually depend on) **the `v1.7.0` namespace is
empty** — all verify true. That last one was checked in four places: `git tag -l
'v1.7*'` empty, `git ls-remote --tags origin 'v1.7*'` empty, no v1.7 row in `gh
release list`, no v1.7 draft.

**Suggested spec correction for a future leg:** P3 should read "the five most
recent tags are `v1.3.0`–`v1.6.0`, atop `v1.0.0`/`v1.1.0`/`v1.2.0`".

## 2. Phase 1 — version + CHANGELOG

**Commit `4169d840d1bfb2c313974385dddba63c9da66e43` — `release: v1.7.0`.**

Exactly three files, no code:

```
 CHANGELOG.md | 50 ++++++++++++++++++++++++++++++++++++++++++++++++++
 Cargo.lock   |  6 +++---
 Cargo.toml   |  2 +-
 3 files changed, 54 insertions(+), 4 deletions(-)
```

- **Cargo.toml** — `[workspace.package] version` `1.6.0` → `1.7.0`, one line.
- **Cargo.lock** — the diff is exactly six lines: the `version` field of
  `usage-cli`, `usage-core`, `usage-ui`. **No third-party crate moved.**
- **CHANGELOG.md** — the entry was **extracted programmatically from
  `prompts/m17-release.md`'s own bytes and never retyped**, the discipline the
  §4a patches use. The script asserted the anchor `## [1.6.0] - 2026-08-05`
  appeared exactly once before, refused to run if the entry was already present,
  and asserted the entry appeared exactly once after. 2650 bytes, 49 lines,
  inserted immediately above the 1.6.0 heading, which moved from line 8 to
  line 58 (file 356 → 406 lines).

Byte fidelity confirmed two ways: `cmp` of `CHANGELOG.md` lines 8–56 against
spec lines 61–109 is **byte-identical**, and a UTF-8 re-read counts 8 em dashes
(U+2014), 3 middots (U+00B7) and **zero** U+FFFD replacement characters. No
link-reference line was added at the file's foot, as instructed — consistent
with current practice, since the foot still carries only `[1.2.0]`/`[1.1.0]`/
`[1.0.0]` and the convention was dropped after 1.2.0.

No §4.1 path touched. `git diff v1.6.0..HEAD -- .github/` is empty.

### Local §3 bar — green, on the tree that became `4169d84`

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | clean |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean |
| `cargo test --workspace --locked` | **473 passed, 0 failed** (cli 64 + cli-integration 13 + core 156 + ui 240 + 0 doc-tests) |
| `python tools/check-invariants.py` | `OK: 8 invariants, 30 test bindings, tags and manifest set-equal, SECURITY.md id set matches` |

### Phase 1 push and CI — 8/8 green, waited in the FOREGROUND

The push carried **three** commits (`83cc5d6`, `e333a8f`, `4169d84`), as P2 said
it would. `5aff938..4169d84 main -> main` at 2026-08-08 03:47:33Z.

Run **[31237987499](https://github.com/cipherpine/quotapane/actions/runs/31237987499)**
on `4169d84`, watched with `gh run watch 31237987499 --exit-status --interval 20`
(exit 0). Created 03:47:35Z, completed 03:51:17Z, conclusion **success**.

| Required check | Conclusion | Started → completed (UTC) |
|---|---|---|
| build & test (windows-latest) | success | 03:47:39 → 03:51:17 |
| build & test (ubuntu-latest) | success | 03:47:39 → 03:49:07 |
| build & test (macos-latest) | success | 03:47:40 → 03:49:37 |
| cargo-deny (licenses, bans, advisories, sources) | success | 03:47:39 → 03:48:05 |
| cargo-audit (RustSec advisories) | success | 03:47:39 → 03:50:46 |
| gitleaks — full-history secret scan | success | 03:47:39 → 03:47:43 |
| invariants — manifest, docs, and tests agree | success | 03:47:45 → 03:47:48 |
| invariant 4 — no telemetry | success | 03:47:39 → 03:47:45 |

## 3. Phase 2 — the rc tag

**`v1.7.0-rc.1` → `4169d840d1bfb2c313974385dddba63c9da66e43`.**

Created **lightweight** (`git cat-file -t v1.7.0-rc.1` → `commit`), matching the
form of `v1.4.1`, `v1.5.0` and `v1.6.0`; `v1.0.0`–`v1.4.0` are annotated tag
objects, and the style changed at `v1.4.1` and has held for the last three
releases. Pushed 2026-08-08 03:53:00Z.

**`release.yml` untouched**, proven by blob identity rather than by inspection:

```
$ git rev-parse HEAD:.github/workflows/release.yml
44860bab2a3d626010e617cbae007656ccae715e
$ git rev-parse v1.6.0:.github/workflows/release.yml
44860bab2a3d626010e617cbae007656ccae715e
```

## 4. The release run — 3/3 green, waited in the FOREGROUND

Run **[31238195378](https://github.com/cipherpine/quotapane/actions/runs/31238195378)**,
`gh run watch 31238195378 --exit-status --interval 25` (exit 0). Created
03:53:02Z, completed 03:57:47Z, conclusion **success**.

| Job | Conclusion | Started → completed (UTC) |
|---|---|---|
| build (x86_64-unknown-linux-gnu) | success | 03:53:05 → 03:56:05 |
| build (x86_64-pc-windows-msvc) | success | 03:53:05 → 03:57:27 |
| checksum, sign, attest, draft | success | 03:57:37 → 03:57:47 |

Draft: `isDraft: true`, `isPrerelease: false`, `publishedAt: null`, four assets.
**Nothing published.**

```
618b0291d592bb3dfd68690ca30a42e8c4296e21449432a1f333fdcc487cbee7  quotapane-v1.7.0-rc.1-x86_64-pc-windows-msvc.zip
c99f8fe20ab4c9464185ce4e93c0b75e1a4df1b8e2d1bf8133bb9f1260ce3e6f  quotapane-v1.7.0-rc.1-x86_64-unknown-linux-gnu.tar.gz
```

`TOOLCHAIN.txt` identical in both archives:

```
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
```

## 5. `tools/release-verify.sh v1.7.0-rc.1` — complete output

Run verbatim in Git Bash. The script ran to completion; no tooling failure.

```
PASS  step 1: downloaded 4 assets for v1.7.0-rc.1
PASS  step 2: sha256sum -c SHA256SUMS
PASS  step 3: cosign verify-blob (Verified OK)
PASS  step 4a: attestation quotapane-v1.7.0-rc.1-x86_64-pc-windows-msvc.zip
PASS  step 4b: attestation quotapane-v1.7.0-rc.1-x86_64-unknown-linux-gnu.tar.gz
PASS  R1: both attestation subjects digest-match disk
PASS  step 5: inventory + identical TOOLCHAIN.txt
PASS  step 6: shipped CLI runs (quotapane-cli 1.7.0, matches 1.7.0)
PASS  NC1/R3: zip bytes changed (618b0291d592bb3dfd68690ca30a42e8c4296e21449432a1f333fdcc487cbee7 -> 7c5a594bceffdb8dd6cc79ce99912d4a2cc08c2266d6eb007899b2757c8fff6e)
PASS  NC1: checksum names the tampered file specifically
PASS  NC1: attestation refuses tampered zip
PASS    restored: quotapane-v1.7.0-rc.1-x86_64-pc-windows-msvc.zip digest back to pristine
PASS  NC2: wrong identity rejected on the identity check specifically
PASS  NC3: wrong issuer rejected on the issuer check specifically
PASS  NC4/R3: SHA256SUMS bytes changed (53253dc5c2ea2e18bcd8291b87975bad95575bcca071d7cdc1ae6324acc2b896 -> 36c0c0f9a25bed7c296a8a73480144964acfb1db965b16889c5753c884791eb9)
PASS  NC4: cosign rejects tampered SHA256SUMS on the signature check
PASS    restored: SHA256SUMS digest back to pristine
PASS  NC5/R4: wrong-repo target cli/cli confirmed to exist
PASS  NC5: real repo, real digest-miss (no attestations for our artifact)
PASS  NC6: unrelated file has no attestation
PASS  sweep: all artifacts pristine, steps 2-4 re-run clean
RESULT: PASS — v1.7.0-rc.1: six steps, six controls, R1-R4
```

## 6. Content spot-check — three of four found, and the fourth needs a ruling

Evidence, not a gate, exactly as the spec frames it. Method: download both
archives fresh from the draft, unpack, and search the **raw bytes** of each GUI
binary with Python `bytes.find` — not `strings`, which is **not installed on
this machine** and silently produced empty output on the first attempt (that
first result was discarded as vacuous rather than read as absence).

| Needle | `quotapane.exe` | `quotapane` (linux) |
|---|---|---|
| `your turn` | **FOUND** @ 0x53faf3 | **FOUND** @ 0xa5033 |
| `in the loop` | **FOUND** @ 0x53fae8 | **FOUND** @ 0xa5028 |
| `// hide older` | **ABSENT** | **ABSENT** |

Shipped README carries `agents` **5 times** in both archives, and its SHA-256 is
byte-identical to the repo's `README.md`.

**So: three of four found. Per the spec, a partial miss is a limitation of the
spot-check and not a stop; only all four absent would be a signal.** No byte of
the tree was changed to chase it.

### Characterising the miss, for the Leg B ruling

I went further than the spec asked because the result did not match the
mechanism the spec anticipated, and the top tier should not have to re-derive
this.

The sibling strings **are** present, in one tight cluster:

```
// no agent sessions in the last 24h // nothing active in the last 2h
// <fmt> older today
in the loopyour turn
usageagents
```

That `// ` + placeholder + ` older today` run is the **format template** for
`format!("// {older} older today")` — the literal pieces survive, the
interpolated whole never existed. `in the loopyour turn` shows the two turn
phrases merged adjacently in the string table. `// hide older` is nowhere in the
8.5 MB binary; neither is the fragment `hide older`.

Two controls:

- **A clean local release build reproduces it exactly.** `cargo build --release`
  → `// hide older` ABSENT, siblings present at the same relative offsets. So it
  is a property of `[profile.release]` (`lto = true`, `codegen-units = 1`,
  `strip = true`), **not** of CI, the tag, or the artifact pipeline.
- **The debug build contains it.** `target/debug/quotapane.exe` → `// hide
  older` **FOUND**. The string exists in source and survives an unoptimised
  build.

The spec predicted "a string const that is only ever formatted into a larger
literal can be folded away". **This const does not match that description** — it
is `const HIDE_OLDER_LINE: &str = "// hide older";` returned whole via
`.to_string()` from `older_toggle_line`, not interpolated into anything. That is
why I am flagging it rather than filing it as an expected limitation.

What is already known-good: the UI test
`the_pane_opens_on_the_last_two_hours_and_keeps_the_rest_one_click_away`
asserts the expanded pane paints a line `== HIDE_OLDER_LINE`, and it passes —
but in a test build, which is not the release profile. **Nothing in this session
has exercised the expanded foot line in a release binary**, and doing so needs a
GUI, which is §4.5 territory and the owner's alone.

**Recommended for Leg B, in preference order:** (a) the owner clicks the toggle
once in the shipped rc binary and confirms the line reads `// hide older` — one
look settles it; or (b) the top tier rules it an accepted spot-check limitation
and Phase 3 proceeds unchanged. I have not changed code and make no
recommendation beyond putting the evidence on the record.

## 7. Deviations

**D1 — P3's tag enumeration is incomplete (eight tags, not five).** Raised with
the owner before any mutation; ruled an incomplete enumeration; proceeded.
Full argument in §1.

**D2 — the spec does not name Phase 2's report path or commit subject.** Spec
line 136 says only "write the report, commit it, push". `prompts/m13-release.md`
named `reports/m13-release-rc.md` explicitly. The operator's Leg A instructions
supplied both — `reports/m17-release-rc.md` and `reports: M17-RELEASE Leg A — rc
verified` — matching the M11d convention in `reports/README.md`, and that is
what this file uses. Recorded because the spec itself is silent.

**D3 — the spec does not state the session's model tier.** `DECISIONS.md` §6
says in bold that every goal prompt states it, and `CLAUDE.md`'s handoff format
requires "(1) the model to set". Spec line 8 gives a session *mode* ("attended
CLI session"), not a tier. This session ran on **Opus 5**, which `CLAUDE.md`
places at the top tier, so everything attempted was in bounds — including Phase
1, which touches no §4.1 path anyway. Flagged so Phase 4, which does land bytes
in `DECISIONS.md`, is dispatched with the tier stated.

**D4 — the dictated CHANGELOG entry drops the JSON-surface and dependency
line.** Every entry from 1.1.0 through 1.6.0 states the CLI JSON surface's
status; 1.4.1, 1.5.0 and 1.6.0 each close with a variant of "No JSON key changed
in this release. Zero new dependencies." The 1.7.0 entry has neither. **Both
claims would have been true**: `git diff --stat v1.6.0..HEAD -- crates/` touches
only `usage-core/{agents.rs,lib.rs}` and `usage-ui/{config.rs,main.rs}`, with
zero `usage-cli` changes, and the Phase 1 lock diff is exactly the three member
versions. `docs/cli-json.md`'s stability contract promises new keys "are
announced here", and readers have six releases of training to look for that
closing line. The spec says insert **VERBATIM**, so it was inserted verbatim and
not corrected. **This becomes the published release body in Phase 3** — a Leg B
ruling is cheap now and expensive after publication.

**D5 — the pinned date `2026-08-08` is perishable.** The entry heading and Phase
4's DECISIONS stamp both hard-code `2026-08-08`, and every prior heading's date
equals its tag's date. The Phase 1 commit is 2026-08-08 03:40 UTC, so it is
correct today. But Phase 3 waits on a hand-carried paste with no deadline: if
`v1.7.0` is tagged after 2026-08-08 24:00 UTC, `[1.7.0] - 2026-08-08` becomes
the first heading in the file whose date does not match its tag, and Phase 4's
stamp would assert a publication date that did not happen. Both are pinned
("date literal as written", "never retype"), so no session can correct either
without a top-tier ruling.

**D6 — spot-check needle vs CHANGELOG prose.** The spec's binary needle is
`// hide older`; the CHANGELOG describes `// N older today`. Both are real
states of one widget, so this is not a contradiction — but the needle the spec
names is the expanded-state toggle the CHANGELOG never mentions, and the
formatted one cannot survive as a contiguous literal by construction. Noted so
it is on the record rather than rediscovered.

## 8. Housekeeping

`.git` swept after every git operation: no `*.lock`, no
`objects/maintenance.lock`, no `objects/*/tmp_obj_*` at any checkpoint. Nothing
needed moving to `_to_delete/git-stale/`. Working tree clean throughout; the
only untracked working files were under the session scratchpad, outside the
repo.

The orphaned run `31121996517` was read once, read-only, and **not touched, not
cancelled, not re-run** (P4).

## 9. State at the stop, and what Leg B needs

| | |
|---|---|
| `origin/main` | `4169d84` — `release: v1.7.0` (+ this report commit) |
| Tag `v1.7.0-rc.1` | exists, lightweight, on `4169d84`, pushed |
| Draft for `v1.7.0-rc.1` | exists, 4 assets, **unpublished** |
| Tag `v1.7.0` | **does not exist** |
| Published releases | unchanged — `v1.6.0` is still Latest |
| `DECISIONS.md` | untouched; no M17 stamp |
| Phase 3 | **not begun** |

**Rulings Leg B should carry:**

1. The `// hide older` spot-check miss (§6) — accept as a limitation, or have
   the owner confirm the toggle in the rc binary first.
2. D4 — whether the CHANGELOG entry ships without its JSON/dependency closing
   line, given it becomes the published release body.
3. D5 — whether the pinned `2026-08-08` still holds when the tag is actually
   cut.

Acceptance is the owner's (§4.8). Nothing here is self-accepted; no visual was
reviewed and no screen was captured (§4.5).
