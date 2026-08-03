# M11-VERIFY — adversarial verification end-gate report

**Session:** floor (Opus, Claude Code), 2026-08-03.
**Subject:** M11 process-hardening slice — commits `e09d702` (m11a invariant
manifest), `c5a0eec` (m11b release-verify), `fbcd892` (m11c release template),
`72043dd` (m11d handover automation), launched by `10e17ce`.
**Tree verified:** `10e17cebc1fc99cc811ef4af89e0c46349c7397d` (`prompts: M11-VERIFY
launcher`, 2026-08-03T22:40:33Z), clean.
**Toolchain:** rustc 1.97.0 (2d8144b78 2026-07-07), cargo 1.97.0, Python 3.14.4,
gh 2.92.0, cosign v3.1.2, host MINGW64_NT-10.0-26200.
**Mandate:** verify only. Nothing on `main` was changed. Every issue found is
reported below, not fixed — per the launcher and §4.1.

**Verdict: M11 does what it claims, on every point the spec asked me to test.**
All four required mutations were caught with the exact error the spec predicted;
the release verifier passed end to end. Three findings are recorded in §6 — all
are *scope* limits of the new machinery rather than defects in it, and none
invalidate a result above. Acceptance is the owner's.

---

## 1. §3 verification bar on main as-is

Run in the canonical §3 form, which is a strict superset of the launcher's
wording (`--all` and `--locked` added):

| Command | Exit |
|---|---|
| `cargo fmt --all --check` | **0** |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | **0** |
| `cargo test --workspace --locked` | **0** |

Clippy emitted no warnings (`Finished dev profile ... in 7.17s`, no diagnostics).
Test totals, per binary:

```
     Running unittests src\main.rs (quotapane_cli)
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running tests\cli.rs (cli)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running unittests src\lib.rs (usage_core)
test result: ok. 113 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
     Running unittests src\main.rs (quotapane)
test result: ok. 131 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
   Doc-tests usage_core
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

40 + 8 + 113 + 131 = **292 tests, 0 failed, 0 ignored** — matches the expected
count exactly.

The M11a code changes were independently confirmed to be comment-only:
`git diff e09d702~1 e09d702 -- crates/` is 22 added lines, every one of them a
`// INV:<ids> — registered in invariants.manifest (checked in CI)` comment. No
statement, signature, or attribute was touched. The bar holding is therefore the
expected result, not a coincidence.

## 2. Checker baseline on main

```
$ python3 tools/check-invariants.py
OK: 7 invariants, 24 test bindings, tags and manifest set-equal, SECURITY.md id set matches.
EXIT: 0
```

The binding count is arithmetically consistent with the manifest: inv 1 (1) +
inv 2 (8) + inv 3 (4) + inv 5 (1) + inv 6 (1) + inv 7 (9) = 24. Invariant 4 is
`kind: absence` with no test, enforced by CI job `no-telemetry`.

## 3. Mutation testing of the checker

Performed on scratch branch `scratch/m11-mutation`, cut from `10e17ce`. No commit
was ever made on it, it was never pushed, and it was deleted at the end (§7 has
the restoration proof). Each mutation was applied, the checker run, then reverted
with `git checkout --` and the baseline re-confirmed `OK` before the next.

| # | Mutation | Caught? | Exit |
|---|---|---|---|
| a | delete a tagged test function (`explicit_zeroize_wipes_contents`, secret.rs) | **yes — 2 violations** | 1 |
| b | delete a `test:` line from `invariants.manifest` | **yes — tag side** | 1 |
| c | remove a `// INV` tag from source | **yes — manifest side** | 1 |
| d | renumber a SECURITY.md invariant (7 → 8) | **yes — id drift** | 1 |

Exact error lines produced:

**(a)** deleted source lines 100–110 of
`crates/usage-core/src/credentials/secret.rs` (tag + `#[test]` + fn body):

```
FAIL: invariant traceability broken (2 violation(s)):
  - invariant 2: fn explicit_zeroize_wipes_contents not found in crates/usage-core/src/credentials/secret.rs
  - manifest lists crates/usage-core/src/credentials/secret.rs::explicit_zeroize_wipes_contents for invariant 2 but the source carries no matching // INV:2 tag
```

