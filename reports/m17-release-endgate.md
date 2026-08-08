# M17-RELEASE Leg C — v1.7.0 published; M14–M16 stamped and closed

**Session:** attended CLI session (Opus 5 — top tier per `CLAUDE.md`'s routing
table), 2026-08-08T05:0xZ. The dispatcher remains paused at the owner's
request; this leg arrived as a hand-carried paste, as Legs A and B did.
**Spec:** `prompts/m17-release.md`, **LEG C only — Phase 4.**
**Tree footprint of this leg:** `DECISIONS.md` (the spec's single §4a
replacement) and this report. Nothing else.
**Host toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0
(c980f4866 2026-06-30), Python 3.14.4, gh 2.92.0 (2026-04-28), cosign v3.1.2,
host MINGW64_NT-10.0-26200.

> **Verdict: the release is closed.** `v1.7.0` is published —
> `publishedAt: 2026-08-08T05:00:20Z`, `isDraft: false`, `isPrerelease: false`,
> the URL has moved from `…/untagged-e70d36974426e62566d8` to
> `…/releases/tag/v1.7.0`, and `releases/latest` now resolves to `v1.7.0`.
> The published bytes were re-measured independently this leg — downloaded
> fresh, checksum-verified, cosign-verified and provenance-verified **after**
> publication — and are byte-identical to the four digests Leg B verified
> against the draft. Publishing changed metadata, not artifacts.
> `DECISIONS.md` carries the M14–M16 stamp, applied byte-for-byte from the
> spec's own bytes under §4a and proved in both directions. Nothing was
> published, tagged, pruned, edited, re-run or authored by this session.

---

## 1. The publish gate (Leg C precondition — STOP if unmet)

Checked before touching anything, exactly as the Leg C paste requires.

```
$ gh release view v1.7.0 --json tagName,isDraft,publishedAt,url
{"isDraft":false,"publishedAt":"2026-08-08T05:00:20Z","tagName":"v1.7.0",
 "url":"https://github.com/cipherpine/quotapane/releases/tag/v1.7.0"}

$ gh api repos/cipherpine/quotapane/releases/latest -q .tag_name
v1.7.0
```

| Gate | Required | Observed | |
|---|---|---|---|
| C1 | `publishedAt` non-null | `2026-08-08T05:00:20Z` | ✅ |
| C2 | Not a draft | `isDraft: false` | ✅ |
| C3 | Not a prerelease | `isPrerelease: false` | ✅ |
| C4 | URL is the tag URL, not `untagged-…` | `…/releases/tag/v1.7.0` (Leg B §5 recorded `…/untagged-e70d36974426e62566d8`) | ✅ |
| C5 | `v1.7.0` is Latest | `gh release list`: `QuotaPane v1.7.0  Latest  v1.7.0  2026-08-08T05:00:20Z`; `releases/latest` → `v1.7.0` | ✅ |
| C6 | Tag unmoved since Leg B | `v1.7.0` -> `1f73888aa1e03a4df54b33c20e3eb14d48449eaa`, local and remote, `git cat-file -t` → `commit` | ✅ |

The gate is met by this session's own `gh` reads. Publication was the owner's
act; this session did not publish, and would not have.

Proceeded to the §4a patch.

---

## 2. The §4a replacement — `DECISIONS.md` only

### 2.1 Extraction — programmatic, from the spec's bytes

`OLD`/`NEW` were **never retyped.** A one-shot script outside the repository
(per the Leg A ruling that one-shot tools do not accumulate in the tree)
located the single `### Patch A -> DECISIONS.md` header, then the single
fenced block following the `OLD:` line and the single fenced block following
the `NEW:` line, and hashed everything it touched:

```
spec  m17-release.md            11549 bytes  sha256 d520535ad081fb6dbda6ac40ec7dca8eb6aaf7a789f0931ecc0c240a440a52ac
dec   DECISIONS.md              15311 bytes  sha256 fb637c895582c8547e5f3e2d51a3edb0048d8d40055f2757d317a7b41022e007

OLD    216 bytes  sha256 026830e2e00c9880d213740b70ba9606b4ef24260780ce59ae051f6234e46ead
NEW   2456 bytes  sha256 567523e0fda555ecc452a4d1bda7437e66fb7c00947c0fb20177ea91638ae42e
```

**The lengths match the spec's top-tier pre-flight exactly: OLD 216 bytes, NEW
2456 bytes.** The script asserts, and exits non-zero on, any of: more or fewer
than one `### Patch A` header; more or fewer than one `OLD:`/`NEW:` line; a
missing opening fence; either string empty; the two equal; a `CR` byte in
either; a newline inside either; `NEW` not beginning with `OLD`; or either
length differing from the pre-flighted value.

The pre-patch `DECISIONS.md` sha256 (`fb637c89…`) is the same value
`reports/m13-release-endgate.md` §2.2 recorded as *its* post-patch hash — the
file has not been touched by anything between v1.6.0's close and this leg.

### 2.2 Uniqueness — before and after

```
before: DECISIONS.md contains OLD x1, NEW x0
after : DECISIONS.md contains NEW x1, OLD x1   (OLD x1 is inside NEW, by design)

splice at byte offset 9603; delta +2240 bytes  (= -len(OLD) 216 + len(NEW) 2456)
result 17551 bytes  sha256 7fedfda8ba93719357347ba53ccb462e604c2cd70270a3f69f94375187a7db6a
newline count unchanged: 65
```

