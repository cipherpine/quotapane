# M13-RELEASE Leg B — v1.6.0 tagged, verified, draft standing

**Session:** floor (Opus, Claude Code), headless under the M11d dispatcher,
2026-08-05T22:57Z.
**Spec:** `prompts/m13-release.md`, LEG B only (Phase 3).
**Queue file:** `prompts/queue/m13-release-b.md` — written by the top tier
after independently verifying `reports/m13-release-rc.md` (commit `9e8a5b9`);
that act is the Phase-3 go-ahead.
**Tag:** `v1.6.0` -> `d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e`
(`release: v1.6.0`), the same commit the rc verified.
**Host toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0
(c980f4866 2026-06-30), Python 3.14.4, gh 2.92.0 (2026-04-28), cosign
v3.1.2, host MINGW64_NT-10.0-26200.
**Release toolchain (in-archive `TOOLCHAIN.txt`):** rustc 1.97.1
(8bab26f4f 2026-07-14), cargo 1.97.1 (c980f4866 2026-06-30).

> **Verdict: PASS.** `tools/release-verify.sh v1.6.0` returned
> `RESULT: PASS — v1.6.0: six steps, six controls, R1-R4`, exit code 0,
> 21 PASS / 0 FAIL. Only after that verdict were the rc tag and rc draft
> pruned. The `v1.6.0` draft release stands, **unpublished**
> (`publishedAt: null`); `v1.5.0` remains `Latest`. Nothing was published;
> publishing is the owner's.

**Draft URL (the owner publishes from here):**
<https://github.com/cipherpine/quotapane/releases/tag/untagged-b244fb45915bfbb53c98>

---

## 1. Leg-B preconditions

| # | Required | Observed | |
|---|---|---|---|
| B1 | `reports/m13-release-rc.md` exists on `main` | `git ls-tree -r --name-only main -- reports/` lists it | ✅ |
| B2 | No `v1.6.0` tag exists | local `git tag`: `v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0 v1.6.0-rc.1`; `git ls-remote --tags origin`: identical set — **no `v1.6.0`** | ✅ |
| B3 | Tree clean | `git status --porcelain` empty | ✅ |
| B4 | Local == origin | `HEAD` = `origin/main` = `9e8a5b907c6a7e3c6b81c5ba83a470d85248e67b` | ✅ |
| B5 | Phase-1 commit named by subject, not SHA | `git rev-list --all --grep='^release: v1\.6\.0$'` → exactly one commit, `d86cb5a` | ✅ |
| B6 | rc tag points at that commit | `v1.6.0-rc.1` -> `d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e` | ✅ |
| B7 | Nothing published yet | `gh release list`: `v1.6.0-rc.1 Draft`, `v1.5.0 Latest` | ✅ |

No mismatch. Proceeded.

**CI on `d86cb5a` was re-read in this session**, not carried from the rc
report, via the check-runs API. Eleven check-runs, all `success` — the 8
required CI checks plus the 3 release-run jobs the rc tag produced against the
same commit:

```
success  build & test (macos-latest)
success  build & test (ubuntu-latest)
success  build & test (windows-latest)
success  cargo-audit (RustSec advisories)
success  cargo-deny (licenses, bans, advisories, sources)
success  gitleaks — full-history secret scan
success  invariant 4 — no telemetry
success  invariants — manifest, docs, and tests agree
success  build (x86_64-pc-windows-msvc)          # rc release run
success  build (x86_64-unknown-linux-gnu)        # rc release run
success  checksum, sign, attest, draft           # rc release run
```

**Dispatcher rule observed.** Every wait in this session was a blocking
foreground call (`gh run watch --exit-status --interval 20`). Nothing was
backgrounded — a headless session's background tasks die with it.

**One tool-invocation error, recorded because it touched the tagging step.**
The first attempt to resolve the release commit passed an invalid flag
(`git rev-list --fixed-strings=false`), which exited 1 and produced an empty
commit variable; the follow-on `git tag`/`git push` in the same compound
command therefore failed loudly with `Failed to resolve '' as a valid ref` and
`src refspec v1.6.0 does not match any`. **No tag, local or remote, was
created by that attempt** — verified by re-listing tags before retrying
(`v1.0.0 … v1.5.0 v1.6.0-rc.1`, nothing else). The command was re-run without
the invalid flag, resolving to exactly one commit. Nothing was worked around;
the failure was a typo in this session's own scaffolding, not in any project
tool.

---

## 2. Tagging

Lightweight tag, matching the form of every prior release tag
(`git cat-file -t v1.5.0` → `commit`):