Both halves of the checker fire independently — the existence check (§4) and the
set-equality check (§5). The test is named explicitly in each.

**(b)** deleted manifest line 38,
`test: crates/usage-core/src/egress/mod.rs::allowlisted_hosts_pass_host_check`:

```
FAIL: invariant traceability broken (1 violation(s)):
  - source tags crates/usage-core/src/egress/mod.rs::allowlisted_hosts_pass_host_check with // INV:3 but the manifest has no such entry
```

**(c)** deleted source line 79 of `secret.rs`, the `// INV:2` tag above
`debug_and_display_are_redacted` (fn body untouched):

```
FAIL: invariant traceability broken (1 violation(s)):
  - manifest lists crates/usage-core/src/credentials/secret.rs::debug_and_display_are_redacted for invariant 2 but the source carries no matching // INV:2 tag
```

**(d)** renumbered the SECURITY.md list item `7. **Proxy is opt-in…` to `8.`,
text otherwise byte-identical:

```
FAIL: invariant traceability broken (1 violation(s)):
  - id drift: SECURITY.md lists invariants [1, 2, 3, 4, 5, 6, 8] but the manifest lists [1, 2, 3, 4, 5, 6, 7]
```

**All four required mutations were caught, each with the specific error the
launcher predicted.** No required mutation was missed.

## 4. Beyond-spec adversarial probes

The launcher's four mutations all break *naming*. I additionally probed the class
the checker cannot see by construction — mutations that leave every name in place.
These are the source of findings F1 and F2.

| # | Probe | checker | fmt | clippy | test | Detected by anything? |
|---|---|---|---|---|---|---|
| e2 | comment out a whole tagged test, rustfmt-stable indentation | OK (0) | 0 | 0 | 0 | **no** |
| f | add `#[ignore]` to a tagged test (one line) | OK (0) | 0 | 0 | 0 | **no** |
| g | rewrite invariant 3's *claim text* in SECURITY.md, keep its number | OK (0) | — | — | — | **no** |

Detail:

**(e2)** The invariant-2 test `explicit_zeroize_wipes_contents` was commented out
in full — tag, `#[test]`, and body — preserving block indentation so rustfmt is
satisfied. The checker still reports `OK: 7 invariants, 24 test bindings, …`,
because its regexes match `fn explicit_zeroize_wipes_contents(` and `// INV:2`
inside comment text. `usage_core` dropped from 113 to **112** passing tests
(workspace 292 → 291) and every gate stayed green.

*(A first attempt that commented out the block at column 0 was caught — by
`cargo fmt --all --check`, exit 1, on comment indentation only. That is an
incidental catch, not coverage: correcting the indentation defeats it, which is
what e2 shows.)*

**(f)** Inserting a single `#[ignore]` line after `#[test]` produced:

```
test result: ok. 112 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
```

Checker `OK`, fmt 0, clippy 0, `cargo test` exit 0. One added line silently
retires a trust-boundary test; the only trace is an `ignored` count nothing
asserts on.

**(g)** SECURITY.md invariant 3 was rewritten from *"Deny-by-default egress …
A request to any other host is a hard error"* to *"Best-effort egress hygiene.
Outbound requests usually pass through a single client. Unrecognized hosts are
logged and allowed."* — its number left at 3. The checker reports `OK`. It
compares id **sets**, never claim text.

## 5. Release verifier dry run

Run verbatim in Git Bash, unmodified, against the published v1.4.1 release.

