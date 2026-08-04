# M12-RELEASE Leg C — v1.5.0 published; M12 stamped and closed

**Session:** floor (Opus, Claude Code), headless under the M11d dispatcher,
2026-08-04.
**Spec:** `prompts/m12-release.md`, LEG C only (Phase 4).
**Queue file:** `prompts/queue/m12-release-c.md` — written by the top tier
after the owner confirmed publication.
**Tree footprint of this leg:** `DECISIONS.md` (Patch A, §4a verify-and-commit)
and this report. Nothing else.

> **Verdict: the release is closed.** `v1.5.0` is published
> (`publishedAt: 2026-08-04T21:25:12Z`, `isDraft: false`, `isPrerelease:
> false`, and the repository's `releases/latest` now resolves to `v1.5.0`).
> The published bytes were re-measured independently this leg and are
> byte-identical to the draft Leg B verified. `DECISIONS.md` carries the M12
> stamp, applied byte-for-byte from the spec's own bytes under §4a. Nothing
> was published, tagged, pruned, or authored by this session.

---

## 1. The publish gate (§Leg C precondition — STOP if unmet)

```
$ gh release view v1.5.0 --json tagName,isDraft,isPrerelease,publishedAt,createdAt,url,name
{"createdAt":"2026-08-04T13:23:56Z","isDraft":false,"isPrerelease":false,
 "name":"QuotaPane v1.5.0","publishedAt":"2026-08-04T21:25:12Z","tagName":"v1.5.0",
 "url":"https://github.com/cipherpine/quotapane/releases/tag/v1.5.0"}

$ gh api repos/cipherpine/quotapane/releases/latest --jq .tag_name
v1.5.0
```

| Gate | Required | Observed | |
|---|---|---|---|
| C1 | `publishedAt` non-null | `2026-08-04T21:25:12Z` | ✅ |
| C2 | Not a draft | `isDraft: false` | ✅ |
| C3 | Not a prerelease | `isPrerelease: false` | ✅ |
| C4 | Release URL is now the tag URL, not `untagged-…` | `…/releases/tag/v1.5.0` (was `…/untagged-f8580f04406f335eecb7` in Leg B §3) | ✅ |
| C5 | `v1.5.0` is Latest | `gh release list`: `QuotaPane v1.5.0  Latest  v1.5.0  2026-08-04T21:25:12Z`; `releases/latest` → `v1.5.0` | ✅ |

The gate is met. Proceeded to Patch A.

---

## 2. Patch A — §4a verify-and-commit, `DECISIONS.md` only

### 2.1 A recorded discrepancy in the spec, resolved by the queue file

The spec's Leg C prose reads *"Then **two** §4a replacements"*, but the spec
contains exactly **one** patch block. Verified mechanically rather than by
eye:

```
$ grep -n "Patch\|OLD:\|NEW:" prompts/m12-release.md
118:Patch A:
119:OLD: …
120:NEW: …
```

One `Patch A:` marker, one `OLD:` line, one `NEW:` line — no `Patch B`
anywhere in the file. The extraction script asserts this and would have
stopped had it found otherwise.

The Leg C queue file, written by the top tier after the spec, names the work
singular: *"apply the spec's Patch A to DECISIONS.md."* That is the top tier's
own resolution, and it matches the bytes that exist. **This session applied
Patch A and nothing else.** Authoring a second replacement was never an
option: §4a.3 forbids this session originating protected-path bytes, and no
second patch's bytes exist to verify. Recorded here as a fact for the top
tier — the word "two" appears to be spec drafting residue, and
`prompts/m12-release.md` was not edited to correct it (it is a spec of record
and not this session's to author).

### 2.2 Extraction — programmatic, from the spec's bytes

`OLD`/`NEW` were never retyped. They were located structurally (the two lines
immediately following the `Patch A:` marker), prefix-stripped, and hashed:

```
spec sha256    : aabd57360379e87382bf1016f7c1e204e6d4fa6e2475a8310f3d6e4e66dd81a5
                 (prompts/m12-release.md, unmodified, as committed in d2afb47)
Patch A: marker at spec line 118
OLD: lines in spec -> [119]      NEW: lines in spec -> [120]
OLD len=83  sha256=f6c40913f69c8dc62dadbf7d23ef1c841d3a44857eceafc0a6b20ad4e4b2fd3b
NEW len=819 sha256=13e9d3dc45f1a55dec3aabf21d13059e35c0d1a3a8a024e953ba67fae618ccba
```

The script asserted, and would have exited non-zero on any of: no `Patch A:`
marker; more or fewer than one `OLD:`/`NEW:` pair; `OLD:`/`NEW:` not being the
two lines immediately after the marker.

### 2.3 Uniqueness — before and after

```
count(OLD) in DECISIONS.md before : 1     ← unique anchor
count(NEW) in DECISIONS.md before : 0     ← not already applied
count("M12") in DECISIONS.md before: 0    ← P2's "no M12 stamp yet" still true

count(NEW) in DECISIONS.md after  : 1     ← applied exactly once
count(OLD) in DECISIONS.md after  : 0     ← anchor fully consumed
```

Both files are LF-only with no BOM (`CRLF=0`, `BOM=False`), and the write was
done in binary, so no line-ending or encoding normalisation could occur. The
post-write assertions confirm it:

```
before: 13639 bytes  sha256=626779acef5916eb7a6a8a205e065e8f2b6831fd028a7bd26f9f9d0dbfc2f900
after : 14375 bytes  sha256=88c5751b057558bcaba7166b069b1a4acdc69578fa70436ca680d7abae10250c
delta : +736 bytes (= -len(OLD) 83 + len(NEW) 819)   ← exact arithmetic, no drift
LF count unchanged: True (65)      CR bytes in file: 0
independent rebuild from spec bytes matches disk: True
```

The last line is a second, independent derivation: the expected file was
rebuilt from the original `DECISIONS.md` bytes plus the spec's `OLD`/`NEW` and
its SHA-256 compared to what is actually on disk. They match.

### 2.4 Diff scope — one line, one insertion, one file

```
$ git status --porcelain
 M DECISIONS.md

$ git diff --stat
 DECISIONS.md | 2 +-
 1 file changed, 1 insertion(+), 1 deletion(-)
```

`git diff --word-diff=plain -U0` reports a **single** `{+…+}` insertion on
line 24 and **no** `[-…-]` deletion — i.e. the roadmap line gained the M12
sentence and lost nothing:

```
… the top tier's Phase-2→3 go-ahead (owner decisions 2026-08-03).
{+· **M12 CLI automation ✅ (v1.5.0 published — owner-accepted 2026-08-04)**: `--fail-at <N>`
(exit 3; the gate covers every reported window incl. per-model buckets — proven live by the
owner tripping on a per-model quota the text summary doesn't even display), `--watch <SECS>`
(180s floor, RFC 3339 separators, NDJSON), documented exit codes, docs/cli-json.md stability
contract; zero new deps, no JSON key changed. First slice executed end to end under the M11d
headless dispatcher (owner touch: one trust dialog and pushes); the floor mutation-tested its
own tests 8/8, found and closed two of its own escapes, and honestly flagged the one untested
hop (exit-3 wiring), closed by an owner live run (owner decisions 2026-08-04).+}
Post-1.0 backlog: …
```

(Line-wrapped here for readability only; on disk it is part of the single long
§2 roadmap line, exactly as the spec's `NEW` bytes specify.)

**No other protected path changed.** Filtering the changed-file list against
§4.1's full path set returns `DECISIONS.md` and nothing else — no
`crates/usage-core/src/egress/**`, `crates/usage-core/src/credentials/**`,
`deny.toml`, `SECURITY.md`, `THREAT_MODEL.md`, `.github/**`, `.cargo/**`, or
`.claude/**`.

### 2.5 §4a checklist

| §4a requirement | How it was met |
|---|---|
| 1. Pre-authored at the top tier | The `NEW` bytes are the spec's own line 120, authored at the top tier in `d2afb47` and supplied verbatim in the goal prompt |
| 2. Verified byte-for-byte before committing | §2.2–2.4: SHA-256 of both operands, exact length arithmetic, count-before/count-after, independent rebuild, single-file diff |
| 3. Authors nothing itself | Zero characters typed into `DECISIONS.md` by this session; the replacement is a byte operation on extracted spec bytes. No fix-ups, no reformatting, no opportunistic edits — including none to the spec's "two" wording (§2.1) |

---

## 3. The v1.5.0 release ledger

### 3.1 Commits — build through close

Base `3cbd1f2` *prompts: M12 spec + launcher — CLI automation (v1.5.0 slice)*.

| SHA | Subject | Leg / phase | Author |
|---|---|---|---|
| `8f0999b` | `cli: --fail-at gate logic and flag parsing` | M12 build P1 | floor |
| `bac6b29` | `cli: --watch mode with NDJSON output` | M12 build P2 | floor |
| `26fe690` | `docs: CLI automation + the JSON stability contract` | M12 build P3 | floor |
| `812ac32` | `reports: M12 end-gate` | M12 build report | floor |
| `d2afb47` | `prompts: M12-RELEASE spec + launchers — v1.5.0` | release spec | top tier |
| `5c0c85d` | `release: v1.5.0` | **Leg A Phase 1** — version 1.4.1→1.5.0, `Cargo.lock` (3 workspace members only), CHANGELOG entry | floor |
| `61d67cc` | `reports: M12-RELEASE Leg A — rc verified` | Leg A report | floor |
| `9ced94c` | `tools: release-verify — rc tags carry the base crate version (step 6)` | step-6 fix | top tier |
| `ae629ef` | `reports: M12-RELEASE Leg A-2 — rc re-verified after step-6 fix` | Leg A-2 report | floor |
| `fb1f4a5` | `reports: M12-RELEASE Leg B — draft verified` | Leg B report | floor |
| `78f1b5a` | `tools: dispatch — live per-turn progress via stream-json; utf-8 jsonl logs` | dispatcher tooling | owner (see §6) |
| *this commit* | `docs: v1.5.0 published; M12 accepted (owner)` | **Leg C Phase 4** | floor |

`v1.5.0` -> `5c0c85da8a0eb5b1180551a8c82133655d294636` (`release: v1.5.0`) —
the *same* commit the rc verified, tagged in Leg B and never moved.

### 3.2 Tags and releases, final state

```
$ git tag --list
v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0

$ git ls-remote --tags origin | grep 1.5.0
5c0c85da8a0eb5b1180551a8c82133655d294636  refs/tags/v1.5.0

$ gh release list --limit 5
QuotaPane v1.5.0   Latest   v1.5.0   2026-08-04T21:25:12Z
QuotaPane v1.4.1            v1.4.1   2026-08-01T21:10:03Z
QuotaPane v1.4.0            v1.4.0   2026-07-31T15:24:09Z
QuotaPane v1.3.0            v1.3.0   2026-07-30T04:25:00Z
QuotaPane v1.2.0            v1.2.0   2026-07-29T15:35:49Z
```

No `v1.5.0-rc.1` tag and no rc draft survive, locally or on the remote —
pruned in Leg B §6 only after that leg's `RESULT: PASS`. Seven tags, one per
release, exactly as before plus `v1.5.0`.

### 3.3 Verification runs — the whole chain

| Run | Tag | Verdict | Recorded in |
|---|---|---|---|
| Leg A Phase 2 | `v1.5.0-rc.1` | `RESULT: FAIL — 1 check(s) failed` (20 PASS / 1 FAIL, exit 1) — step 6 grepped the binary for the full tag `1.5.0-rc.1`; a correctly-built rc cannot pass. Not worked around; escalated. | `reports/m12-release-rc.md` §4–5 |
| Leg A-2 (after top-tier fix `9ced94c`) | `v1.5.0-rc.1` | `RESULT: PASS — six steps, six controls, R1-R4` (21 PASS / 0 FAIL, exit 0), same artifact bytes re-judged | `reports/m12-release-rc.md` (re-run section) |
| Leg B Phase 3 | `v1.5.0` | `RESULT: PASS — v1.5.0: six steps, six controls, R1-R4` (21 PASS / 0 FAIL, exit 0) | `reports/m12-release-draft.md` §4 |

`tools/release-verify.sh` was run verbatim each time, and edited only once —
by the top tier, in `9ced94c`. No floor session modified it.

### 3.4 Published artifacts — re-measured this leg

The four published assets were downloaded fresh into a scratch `mktemp -d`
outside the repository and hashed. This is a **post-publication** measurement,
independent of Leg B's:

```
36081d697d293a21fd26eb143a193bab10d41e4b7cf2d014268c2d97cfd11ab9  quotapane-v1.5.0-x86_64-pc-windows-msvc.zip
5d798d79c1b4b10e1d55a41eb40500105bcb772a7960474aca7cd18ad0c9b304  quotapane-v1.5.0-x86_64-unknown-linux-gnu.tar.gz
a7037b020842a7a94dd727cfa2dfcdd2cc0b98d40f070bea7a84ca45a46742fc  SHA256SUMS
f6d3a59370530227c3e4da467c2c7e0b2b2ec44b11ec541c13c86f3b7061a4b0  SHA256SUMS.sigstore.json

$ sha256sum -c SHA256SUMS
quotapane-v1.5.0-x86_64-pc-windows-msvc.zip: OK
quotapane-v1.5.0-x86_64-unknown-linux-gnu.tar.gz: OK
exit=0
```

**All four digests are identical to Leg B §5's**, and identical to the digests
the GitHub API reports for the release assets. Publishing changed no bytes:
what the owner published is exactly what Leg B verified against cosign and the
provenance attestations. This also closes one of the two gaps Leg B §7 flagged
as *not captured* — a post-prune byte-level re-measurement. (The other, the
in-archive `TOOLCHAIN.txt` contents, remains uncaptured; step 5 asserts the two
archives carry an identical one but does not print it. Stated, not omitted.)

The scratch directory was removed; nothing was copied into the tree. Only
`gh release download` and `sha256sum` were run — no binary from the archives
was executed this leg, and no credential file was touched.

### 3.5 Release pipeline integrity

`release.yml` is the same blob at `v1.4.1`, `v1.5.0-rc.1` (Leg B §3) and
`v1.5.0`:

```
v1.4.1  44860bab2a3d626010e617cbae007656ccae715e
v1.5.0  44860bab2a3d626010e617cbae007656ccae715e
```

The release workflow was not touched anywhere in M12.

### 3.6 The published release body

The body the owner pasted reconciles with the CHANGELOG:

- Its content section is the `## [1.5.0] - 2026-08-04` entry from
  `CHANGELOG.md:10-43`. It is **not** byte-identical — the paste soft-wraps
  differently (e.g. `…scripts and\nagents.` in the file vs `…scripts and
  agents.` in the body) — but after whitespace normalisation the two are
  identical (`sha256(norm) = 31569643fc36e91a…` both sides). No word, link, or
  claim differs; the rendered Markdown is the same.
- Its footer is exactly the text Leg B §9 prescribed, including the retargeted
  compare link
  `https://github.com/cipherpine/quotapane/compare/v1.4.1...v1.5.0`.

Carried forward unchanged from Leg B §9, as a fact and not a change: the
footer says *"Verifying a release"* while the README's actual heading is
`## Verify a release` — consistent with every release body since v1.0.0.
`README.md` was not touched by this leg either.

---

## 4. Version and content on disk

| Fact | Value |
|---|---|
| `Cargo.toml:10` | `version = "1.5.0"` |
| Tests | **321 passed, 0 failed, 0 ignored** (64 + 13 + 113 + 131 + 0 doc-tests) — unchanged since the Phase-1 commit |
| Shipped CLI content (Leg A §6) | both binaries carry `fail-at: ` and the `exit codes:` help block; shipped README's Verify section byte-identical to v1.4.1's (`7055e902…`) |

---

## 5. §3 verification bar for this push

Run locally before the commit, though the commit is documentation-only —
DECISIONS §3 requires it for *every* push:

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | **321 passed, 0 failed, 0 ignored** (64 + 13 + 113 + 131 + 0) |
| `python3 tools/check-invariants.py` | `OK: 7 invariants, 24 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |

---

## 6. State left behind, and one thing this push carries that isn't this leg's

- `main` = `78f1b5a` + this leg's single commit. **This leg's entire tree
  footprint is `DECISIONS.md` (Patch A) and this report.**
- **Carried, not authored:** `78f1b5a` *"tools: dispatch — live per-turn
  progress via stream-json; utf-8 jsonl logs"* was already committed locally
  and unpushed when this session started (`HEAD` = `78f1b5a`, `origin/main` =
  `fb1f4a5`). It is the **owner's own** commit (`Justin Parsons
  <282068396+cipherpine@users.noreply.github.com>`, 2026-08-04T16:08:41Z),
  touches exactly one file — `tools/dispatch.ps1`, +44/−8 — and touches **no**
  §4.1 protected path. This session did not create, amend, reorder, or modify
  it; this leg's push simply carries it to `origin`, the same way Leg A's push
  carried `d2afb47` and Leg A-2's carried `9ced94c`. Flagged because a push
  landing a commit the session did not author is a fact the owner should see
  stated, not discovered.
- Tag `v1.5.0` -> `5c0c85d`, published. `v1.4.1` is no longer Latest.
- No dependency added, removed, or pinned. No code, `.github/`, `.claude/`,
  `.cargo/`, `assets/`, `README.md`, `CHANGELOG.md`, `Cargo.*`, `tools/`,
  `SECURITY.md`, `THREAT_MODEL.md`, or `deny.toml` change from this session.
- Nothing published, tagged, pruned, or deleted by this session. Publication
  was the owner's act, at 2026-08-04T21:25:12Z.
- No credential file was read; no token material was handled, printed, or
  logged.

---

## 7. §4 conditions hit, and deviations

- **§4.1 / §4a — DECISIONS.md.** Hit by design; the spec's sole authorised
  exception. Discharged as a verify-and-commit under §4a with the evidence in
  §2. Nothing in those paths was authored.
- **§4.7 — spec vs. reality: "two §4a replacements", one patch block.**
  Recorded in §2.1 rather than improvised around. It is not a blocking
  conflict: the queue file (top tier, written later) names Patch A singular,
  the bytes for a second patch do not exist, and authoring one is forbidden by
  §4a.3 — so the §4a-compliant action set contains exactly one member, and
  that is what ran. **The top tier should confirm no second amendment was
  intended.** If one was, it needs top-tier authoring and its own leg; this
  session will not guess at it.
- No other deviation. The spec's DO-NOT list was honoured in full: nothing
  published, no code/`.github/`/`assets/`/`README.md`/§4.1 path touched beyond
  the authorised `DECISIONS.md` replacement, no dependency added, no leg
  boundary crossed.

---

## 8. What happens next

**Nothing is queued.** M12-RELEASE is complete across all four phases:

1. M12 build (P1–P3) — owner-accepted, `reports/m12-endgate.md`.
2. Leg A / A-2 — rc cut and verified, `reports/m12-release-rc.md`.
3. Leg B — `v1.5.0` tagged, verified, draft raised, `reports/m12-release-draft.md`.
4. Leg C — published by the owner, stamped in `DECISIONS.md`, this report.

For the owner / top tier, in descending order of importance:

1. **Confirm the §2.1 "two replacements" reading** — the one open question
   this leg leaves behind.
2. Optional, previously deferred: the release-body *"Verifying a release"* vs
   README *"Verify a release"* wording (§3.6), untouched since v1.0.0.
3. Post-1.0 backlog is unchanged by this release: packaging
   (WinGet/Homebrew/AUR), deferred M5 features, dead `RateLimitHeaders`
   cleanup, dormant-cadence decision.

This session ends here, as instructed — the hard stop is the session end.
