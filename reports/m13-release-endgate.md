# M13-RELEASE Leg C — v1.6.0 published; M13 stamped and closed

**Session:** floor (Opus, Claude Code), headless under the M11d dispatcher,
2026-08-06T00:49Z.
**Spec:** `prompts/m13-release.md`, LEG C only (Phase 4).
**Queue file:** `prompts/queue/m13-release-c.md` — written by the top tier
after the owner confirmed publication; that act is the Phase-4 go-ahead.
**Tree footprint of this leg:** `DECISIONS.md` (the spec's single §4a
replacement) and this report. Nothing else.
**Host toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0
(c980f4866 2026-06-30), Python 3.14.4, gh 2.92.0 (2026-04-28), cosign v3.1.2,
host MINGW64_NT-10.0-26200.

> **Verdict: the release is closed.** `v1.6.0` is published —
> `publishedAt: 2026-08-06T00:42:23Z`, `isDraft: false`, `isPrerelease: false`,
> and `releases/latest` now resolves to `v1.6.0`. The published bytes were
> re-measured independently this leg and are byte-identical to the four digests
> Leg B verified against cosign and the provenance attestations; publishing
> changed metadata, not artifacts. `DECISIONS.md` carries the M13 stamp,
> applied byte-for-byte from the spec's own bytes under §4a and proved in both
> directions. Nothing was published, tagged, pruned, edited, or authored by
> this session.

---

## 1. The publish gate (Leg C precondition — STOP if unmet)

```
$ gh release view v1.6.0 --json tagName,name,isDraft,isPrerelease,publishedAt,createdAt,url,targetCommitish
{"createdAt":"2026-08-05T14:49:07Z","isDraft":false,"isPrerelease":false,
 "name":"QuotaPane v1.6.0","publishedAt":"2026-08-06T00:42:23Z","tagName":"v1.6.0",
 "targetCommitish":"main","url":"https://github.com/cipherpine/quotapane/releases/tag/v1.6.0"}

$ gh api repos/cipherpine/quotapane/releases/latest -q .tag_name
v1.6.0
```

| Gate | Required | Observed | |
|---|---|---|---|
| C1 | `publishedAt` non-null | `2026-08-06T00:42:23Z` | ✅ |
| C2 | Not a draft | `isDraft: false` | ✅ |
| C3 | Not a prerelease | `isPrerelease: false` | ✅ |
| C4 | URL is the tag URL, not `untagged-…` | `…/releases/tag/v1.6.0` (was `…/untagged-b244fb45915bfbb53c98` in Leg B §3) | ✅ |
| C5 | `v1.6.0` is Latest | `gh release list`: `QuotaPane v1.6.0  Latest  v1.6.0  2026-08-06T00:42:23Z`; `releases/latest` → `v1.6.0` | ✅ |
| C6 | Tag unmoved since Leg B | `v1.6.0` -> `d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e` (`release: v1.6.0`), local and remote | ✅ |

The gate is met by this session's own `gh` reads. The queue file's addendum
stated the same facts from the top tier's check; it was treated as context, not
as a substitute — every value in the table above was read here.

Proceeded to the §4a patch.

---

## 2. The §4a replacement — `DECISIONS.md` only

### 2.1 Extraction — programmatic, from the spec's bytes

`OLD`/`NEW` were never retyped. A one-shot script **outside the repository**
(`C:\tmp\m13c\apply.py` — per the Leg A top-tier ruling that one-shot tools do
not accumulate in the tree) located them structurally as the single line
beginning `OLD: ` and the single line beginning `NEW: `, prefix-stripped both,
and hashed everything it touched:

```
spec  prompts/m13-release.md   6199 bytes  sha256 b34841c406738bcb78c594e38c2e9c7671d5a702db2294774dc3c4d402129254
dec   DECISIONS.md            14375 bytes  sha256 88c5751b057558bcaba7166b069b1a4acdc69578fa70436ca680d7abae10250c

OLD     75 bytes  sha256 e01a24975684ec3055e0e262671ec48382e5074c74c689ee49494b0557196f77
NEW   1011 bytes  sha256 980d4803348b1baece2852e859927e359d60c7d8dd994b4371ceb6a3c825c1d2
```

The script asserts, and exits non-zero on, any of: more or fewer than one
`OLD: ` line; more or fewer than one `NEW: ` line; either string empty; the two
equal; a stray `CR` byte in either. The spec's `DECISIONS.md` sha256
(`88c5751b…`) is the same value M12-RELEASE Leg C §2.3 recorded as its
*post*-patch hash — the file has not been touched between the two releases.