`OLD x1` before is the unique anchor; `NEW x0` before confirms the stamp was
not already applied. **The `OLD x1` remaining after is the expected result,
not a failed patch** — the spec states it explicitly, because `NEW` *begins
with* `OLD`: the stamp is appended to the end of the ledger paragraph rather
than replacing it. The two assertions that matter are OLD ×1 **before** and
NEW ×1 **after**, and both hold.

Both files are LF-only with no `CR` byte anywhere (asserted on the extracted
strings, on the `HEAD` blob, and on the written file), and the
read/replace/write ran entirely in **binary**, so no line-ending or encoding
normalisation could occur. Beyond the counts, the script asserts the splice is
surgical: the bytes before offset 9603 are unchanged, the bytes at the splice
are exactly `NEW`, and the bytes after are the original tail.

### 2.3 Verified in both directions, independently of the applying script

A separate check compared the working tree against the committed blob rather
than re-running the same code path:

```
HEAD blob sha256                     : fb637c895582c8547e5f3e2d51a3edb0048d8d40055f2757d317a7b41022e007
disk      sha256                     : 7fedfda8ba93719357347ba53ccb462e604c2cd70270a3f69f94375187a7db6a
forward  HEAD.replace(OLD,NEW) == disk : True
reverse  disk.replace(NEW,OLD) == HEAD : True
NEW on disk exactly once             : True
OLD on disk exactly once (inside NEW) : True
no CR bytes in disk / HEAD           : True / True
newlines HEAD / disk                 : 65 / 65
bytes    HEAD / disk                 : 15311 / 17551
'Post-1.0 backlog:' occurrences      : 1
```

The **reverse** direction is the strong claim: undoing the substitution on the
on-disk file reproduces `git show HEAD:DECISIONS.md` byte-for-byte. That is
only possible if the sole difference between HEAD and the working tree is the
spec's `OLD` → `NEW` substitution — no whitespace touch-up, no stray
character, nothing else.

`Post-1.0 backlog:` still occurs exactly once, immediately before the new
sentence — the anchor was consumed and re-emitted by `NEW`, not duplicated.

### 2.4 Diff scope — one file, one line

```
$ git status --porcelain
 M DECISIONS.md

$ git diff --numstat
1	1	DECISIONS.md
```

One file, one insertion, one deletion — §2's roadmap line, which is a single
very long line, replaced by itself plus the M14–M16 sentence. **No other
protected path changed.** `git diff --name-only HEAD` returns `DECISIONS.md`
and nothing else: no `crates/usage-core/src/egress/**`, no
`crates/usage-core/src/credentials/**`, no `deny.toml`, `SECURITY.md`,
`THREAT_MODEL.md`, `.github/**`, `.cargo/**`, or `.claude/**`.

### 2.5 §4a checklist

| §4a requirement | How it was met |
|---|---|
| 1. Pre-authored at the top tier | The `NEW` bytes are the spec's own Patch A block, authored at the top tier in `e333a8f` (`prompts: the v1.7.0 release spec — M14+M15+M16`) and supplied verbatim; this session is itself top tier (Opus 5) |
| 2. Verified byte-for-byte before committing | §2.1–2.4: SHA-256 of every operand, the spec's pre-flighted lengths re-asserted, exact length arithmetic, count-before/count-after, byte-range splice assertions, an independent **reverse** reconstruction of the HEAD blob, and a single-file one-line diff |
| 3. Authors nothing itself | Zero characters were typed into `DECISIONS.md` by this session; the change is a binary substitution of bytes extracted from the spec. No fix-ups, no reformatting, no opportunistic edits — including none to the stale backlog phrases flagged in §7.2, which this session may observe but not touch |

---

## 3. The M17-RELEASE arc, end to end

### 3.1 The release ledger

Base `a9b4bdc` *docs: v1.6.0 published; M13 accepted (owner)* — M13's Leg C.

