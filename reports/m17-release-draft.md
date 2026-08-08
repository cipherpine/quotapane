# M17-RELEASE Leg B — v1.7.0 tagged and verified, draft standing

**Session:** attended CLI session (Opus, Claude Code), 2026-08-08T04:19Z–04:40Z.
The dispatcher is paused at the owner's request; this leg arrived as a
hand-carried paste.
**Spec:** `prompts/m17-release.md`, Phase 3 — plus two top-tier rulings carried
in the Leg B paste, which **is** the Phase-3 go-ahead the spec requires.
**Tag:** `v1.7.0` -> `1f73888aa1e03a4df54b33c20e3eb14d48449eaa`
(`changelog: v1.7.0 carries the no-new-deps line the last three releases
carry`) — **not** `4169d84`, by top-tier ruling (2); recorded as a deviation in
§8.
**Host toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0
(c980f4866 2026-06-30), Python 3.14.4, gh 2.92.0 (2026-04-28), cosign v3.1.2,
host MINGW64_NT-10.0-26200.
**Release toolchain (in-archive `TOOLCHAIN.txt`, identical in both archives):**
rustc 1.97.1 (8bab26f4f 2026-07-14), cargo 1.97.1 (c980f4866 2026-06-30).

> **Verdict: PASS.** `tools/release-verify.sh v1.7.0` returned
> `RESULT: PASS — v1.7.0: six steps, six controls, R1-R4`, exit 0,
> 21 PASS / 0 FAIL. Only after that verdict were the rc tag and rc draft
> pruned. The `v1.7.0` draft release stands, **unpublished**
> (`publishedAt: null`); `v1.6.0` remains `Latest`. Nothing was published;
> publishing is the owner's.

**Draft URL (the owner publishes from here):**
<https://github.com/cipherpine/quotapane/releases/tag/untagged-e70d36974426e62566d8>

---

## 1. The two rulings this leg executed

Both arrived in the Leg B paste, both recorded here as instructed. Neither was
re-litigated.

**Ruling (1) — the CHANGELOG consistency line.** The 1.7.0 entry was missing
the line the last three releases all close with. The top tier verified both of
its claims before authoring the edit (`crates/usage-cli/` and `docs/cli-json.md`
untouched since v1.6.0; no new `Serialize` surface in core; the only
`Cargo.toml` delta in the range is the version bump). Executed in §3.

**Ruling (2) — `v1.7.0` is tagged on the amended commit, not on `4169d84`.**
A deliberate deviation from the spec's "tag the verified commit", made at the
top tier and not by this session: a markdown line that is not compiled changes
no pipeline input, and Phase 3's fresh release-verify against the real v1.7.0
draft covers the amended tree end to end. Executed in §4, evidenced in §8.

**Carried in as closed, and left closed.** The `// hide older` spot-check was a
false negative — the owner confirmed the toggle behaviourally in a release
build. P3's five-tag enumeration is incomplete (nine tags exist); its operative
content — the `v1.7.0` namespace is empty — was verified true. The orphaned run
`31121996517` was **not touched**: not viewed for state, not re-run, not
cancelled.

---

## 2. Leg-B preconditions

Verified against the working tree and the remote before any mutation.

| # | Required | Observed | |
|---|---|---|---|
| B1 | `origin/main` = `09e8eb4` | `git rev-parse HEAD origin/main` → both `09e8eb4bdc4376dc6fb0b46d3fb263e62c46f3d9` | ✅ |
| B2 | Tree clean | `git status --porcelain` empty | ✅ |
| B3 | Workspace version `1.7.0` | `Cargo.toml:10` `version = "1.7.0"` | ✅ |
| B4 | `v1.7.0-rc.1` on `4169d84` | `git rev-list -n1 v1.7.0-rc.1` → `4169d840d1bfb2c313974385dddba63c9da66e43` | ✅ |
| B5 | No `v1.7.0` tag anywhere | local `git tag -l`: nine tags, `v1.0.0`…`v1.6.0` + `v1.7.0-rc.1`; `git ls-remote --tags origin \| grep v1.7.0` → only `refs/tags/v1.7.0-rc.1` | ✅ |
| B6 | Draft unpublished | `gh release list`: `v1.7.0-rc.1 draft=true`, `v1.6.0 latest=true` | ✅ |
| B7 | 473 tests | 64 + 13 + 156 + 240 + 0 doc-tests = 473, all passing | ✅ |