The spec carries exactly one patch and describes exactly one: *"One §4a
replacement, DECISIONS.md only."* No M12-style singular/plural drift this time.

### 2.2 Uniqueness — before and after

```
before: DECISIONS.md contains OLD x1, NEW x0
after:  DECISIONS.md contains NEW x1, OLD x0

splice at byte offset 8609; delta +936 bytes  (= -len(OLD) 75 + len(NEW) 1011)
result 15311 bytes  sha256 fb637c895582c8547e5f3e2d51a3edb0048d8d40055f2757d317a7b41022e007
newline count unchanged: 65
```

`OLD x1` before is the unique anchor P2 requires; `NEW x0` before confirms the
stamp was not already applied. Both files are LF-only with no CR byte anywhere
(asserted on the extracted strings, on the HEAD blob, and on the written file),
and the read/replace/write ran entirely in **binary**, so no line-ending or
encoding normalisation could occur.

Beyond the counts, the script asserts the splice is surgical: the bytes before
offset 8609 are unchanged, the bytes at the splice are exactly `NEW`, and the
bytes after are the original tail — i.e. nothing outside the substitution
moved.

### 2.3 Verified in both directions, independently of the applying script

A separate check compared the working tree against the committed blob rather
than re-running the same code path:

```
HEAD blob sha256                 : 88c5751b057558bcaba7166b069b1a4acdc69578fa70436ca680d7abae10250c
disk      sha256                 : fb637c895582c8547e5f3e2d51a3edb0048d8d40055f2757d317a7b41022e007
forward  HEAD.replace(OLD,NEW) == disk : True
reverse  disk.replace(NEW,OLD) == HEAD : True
NEW on disk exactly once         : True
OLD absent on disk               : True
no CR bytes in disk file / HEAD  : True / True
newlines head/disk               : 65 / 65
```

The **reverse** direction is the strong claim: undoing the substitution on the
on-disk file reproduces `git show HEAD:DECISIONS.md` byte-for-byte. That is
only possible if the sole difference between HEAD and the working tree is the
spec's `OLD` → `NEW` substitution — no whitespace touch-up, no stray character,
nothing else.

### 2.4 Diff scope — one file, one line

```
$ git status --porcelain
 M DECISIONS.md

$ git diff --numstat
1	1	DECISIONS.md
```

One file, one insertion, one deletion — the §2 roadmap line, which is a single
very long line, replaced by itself plus the M13 sentence. **No other protected
path changed.** The changed-file list against §4.1's full path set returns
`DECISIONS.md` and nothing else: no `crates/usage-core/src/egress/**`, no
`crates/usage-core/src/credentials/**`, no `deny.toml`, `SECURITY.md`,
`THREAT_MODEL.md`, `.github/**`, `.cargo/**`, or `.claude/**`.

The stamp as it now reads on disk (line-wrapped here for legibility only; on
disk it is part of the single §2 roadmap line, exactly as `NEW` specifies):

```
· **M13 pace follow-ons ✅ (v1.6.0 published — owner-accepted 2026-08-05; owner will re-look
at sparklines in production)**: config.cfg key=value preferences (theme.cfg migrated, read as
fallback, never written again); opt-in history.jsonl (timestamps/labels/percentages only,
256 KiB keep-newest-half) reseeding the pace ring at launch; 24h painter sparklines,
legibility-iterated in M13-R1 after round-1 §4.5 feedback (full-alpha stroke + fill + now-dot
+ `24h` tag, demo window sized to fit); dep-free time-aware alerts — banner, cardinal tray
ring, `ALERT — ` tooltip, RequestUserAttention — with OS toasts declined by ADR (tray-icon
exposes no balloon API; the window is always-on-top) and threshold mode as fallback for
unknown-duration windows. Invariant 1 rewritten under the M11 checker in the same commit as
the behavior. First slice §4.5-reviewed across two rounds fully headless (owner decisions
2026-08-04/05).
```

`Post-1.0 backlog:` still occurs exactly once in the file, immediately after
the new sentence — the anchor was consumed and re-emitted by `NEW`, not
duplicated.

### 2.5 §4a checklist