| SHA | Subject | Leg / phase | Tier |
|---|---|---|---|
| `e77e5d0` | `prompts: M14 density-pass spec + launcher — resizable height, freshness dot` | M14 spec | top tier |
| `db5db97` | `prompts: M15 agents-pane spec + launcher — local session visibility` | M15 spec | top tier |
| `618b0b9` | `M14: a height of your own — resizable window, grip, snap-to-fit` | M14 build | floor |
| `25415b8` | `M14: the freshness dot — a row back from the footer` | M14 build | floor |
| `921138a` | `reports: M14 end-gate — density pass shipped, three deviations flagged` | M14 report | floor |
| `9947e86` | `M15: who is working right now — the agents scanner, metadata only` | M15 build | floor |
| `f7adb72` | `M15: usage // agents — a second view in the same 320px pane` | M15 build | floor |
| `b6fac53` | `reports: M15 end-gate — the agents pane shipped, seven deviations flagged` | M15 report | floor |
| `6c87626` | `reports: M15 end-gate addendum — the tip's ubuntu job is stalled on GitHub` | M15 addendum | floor |
| `5dc246b` | `M16 spec: the agents pane, second pass — turn state, pulse, and a two-hour pane` | M16 spec | top tier |
| `eb5ae3e` | `M16: whose move is it — turn state, pulse, and a fence for the allowlist` | M16 Phase 1 | floor |
| `93c1e70` | `reports: M16 end-gate — phase 1 shipped, phase 2 blocked on the Actions outage` | M16 report | floor |
| `b4ed4c7` | `M16: the pane opens on what is alive` | M16 Phase 2 | floor |
| `5aff938` | `reports: M16b end-gate — phase 2 shipped, CI green for phase 1 too` | M16b report | floor |
| `83cc5d6` | `prompts: the M16 Phase 2 goal prompt, on the record` | record | top tier |
| `e333a8f` | `prompts: the v1.7.0 release spec — M14+M15+M16` | release spec | top tier |
| `4169d84` | `release: v1.7.0` | **Leg A Phase 1** — version 1.6.0→1.7.0, `Cargo.lock` (3 workspace members only), CHANGELOG entry | Opus 5 |
| `09e8eb4` | `reports: M17-RELEASE Leg A — rc verified` | Leg A report | Opus 5 |
| `1f73888` | `changelog: v1.7.0 carries the no-new-deps line the last three releases carry` | **Leg B ruling (1)** — **the tagged commit** | Opus |
| `1718626` | `reports: M17-RELEASE Leg B — v1.7.0 tagged and verified, draft standing` | Leg B report | Opus |
| *this commit* | `docs: v1.7.0 published; M14-M16 accepted (owner)` | **Leg C Phase 4** | Opus 5 |

M14, M15 and M16 have their own end-gates in `reports/` (`m14-endgate.md`,
`m15-endgate.md`, `m16-endgate.md`, `m16b-endgate.md`); this report covers the
**release** arc and does not restate them.

### 3.2 The three legs

| Leg | Spec phases | Ended at | Report |
|---|---|---|---|
| **A** | P1–P4 preconditions, Phase 1, Phase 2 | The mandatory HARD STOP after the rc verified; no `v1.7.0` tag, nothing published | `reports/m17-release-rc.md` |
| **B** | Phase 3, on the paste that **is** the go-ahead | `v1.7.0` tagged and verified, rc pruned, draft standing unpublished; handed back the draft URL | `reports/m17-release-draft.md` |
| **C** | Phase 4, after the owner confirmed publication | This commit — end-gate report + the §4a stamp | this file |

Both hard stops were honoured. Neither leg crossed into the next; each ended
at the boundary the spec drew, and Leg B did not begin without the paste.

**The foreground rule held in all three legs.** Every CI and Release wait was a
blocking `gh run watch <id> --exit-status` (exit 0) or a foreground poll — never
a background watcher, never a notification, never poll-and-forget.

### 3.3 Tags and releases, final state

```
$ git tag --list
v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0 v1.6.0 v1.7.0

$ git ls-remote --tags origin refs/tags/v1.7.0
1f73888aa1e03a4df54b33c20e3eb14d48449eaa  refs/tags/v1.7.0

$ git ls-remote --tags origin | grep -ci rc
0

$ gh release list --limit 9
QuotaPane v1.7.0   Latest   v1.7.0   2026-08-08T05:00:20Z
QuotaPane v1.6.0            v1.6.0   2026-08-06T00:42:23Z
QuotaPane v1.5.0            v1.5.0   2026-08-04T21:25:12Z
QuotaPane v1.4.1            v1.4.1   2026-08-01T21:10:03Z
QuotaPane v1.4.0            v1.4.0   2026-07-31T15:24:09Z
QuotaPane v1.3.0            v1.3.0   2026-07-30T04:25:00Z
QuotaPane v1.2.0            v1.2.0   2026-07-29T15:35:49Z
QuotaPane v1.1.0            v1.1.0   2026-07-29T03:46:40Z
QuotaPane v1.0.0            v1.0.0   2026-07-29T00:40:33Z
```

Nine tags, nine releases, one-to-one, one Latest. No `v1.7.0-rc.1` tag and no
rc draft survive, locally or on the remote — pruned in Leg B §9 only after that
leg's `RESULT: PASS`.

### 3.4 CI and Release runs — the whole M17-RELEASE chain

Every run below was watched to completion in the foreground and read back from
the check-runs API rather than trusted from the watcher.

