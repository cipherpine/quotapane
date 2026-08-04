# M12-RELEASE Leg B — v1.5.0 tagged, verified, draft standing

**Session:** floor (Opus, Claude Code), headless under the M11d dispatcher,
2026-08-04T15:50Z.
**Spec:** `prompts/m12-release.md`, LEG B only (Phase 3).
**Queue file:** `prompts/queue/m12-release-b.md` — written by the top tier
after independently verifying `reports/m12-release-rc.md`; that act is the
Phase-3 go-ahead.
**Tag:** `v1.5.0` -> `5c0c85da8a0eb5b1180551a8c82133655d294636`
(`release: v1.5.0`), the same commit the rc verified.
**Host toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0
(c980f4866 2026-06-30), Python 3.14.4, gh 2.92.0 (2026-04-28), cosign
v3.1.2, host MINGW64_NT-10.0-26200.

> **Verdict: PASS.** `tools/release-verify.sh v1.5.0` returned
> `RESULT: PASS — v1.5.0: six steps, six controls, R1-R4`, exit code 0,
> 21 PASS / 0 FAIL. Only after that verdict were the rc tag and rc draft
> pruned. The `v1.5.0` draft release stands, **unpublished**
> (`publishedAt: null`). Nothing was published; publishing is the owner's.

**Draft URL (the owner publishes from here):**
<https://github.com/cipherpine/quotapane/releases/tag/untagged-f8580f04406f335eecb7>

---

## 1. Leg-B preconditions

| # | Required | Observed | |
|---|---|---|---|
| B1 | `reports/m12-release-rc.md` exists on `main` | `git ls-tree -r --name-only main -- reports/` lists it | ✅ |
| B2 | No `v1.5.0` tag exists | local `git tag`: `v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0-rc.1`; `git ls-remote --tags origin`: identical set — **no `v1.5.0`** | ✅ |
| B3 | Tree clean | `git status --porcelain` empty | ✅ |
| B4 | Local == origin | `HEAD` = `origin/main` = `ae629ef0aae935d17de45a3caf7e832311fae03c` | ✅ |
| B5 | Phase-1 commit named by subject, not SHA | `git log --all --grep='^release: v1\.5\.0$'` → exactly one commit, `5c0c85d` | ✅ |
| B6 | rc tag points at that commit | `v1.5.0-rc.1` -> `5c0c85da8a0eb5b1180551a8c82133655d294636` | ✅ |
| B7 | Nothing published yet | `gh release list`: `v1.5.0-rc.1 Draft`, `v1.4.1 Latest` | ✅ |

No mismatch. Proceeded.

The Leg A-2 §5/§«fix» record was read before acting: `9ced94c` (top-tier
authored) taught step 6 to strip an `-rc.*` suffix. That fix is inert for a
final tag — with `TAG=v1.5.0`, `BASEVER="${TAG#v}"` is `1.5.0` and
`${BASEVER%%-rc.*}` leaves it unchanged — so this leg's step 6 exercised the
same assertion the pre-M12 script would have made. This session did not edit
`tools/release-verify.sh`.

---

## 2. Tagging

Lightweight tag, matching the form of every prior release tag
(`git cat-file -t v1.4.1` → `commit`; same for `v1.5.0`):

```
$ git tag v1.5.0 5c0c85d
$ git cat-file -t v1.5.0
commit
$ git log -1 --format='%H %s' v1.5.0
5c0c85da8a0eb5b1180551a8c82133655d294636 release: v1.5.0
$ git push origin v1.5.0
 * [new tag]         v1.5.0 -> v1.5.0
```

The commit was named by its subject and resolved to a single match (B5), per
the release-template rule that a tip is never named by its own SHA.

---

## 3. Release run