| §4a requirement | How it was met |
|---|---|
| 1. Pre-authored at the top tier | The `NEW` bytes are the spec's own `NEW:` line, authored at the top tier in `6f80e56` (`prompts: M13-RELEASE spec + launchers — v1.6.0`) and supplied verbatim in the goal prompt |
| 2. Verified byte-for-byte before committing | §2.1–2.4: SHA-256 of every operand, exact length arithmetic, count-before/count-after, byte-range splice assertions, an independent **reverse** reconstruction of the HEAD blob, and a single-file one-line diff |
| 3. Authors nothing itself | Zero characters were typed into `DECISIONS.md` by this session; the change is a binary substitution of bytes extracted from the spec. No fix-ups, no reformatting, no opportunistic edits — including none to the two now-partly-stale backlog phrases flagged in §6.2, which this session may observe but not touch |

---

## 3. The v1.6.0 release ledger

### 3.1 Commits — build through close

Base `062376f` *docs: v1.5.0 published; M12 accepted (owner)* (M12's Leg C).

| SHA | Subject | Leg / phase | Author |
|---|---|---|---|
| `1f89673` | `prompts: M13 spec + launcher — pace follow-ons (v1.6.0 slice)` | M13 spec | top tier |
| `b0ca0c5` | `ui: config.cfg — key=value preferences with theme.cfg migration` | M13 build P1 | floor |
| `8f2dc3e` | `core,ui: opt-in disk history; forecasts survive restart` | M13 build P2 | floor |
| `943c8c5` | `ui: 24h sparkline strip per provider (history-fed)` | M13 build P3 | floor |
| `033d20a` | `ui: time-aware quota alerts — banner, tray ring, attention (dep-free)` | M13 build P4 | floor |
| `3687e60` | `ui: keep the alert tooltip prefix compiled on tray-less platforms` | P4 CI fix | floor |
| `8cb622c` | `reports: M13 end-gate` | M13 build report | floor |
| `aaa4b03` | `reports: M13 end-gate — record the report commit's own CI run` | CI follow-up | floor |
| `59be776` | `prompts: M13-R1 spec + launcher — sparkline legibility iteration` | R1 spec | top tier |
| `ed4f130` | `M13-R1: make the sparkline read as an instrument` | R1 build (§4.5 round 1 feedback) | floor |
| `1309957` | `reports: M13-R1 end-gate` | R1 report | floor |
| `6f80e56` | `prompts: M13-RELEASE spec + launchers — v1.6.0` | release spec | top tier |
| `d86cb5a` | `release: v1.6.0` | **Leg A Phase 1** — version 1.5.0→1.6.0, `Cargo.lock` (3 workspace members only), CHANGELOG entry, `git rm tools/m13-apply-p1-patches.py` | floor |
| `9e8a5b9` | `reports: M13-RELEASE Leg A-2 — rc verified` | Leg A-2 report | floor |
| `e08274b` | `reports: M13-RELEASE Leg B — v1.6.0 tagged and verified, draft standing` | Leg B report | floor |
| *this commit* | `docs: v1.6.0 published; M13 accepted (owner)` | **Leg C Phase 4** | floor |

`v1.6.0` -> `d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e` (`release: v1.6.0`) —
the *same* commit the rc verified, tagged in Leg B and never moved (C6).

`033d20a` is the one red CI run in the M13 chain; it was diagnosed and closed by
`3687e60` in the same milestone and is already end-gated in
`reports/m13-endgate.md`. It is recorded here for completeness of the ledger,
not as an open item.

### 3.2 Tags and releases, final state

```
$ git tag --list
v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0 v1.6.0

$ git ls-remote --tags origin refs/tags/v1.6.0
d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e  refs/tags/v1.6.0

$ git ls-remote --tags origin | grep -i rc
(no rc tags on remote)

$ gh release list --limit 8
QuotaPane v1.6.0   Latest   v1.6.0   2026-08-06T00:42:23Z
QuotaPane v1.5.0            v1.5.0   2026-08-04T21:25:12Z
QuotaPane v1.4.1            v1.4.1   2026-08-01T21:10:03Z
QuotaPane v1.4.0            v1.4.0   2026-07-31T15:24:09Z
QuotaPane v1.3.0            v1.3.0   2026-07-30T04:25:00Z
QuotaPane v1.2.0            v1.2.0   2026-07-29T15:35:49Z
QuotaPane v1.1.0            v1.1.0   2026-07-29T03:46:40Z
QuotaPane v1.0.0            v1.0.0   2026-07-29T00:40:33Z
```

No `v1.6.0-rc.1` tag and no rc draft survive, locally or on the remote — pruned
in Leg B §6 only after that leg's `RESULT: PASS`. Eight tags, one per release,
eight releases, one Latest.

### 3.3 Verification runs — the whole chain

| Run | Tag | Verdict | Recorded in |
|---|---|---|---|
| Leg A Phase 2 | `v1.6.0-rc.1` | `RESULT: PASS — six steps, six controls, R1-R4` (21 PASS / 0 FAIL, exit 0). One spot-check needle (`24h`) did not match and was diagnosed as an unsatisfiable assertion, not a defect (§6.1). | `reports/m13-release-rc.md` §3–§5 |
| Leg B Phase 3 | `v1.6.0` | `RESULT: PASS — v1.6.0: six steps, six controls, R1-R4` (21 PASS / 0 FAIL, exit 0), **run twice** — before the prune and again after, byte-identical output | `reports/m13-release-draft.md` §4, §7 |

`tools/release-verify.sh` was run verbatim each time and was **not edited by
any session in M13** (its last change is M12's top-tier `9ced94c`). No run was
repeated in modified form.

### 3.4 Published artifacts — re-measured this leg, post-publication

The four published assets were downloaded fresh into a scratch `mktemp -d`
outside the repository and hashed. This is a **post-publication** measurement,
independent of Leg B's:

```
998377798e048bc2b271dad5a5ea05e8857478b531a684a0c6e7e615c2614a52  quotapane-v1.6.0-x86_64-pc-windows-msvc.zip
8e5f39b5b8b0b524ff8ae8dc96e7ac3056355634ad577c0a9dc3d893d28d467e  quotapane-v1.6.0-x86_64-unknown-linux-gnu.tar.gz
a0e2b87dc42e92b6b92e5b68b3292d8c949b15b5f20ae0e86135c0b9a4885b43  SHA256SUMS
0110a204eebf903eac58e6aea229d49dde7904ac4b5fcc8b0a5ae1e1e159d795  SHA256SUMS.sigstore.json

$ sha256sum -c SHA256SUMS
quotapane-v1.6.0-x86_64-pc-windows-msvc.zip: OK
quotapane-v1.6.0-x86_64-unknown-linux-gnu.tar.gz: OK
exit=0
```

**All four digests are identical to Leg B §5's**, and identical to the
server-side digests the API reports for the published assets:

```
$ gh api repos/cipherpine/quotapane/releases/tags/v1.6.0 -q '.assets[] | "\(.digest)  \(.name)  \(.size)  \(.state)"'
sha256:998377798e048bc2b271dad5a5ea05e8857478b531a684a0c6e7e615c2614a52  quotapane-v1.6.0-x86_64-pc-windows-msvc.zip       4801041  uploaded
sha256:8e5f39b5b8b0b524ff8ae8dc96e7ac3056355634ad577c0a9dc3d893d28d467e  quotapane-v1.6.0-x86_64-unknown-linux-gnu.tar.gz  5991257  uploaded
sha256:a0e2b87dc42e92b6b92e5b68b3292d8c949b15b5f20ae0e86135c0b9a4885b43  SHA256SUMS                                            225  uploaded
sha256:0110a204eebf903eac58e6aea229d49dde7904ac4b5fcc8b0a5ae1e1e159d795  SHA256SUMS.sigstore.json                            10239  uploaded
```

Three independent measurements now agree across the draft→published boundary:
Leg B's pre-publication download, Leg B's post-prune server-side read, and this
leg's post-publication download. **Publishing changed no bytes** — what the
owner published is exactly what Leg B verified against cosign and the
provenance attestations, sizes included.

The scratch directory was removed; nothing was copied into the tree. Only
`gh release download` and `sha256sum` were run — **no binary from the archives
was executed this leg**, and no credential file was touched.

### 3.5 Release pipeline integrity

`release.yml` is the same blob at every tag the pipeline has built since v1.4.1:

```
v1.4.1  44860bab2a3d626010e617cbae007656ccae715e
v1.5.0  44860bab2a3d626010e617cbae007656ccae715e
v1.6.0  44860bab2a3d626010e617cbae007656ccae715e
```

The release workflow was not touched anywhere in M13.

### 3.6 The published release body

The body reconciles with the CHANGELOG — and this time **byte-identically**,
which M12 Leg C §3.6 could only claim after whitespace normalisation:

```
CHANGELOG.md `## [1.6.0] - 2026-08-05` entry body : 36 lines
published release body, up to the `---` footer    : 36 lines
BYTE-IDENTICAL                                    : True
```

The footer is exactly the text Leg B §10 prescribed, with the retargeted
compare link `https://github.com/cipherpine/quotapane/compare/v1.5.0...v1.6.0`.

Two facts about the body, offered as facts and not as changes (the body is
owner-territory metadata; this session did not edit the release):

- The `**Full Changelog**` line appears **twice**, identically, as the last two
  lines (body lines 44 and 45). Both carry the correct `v1.5.0...v1.6.0`
  compare link, so the duplication is cosmetic. The queue file's addendum
  flagged this and explicitly placed it outside this leg's scope.
- Carried forward unchanged since v1.0.0: the footer says *"Verifying a
  release"* while the README's actual heading is `## Verify a release`.
  `README.md` was not touched by this leg either.

---

## 4. Version and content on disk

| Fact | Value |
|---|---|
| `Cargo.toml:10` | `version = "1.6.0"` |
| Tests | **386 passed, 0 failed, 0 ignored** (64 + 13 + 127 + 182 + 0 doc-tests) — matches the spec's P1 count, unchanged since the Phase-1 commit |
| Invariant checker | `OK: 7 invariants, 24 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |
| Shipped GUI content (Leg A-2 §5) | both binaries carry `alert: `, `ALERT — `, `refilled: `, `history.jsonl`, `config.cfg`, `alert_mode`; the `24h` tag ships as immediate stores, not a rodata string (§6.1) |
| Shipped README (Leg A-2) | carries "Theming and preferences" |

---

## 5. §3 verification bar for this push

Run locally before the commit, though the commit is documentation-only —
DECISIONS §3 requires it for *every* push:

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | **386 passed, 0 failed, 0 ignored** (64 + 13 + 127 + 182 + 0) |
| `python3 tools/check-invariants.py` | `OK: 7 invariants, 24 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |

### CI for this leg's commits

The stamp commit's own run —
<https://github.com/cipherpine/quotapane/actions/runs/31061029903> on
`a9b4bdc` — is **success**, watched to completion in the foreground
(`gh run watch --exit-status --interval 20` → exit 0). All 8 required checks,
read back from the check-runs API (`total_count: 8`):

```
success  build & test (macos-latest)
success  build & test (ubuntu-latest)
success  build & test (windows-latest)
success  cargo-audit (RustSec advisories)
success  cargo-deny (licenses, bans, advisories, sources)
success  gitleaks — full-history secret scan
success  invariant 4 — no telemetry
success  invariants — manifest, docs, and tests agree
```

Recorded here in the follow-up commit, per the M13 pattern established by
`aaa4b03`. That follow-up commit is the pattern's terminus — it triggers a run
of its own, which cannot be written into the file it would document; its result
is reported in this session's handover instead.

---

## 6. Open items carried to the top tier

### 6.1 The Phase-2 `24h` spot-check clause — still open, still not this leg's

Leg A-2 §5 proved that the spec's Phase-2 content spot-check asks for a `24h`
needle that **no correctly-built LTO release binary can carry**: at
`lto = true, codegen-units = 1, strip = true` the three-byte literal is
materialized as immediate stores (`mov word [rax], 0x3432` / `mov byte [rax+2],
0x68`) rather than kept as a rodata string, and the instruction bytes are
byte-identical between a local release build of the same commit and the shipped
artifact. The sparkline tag ships. Leg B §8 recorded that Leg B's gate does not
depend on the clause.

**Leg C's spec text contains no content spot-check either**, so this leg did
not depend on it. It remains an open **wording** decision for the top tier,
with nothing in the artifact known to be wrong. Carried here as the M13-RELEASE
end-gate rather than closed, exactly as Leg B asked.

### 6.2 Two backlog phrases in `DECISIONS.md` that v1.6.0 has overtaken

Observed while verifying the patch; **not changed**, because §4a.3 forbids this
session originating or modifying protected-path bytes and the spec authorised
exactly one substitution:

1. §2's `Post-1.0 backlog:` still reads *"deferred M5 features
   (history/sparklines, forecast, thresholds/alerts, OtelSource, CLI
   User-Agent parity)"*. Of those, history/sparklines and thresholds/alerts
   shipped in **v1.6.0** and forecast-to-limit shipped in **v1.3.0** (M8).
   Only `OtelSource` and CLI User-Agent parity are still deferred.
2. §2's M5 entry still reads *"Deferred to a post-1.0 milestone:
   history/sparklines, forecast-to-limit, thresholds/alerts, the token-free
   `OtelSource`, and the expanded-window bottom polish."* — same overtaking.

Neither is a release defect and neither blocks anything; both are doc-truth
drift that a top-tier amendment could clear in one line each, in whatever leg
the top tier chooses. Flagged rather than fixed.

---

## 7. State left behind

- `main` = `e08274b` + this leg's commits. **This leg's entire tree footprint
  is `DECISIONS.md` (the one §4a replacement) and this report.**
- No commit authored by anyone else was carried by this push — `HEAD` was
  `origin/main` = `e08274b` when the session started (unlike M12 Leg C, which
  had to carry an owner commit).
- Tag `v1.6.0` -> `d86cb5a`, published. `v1.5.0` is no longer Latest.
- No dependency added, removed, or pinned. No change from this session to code,
  `.github/`, `.claude/`, `.cargo/`, `assets/`, `README.md`, `CHANGELOG.md`,
  `Cargo.*`, `tools/`, `prompts/`, `SECURITY.md`, `THREAT_MODEL.md`, or
  `deny.toml`.
- Nothing published, tagged, pruned, or deleted by this session. Publication
  was the owner's act, at 2026-08-06T00:42:23Z. The release object itself was
  not edited — including the duplicated `Full Changelog` line (§3.6).
- The extraction/apply script lives outside the repository at
  `C:\tmp\m13c\apply.py` and was never staged; per the Leg A ruling, one-shot
  tools do not accumulate in the tree. This report is the audit trail.
- No credential file was read; no token material was handled, printed, or
  logged.
- **Dispatcher rules observed.** Every CI wait was a blocking foreground
  `gh run watch --exit-status --interval 20` — nothing was backgrounded,
  because a headless session's background tasks die with it. `.git` was checked
  for stale lock files after **every** git operation, including between the two
  commits and after both pushes; **none were ever produced**, so nothing needed
  sweeping into `_to_delete/git-stale/` (which retains 280 files from earlier
  sessions, untouched by this one). Stated this way deliberately: the queue
  file asked for a sweep, and the honest report is that the condition it guards
  against never arose — not that a sweep was performed.

---

## 8. §4 conditions hit, and deviations

- **§4.1 / §4a — `DECISIONS.md`.** Hit by design; the spec's sole authorised
  exception. Discharged as a verify-and-commit under §4a with the evidence in
  §2, including a reverse reconstruction of the pre-patch blob. Nothing in
  those paths was authored.
- **§4.7 — none blocking.** §6.2's stale backlog phrases are drift inside an
  already-protected file, not a precondition mismatch: every P1/P2 precondition
  the spec states held (tip `e08274b`, tree clean, tags `v1.0.0`–`v1.6.0`,
  version `1.6.0`, 386 tests, no M13 stamp before this commit). They are
  reported, not worked around.
- No other deviation. The spec's DO-NOT list was honoured in full: nothing
  published, no code / `.github/` / `assets/` / `README.md` / §4.1 path touched
  beyond the authorised `DECISIONS.md` replacement, no dependency added, no leg
  boundary crossed.

---

## 9. What happens next

**Nothing is queued.** M13-RELEASE is complete across all four phases:

1. M13 build (P1–P4) + M13-R1 — owner-accepted 2026-08-05 across two §4.5
   rounds, `reports/m13-endgate.md` and `reports/m13-r1-endgate.md`.
2. Leg A / A-2 — v1.6.0 cut, rc tagged and verified,
   `reports/m13-release-rc.md`.
3. Leg B — `v1.6.0` tagged, verified twice, rc pruned, draft raised,
   `reports/m13-release-draft.md`.
4. Leg C — published by the owner, stamped in `DECISIONS.md`, this report.

For the owner / top tier, in descending order of importance:

1. **The owner's production re-look at sparklines** — recorded in the M13
   stamp itself; the spec states nothing gates on it.
2. **The `24h` spot-check wording** (§6.1) — a top-tier decision on the spec
   template, with the artifact proven sound either way.
3. **The two overtaken backlog phrases** (§6.2) — a one-line top-tier
   amendment each, whenever convenient.
4. Optional, previously deferred: the duplicated `Full Changelog` line in the
   v1.6.0 release body (§3.6), and the release-body *"Verifying a release"* vs
   README *"Verify a release"* wording, untouched since v1.0.0.
5. Post-1.0 backlog after this release: packaging (WinGet/Homebrew/AUR),
   `OtelSource`, CLI User-Agent parity, dead `RateLimitHeaders` cleanup,
   dormant-cadence decision.

This session ends here, as instructed — the hard stop is the session end.