```
host: MINGW64_NT-10.0-26200  |  cosign: GitVersion: v3.1.2  |  gh: gh version 2.92.0 (2026-04-28)
$ tools/release-verify.sh v1.4.1
PASS  step 1: downloaded 4 assets for v1.4.1
PASS  step 2: sha256sum -c SHA256SUMS
PASS  step 3: cosign verify-blob (Verified OK)
PASS  step 4a: attestation quotapane-v1.4.1-x86_64-pc-windows-msvc.zip
PASS  step 4b: attestation quotapane-v1.4.1-x86_64-unknown-linux-gnu.tar.gz
PASS  R1: both attestation subjects digest-match disk
PASS  step 5: inventory + identical TOOLCHAIN.txt
PASS  step 6: shipped CLI runs (quotapane-cli 1.4.1)
PASS  NC1/R3: zip bytes changed (9ec2841822ba717b89fc6275e0a305e56ba951dbc3fc3f0fde27c874cf064a93 -> dff1a1ea03bab270248bb531df376005fb84b241e5bba94a848390da81c5b788)
PASS  NC1: checksum names the tampered file specifically
PASS  NC1: attestation refuses tampered zip
PASS    restored: quotapane-v1.4.1-x86_64-pc-windows-msvc.zip digest back to pristine
PASS  NC2: wrong identity rejected on the identity check specifically
PASS  NC3: wrong issuer rejected on the issuer check specifically
PASS  NC4/R3: SHA256SUMS bytes changed (8ee460b544101a9195bdddbcfa621fc7f96a0431fc3bb846d9467df367c96360 -> d7fda6baaa38b3e662e198251aad6c8c731671eb73581e9afba8df2d8116ec0b)
PASS  NC4: cosign rejects tampered SHA256SUMS on the signature check
PASS    restored: SHA256SUMS digest back to pristine
PASS  NC5/R4: wrong-repo target cli/cli confirmed to exist
PASS  NC5: real repo, real digest-miss (no attestations for our artifact)
PASS  NC6: unrelated file has no attestation
PASS  sweep: all artifacts pristine, steps 2-4 re-run clean
RESULT: PASS — v1.4.1: six steps, six controls, R1-R4
SCRIPT_EXIT=0
```

**RESULT: PASS**, exit 0. All six steps ran on this host (step 6 was executed, not
skipped — the Windows branch found `quotapane-cli.exe`), all six negative controls
asserted their specific error, R1–R4 all satisfied. The script did not fail to
run; no tooling STOP was triggered.

## 6. Findings

None of these blocked a result above, and per the launcher I fixed nothing.

### F1 — Tag/manifest coherence does not imply the test still runs (medium)

`tools/check-invariants.py` proves that a *name* exists and that the manifest and
tags agree about it. It cannot see whether the named test executes. Probes (e2)
and (f) each removed a trust-boundary test from the run — one by commenting it
out, one with `#[ignore]` — and the checker, `cargo fmt`, `cargo clippy` and
`cargo test` were all green (§4). Invariant 2 lost a binding with no signal
anywhere in the §3 bar or CI.

Why it matters here specifically: the checker's own header sells it as making
security claims "machine-checked against tests", and `SECURITY.md` §Security
invariants now tells the reader CI "fails on any drift between that file, this
list, and the tests in the tree". Silently retiring a test is drift the reader
would reasonably expect this job to catch, and it does not.

Cheapest closures I can see, for the top tier to weigh — **not** applied:
assert a `#[test]` attribute within the tag's lookahead window and reject
`#[ignore]` on a tagged test; and/or pin the workspace test total so 292 → 291
turns CI red. Both are changes to a §4.1 path and are the top tier's to author.

### F2 — The checker compares invariant id sets, never claim text (low–medium)

Probe (g): invariant 3 was rewritten in SECURITY.md from deny-by-default with a
hard error to "unrecognized hosts are logged and allowed" while keeping its
number, and the checker reported `OK`. A weakened, false security claim in the
project's authoritative security document passes the job whose stated purpose is
keeping that document honest against the code. The numbering is machine-checked;
the meaning is not. This is arguably inherent — no checker reads prose — but the
gap is worth stating explicitly in the manifest header so the guarantee is not
overread. Human review of SECURITY.md prose remains load-bearing and unautomated.

### F3 — `release-verify.sh` verdict claims "six steps" even when step 6 skipped (low, latent)

`tools/release-verify.sh:106-108` records a skip when the host is neither
MinGW/MSYS/Cygwin nor Linux:

```
note "step 6: SKIP — no binary for this host platform (record as skipped, not passed)"
```

The skip correctly does **not** increment `FAILURES`. But the verdict at
`:184` is unconditional:

```
echo "RESULT: PASS — $TAG: six steps, six controls, R1-R4"
```

On a macOS host (`uname -s` = `Darwin` matches neither branch, and the release
ships no macOS artifact), five steps run and the script still prints a verdict
claiming six. The author clearly anticipated the skip — the note says "record as
skipped, not passed" — but the machine-readable verdict does not carry it, so the
line a release spec pastes would overstate coverage.