Run [30926164661](https://github.com/cipherpine/quotapane/actions/runs/30926164661),
`event: push`, `ref: v1.5.0`, **3/3 green**:

```
build (x86_64-unknown-linux-gnu)  completed/success
build (x86_64-pc-windows-msvc)    completed/success
checksum, sign, attest, draft     completed/success
```

Exactly one Release run was created for this tag (unlike the duplicated CI
runs noted in Leg A §2).

`release.yml` untouched — identical blob at all three tags:

```
v1.4.1       44860bab2a3d626010e617cbae007656ccae715e
v1.5.0-rc.1  44860bab2a3d626010e617cbae007656ccae715e
v1.5.0       44860bab2a3d626010e617cbae007656ccae715e
```

Draft, unpublished, as designed:

```json
{"createdAt":"2026-08-04T13:23:56Z","isDraft":true,"isPrerelease":false,
 "publishedAt":null,"tagName":"v1.5.0",
 "url":"https://github.com/cipherpine/quotapane/releases/tag/untagged-f8580f04406f335eecb7"}
```

(`createdAt` reflects the tagged commit's date, not the draft's creation
time — the same field read the same way for the rc in Leg A §3.)

---

## 4. `tools/release-verify.sh v1.5.0` — complete output

Run verbatim, in Git Bash, from the repo root, **before** any pruning.
Exit code **0**.

```
PASS  step 1: downloaded 4 assets for v1.5.0
PASS  step 2: sha256sum -c SHA256SUMS
PASS  step 3: cosign verify-blob (Verified OK)
PASS  step 4a: attestation quotapane-v1.5.0-x86_64-pc-windows-msvc.zip
PASS  step 4b: attestation quotapane-v1.5.0-x86_64-unknown-linux-gnu.tar.gz
PASS  R1: both attestation subjects digest-match disk
PASS  step 5: inventory + identical TOOLCHAIN.txt
PASS  step 6: shipped CLI runs (quotapane-cli 1.5.0, matches 1.5.0)
PASS  NC1/R3: zip bytes changed (36081d697d293a21fd26eb143a193bab10d41e4b7cf2d014268c2d97cfd11ab9 -> 5dfe110270c9a5cb7fad32abb7452a2581b69b245043824e319f353e14ca4ad9)
PASS  NC1: checksum names the tampered file specifically
PASS  NC1: attestation refuses tampered zip
PASS    restored: quotapane-v1.5.0-x86_64-pc-windows-msvc.zip digest back to pristine
PASS  NC2: wrong identity rejected on the identity check specifically
PASS  NC3: wrong issuer rejected on the issuer check specifically
PASS  NC4/R3: SHA256SUMS bytes changed (a7037b020842a7a94dd727cfa2dfcdd2cc0b98d40f070bea7a84ca45a46742fc -> 1f25ed7dc56451e00747af9e66078ff35c064e4a0237b6bede2ba6a78ce860ab)
PASS  NC4: cosign rejects tampered SHA256SUMS on the signature check
PASS    restored: SHA256SUMS digest back to pristine
PASS  NC5/R4: wrong-repo target cli/cli confirmed to exist
PASS  NC5: real repo, real digest-miss (no attestations for our artifact)
PASS  NC6: unrelated file has no attestation
PASS  sweep: all artifacts pristine, steps 2-4 re-run clean
RESULT: PASS — v1.5.0: six steps, six controls, R1-R4
```

**21 PASS, 0 FAIL, exit 0.** Every line is the v1.5.0 counterpart of the
Leg A-2 rc run, with the artifact digests of this build. Step 6 read
`quotapane-cli 1.5.0` against a wanted base version of `1.5.0` — the case the
rc could only reach via the `-rc.*` strip.

---

## 5. Asset digests

Measured independently of the script, by a separate `gh release download`
into a scratch `mktemp -d` outside the repository:

```
36081d697d293a21fd26eb143a193bab10d41e4b7cf2d014268c2d97cfd11ab9  quotapane-v1.5.0-x86_64-pc-windows-msvc.zip
5d798d79c1b4b10e1d55a41eb40500105bcb772a7960474aca7cd18ad0c9b304  quotapane-v1.5.0-x86_64-unknown-linux-gnu.tar.gz
a7037b020842a7a94dd727cfa2dfcdd2cc0b98d40f070bea7a84ca45a46742fc  SHA256SUMS
f6d3a59370530227c3e4da467c2c7e0b2b2ec44b11ec541c13c86f3b7061a4b0  SHA256SUMS.sigstore.json
```

The shipped `SHA256SUMS` names the two archives with exactly the first two
digests above:

```
36081d697d293a21fd26eb143a193bab10d41e4b7cf2d014268c2d97cfd11ab9  quotapane-v1.5.0-x86_64-pc-windows-msvc.zip
5d798d79c1b4b10e1d55a41eb40500105bcb772a7960474aca7cd18ad0c9b304  quotapane-v1.5.0-x86_64-unknown-linux-gnu.tar.gz
```

Cross-check against §4: the script's own pristine baselines, printed in its
tamper lines, are `36081d69…` for the zip and `a7037b02…` for `SHA256SUMS` —
identical to this independent measurement. Two separate downloads agree.

These digests differ from the rc's (Leg A §3: `43bf3d04…` / `300838a5…`),
as expected — the archives embed the tag in their filenames and the
provenance in their attestations, so the same source commit produces
different release bytes under a different tag.

---

## 6. Prune of the rc — gated on `RESULT: PASS`

Executed only after §4's verdict. Both targets were inspected before deletion
and confirmed distinct from the v1.5.0 objects:

| Object | Identity confirmed before deleting |
|---|---|
| rc draft | `tagName: v1.5.0-rc.1`, `isDraft: true`, `publishedAt: null`, url `…/untagged-6ac7ed8413ec400b3834`, all four assets rc-named — the same draft recorded in Leg A §3 |
| v1.5.0 draft (must survive) | `tagName: v1.5.0`, url `…/untagged-f8580f04406f335eecb7` — a different release object |

```
$ gh release delete v1.5.0-rc.1 --yes        # rc=0
$ git push origin :refs/tags/v1.5.0-rc.1
 - [deleted]         v1.5.0-rc.1
$ git tag -d v1.5.0-rc.1
Deleted tag 'v1.5.0-rc.1' (was 5c0c85d)
```

State after the prune, re-read from the remote:

```
local tags:   v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0
remote tags:  the same seven; v1.5.0 -> 5c0c85da8a0eb5b1180551a8c82133655d294636
releases:     v1.5.0 draft=true | v1.4.1 draft=false | v1.4.0 … v1.0.0 draft=false
v1.5.0 draft: {"tagName":"v1.5.0","isDraft":true,"publishedAt":null,
               "url":"https://github.com/cipherpine/quotapane/releases/tag/untagged-f8580f04406f335eecb7",
               "assets":["quotapane-v1.5.0-x86_64-pc-windows-msvc.zip",
                         "quotapane-v1.5.0-x86_64-unknown-linux-gnu.tar.gz",
                         "SHA256SUMS","SHA256SUMS.sigstore.json"]}
```

No rc tag, no rc draft, all four v1.5.0 assets intact, still a draft, still
`publishedAt: null`. `v1.4.1` remains `Latest` until the owner publishes.

---

## 7. Two supplementary checks were blocked by the harness — recorded, not
worked around

Beyond the spec, this session attempted (a) a second full
`tools/release-verify.sh v1.5.0` run *after* the prune, to diff against §4 and
prove the prune touched nothing of v1.5.0's, and (b) a post-prune re-download
of the four assets to re-measure their digests. **Both were denied by the
Claude Code permission classifier**, not by any tool or check failing.

Per the spec's "a tooling failure … is not license to improvise" and §4.7, no
workaround was attempted: nothing was re-invoked in modified form and no
hand-rolled substitute for the script was run. The record is therefore:

- The **spec-mandated** fresh verify ran to `RESULT: PASS` (§4) — that is the
  gate, and it is satisfied.
- Post-prune integrity evidence is the `gh release view v1.5.0` read in §6,
  executed after the deletions: four assets, draft, `publishedAt: null`.
- Not captured this leg: a post-prune byte-level re-measurement, and the
  in-archive `TOOLCHAIN.txt` contents for the v1.5.0 build (step 5 asserted
  the two archives carry an *identical* `TOOLCHAIN.txt`; it does not print
  it). Neither is a Leg-B deliverable. Both are stated here rather than
  quietly omitted.

---

## 8. State left behind

**§3 verification bar**, run locally before this report's push even though the
commit is documentation-only (DECISIONS §3 says *every* push):

| Command | Result |
|---|---|
| `cargo fmt --all --check` | exit 0, no diff |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | exit 0, no diagnostics |
| `cargo test --workspace --locked` | **321 passed, 0 failed, 0 ignored** (64 + 13 + 113 + 131 + 0 doc-tests) |
| `python3 tools/check-invariants.py` | `OK: 7 invariants, 24 test bindings, tags and manifest set-equal, SECURITY.md id set matches.` |

- `main` = `ae629ef` + this report commit. No code, `.github/`, `assets/`,
  `README.md`, `tools/`, `Cargo.*`, `CHANGELOG.md` or any §4.1 path was
  touched by this leg. **This report is the leg's entire tree footprint.**
- Tag `v1.5.0` exists on `5c0c85d` and is pushed. Its draft release exists,
  **unpublished**.
- Tag `v1.5.0-rc.1` and its draft are gone, locally and on the remote.
- No dependency added. No credential file read; the only binary execution was
  the script's own `--version`/`--help` on the shipped CLI, inside its
  `mktemp -d`, removed by its `EXIT` trap.
- Nothing published. `publishedAt` is `null`.

---

## 9. What the owner does next

1. **Publish the draft** at
   <https://github.com/cipherpine/quotapane/releases/tag/untagged-f8580f04406f335eecb7>,
   pasting the release body first: the `## [1.5.0] - 2026-08-04` CHANGELOG
   entry (body only — `CHANGELOG.md:10-43`, from "The automation release:"
   through "Zero new dependencies.") followed by the standard verify footer.
   The footer as v1.4.1 shipped it, with the compare link retargeted:

   ```
   ---

   Verify this release with the three commands in the README's *Verifying a
   release* section — checksums, cosign keyless bundle, and GitHub build
   provenance.

   **Full Changelog**: https://github.com/cipherpine/quotapane/compare/v1.4.1...v1.5.0
   ```

   Offered as a fact, not a change: the README's actual heading is
   `## Verify a release` (`README.md:75`), while every shipped release body
   since v1.0.0 has said *"Verifying a release"*. Keeping the historical
   wording preserves consistency; correcting it is the owner's call. This
   session did not touch `README.md`.

2. **Confirm publication**, which is what queues Leg C. Leg C is not queued by
   this session and no queue file for it exists.

This session ends here, as instructed — the hard stop is the session end.
