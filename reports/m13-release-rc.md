# M13-RELEASE Leg A-2 — v1.6.0 rc verification

**Session:** floor (Opus, Claude Code), headless under the M11d dispatcher,
2026-08-05.
**Spec:** `prompts/m13-release.md` Phase 2, dispatched as
`prompts/queue/m13-release-a2.md` (Leg A-2 — Phase 2 only; Phase 1 landed in
the preceding session).
**Phase-1 commit:** `d86cb5a` (`release: v1.6.0`), parent `6f80e56`
(`prompts: M13-RELEASE spec + launchers — v1.6.0`).
**rc tag:** `v1.6.0-rc.1` -> `d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e`.
**Host toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0,
Python 3.14.4, gh 2.92.0 (2026-04-28), cosign v3.1.2, host
MINGW64_NT-10.0-26200.
**Release toolchain (in-archive `TOOLCHAIN.txt`):** rustc 1.97.1
(8bab26f4f 2026-07-14), cargo 1.97.1 (c980f4866 2026-06-30).

> **Verdict: Phase 2 met its gate.** `tools/release-verify.sh v1.6.0-rc.1`
> returned **`RESULT: PASS`**, exit 0 — 21 PASS, 0 FAIL, six steps, six
> negative controls, R1–R4. The release run was 3/3 green and the draft is
> unpublished.
>
> **One spot-check needle did not match, and it is the needle that is wrong,
> not the release.** The spec asks that both shipped GUI binaries contain
> `alert: ` and `24h`. `alert: ` is present in both; `24h` is present in
> **neither** — because at `lto = true, codegen-units = 1, strip = true` the
> three-byte literal is materialized as two immediate stores rather than kept
> as a rodata string. This is proven, not inferred: a local
> `cargo build --workspace --release --locked` of this exact commit reproduces
> the absence, and the instruction bytes that write `'2','4','h'` are
> **byte-identical** between that local build and the shipped artifact (§5).
> The sparkline tag ships. No correctly-built release binary can satisfy the
> needle as written.
>
> Per DECISIONS §4.7 nothing was worked around: `tools/release-verify.sh` was
> not edited, no run was repeated in modified form, and the spec was not
> amended. **Whether to reword the spot-check clause is the top tier's call
> (§7), exactly as the step-6 defect was in M12-RELEASE §5.**
>
> **No `v1.6.0` tag exists. Nothing is published.** Leg B was not entered.

---

## 1. Preconditions