No mismatch. Proceeded.

**Foreground rule observed.** Every wait in this leg was a blocking foreground
call (`gh run watch <id> --exit-status`, exit 0). Nothing was backgrounded, no
notification was used, no poll-and-forget.

---

## 3. Ruling (1) — the CHANGELOG amendment

**Pre-flight, before touching the file.** `CHANGELOG.md` is LF-only (0 CRLF).
The heading is part of the anchor because the bare phrase occurs twice — the
count below proves it:

```
OLD len 48 count 1
NEW len 109 count 0
bare phrase 'as every other claim.' count: 2
```

**After the edit**, the two assertions that matter:

```
CRLF count: 0
OLD after: 0   NEW after: 1
```

The whole diff, which is the whole of this leg's source footprint:

```diff
@@ -55,6 +55,8 @@ working.
   explicit forbidden list — with the same machine-checked traceability
   as every other claim.
 
+No JSON key changed in this release. Zero new dependencies.
+
 ## [1.6.0] - 2026-08-05
```

`1 file changed, 2 insertions(+)`. Nothing else changed — no reformatting, no
opportunistic edit, no other file staged.

**§3 bar before the push:**

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | **473 passed, 0 failed, 0 ignored** (64 + 13 + 156 + 240 + 0) |

Commit `1f73888` `changelog: v1.7.0 carries the no-new-deps line the last three
releases carry`, pushed `09e8eb4..1f73888`.