Latent, not active: release specs pin Git Bash (`prompts/release-template.md`
Phase 2), and step 6 genuinely ran in this dry run. It cannot convert a failed
cryptographic control into a pass — `FAILURES` still governs the exit code. I
flag it because a verdict string that overclaims is precisely the class of drift
this project treats as a defect.

### Checked and clear

- **`--ignore-missing` scope (step 2/NC1).** A candidate concern that a
  `SHA256SUMS` entry could be silently skipped. Confirmed harmless: the file lists
  exactly the two archives, and step 1 hard-fails unless both are on disk before
  step 2 runs. Verified against the live release.
- **`invariants` job is genuinely gating.** It is not merely present in `ci.yml`;
  it is in the `protect-main` ruleset's required contexts as
  `invariants — manifest, docs, and tests agree` — now 8 required checks, up
  from the 7 recorded in DECISIONS.md §2.
- **No path filters on CI triggers.** `on: push: branches: [main]` /
  `pull_request:` with no `paths:` key, so the job cannot be skipped by a diff
  that touches only manifest or docs.
- **Checker directionality on relocation.** Tag discovery scans `crates/**/*.rs`
  only. Moving an invariant test outside `crates/` fails the manifest side rather
  than passing silently — the safe direction.

## 7. §4 log — protected paths, and proof nothing landed

Mutations (a)–(d) and probes (e2)/(f)/(g) all perturb §4.1 protected paths by
construction: `invariants.manifest`, `SECURITY.md`, and security-invariant tests
under `crates/usage-core/src/credentials/`. This is inherent to the assignment —
a checker over protected paths cannot be mutation-tested without perturbing them.
The launcher scoped it accordingly ("scratch branch, never pushed; delete after";
"you change nothing on main"), so nothing was *authored*: every perturbation was
throwaway, reverted immediately, and no bytes were committed anywhere.

Restoration proof, after the last probe:

```
$ git checkout -- SECURITY.md
$ git status --short          # empty
$ git checkout main
$ git branch -D scratch/m11-mutation
Deleted branch scratch/m11-mutation (was 10e17ce).
$ git rev-parse HEAD
10e17cebc1fc99cc811ef4af89e0c46349c7397d
$ git status --short          # empty
$ git ls-remote --heads origin | grep -i scratch
no scratch ref on origin
$ python3 tools/check-invariants.py
OK: 7 invariants, 24 test bindings, tags and manifest set-equal, SECURITY.md id set matches.
$ cargo fmt --all --check     # exit 0
```

The scratch branch was deleted still pointing at `10e17ce` — it never carried a
commit. `main` is byte-identical to its pre-verification state.

One process note for the record: while mutation (d) was applied, the harness
surfaced an automated file-change notice for `SECURITY.md` describing the edit as
intentional and advising against reverting it. That notice was triggered by my
own mutation write, carried no owner intent, and was disregarded; the file was
reverted as the spec requires. Flagged because an automated notice that reads
like authorization is exactly the sort of thing §4.7 says to distrust.

## 8. Deviations from the launcher

- **§3 bar run in the stricter canonical form.** The launcher wrote
  `cargo fmt --check` and clippy without `--locked`; I ran DECISIONS §3's
  `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --
  -D warnings`, and `cargo test --workspace --locked`. Strict superset, same
  green result.
- **Three probes beyond the four required mutations** (§4). Added under the
  launcher's own standard that "a mutation the checker misses is a finding"; they
  are the sole basis for F1 and F2.
- Nothing else. No fix, patch, or workaround was applied anywhere.

## 9. What the owner must do next

1. **Accept or reject M11** — acceptance is yours; this report claims only that
   the slice was verified, not that it is accepted.
2. **Rule on F1** — whether the `invariants` job should also prove tagged tests
   *run* (`#[test]` assertion + `#[ignore]` rejection, and/or a pinned test
   total). This is the only finding with real teeth. It is a §4.1 authoring
   change and needs the top tier.
3. **Rule on F2 and F3** — both are honesty-of-claim items: whether the manifest
   header should state that prose is not machine-checked, and whether the
   `RESULT:` line should name the steps actually run. Neither is urgent.
4. **Optionally update DECISIONS.md §2** — the recorded "7 status checks" is now
   8 with `invariants` added to the ruleset.
</content>
</invoke>