| Commit | Run | Workflow | Conclusion | Created |
|---|---|---|---|---|
| `4169d84` | [31237987499](https://github.com/cipherpine/quotapane/actions/runs/31237987499) | CI | success (8/8 required) | 03:47:35Z |
| `4169d84` | [31238195378](https://github.com/cipherpine/quotapane/actions/runs/31238195378) | Release (`v1.7.0-rc.1`) | success (3/3) | 03:53:02Z |
| `09e8eb4` | [31238672008](https://github.com/cipherpine/quotapane/actions/runs/31238672008) | CI | success (8/8) | 04:05:35Z |
| `1f73888` | [31239179139](https://github.com/cipherpine/quotapane/actions/runs/31239179139) | CI | success (8/8) | 04:19:22Z |
| `1f73888` | [31239319812](https://github.com/cipherpine/quotapane/actions/runs/31239319812) | Release (`v1.7.0`) | success (3/3) | 04:23:08Z |
| `1718626` | [31239755258](https://github.com/cipherpine/quotapane/actions/runs/31239755258) | CI | success (8/8) | 04:34:23Z |

Re-read this leg, per commit, from `/commits/<sha>/check-runs`:

```
4169d84  total=11  all success   (8 CI + 3 rc Release)
09e8eb4  total=8   all success
1f73888  total=11  all success   (8 CI + 3 v1.7.0 Release)
1718626  total=8   all success
```

**No red run anywhere in the M17-RELEASE chain.** The eight required checks are
`build & test (windows-latest / ubuntu-latest / macos-latest)`, `cargo-deny`,
`cargo-audit`, `gitleaks`, `invariant 4 — no telemetry`, and `invariants —
manifest, docs, and tests agree`.

### 3.5 Verification runs — six steps, six controls, twice at the final tag

| Run | Tag | Verdict | Recorded in |
|---|---|---|---|
| Leg A Phase 2 | `v1.7.0-rc.1` | `RESULT: PASS — v1.7.0-rc.1: six steps, six controls, R1-R4` (21 PASS / 0 FAIL, exit 0) | `reports/m17-release-rc.md` §5 |
| Leg B Phase 3, pre-prune | `v1.7.0` | `RESULT: PASS — v1.7.0: six steps, six controls, R1-R4` (21 PASS / 0 FAIL, exit 0) | `reports/m17-release-draft.md` §6 |
| Leg B Phase 3, post-prune | `v1.7.0` | byte-identical output, `diff` empty across all 22 lines | `reports/m17-release-draft.md` §9 |

`tools/release-verify.sh` was run **verbatim** each time and was not edited by
any session in M17. No run was repeated in modified form, and no leg
improvised the manual standard.

### 3.6 Published artifacts — re-measured this leg, post-publication

The four published assets were downloaded fresh into a scratch directory
outside the repository and hashed. This is a **post-publication** measurement,
independent of Leg B's:

```
5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f  quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
cf4b22ada89803dfbfc91ee2aa594cd242cca386c5189edf438f648dc12cf7a4  quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz
2f169f6d87e2e8787484896639634e2ed5e5337d5aa6c3cbca55065fb85c31bf  SHA256SUMS
392934f6920563307c23950119062673022c1e46a6f9d7b0d9aafc0a424881d6  SHA256SUMS.sigstore.json

$ sha256sum -c SHA256SUMS
quotapane-v1.7.0-x86_64-pc-windows-msvc.zip: OK
quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz: OK
exit=0
```

**All four digests are identical to Leg B §7's**, and identical to the
server-side digests the API reports for the published assets:

```
$ gh api repos/cipherpine/quotapane/releases/tags/v1.7.0 -q '.assets[] | "\(.digest)  \(.name)  \(.size)  \(.state)"'
sha256:5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f  quotapane-v1.7.0-x86_64-pc-windows-msvc.zip       4846430  uploaded
sha256:cf4b22ada89803dfbfc91ee2aa594cd242cca386c5189edf438f648dc12cf7a4  quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz  6033761  uploaded
sha256:2f169f6d87e2e8787484896639634e2ed5e5337d5aa6c3cbca55065fb85c31bf  SHA256SUMS                                            225  uploaded
sha256:392934f6920563307c23950119062673022c1e46a6f9d7b0d9aafc0a424881d6  SHA256SUMS.sigstore.json                            10403  uploaded
```

Signature and provenance were re-checked against the **published** bytes, not
merely the digests:

```
$ cosign verify-blob SHA256SUMS --bundle SHA256SUMS.sigstore.json \
    --certificate-identity-regexp '^https://github[.]com/cipherpine/quotapane/[.]github/workflows/release[.]yml@refs/tags/' \
    --certificate-oidc-issuer 'https://token.actions.githubusercontent.com'
Verified OK                                                        (exit 0)

$ gh attestation verify <each archive> -R cipherpine/quotapane     (exit 0, both)
```

The provenance is a **single SLSA v1 attestation carrying both archives as
subjects**, and both subject digests match the bytes on disk — the R1 check,
re-run after publication:

```
predicateType : https://slsa.dev/provenance/v1
SAN           : https://github.com/cipherpine/quotapane/.github/workflows/release.yml@refs/tags/v1.7.0
issuer        : https://token.actions.githubusercontent.com
srcRepo / ref : https://github.com/cipherpine/quotapane  refs/tags/v1.7.0
subjects      : 2
  quotapane-v1.7.0-x86_64-pc-windows-msvc.zip        attested == on disk   MATCH
  quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz   attested == on disk   MATCH
```

*(Worth recording for the next leg that reads this JSON: because one attestation
carries two subjects, a reader that prints only `subject[0]` will report the
Windows archive no matter which file it verified. That is a property of the
reader, not of the attestation; the check above enumerates all subjects.)*

In-archive `TOOLCHAIN.txt`, read out of the published Linux archive and
matching Leg A's and Leg B's:

```
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
```

Four independent measurements now agree across the draft→published boundary:
Leg A's rc-time digests for the pipeline, Leg B's pre-publication download,
Leg B's post-prune server-side read, and this leg's post-publication download
plus fresh cosign and provenance verification. **Publishing changed no bytes.**

The scratch directory lives outside the repository; nothing was copied into
the tree. Only `gh release download`, `sha256sum`, `cosign verify-blob`,
`gh attestation verify` and a `tar -O` read of `TOOLCHAIN.txt` were run —
**no binary from the archives was executed this leg**, and no credential file
was touched.

### 3.7 Release pipeline integrity

`release.yml` is the same blob at every tag the pipeline has built since
v1.4.1, and at `HEAD`:

```
v1.4.1  44860bab2a3d626010e617cbae007656ccae715e
v1.5.0  44860bab2a3d626010e617cbae007656ccae715e
v1.6.0  44860bab2a3d626010e617cbae007656ccae715e
v1.7.0  44860bab2a3d626010e617cbae007656ccae715e
HEAD    44860bab2a3d626010e617cbae007656ccae715e
```

The release workflow was not touched anywhere in M14–M17.

### 3.8 The published release body — recorded as fact, not as a change

The body is owner territory; this session did not edit the release. Two reads
were taken during this leg, both between publication (05:00:20Z) and
05:08:24Z, and they disagreed — so both are on the record:

- **First read:** the body was **84 bytes** — GitHub's auto-generated
  `**Full Changelog**: …/compare/v1.6.0...v1.7.0` line and nothing else. No
  CHANGELOG entry, no verify footer.
- **Re-read minutes later:** **2934 bytes**, and the owner had pasted the body
  Leg B §11 prescribed.

The second state reconciles with the CHANGELOG **byte-identically**:

```
CHANGELOG.md `## [1.7.0] - 2026-08-08` entry body : 49 lines, 2686 bytes
published release body, up to the `---` footer    : 49 lines, 2686 bytes
BYTE-IDENTICAL                                    : True
```

The footer is exactly the text Leg B §11 prescribed, with the retargeted
compare link `https://github.com/cipherpine/quotapane/compare/v1.6.0...v1.7.0`.

Two notes, offered as facts:

- **The duplicated `Full Changelog` line did not recur.** v1.6.0's body carried
  it twice (flagged in `reports/m13-release-endgate.md` §3.6); v1.7.0's carries
  it exactly once. That carried item is closed by observation.
- Carried forward unchanged since v1.0.0: the footer says *"Verifying a
  release"* while the README's actual heading is `## Verify a release`.
  `README.md` was not touched by this leg either.

The body edit changed nothing else: `publishedAt`, `isDraft`, `isPrerelease`,
`tagName`, the tag target and all four asset digests were re-read after it and
are unchanged.

---

## 4. The five items this leg was asked to put on the record

### 4.1 P3's tag enumeration was wrong — and its operative content verified

P3 says *"Tags exactly `v1.3.0`, `v1.4.0`, `v1.4.1`, `v1.5.0`, `v1.6.0`"* —
five. That was never true of this repository. Leg A found **eight** (those five
plus `v1.0.0`, `v1.1.0`, `v1.2.0`); with `v1.7.0` now cut the repo stands at
**nine**. Nothing P3 lists was missing; the three unlisted tags are the three
*oldest*, a clean prefix, each recorded in `DECISIONS.md` §2 as an
owner-accepted publication, each with a published non-draft release, one-to-one
in both directions. `prompts/m13-release.md`'s equivalent line reads "Tags
exactly v1.0.0–v1.5.0" — the full range. The M17 author truncated the list.

Raised with the owner **before any mutation** and ruled an incomplete
enumeration in the spec; an independent read-only audit reached the same verdict
separately (`DIFFERS`, `stop_recommended: false`). It was **not** treated as a
§4.7 conflict, and the reason is that P3's *operative* content — the part
Phases 2 and 3 actually depend on — verified true in every particular:

- `v1.6.0` was Latest at Leg A. ✅
- No M17 stamp existed in `DECISIONS.md`. ✅ (created by this commit)
- The `v1.7.0` namespace was empty — checked in four places: `git tag -l
  'v1.7*'` empty, `git ls-remote --tags origin 'v1.7*'` empty, no v1.7 row in
  `gh release list`, no v1.7 draft. ✅
- Local and remote tag SHAs were byte-identical for all eight. ✅

**Suggested spec correction, carried forward from Leg A:** P3 should read *"the
five most recent tags are `v1.3.0`–`v1.6.0`, atop `v1.0.0`/`v1.1.0`/`v1.2.0`"*
— or, better for the template, name the range rather than enumerate.

### 4.2 The `// hide older` spot-check — a false negative, closed behaviourally

Phase 2's content spot-check asked for four needles in the shipped artifacts.
Leg A found three: `your turn` and `in the loop` in both GUI binaries, and
`agents` five times in the shipped README (byte-identical to the repo's).
`// hide older` was **absent from both binaries**, and absent from the fragment
`hide older` too.

Leg A did not stop — the spec frames the spot-check as evidence, not a gate,
and directs that only all four absent is a signal. It went further and
characterised the miss so the top tier would not have to re-derive it:

- A **clean local `cargo build --release` reproduced it exactly** — absent, with
  the sibling strings present at the same relative offsets. So it is a property
  of `[profile.release]` (`lto = true`, `codegen-units = 1`, `strip = true`),
  not of CI, the tag, or the artifact pipeline.
- The **debug build contains it**, so the string exists in source and survives
  an unoptimised build.
- The spec anticipated "a string const only ever formatted into a larger
  literal". **This const does not match that description** — it is
  `const HIDE_OLDER_LINE: &str = "// hide older";` returned whole via
  `.to_string()`. That mismatch is why Leg A flagged it rather than filing it
  as an expected limitation.
- The UI test `the_pane_opens_on_the_last_two_hours_and_keeps_the_rest_one_click_away`
  asserts the expanded pane paints a line `== HIDE_OLDER_LINE` and passes — but
  in a test build, not the release profile.

**Closed by the owner, behaviourally, in a release build** — the owner clicked
the toggle and confirmed the line reads `// hide older`. That was Leg A's own
first-preference remedy, and it settles the only thing the byte search could not
reach: §4.5 is the owner's alone, and a GUI check was never a session's to make.
Leg B carried it in as closed and left it closed; this leg does the same.

The residue is a **wording** matter, not an artifact matter: the needle the spec
names (`// hide older`, the expanded-state toggle) is not the string the
CHANGELOG describes (`// N older today`, which is `format!`-built and cannot
survive as a contiguous literal by construction). Both are real states of one
widget. Carried to §7.1 as a spec-template item, with nothing in the artifact
known to be wrong.

### 4.3 Leg B's two top-tier rulings — recorded, not re-argued

Both arrived in the Leg B paste, which **is** the Phase-3 go-ahead the spec
requires. Neither was re-litigated by Leg B, and neither is re-argued here.

**Ruling (1) — the CHANGELOG gets its consistency line.** Leg A's deviation D4
observed that every entry from 1.1.0 through 1.6.0 states the CLI JSON
surface's status, and that 1.4.1/1.5.0/1.6.0 each close with a variant of *"No
JSON key changed in this release. Zero new dependencies."* The dictated 1.7.0
entry had neither, and it was about to become the published release body.

*Reasoning as recorded:* the top tier verified both claims before authoring the
edit — `crates/usage-cli/` and `docs/cli-json.md` untouched since v1.6.0, no new
`Serialize` surface in core, and the only `Cargo.toml` delta in the range being
the version bump. The line is therefore true, and `docs/cli-json.md`'s stability
contract promises new keys "are announced here", so readers have six releases of
training to look for it. Executed as a two-line insert in `1f73888`:

```diff
   explicit forbidden list — with the same machine-checked traceability
   as every other claim.
 
+No JSON key changed in this release. Zero new dependencies.
+
 ## [1.6.0] - 2026-08-05
```

**Ruling (2) — `v1.7.0` is tagged on `1f73888`, not on the rc's commit
`4169d84`.** A deliberate deviation from the spec's Phase 3 wording ("tag
`v1.7.0` on the verified commit"), made at the top tier and not by the session
that executed it.

*Reasoning as recorded:* a markdown line that is not compiled changes no
pipeline input, and Phase 3's fresh `release-verify` ran against the real
`v1.7.0` draft built from the amended tree end to end rather than inheriting the
rc's verdict. The bound was re-verified this leg:

```
$ git diff --stat 4169d84 1f73888
 CHANGELOG.md              |   2 +
 reports/m17-release-rc.md | 361 +++++++++++++++++++++++++++++++++++++++++++++
 2 files changed, 363 insertions(+)

$ git diff --name-only 4169d84 1f73888 -- crates/ Cargo.toml Cargo.lock .github/ assets/ README.md tools/ | wc -l
0
```

Two markdown files, and **neither is shipped** — the release archive inventory
is `LICENSE-APACHE`, `LICENSE-MIT`, `README.md`, `TOOLCHAIN.txt`,
`quotapane-cli(.exe)`, `quotapane(.exe)`, with no `CHANGELOG.md` and no
`reports/`. The rc's role (prove the pipeline) is undisturbed; the final tag's
verification (prove *these* artifacts) is what Leg B §6 and §3.6 above are.

### 4.4 `gh release delete --cleanup-tag` also removed the local tag

Leg B pruned the rc only after `RESULT: PASS`, having first confirmed the two
release objects were distinct (rc draft `tagName: v1.7.0-rc.1`; the `v1.7.0`
draft at `…/untagged-e70d36974426e62566d8`, 4 assets — a different object).

```
$ gh release delete v1.7.0-rc.1 --yes --cleanup-tag     # exit 0
$ gh release view v1.7.0-rc.1
release not found
$ git ls-remote --tags origin | grep -c rc
0
```

The follow-up `git tag -d v1.7.0-rc.1` then reported **`error: tag
'v1.7.0-rc.1' not found`** — because `--cleanup-tag` had removed the *local*
tag as well as the remote ref, not only the remote one as the sequence assumed.

**Recorded because it is a tool behaviour worth knowing, not a surprise that
changed the outcome.** The intended end state — no rc tag, local or remote, and
no rc release object — is exactly what obtains, and `git tag -l` lists nine tags
with no rc among them (§3.3). A future spec that scripts the prune should
either drop the follow-up `git tag -d` or tolerate its non-zero exit; treating
that "not found" as a failure would be a false alarm.

### 4.5 The orphaned run `31121996517` — untouched throughout

P4 describes it precisely: an outage-corrupted record on `b6fac53` whose run
header reads `queued` while its attempt 4 reads `completed`/`failure`, which
GitHub's API will neither run nor reap. `ci.yml` declares no `concurrency`
group, so it blocks nothing.

- **Leg A** read it once, read-only, to record its state (`queued` /
  `conclusion: null` / `run_attempt: 4`). Not cancelled, not re-run, not
  treated as a §4.6 red.
- **Leg B** did not touch it at all — not viewed for state, not re-run, not
  cancelled.
- **Leg C** did not touch it at all. It was not queried, not viewed, not
  re-run, not cancelled. The Leg C paste forbids touching it and this session
  did not, including not reading it to confirm it is still there.

It survives as outage debris, exactly as found. It is stamped as such in the
`DECISIONS.md` sentence this commit applies, so it is now on the standing
record and future sessions need not rediscover it.

---

## 5. Version and content on disk

| Fact | Value |
|---|---|
| `Cargo.toml:10` | `version = "1.7.0"` |
| Tests | **473 passed, 0 failed, 0 ignored** (cli 64 + cli-integration 13 + core 156 + ui 240 + 0 doc-tests) — matches the spec's P1 count, unchanged since the Phase-1 commit |
| Invariant checker | `OK: 8 invariants, 30 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |
| Shipped GUI content (Leg A §6) | both binaries carry `your turn` and `in the loop`; `// hide older` is not a rodata string under LTO and was closed behaviourally by the owner (§4.2) |
| Shipped README (Leg A §6) | carries `agents` 5 times, SHA-256 byte-identical to the repo's `README.md` |
| `DECISIONS.md` | 17551 bytes, carries the M14–M16 stamp; `v1.7.0` occurs once |

---

## 6. §3 verification bar for this push

Run locally before the commit, though the commit is documentation-only —
`DECISIONS.md` §3 requires it for *every* push:

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | **473 passed, 0 failed, 0 ignored** (64 + 13 + 156 + 240 + 0) |
| `python tools/check-invariants.py` | `OK: 8 invariants, 30 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |

This leg's CI result is reported in the session handover rather than in this
file: the stamp commit triggers a run of its own, which cannot be written into
the file it would document. M13 closed that loop with a follow-up commit
(`7703b49`); the Leg C paste queues nothing further, so the handover is the
terminus this time.

---

## 7. Open items carried to the top tier

None of these blocks anything. All are flagged rather than fixed — §4a.3
forbids this session originating or modifying protected-path bytes, and the
spec authorised exactly one substitution.

### 7.1 Spec-template items from the M17 release spec

1. **P3's tag enumeration** (§4.1) — enumerate a range, not a list, or the
   error recurs at v1.8.0.
2. **The Phase-2 spot-check needle** (§4.2) — `// hide older` cannot be
   guaranteed as a rodata string under `lto = true`, and the CHANGELOG
   describes the `format!`-built `// N older today`, which cannot survive as a
   contiguous literal at all. This is the *third* release whose spot-check
   produced a needle the release profile can legitimately fold (M13's was
   `24h`, closed the same way in `reports/m13-release-endgate.md` §6.1). The
   template should say so, or choose needles that are provably rodata.
3. **The spec never states the session's model tier.** `DECISIONS.md` §6 says
   in bold that every goal prompt does, and `CLAUDE.md`'s handoff format
   requires "(1) the model to set". Spec line 8 gives a session *mode*
   ("attended CLI session"), not a tier. Leg A flagged this precisely because
   Phase 4 lands bytes in a §4.1 path; all three legs happened to run at the
   top tier, so nothing was out of bounds, but that was not guaranteed by the
   spec.
4. **The `--cleanup-tag` follow-up** (§4.4) — drop the redundant `git tag -d`
   from the prune sequence or tolerate its exit code.
5. **Leg A's D5, now resolved by fact:** the pinned date `2026-08-08` was
   perishable — it would have gone stale had Phase 3 waited past midnight UTC.
   It did not: `v1.7.0` was tagged 04:23Z and published 05:00:20Z on
   2026-08-08, so the CHANGELOG heading and the stamp both match reality.
   Recorded as closed, and as a reason future specs should derive the date
   rather than pin it.

### 7.2 Doc-truth drift inside `DECISIONS.md`, still open

`reports/m13-release-endgate.md` §6.2 flagged two phrases that v1.6.0 had
already overtaken. **Both are still present and this patch carries them
forward unchanged**, because `NEW` begins with `OLD` and re-emits the backlog
sentence verbatim:

1. §2's `Post-1.0 backlog:` still lists *"deferred M5 features
   (history/sparklines, forecast, thresholds/alerts, OtelSource, CLI
   User-Agent parity)"*. Of those, history/sparklines and thresholds/alerts
   shipped in **v1.6.0** and forecast-to-limit in **v1.3.0**. Only
   `OtelSource` and CLI User-Agent parity are still deferred.
2. §2's M5 entry still reads *"Deferred to a post-1.0 milestone:
   history/sparklines, forecast-to-limit, thresholds/alerts, the token-free
   `OtelSource`, and the expanded-window bottom polish."* — same overtaking.

**New this release, and worth a look:** item 2's last clause, *"the
expanded-window bottom polish"*, and the M5 entry's *"the expanded-state bottom
cutoff is accepted, with polish queued post-1.0"*, both describe a window that
could not change height. **M14 gave the window a user-settable height with a
double-click snap-to-fit**, which is plausibly the polish those two phrases were
waiting for. Flagged, not asserted — whether M14 discharges them is a top-tier
call, and it is one line either way.

---

## 8. State left behind

- `main` = `1718626` + this commit. **This leg's entire tree footprint is
  `DECISIONS.md` (the one §4a replacement) and this report.**
- No commit authored by anyone else was carried by this push — `HEAD` was
  `origin/main` = `1718626` when the session started.
- Tag `v1.7.0` -> `1f73888`, published. `v1.6.0` is no longer Latest.
- No dependency added, removed, or pinned. No change from this session to code,
  `.github/`, `.claude/`, `.cargo/`, `assets/`, `README.md`, `CHANGELOG.md`,
  `Cargo.*`, `tools/`, `prompts/`, `SECURITY.md`, `THREAT_MODEL.md`, or
  `deny.toml`.
- Nothing published, tagged, pruned, deleted, re-run or cancelled by this
  session. Publication was the owner's act, at 2026-08-08T05:00:20Z. The
  release object itself was not edited — including its body, which the owner
  completed during this leg (§3.8).
- The orphaned run `31121996517` was **not touched** — not queried, not viewed,
  not re-run, not cancelled (§4.5).
- The extraction/apply script lives outside the repository in this session's
  scratch directory and was never staged; per the Leg A ruling, one-shot tools
  do not accumulate in the tree. This report is the audit trail.
- No credential file was read; no token material was handled, printed, or
  logged. No `~/.claude/**` or `~/.codex/**` path was read.
- No binary from the release archives was executed this leg.
- **Foreground rule observed.** Every CI wait was a blocking foreground
  `gh run watch --exit-status` — nothing backgrounded, no notification, no
  poll-and-forget.
- **Housekeeping.** `.git` was checked for stale lock files at this leg's
  checkpoints — before the patch, before the commit, and after the push: no
  `.git/*.lock`, no `.git/objects/maintenance.lock`, no
  `.git/objects/*/tmp_obj_*` at any of them. Stated accurately: the condition
  `_to_delete/git-stale/` guards against never arose, so nothing was swept —
  not that a sweep was performed. This leg's git operations were overwhelmingly
  read-only (`rev-parse`, `log`, `diff`, `ls-remote`, `show`); the only writes
  were the commit and the push.

---

## 9. §4 conditions hit, and deviations

- **§4.1 / §4a — `DECISIONS.md`.** Hit by design; the spec's sole authorised
  exception. Discharged as a verify-and-commit under §4a with the evidence in
  §2, including a reverse reconstruction of the pre-patch blob. Nothing in
  those paths was authored by this session.
- **§4.5 — untouched.** No visual was reviewed and no screen was captured. The
  one visual fact this arc depended on — the `// hide older` toggle in a
  release build — was the owner's own confirmation (§4.2).
- **§4.7 — none blocking.** Every Leg C precondition the paste states held
  (`publishedAt` non-null, `isDraft: false`). The historical P3 enumeration
  error (§4.1) was ruled by the owner in Leg A and is reported, not worked
  around; §7.2's stale phrases are drift inside an already-protected file, not
  a precondition mismatch.
- **§4.8 — respected.** Acceptance is the owner's. M14, M15 and M16 were
  accepted by the owner on 2026-08-08; publication was the owner's click.
  Nothing here is self-accepted.
- **Deviations from the Leg C paste: none.** One commit, exactly the two files
  it names, the commit message verbatim. The DO NOT list was honoured in full:
  nothing published, no `.github/` touched, the orphaned run untouched, M18 not
  begun.

---

## 10. What happens next

**Nothing is queued.** M17-RELEASE is complete across all four phases:

1. M14 / M15 / M16 build — owner-accepted 2026-08-08, `reports/m14-endgate.md`,
   `m15-endgate.md`, `m16-endgate.md`, `m16b-endgate.md`.
2. Leg A — v1.7.0 cut, rc tagged and verified, `reports/m17-release-rc.md`.
3. Leg B — `v1.7.0` tagged on `1f73888`, verified twice, rc pruned, draft
   raised, `reports/m17-release-draft.md`.
4. Leg C — published by the owner, stamped in `DECISIONS.md`, this report.

For the owner / top tier, in descending order of importance:

1. **M18 has no spec.** It was not begun, and this session did not draft one.
2. **The spec-template items** (§7.1) — five small corrections that would stop
   the same three findings recurring at v1.8.0, the tag enumeration and the
   spot-check needle being the ones that have now cost two releases each.
3. **The two overtaken backlog phrases** (§7.2) — a one-line top-tier amendment
   each, plus the new question of whether M14's resizable height discharges the
   "expanded-window bottom polish" clause.
4. Optional, untouched since v1.0.0: the release-body *"Verifying a release"*
   vs README *"Verify a release"* wording.
5. Post-1.0 backlog after this release: packaging (WinGet/Homebrew/AUR),
   `OtelSource`, CLI User-Agent parity, dead `RateLimitHeaders` cleanup,
   dormant-cadence decision.

This session ends here, as instructed — the hard stop is the session end.