**CI run [31239179139](https://github.com/cipherpine/quotapane/actions/runs/31239179139)**,
watched in the foreground to completion (exit 0), `success`,
04:19:22Z → 04:22:39Z. All **8 required checks green**, read back from the
check-runs API rather than the watcher:

```
success  build & test (macos-latest)
success  build & test (ubuntu-latest)
success  build & test (windows-latest)
success  cargo-audit (RustSec advisories)
success  cargo-deny (licenses, bans, advisories, sources)
success  gitleaks — full-history secret scan
success  invariant 4 — no telemetry
success  invariants — manifest, docs, and tests agree
total_count: 8
```

---

## 4. Tagging

Lightweight, matching the form of every prior release tag
(`git cat-file -t` on `v1.4.1`, `v1.5.0`, `v1.6.0`, `v1.7.0-rc.1` → `commit`
for all four):

```
$ git tag v1.7.0 1f73888
$ git rev-list -n1 v1.7.0
1f73888aa1e03a4df54b33c20e3eb14d48449eaa
$ git cat-file -t v1.7.0
commit
$ git push origin v1.7.0
 * [new tag]         v1.7.0 -> v1.7.0
$ git ls-remote --tags origin | grep 1.7.0
1f73888aa1e03a4df54b33c20e3eb14d48449eaa	refs/tags/v1.7.0
```

---

## 5. Release run

Run [31239319812](https://github.com/cipherpine/quotapane/actions/runs/31239319812),
`event: push`, `ref: v1.7.0`, created 04:23:08Z, **3/3 green**, watched to
completion in the foreground (`gh run watch --exit-status` → exit 0):

```
success  build (x86_64-pc-windows-msvc)      4m41s
success  build (x86_64-unknown-linux-gnu)    3m4s
success  checksum, sign, attest, draft       14s
```

Exactly one Release run exists for this tag (`gh run list --workflow=release.yml`
filtered on `headBranch == "v1.7.0"` → count 1).

`release.yml` untouched — identical blob at all four tags, so v1.7.0 exercised
the same pipeline that built v1.4.1, v1.5.0 and v1.6.0:

```
v1.4.1   44860bab2a3d626010e617cbae007656ccae715e
v1.5.0   44860bab2a3d626010e617cbae007656ccae715e
v1.6.0   44860bab2a3d626010e617cbae007656ccae715e
v1.7.0   44860bab2a3d626010e617cbae007656ccae715e
```

Draft, unpublished, as designed:

```json
{"createdAt":"2026-08-08T04:19:17Z","isDraft":true,"isPrerelease":false,
 "publishedAt":null,"tagName":"v1.7.0","targetCommitish":"main",
 "url":"https://github.com/cipherpine/quotapane/releases/tag/untagged-e70d36974426e62566d8"}
```

(`createdAt` reflects the tagged commit's date, not the draft's creation time —
the same field read the same way in every prior leg.)

With the release jobs included, `1f73888` now carries **11 check-runs, all
`success`** — the 8 required CI checks plus the 3 release-run jobs.

---

## 6. `tools/release-verify.sh v1.7.0` — complete output

Run **verbatim**, in Git Bash, from the repo root, **before** any pruning.
Exit code **0**.

```
PASS  step 1: downloaded 4 assets for v1.7.0
PASS  step 2: sha256sum -c SHA256SUMS
PASS  step 3: cosign verify-blob (Verified OK)
PASS  step 4a: attestation quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
PASS  step 4b: attestation quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz
PASS  R1: both attestation subjects digest-match disk
PASS  step 5: inventory + identical TOOLCHAIN.txt
PASS  step 6: shipped CLI runs (quotapane-cli 1.7.0, matches 1.7.0)
PASS  NC1/R3: zip bytes changed (5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f -> 8bf5e0e527dbe9af7b38271ff91dca6a65ea85506e844e0d16733a962d9adaaa)
PASS  NC1: checksum names the tampered file specifically
PASS  NC1: attestation refuses tampered zip
PASS    restored: quotapane-v1.7.0-x86_64-pc-windows-msvc.zip digest back to pristine
PASS  NC2: wrong identity rejected on the identity check specifically
PASS  NC3: wrong issuer rejected on the issuer check specifically
PASS  NC4/R3: SHA256SUMS bytes changed (2f169f6d87e2e8787484896639634e2ed5e5337d5aa6c3cbca55065fb85c31bf -> f1b366e7ee0fc9a238c6c3caae69a7c3104de96c6c232810f1c1b77e06a2c46b)
PASS  NC4: cosign rejects tampered SHA256SUMS on the signature check
PASS    restored: SHA256SUMS digest back to pristine
PASS  NC5/R4: wrong-repo target cli/cli confirmed to exist
PASS  NC5: real repo, real digest-miss (no attestations for our artifact)
PASS  NC6: unrelated file has no attestation
PASS  sweep: all artifacts pristine, steps 2-4 re-run clean
RESULT: PASS — v1.7.0: six steps, six controls, R1-R4
```

**21 PASS, 0 FAIL, exit 0.** The script was not edited and nothing was
re-invoked in modified form. Step 6 read `quotapane-cli 1.7.0` against a wanted
base version of `1.7.0`.

The block above is byte-identical to the post-prune re-run captured to a file
(§9) — `diff` reports no output across all 22 lines, which is simultaneously the
transcription check and the proof that the prune touched nothing.

---

## 7. Asset digests

Measured independently of the script, by a separate `gh release download` into
a scratch directory outside the repository:

```
5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f  quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
cf4b22ada89803dfbfc91ee2aa594cd242cca386c5189edf438f648dc12cf7a4  quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz
2f169f6d87e2e8787484896639634e2ed5e5337d5aa6c3cbca55065fb85c31bf  SHA256SUMS
392934f6920563307c23950119062673022c1e46a6f9d7b0d9aafc0a424881d6  SHA256SUMS.sigstore.json
```

The shipped `SHA256SUMS` names the two archives with exactly the first two
digests above:

```
5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f  quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
cf4b22ada89803dfbfc91ee2aa594cd242cca386c5189edf438f648dc12cf7a4  quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz
```

Three independent measurements agree: this download, the script's own pristine
baselines printed in its tamper lines (`5b465e54…` for the zip, `2f169f6d…` for
`SHA256SUMS`), and GitHub's server-side asset digests read back from the API
after the prune (§9).

In-archive `TOOLCHAIN.txt`, read out of both archives and identical (step 5
asserts identity but does not print it):

```
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
```

---

## 8. The deviation, on the record

**Deviation:** the spec's Phase 3 says "tag `v1.7.0` on the verified commit",
meaning `4169d84`, the commit the rc verified. This leg tagged `1f73888`
instead, per ruling (2). The ruling is the top tier's; this section records it
and the evidence that bounds it.

`1f73888` differs from `4169d84` by exactly two files, both markdown:

```
$ git diff --stat 4169d84 1f73888
 CHANGELOG.md              |   2 +
 reports/m17-release-rc.md | 361 +++++++++++++++++++++++++++++++++++++++++++++
 2 files changed, 363 insertions(+)
```

`reports/m17-release-rc.md` is Leg A's own report (commit `09e8eb4`);
`CHANGELOG.md` is ruling (1)'s two lines. No `.rs`, no `Cargo.toml`, no
`Cargo.lock`, no `.github/`, no `tools/`, no `assets/`, no `README.md`.

The bound is stronger than "not compiled": **neither file is shipped.** The
release archive inventory is seven entries and contains no `CHANGELOG.md` and no
`reports/`:

```
quotapane-v1.7.0-x86_64-pc-windows-msvc/          (dir)
  LICENSE-APACHE   LICENSE-MIT   README.md   TOOLCHAIN.txt
  quotapane-cli.exe   quotapane.exe
```

So the amended tree cannot have moved a shipped byte, and in any case the
verification in §6 ran fresh against the real `v1.7.0` draft built from
`1f73888` — six steps, six negative controls, R1–R4 — rather than inheriting
the rc's verdict. The rc's role (prove the pipeline) is undisturbed; the final
tag's verification (prove these artifacts) is what §6 is.

---

## 9. Prune of the rc — gated on `RESULT: PASS`

Executed only after §6's verdict. Both targets were inspected before deletion
and confirmed distinct from the `v1.7.0` objects:

| Object | Identity confirmed before deleting |
|---|---|
| rc draft | `tagName: v1.7.0-rc.1`, `isDraft: true` — the draft Leg A recorded |
| v1.7.0 draft (must survive) | `tagName: v1.7.0`, url `…/untagged-e70d36974426e62566d8`, 4 assets — a different release object |

```
$ gh release delete v1.7.0-rc.1 --yes --cleanup-tag     # exit 0
$ gh release view v1.7.0-rc.1
release not found
$ git ls-remote --tags origin | grep -c rc
0
```

`--cleanup-tag` removed the local tag as well as the remote ref: the follow-up
`git tag -d v1.7.0-rc.1` reported `error: tag 'v1.7.0-rc.1' not found`, and
`git tag -l` lists nine tags with no rc among them. Recorded because it is a
tool behaviour worth knowing, not a surprise that changed the outcome — the
intended end state (no rc tag, local or remote) is exactly what obtains.

State after the prune, re-read from the remote:

```
local tags:   v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0 v1.6.0 v1.7.0
remote tags:  v1.7.0 -> 1f73888aa1e03a4df54b33c20e3eb14d48449eaa; no rc
releases:     v1.7.0 draft=true | v1.6.0 latest=true | v1.5.0 … v1.0.0
```

`gh release view v1.7.0` after the prune: still a draft, `publishedAt: null`,
all four assets `state: uploaded`, with server-side digests identical to §7:

```
sha256:5b465e544dfaf62c80bfba6f6093c2a4c674bbce94d414747a9a3b95be86b64f  quotapane-v1.7.0-x86_64-pc-windows-msvc.zip
sha256:cf4b22ada89803dfbfc91ee2aa594cd242cca386c5189edf438f648dc12cf7a4  quotapane-v1.7.0-x86_64-unknown-linux-gnu.tar.gz
sha256:2f169f6d87e2e8787484896639634e2ed5e5337d5aa6c3cbca55065fb85c31bf  SHA256SUMS
sha256:392934f6920563307c23950119062673022c1e46a6f9d7b0d9aafc0a424881d6  SHA256SUMS.sigstore.json
```

**Post-prune re-verification.** `tools/release-verify.sh v1.7.0` was run a
second time after the prune, captured to a file, and diffed against the gate
run:

```
$ diff run1.txt run2.txt
(no output — 22 lines, identical)
```

21 PASS / 0 FAIL / exit 0 both before and after, same digests, same attestation
subjects. `v1.6.0` remains `Latest` until the owner publishes.

---

## 10. State left behind

**§3 bar re-run before this report's push**, even though the commit is
documentation-only (DECISIONS §3 says *every* push):

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | **473 passed, 0 failed, 0 ignored** |
| `python tools/check-invariants.py` | `OK: 8 invariants, 30 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |

- `main` = `1f73888` + this report commit. This leg's entire tree footprint is
  two files: `CHANGELOG.md` (+2 lines, ruling (1)) and this report. No code, no
  `Cargo.*`, no `.github/`, no `assets/`, no `README.md`, no `tools/`, no
  `prompts/`, no `DECISIONS.md`, no §4.1 path was touched.
- Tag `v1.7.0` exists on `1f73888` and is pushed. Its draft release exists,
  **unpublished**.
- Tag `v1.7.0-rc.1` and its draft are gone, locally and on the remote.
- Orphaned run `31121996517` left exactly as found — not viewed, not re-run, not
  cancelled.
- No dependency added. No credential file read. The only binary execution was
  the script's own `--version`/`--help` on the shipped CLI inside its
  `mktemp -d`, removed by its `EXIT` trap. All scratch directories live outside
  the repository; nothing was copied into the tree.
- Housekeeping: `.git` swept after every git operation — no `.git/*.lock`, no
  `.git/objects/maintenance.lock`, no `.git/objects/*/tmp_obj_*` at any point,
  so `_to_delete/git-stale/` was not needed.
- Nothing published. `publishedAt` is `null`.

---

## 11. What the owner does next

1. **Publish the draft** at
   <https://github.com/cipherpine/quotapane/releases/tag/untagged-e70d36974426e62566d8>,
   pasting the release body first: the `## [1.7.0] - 2026-08-08` CHANGELOG entry
   (body only — `CHANGELOG.md:10-58`, from "The visibility release:" through
   "Zero new dependencies.") followed by the standard verify footer, with the
   compare link retargeted:

   ```
   ---

   Verify this release with the three commands in the README's *Verifying a
   release* section — checksums, cosign keyless bundle, and GitHub build
   provenance.

   **Full Changelog**: https://github.com/cipherpine/quotapane/compare/v1.6.0...v1.7.0
   ```

   Unchanged from every prior leg and still offered as a fact, not a change: the
   README's actual heading is `## Verify a release`, while every shipped release
   body since v1.0.0 has said *"Verifying a release"*. Keeping the historical
   wording preserves consistency; correcting it is the owner's call. This
   session did not touch `README.md`.

2. **Confirm publication**, which is what queues Phase 4. Phase 4 needs a
   separate Leg C paste, which does not exist yet. This session did not begin
   it, did not write the end-gate report, and did not touch `DECISIONS.md`.

This session ends here, as instructed — the hard stop is the session end.