| # | Required | Observed | |
|---|---|---|---|
| A2-1 | Tip `d86cb5a`, subject `release: v1.6.0` | `d86cb5a50a86e1408ee7e75ab65c611f4ef9f87e`, subject exact | ✅ |
| A2-1 | Parent `prompts: M13-RELEASE spec + launchers — v1.6.0` | `6f80e56`, subject exact | ✅ |
| A2-2 | Tree clean | `git status --porcelain` empty | ✅ |
| A2-2 | Local tip == `origin/main` | both `d86cb5a50a86…` | ✅ |
| A2-3 | Version `1.6.0` in workspace `Cargo.toml` | `Cargo.toml:10  version = "1.6.0"` | ✅ |
| A2-4 | Tags exactly v1.0.0–v1.5.0; no rc, no `v1.6.0` | `v1.0.0 v1.1.0 v1.2.0 v1.3.0 v1.4.0 v1.4.1 v1.5.0` — nothing else | ✅ |
| A2-5 | CI 8/8 green on `d86cb5a` before tagging | run [31017164177](https://github.com/cipherpine/quotapane/actions/runs/31017164177), `success`; 8/8 below | ✅ |

The CI claim was re-verified in this session against the checks API rather than
taken from the handoff note:

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

No mismatch. Proceeded.

**Dispatcher rule observed.** The prior session ended with work pending because
it waited on a background watcher. Every wait in this session was a blocking
foreground call (`gh run watch --exit-status`, and a foreground poll loop for
the run to appear). Nothing was backgrounded.

---

## 2. Phase 2 — rc tag and release run

`v1.6.0-rc.1` created **lightweight** on `d86cb5a` — matching the form of every
prior release tag (`git cat-file -t v1.4.1` / `v1.5.0` = `commit`) — and
pushed. Release run
[31017836195](https://github.com/cipherpine/quotapane/actions/runs/31017836195),
**3/3 green**:

```
success  build (x86_64-pc-windows-msvc)
success  build (x86_64-unknown-linux-gnu)
success  checksum, sign, attest, draft
```

`release.yml` untouched — identical blob at v1.5.0 and at this tag, so the rc
exercised an unchanged pipeline:

```
$ git rev-parse v1.5.0:.github/workflows/release.yml
44860bab2a3d626010e617cbae007656ccae715e
$ git rev-parse v1.6.0-rc.1:.github/workflows/release.yml
44860bab2a3d626010e617cbae007656ccae715e
```

Draft, unpublished, as designed:

```json
{"createdAt":"2026-08-05T14:49:07Z","isDraft":true,"isPrerelease":false,
 "publishedAt":null,"tagName":"v1.6.0-rc.1",
 "url":"https://github.com/cipherpine/quotapane/releases/tag/untagged-44e1d32170368e4d811c"}
```

Asset digests:

```
13046bedce7acb6a4ff4823da5c6e86adb307b833012e12fe3a6eb8f0ff8a63c  quotapane-v1.6.0-rc.1-x86_64-pc-windows-msvc.zip
df9a4b743884c62d0a78108b66d87ce5248c8d8db7c4dc380ca6a0905de11bf3  quotapane-v1.6.0-rc.1-x86_64-unknown-linux-gnu.tar.gz
88b20c0a47504b042f45419565c5ebed49b07fc97113c03f64cd6e13e5d0f951  SHA256SUMS
fcdb73d0a038b54a857750c7024516aea77d823a57c01418d219ffe9ab21b2fc  SHA256SUMS.sigstore.json
```

`TOOLCHAIN.txt` is identical in both archives
(`cdd372f49763b7ad6faf8210706bc4f7212e18ebbd1abee8ab874ebbd588c4a3`).

---

## 3. `tools/release-verify.sh v1.6.0-rc.1` — complete output

Run verbatim, in Git Bash, from the repo root. Exit code **0**.

```
PASS  step 1: downloaded 4 assets for v1.6.0-rc.1
PASS  step 2: sha256sum -c SHA256SUMS
PASS  step 3: cosign verify-blob (Verified OK)
PASS  step 4a: attestation quotapane-v1.6.0-rc.1-x86_64-pc-windows-msvc.zip
PASS  step 4b: attestation quotapane-v1.6.0-rc.1-x86_64-unknown-linux-gnu.tar.gz
PASS  R1: both attestation subjects digest-match disk
PASS  step 5: inventory + identical TOOLCHAIN.txt
PASS  step 6: shipped CLI runs (quotapane-cli 1.6.0, matches 1.6.0)
PASS  NC1/R3: zip bytes changed (13046bedce7acb6a4ff4823da5c6e86adb307b833012e12fe3a6eb8f0ff8a63c -> 0a05e83b4bf53a05e35105e66279eacecf5a745b721810e28f8b59e43e2bbcba)
PASS  NC1: checksum names the tampered file specifically
PASS  NC1: attestation refuses tampered zip
PASS    restored: quotapane-v1.6.0-rc.1-x86_64-pc-windows-msvc.zip digest back to pristine
PASS  NC2: wrong identity rejected on the identity check specifically
PASS  NC3: wrong issuer rejected on the issuer check specifically
PASS  NC4/R3: SHA256SUMS bytes changed (88b20c0a47504b042f45419565c5ebed49b07fc97113c03f64cd6e13e5d0f951 -> 37f8965b30e4844539d6af6140f8b22118d2d5a0abe3edd4a97f858089bd9681)
PASS  NC4: cosign rejects tampered SHA256SUMS on the signature check
PASS    restored: SHA256SUMS digest back to pristine
PASS  NC5/R4: wrong-repo target cli/cli confirmed to exist
PASS  NC5: real repo, real digest-miss (no attestations for our artifact)
PASS  NC6: unrelated file has no attestation
PASS  sweep: all artifacts pristine, steps 2-4 re-run clean
RESULT: PASS — v1.6.0-rc.1: six steps, six controls, R1-R4
```

**21 PASS, 0 FAIL, exit code 0.** Step 6 — the assertion that failed on the
first M12 rc and was fixed in `9ced94c` — passed on its rc path here:
`quotapane-cli 1.6.0, matches 1.6.0`. The block above was captured to a file at
run time and the transcription diffed back out of this report rather than
retyped (§6).

---

## 4. Content spot-check (spec'd, read-only)

Performed on the rc artifacts downloaded to a scratch directory outside the
repo. Only file bytes were read; no shipped GUI binary was executed and no
credential path was touched.

**Shipped README — `Theming and preferences`: present in both archives.**

| Artifact | occurrences |
|---|---|
| `x_win/…/README.md` | 2 |
| `x_lin/…/README.md` | 2 |

Line 30 is the section heading `## Theming and preferences`; line 86 is the
cross-reference from the install section, which also states the M13 file
promise in the shipped text:

> QuotaPane writes at most two files, both under your config directory:
> `config.cfg` (your preferences) and, only if you turn `history=on`,
> `history.jsonl` … Credentials are never written.

Both archives ship the identical README, byte-identical to the repo's README at
the tagged commit:

```
11fac325a5553980f6d190f9cbff8ff5d48412c48eadd3e484eb47599c18b9d1  x_win/…/README.md
11fac325a5553980f6d190f9cbff8ff5d48412c48eadd3e484eb47599c18b9d1  x_lin/…/README.md
11fac325a5553980f6d190f9cbff8ff5d48412c48eadd3e484eb47599c18b9d1  README.md (working tree)
```

**Shipped GUI binaries — needle counts.** Searched at the byte level (Python
`bytes.count`, not `grep`, to remove any question of binary handling):

| needle | win `quotapane.exe` | linux `quotapane` | |
|---|---|---|---|
| `alert: ` | 2 | 2 | ✅ spec'd |
| `24h` | **0** | **0** | ❌ spec'd — see §5 |
| `refilled: ` | 1 | 1 | context |
| `history.jsonl` | 1 | 1 | context |
| `config.cfg` | 1 | 1 | context |
| `ALERT — ` | 1 | 0 | context |

**On `alert: `, an honest detail:** the count of 2 is not two banners. One
match is the product's banner template, the other is rustls's
`received fatal alert: `. The product occurrence is unambiguous in rodata,
carrying the alert-mode vocabulary and the whole M13 banner grammar:

```
…OAuth token expired pace threshold \x07alert: · · at · >= ·% (·) \n refilled: · · back under ·
```

That is the CHANGELOG's `alert: claude 7d at 85% >= 80% (pace)` and its
`refilled:` counterpart, with both `pace` and `threshold` modes present.

**Platform difference, recorded and explained.** `ALERT — ` and `alert_mode`
appear in the Windows GUI binary and not the Linux one. This is the documented
M3.5 design, not a regression: `tray-icon` is gated to Windows + macOS in
`crates/usage-ui/Cargo.toml` (deliberately no `gtk` feature — "Linux stays
window-only in v1"), so the tray-tooltip path is not compiled on Linux. Every
config key the release parses is still reachable there: `config.cfg`,
`history.jsonl`, `theme`, `history`, `alert_at`, `threshold` and `pace` all
appear in the Linux binary.

---

## 5. The `24h` needle — diagnosis, nothing worked around

**The tag is real source, on a live path.** `crates/usage-ui/src/main.rs:254`:

```rust
const SPARK_TAG: &str = "24h";
```

used unconditionally — no `cfg`, no feature gate — by `render_sparkline`
(`main.rs:2454`, painting at `main.rs:2471-2477`), which is called from the
pane render path at `main.rs:2121`. It cannot be dead-code eliminated. The
workspace's tests include `assert_eq!(SPARK_TAG, "24h")` (`main.rs:4649`);
they are green at this commit — `build & test` succeeded on all three platforms
in run 31017164177 — and the count of 386 is Leg A's §3-bar figure, carried
from the preceding session's report rather than re-run here.

**The literal survives at `-O0` and disappears at `-O3 + LTO`.** Same source,
four binaries:

| Build | Profile | `24h` |
|---|---|---|
| `target/debug/quotapane.exe` (today, 1.6.0 source) | dev | **1** |
| `target/release/quotapane.exe` (built this session, `--locked`, this commit) | `lto = true`, `codegen-units = 1`, `strip = true` | **0** |
| shipped `x_win/…/quotapane.exe` | same | **0** |
| shipped `x_lin/…/quotapane` | same | **0** |

**The local release build reproduces the shipped artifact's entire string
profile**, needle for needle — so the shipped binary is exactly what a correct
release build of this source produces on this machine:

| needle | local release | shipped win |
|---|---|---|
| `24h` | 0 | 0 |
| `alert: ` | 2 | 2 |
| `ALERT — ` | 1 | 1 |
| `refilled: ` | 1 | 1 |
| `history.jsonl` | 1 | 1 |
| `config.cfg` | 1 | 1 |
| `alert_mode` | 1 | 1 |
| `pace` | 17 | 17 |

**Where the three bytes actually went.** `ui.painter().text()` takes
`impl ToString`, so `SPARK_TAG` becomes `String::from("24h")` — an allocation
plus a 3-byte copy. LLVM inlines a copy that short as immediate stores, after
which the rodata literal is unreferenced and dropped. Both binaries contain
exactly one such site, and the surrounding instruction bytes are **identical**:

```
local   0x18814 : 89 85 e8 00 00 00  0f 84 6e 0e 00 00  c6 40 02 68  66 c7 00 32 34  0f 10 45 68 …
shipped 0x18654 : 89 85 e8 00 00 00  0f 84 6e 0e 00 00  c6 40 02 68  66 c7 00 32 34  0f 10 45 68 …
                                                        ^^^^^^^^^^^  ^^^^^^^^^^^^^^
                                        mov byte [rax+2], 0x68 ('h')  mov word [rax], 0x3432 ('2','4')
```

`'2' = 0x32`, `'4' = 0x34`, `'h' = 0x68`. The tag is in the shipped binary — as
instructions that write it, not as a string that sits there. The two offsets
differ only by image layout (local rustc 1.97.0 vs release 1.97.1).

**Conclusion, offered as fact and not as a ruling:** the needle is undetectable
by construction in any LTO release build, so the spot-check clause as written
cannot be satisfied by a correct release — the same shape of defect as
M12-RELEASE §5, where the assertion was wrong and the artifact was sound.
Nothing was edited to make it pass: no tool, no spec, no re-run under a
modified invocation. The `RESULT: PASS` in §3 is the spec's named Leg-B gate
and it is unmodified.

**What this evidence does not cover.** That the strip *renders* legibly is the
owner's eyes (DECISIONS §4.5) — no GUI was launched and no screen was captured.
The M13-RELEASE spec already records the owner's intent to re-look at
sparklines in production, and states nothing gates on it.

---

## 6. Transcription check

The §3 block was not retyped. The verifier's stdout was captured to a scratch
file at run time, and the block as extracted back out of this report was diffed
against it:

```
$ diff <(sed -n '/^PASS  step 1/,/^RESULT/p' reports/m13-release-rc.md) run1.txt
VERBATIM MATCH: OK   (22 lines)
```

---

## 7. State left behind, and what the top tier must decide

**State:**

- `main` = `d86cb5a` `release: v1.6.0`, plus this report commit. CI 8/8 green
  on `d86cb5a`.
- Tag `v1.6.0-rc.1` exists, points at `d86cb5a`, and its draft release exists,
  unpublished. **Neither was pruned** — pruning is Leg B's step, gated on
  `RESULT: PASS`.
- **No `v1.6.0` tag exists. Nothing is published.** `publishedAt` is `null`.
- No code, `.github/`, `assets/`, `README.md`, `tools/`, `prompts/` or §4.1
  path was touched. No dependency added. This report is the leg's entire
  footprint. `git status` is clean apart from it.
- The local `cargo build --release` run in §5 wrote only to `target/`
  (gitignored); `Cargo.lock` was protected by `--locked` and is unchanged.
- Scratch artifacts live in `mktemp -d` directories outside the repository;
  nothing was copied into the tree.

**Decisions that are not this session's:**

1. **The `24h` spot-check clause.** The evidence in §5 says the tag ships and
   the needle is invalid. Whether that closes the clause, whether the spec
   should assert something observable instead (a longer literal, or the
   `SPARK_TAG` unit test that already exists), and whether the release-verify
   standard should say anything about short literals under LTO — top tier,
   recorded like M12 §5. **Nothing in the artifact is known to be wrong.**
2. **Leg B remains unqueued**, as instructed. Leg B is queued only by the top
   tier, and no queue file for it exists. This session ends here.