```
$ git rev-list --all --grep='^release: v1\.6\.0$'
d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e          # exactly one match
$ git tag v1.6.0 d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e
$ git cat-file -t v1.6.0
commit
$ git log -1 --format='%H %s' v1.6.0
d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e release: v1.6.0
$ git push origin v1.6.0
 * [new tag]         v1.6.0 -> v1.6.0
$ git ls-remote --tags origin refs/tags/v1.6.0
d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e	refs/tags/v1.6.0
```

The commit was named by its subject and resolved to a single match (B5), per
the release-template rule that a tip is never named by its own SHA.

---

## 3. Release run

Run [31054142238](https://github.com/cipherpine/quotapane/actions/runs/31054142238),
`event: push`, `ref: v1.6.0`, **3/3 green**, watched to completion in the
foreground (`gh run watch --exit-status` → exit 0):

```
completed/success  build (x86_64-pc-windows-msvc)      4m37s
completed/success  build (x86_64-unknown-linux-gnu)    2m50s
completed/success  checksum, sign, attest, draft       19s
```

Exactly one Release run exists for this tag
(`gh run list --workflow=release.yml`, filtered on `headBranch == "v1.6.0"` →
count 1).

`release.yml` untouched — identical blob at all four tags, so v1.6.0 exercised
the same pipeline that built v1.4.1, v1.5.0 and the rc:

```
v1.4.1       44860bab2a3d626010e617cbae007656ccae715e
v1.5.0       44860bab2a3d626010e617cbae007656ccae715e
v1.6.0-rc.1  44860bab2a3d626010e617cbae007656ccae715e
v1.6.0       44860bab2a3d626010e617cbae007656ccae715e
```

Draft, unpublished, as designed:

```json
{"createdAt":"2026-08-05T14:49:07Z","isDraft":true,"isPrerelease":false,
 "publishedAt":null,"tagName":"v1.6.0",
 "url":"https://github.com/cipherpine/quotapane/releases/tag/untagged-b244fb45915bfbb53c98"}
```

(`createdAt` reflects the tagged commit's date, not the draft's creation
time — the same field read the same way for the rc in Leg A-2 §3. The assets
carry the true upload time, `2026-08-05T22:53:50Z`.)

---

## 4. `tools/release-verify.sh v1.6.0` — complete output

Run verbatim, in Git Bash, from the repo root, **before** any pruning.
Exit code **0**.

```
PASS  step 1: downloaded 4 assets for v1.6.0
PASS  step 2: sha256sum -c SHA256SUMS
PASS  step 3: cosign verify-blob (Verified OK)
PASS  step 4a: attestation quotapane-v1.6.0-x86_64-pc-windows-msvc.zip
PASS  step 4b: attestation quotapane-v1.6.0-x86_64-unknown-linux-gnu.tar.gz
PASS  R1: both attestation subjects digest-match disk
PASS  step 5: inventory + identical TOOLCHAIN.txt
PASS  step 6: shipped CLI runs (quotapane-cli 1.6.0, matches 1.6.0)
PASS  NC1/R3: zip bytes changed (998377798e048bc2b271dad5a5ea05e8857478b531a684a0c6e7e615c2614a52 -> a4c93b869934d1424bd4a934468b4c144f8e57d87592412ea2645ceef7039194)
PASS  NC1: checksum names the tampered file specifically
PASS  NC1: attestation refuses tampered zip
PASS    restored: quotapane-v1.6.0-x86_64-pc-windows-msvc.zip digest back to pristine
PASS  NC2: wrong identity rejected on the identity check specifically
PASS  NC3: wrong issuer rejected on the issuer check specifically
PASS  NC4/R3: SHA256SUMS bytes changed (a0e2b87dc42e92b6b92e5b68b3292d8c949b15b5f20ae0e86135c0b9a4885b43 -> 9371bb0b3749f31828a9bdd8caba96ab223c2b58a38d97349495ccc126eb5649)
PASS  NC4: cosign rejects tampered SHA256SUMS on the signature check
PASS    restored: SHA256SUMS digest back to pristine
PASS  NC5/R4: wrong-repo target cli/cli confirmed to exist
PASS  NC5: real repo, real digest-miss (no attestations for our artifact)
PASS  NC6: unrelated file has no attestation
PASS  sweep: all artifacts pristine, steps 2-4 re-run clean
RESULT: PASS — v1.6.0: six steps, six controls, R1-R4
```

**21 PASS, 0 FAIL, exit 0.** Step 6 read `quotapane-cli 1.6.0` against a wanted
base version of `1.6.0` — for a final tag the `-rc.*` strip added in `9ced94c`
is inert (`BASEVER="${TAG#v}"` is already `1.6.0`), so this leg exercised the
same assertion the pre-M12 script would have made. `tools/release-verify.sh`
was not edited, and nothing was re-invoked in modified form.

The block above was captured to a scratch file at run time and transcribed from
it, not retyped (§7).

---

## 5. Asset digests

Measured independently of the script, by a separate `gh release download` into
a scratch `mktemp -d` outside the repository:

```
998377798e048bc2b271dad5a5ea05e8857478b531a684a0c6e7e615c2614a52  quotapane-v1.6.0-x86_64-pc-windows-msvc.zip
8e5f39b5b8b0b524ff8ae8dc96e7ac3056355634ad577c0a9dc3d893d28d467e  quotapane-v1.6.0-x86_64-unknown-linux-gnu.tar.gz
a0e2b87dc42e92b6b92e5b68b3292d8c949b15b5f20ae0e86135c0b9a4885b43  SHA256SUMS
0110a204eebf903eac58e6aea229d49dde7904ac4b5fcc8b0a5ae1e1e159d795  SHA256SUMS.sigstore.json
```

The shipped `SHA256SUMS` names the two archives with exactly the first two
digests above:

```
998377798e048bc2b271dad5a5ea05e8857478b531a684a0c6e7e615c2614a52  quotapane-v1.6.0-x86_64-pc-windows-msvc.zip
8e5f39b5b8b0b524ff8ae8dc96e7ac3056355634ad577c0a9dc3d893d28d467e  quotapane-v1.6.0-x86_64-unknown-linux-gnu.tar.gz
```

Three independent measurements agree: this download, the script's own pristine
baselines printed in its tamper lines (`998377798e…` for the zip, `a0e2b87dc4…`
for `SHA256SUMS`), and GitHub's server-side asset digests read back from the
API after the prune (§6).

In-archive `TOOLCHAIN.txt` (step 5 asserts the two archives carry an
*identical* copy but does not print it; recorded here because M12 Leg B §7 had
to list it as not captured):

```
rustc 1.97.1 (8bab26f4f 2026-07-14)
cargo 1.97.1 (c980f4866 2026-06-30)
```

These digests differ from the rc's (Leg A-2 §2: `13046bed…` / `df9a4b74…`), as
expected — the archives embed the tag in their filenames and the provenance in
their attestations, so the same source commit produces different release bytes
under a different tag.

---

## 6. Prune of the rc — gated on `RESULT: PASS`

Executed only after §4's verdict. Both targets were inspected before deletion
and confirmed distinct from the v1.6.0 objects:

| Object | Identity confirmed before deleting |
|---|---|
| rc draft | `tagName: v1.6.0-rc.1`, `isDraft: true`, `publishedAt: null`, url `…/untagged-44e1d32170368e4d811c`, all four assets rc-named — the same draft recorded in Leg A-2 §2 |
| v1.6.0 draft (must survive) | `tagName: v1.6.0`, url `…/untagged-b244fb45915bfbb53c98` — a different release object |

```
$ gh release delete v1.6.0-rc.1 --yes        # rc=0
$ git push origin :refs/tags/v1.6.0-rc.1
 - [deleted]         v1.6.0-rc.1
$ git tag -d v1.6.0-rc.1
Deleted tag 'v1.6.0-rc.1' (was d86cb5a)
```

State after the prune, re-read from the remote:

```
local tags:   v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0 v1.6.0
remote tags:  the same eight; v1.6.0 -> d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e
releases:     v1.6.0 Draft | v1.5.0 Latest | v1.4.1 … v1.0.0
$ gh release view v1.6.0-rc.1
release not found                            # rc=1
```

`gh release view v1.6.0` after the prune: still a draft, `publishedAt: null`,
all four assets `state: uploaded`, with server-side digests identical to §5:

```
sha256:998377798e048bc2b271dad5a5ea05e8857478b531a684a0c6e7e615c2614a52  quotapane-v1.6.0-x86_64-pc-windows-msvc.zip
sha256:8e5f39b5b8b0b524ff8ae8dc96e7ac3056355634ad577c0a9dc3d893d28d467e  quotapane-v1.6.0-x86_64-unknown-linux-gnu.tar.gz
sha256:a0e2b87dc42e92b6b92e5b68b3292d8c949b15b5f20ae0e86135c0b9a4885b43  SHA256SUMS
sha256:0110a204eebf903eac58e6aea229d49dde7904ac4b5fcc8b0a5ae1e1e159d795  SHA256SUMS.sigstore.json
```

No rc tag, no rc draft, all four v1.6.0 assets intact. `v1.5.0` remains
`Latest` until the owner publishes.

---

## 7. Post-prune re-verification — the M12 gap, closed

M12 Leg B §7 had to record two supplementary checks as **blocked by the
harness**: a second full verify after the prune, and a post-prune re-download
to re-measure digests. Both ran here.

`tools/release-verify.sh v1.6.0` was run a **second time, after the prune**,
and its output is byte-identical to §4:

```
$ diff run1.txt run2.txt
(no output — 22 lines, identical)
```

That is the proof the prune touched nothing of v1.6.0's: 21 PASS / 0 FAIL /
exit 0 both before and after, same digests, same attestation subjects. The
post-prune digest re-measurement is §6's server-side read, which agrees with
§5's independent download.

One command in this session **was** denied by the Claude Code permission
classifier — a compound state-readback chain combining `git`, `gh release
list`, `gh release view` and a shell conditional. Per §4.7 nothing was
improvised around it: the same reads were re-issued as individual commands
(that is the classifier's evident intent, not a bypass), and every fact in §6
comes from those. No verification was skipped and no substitute for
`tools/release-verify.sh` was hand-rolled.

**Transcription check.** The §4 block was not retyped. The verifier's stdout
was captured to a scratch file at run time, and the block as extracted back out
of this report was diffed against it:

```
$ diff <(sed -n '/^PASS  step 1/,/^RESULT/p' reports/m13-release-draft.md) run1.txt
VERBATIM MATCH: OK   (22 lines)
```

---

## 8. The `24h` spot-check clause — still the top tier's, and Leg B did not
depend on it

Leg A-2 §5 reported that the spec's Phase-2 content spot-check asks for a `24h`
needle that **no correctly-built LTO release binary can carry**, proved the tag
ships as immediate stores rather than a rodata string, and referred the wording
to the top tier.

For the record: **Leg B's spec text contains no content spot-check.** Its gate
is `RESULT: PASS` from a fresh `tools/release-verify.sh`, which this leg has
(§4, twice). The queue file `prompts/queue/m13-release-b.md` carries no
amendment to the Phase-2 clause and none was needed to execute this leg.
Nothing was edited in the spec, the script, or the release. The clause remains
an open top-tier wording decision with **nothing in the artifact known to be
wrong** — carry it into the M13 end-gate rather than treating it as closed.

---

## 9. State left behind

**§3 verification bar**, run locally before this report's push even though the
commit is documentation-only (DECISIONS §3 says *every* push):

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | **386 passed, 0 failed, 0 ignored** (64 + 13 + 127 + 182 + 0 doc-tests) — matches the spec's P1 count |
| `python3 tools/check-invariants.py` | `OK: 7 invariants, 24 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |

- `main` = `9e8a5b9` + this report commit. No code, `.github/`, `assets/`,
  `README.md`, `tools/`, `prompts/`, `Cargo.*`, `CHANGELOG.md`, `DECISIONS.md`
  or any §4.1 path was touched by this leg. **This report is the leg's entire
  tree footprint.**
- Tag `v1.6.0` exists on `d86cb5a` and is pushed. Its draft release exists,
  **unpublished**.
- Tag `v1.6.0-rc.1` and its draft are gone, locally and on the remote.
- No dependency added. No credential file read; the only binary execution was
  the script's own `--version`/`--help` on the shipped CLI, inside its
  `mktemp -d`, removed by its `EXIT` trap. All scratch directories live outside
  the repository; nothing was copied into the tree.
- Nothing published. `publishedAt` is `null`.

---

## 10. What the owner does next

1. **Publish the draft** at
   <https://github.com/cipherpine/quotapane/releases/tag/untagged-b244fb45915bfbb53c98>,
   pasting the release body first: the `## [1.6.0] - 2026-08-05` CHANGELOG
   entry (body only — `CHANGELOG.md:10-45`, from "The memory release:" through
   "Zero new dependencies.") followed by the standard verify footer, with the
   compare link retargeted:

   ```
   ---

   Verify this release with the three commands in the README's *Verifying a
   release* section — checksums, cosign keyless bundle, and GitHub build
   provenance.

   **Full Changelog**: https://github.com/cipherpine/quotapane/compare/v1.5.0...v1.6.0
   ```

   Unchanged from M12 Leg B §9 and still offered as a fact, not a change: the
   README's actual heading is `## Verify a release`, while every shipped release
   body since v1.0.0 has said *"Verifying a release"*. Keeping the historical
   wording preserves consistency; correcting it is the owner's call. This
   session did not touch `README.md`.

2. **Confirm publication**, which is what queues Leg C. Leg C is not queued by
   this session and no queue file for it exists.

This session ends here, as instructed — the hard stop is the session end.
