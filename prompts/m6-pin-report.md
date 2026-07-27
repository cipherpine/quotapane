# M6-PINS — supply-chain pin resolution report

Read-only research for the M6 release workflows. Produced by a floor-tier
session on **2026-07-27** against `main` @ `81bc17b`. **This session authored no
YAML and touched no `.github/**` path** — these values exist so the workflows can
be authored at the top tier from verified input rather than from recall.

`ci.yml`'s own header sets the requirement this report serves:

> *"Supply-chain note: actions are pinned by major tag and kept current by
> Dependabot (.github/dependabot.yml). For release workflows (M6), pin by full
> commit SHA."*

**Method.** Every value below carries the command or URL that produced it. All
`gh api` calls ran against authenticated `gh` as `cipherpine`. Nothing here is
recalled or inferred unless explicitly labelled as derived, and everything that
could not be resolved is in [UNRESOLVED](#unresolved) rather than guessed.

**Freshness caveat.** Tag→SHA bindings are true as of 2026-07-27. A major tag
(`v7`) can be moved by its maintainer at any time; a full SHA cannot. That
asymmetry is the entire point of pinning by SHA, and it also means this report
has a shelf life — re-run before authoring if significant time has passed.

---

## 1. Read these three first

Three findings change what the release workflow can look like, so they lead
rather than sit in a table.

### 1.1 Build provenance does not work on this repo today — BLOCKING

`cipherpine/quotapane` is **private** right now:

```
gh api repos/cipherpine/quotapane --jq '.owner.type, .private'
→ User, true
```

`actions/attest-build-provenance`'s own README states:

> *"If you are on a GitHub Free, GitHub Pro, or GitHub Team plan, artifact
> attestations are only available for public repositories. To use artifact
> attestations in private or internal repositories, you must be on a GitHub
> Enterprise Cloud plan."*
>
> — `gh api repos/actions/attest-build-provenance/contents/README.md`

SECURITY.md:104 promises artifacts are *"published with build provenance /
attestations."* On a Free/Pro/Team plan that promise **cannot be kept until the
repo is public** — which is Prompt G, the *last* prompt in the run order. So the
release workflow either lands after the public flip, or lands with the attest
step present but unexercised.

I could not read the account plan to confirm which case applies — see
[UNRESOLVED](#unresolved). **This needs an owner answer before Prompt D authors
the release workflow**, because it is a sequencing constraint on F and G, not a
detail of the YAML.

### 1.2 `attest-build-provenance` now says to use `actions/attest` instead

> *"As of version 4, `actions/attest-build-provenance` is simply a wrapper on top
> of `actions/attest`. Existing applications may continue to use the
> `attest-build-provenance` action, but new implementations should use
> `actions/attest` instead."*
>
> — same README

This repo is a new implementation. Both are resolved in the table below so the
top tier can choose; the publisher's own guidance points at `actions/attest`.

### 1.3 `dtolnay/rust-toolchain@stable` is a moving branch, and `@v1` is stale

The spec asked for a read on this, so it gets its own section — see
[§3](#3-cross-checks). Short version: `stable` is a **branch**, not a tag, and
the `v1` tag is **11 commits behind master** and has not moved since 2025-08-23.
Neither is a safe thing to inherit uncritically.

---

## 2. Action pins

Resolution method per action:

```sh
gh api repos/<owner>/<repo>/releases/latest --jq '.tag_name'
gh api repos/<owner>/<repo>/git/ref/tags/<tag>          # → object.type + object.sha
gh api repos/<owner>/<repo>/git/tags/<sha> --jq '.object.sha'   # if annotated, deref to commit
gh api repos/<owner>/<repo> --jq '.owner.type, .license.spdx_id'
```

All SHAs below are **full 40-character commit SHAs**, already dereferenced
through annotated tags where applicable.

| Action | Latest release | Full commit SHA to pin | Commit date | License | Publisher |
|---|---|---|---|---|---|
| `actions/checkout` | `v7.0.1` | `3d3c42e5aac5ba805825da76410c181273ba90b1` | 2026-07-17 | MIT | `actions` (Org, GitHub-owned) |
| `actions/upload-artifact` | `v7.0.1` | `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` | 2026-04-10 | MIT | `actions` (Org, GitHub-owned) |
| `actions/download-artifact` | `v8.0.1` | `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` | 2026-03-11 | MIT | `actions` (Org, GitHub-owned) |
| `actions/attest-build-provenance` | `v4.1.1` | `0f67c3f4856b2e3261c31976d6725780e5e4c373` | 2026-06-26 | MIT | `actions` (Org, GitHub-owned) |
| `actions/attest` *(wrapper target, §1.2)* | `v4.2.0` | `f7c74d28b9d84cb8768d0b8ca14a4bac6ef463e6` | 2026-07-16 | MIT | `actions` (Org, GitHub-owned) |
| `sigstore/cosign-installer` | `v4.1.2` | `6f9f17788090df1f26f669e9d70d6ae9567deba6` | 2026-05-07 | Apache-2.0 | `sigstore` (Org, third-party) |
| `dtolnay/rust-toolchain` | `v1` *(see §3.2)* | `e97e2d8cc328f1b50210efc529dca0028893a2d9` | 2025-08-23 | MIT | `dtolnay` (**User**, third-party) |

**Also already in `ci.yml`, not on the requested list** — reported because it is
part of the same supply chain and is a third-party action:

| Action | Latest release | Full commit SHA | Commit date | License | Publisher |
|---|---|---|---|---|---|
| `EmbarkStudios/cargo-deny-action` | `v2.1.1` | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` | 2026-07-13 | Apache-2.0 | `EmbarkStudios` (Org, third-party) |

`ci.yml` currently uses it as `@v2` — a **mutable annotated tag** (`v2` →
tag object `b66acf5e…` → commit `3c63498…`). Same moving-ref exposure as the
others; flagged so the top tier can decide whether the release workflow inherits
it or CI keeps the major-tag convention its header already permits.

### 2.1 Mutable major refs — which of these move

Every `actions/*` major tag is a **maintainer-moved alias**, proven by the major
tag and the point-release tag resolving to the identical commit:

```
gh api repos/actions/checkout/git/ref/tags/v7     → 3d3c42e5aac5ba805825da76410c181273ba90b1
gh api repos/actions/checkout/git/ref/tags/v7.0.1 → 3d3c42e5aac5ba805825da76410c181273ba90b1
```

| Ref | Kind | Moves? |
|---|---|---|
| `actions/checkout@v7` | lightweight tag | **Yes** — alias for v7.0.1 today |
| `actions/upload-artifact@v7` | lightweight tag | **Yes** — alias for v7.0.1 today |
| `actions/download-artifact@v8` | lightweight tag | **Yes** — alias for v8.0.1 today |
| `actions/attest-build-provenance@v4` | **annotated** tag | **Yes** — derefs to v4.1.1's commit |
| `actions/attest@v4` | **annotated** tag | **Yes** — derefs to v4.2.0's commit |
| `sigstore/cosign-installer@v4` | **does not exist** | n/a — see below |
| `dtolnay/rust-toolchain@stable` | **branch** (`refs/heads/`) | **Yes** — see §3.2 |
| `EmbarkStudios/cargo-deny-action@v2` | **annotated** tag | **Yes** — derefs to v2.1.1's commit |

**`sigstore/cosign-installer` publishes no major tag at all.** There is no
`refs/tags/v4`:

```
gh api repos/sigstore/cosign-installer/git/ref/tags/v4 → 404 Not Found
gh api repos/sigstore/cosign-installer/git/matching-refs/tags/v4
→ refs/tags/v4.0.0, v4.1.0, v4.1.1, v4.1.2   (exact point releases only)
```

Its branches are version-named (`0.1.0`, `1.0.0`, …, plus `main` and
`cosign-installer-v3`), so `@v4` would be an **invalid ref, not a floating
one** — a workflow written with `cosign-installer@v4` fails to resolve rather
than silently drifting. Pin `v4.1.2`'s commit.

---

## 3. Cross-checks

### 3.1 Is `v7` really the current major for `actions/checkout`? — Yes

```
gh api 'repos/actions/checkout/releases?per_page=6' --jq '.[] | .tag_name + "  " + .published_at'
→ v7.0.1  2026-07-20T15:10:05Z      ← newest
  v6.1.0  2026-07-20T15:23:28Z
  v5.1.0  2026-07-20T15:27:58Z
  v4.4.0  2026-07-20T15:36:10Z
  v3.7.0  2026-07-20T15:40:05Z
  v2.8.0  2026-07-20T15:43:41Z
```

Major tags `v1`–`v7` all exist; `v7` is the highest and `v7.0.1` is the newest
release, with no v8 prerelease. **`ci.yml`'s `actions/checkout@v7` is correct and
current.** Note the maintainers backport across all live majors on the same day —
so "v7 is newest" is about the major, not about who got the latest patch.

### 3.2 What does `dtolnay/rust-toolchain@stable` resolve to? — and my read

**`stable` is a branch, not a tag:**

```
gh api repos/dtolnay/rust-toolchain/git/ref/heads/stable → commit 4cda84d5c5c54efe2404f9d843567869ab1699d4
gh api repos/dtolnay/rust-toolchain/git/ref/tags/stable  → 404 Not Found
```

- `stable` HEAD = **`4cda84d5c5c54efe2404f9d843567869ab1699d4`**, dated
  2026-07-16, commit message literally `toolchain: stable`.
- The **only tag in the entire repo** is `v1` →
  `e97e2d8cc328f1b50210efc529dca0028893a2d9`, dated **2025-08-23**.

How the refs relate:

```
gh api repos/dtolnay/rust-toolchain/compare/master...stable
→ {"status":"ahead","ahead_by":1,"behind_by":0}
gh api repos/dtolnay/rust-toolchain/compare/master...e97e2d8…
→ {"status":"behind","ahead_by":0,"behind_by":11}
```

So `stable` is a **single-commit overlay on `master`**, re-based forward each
time `master` moves. The overlay's whole job is one line in `action.yml`:

| Ref | `toolchain` input |
|---|---|
| `stable` branch (`4cda84d`) | `required: false`, **`default: stable`** |
| `v1` tag (`e97e2d8`) | **`required: true`**, no default |

*(`gh api repos/dtolnay/rust-toolchain/contents/action.yml?ref=<sha>`)*

**My read** — input, not a decision:

1. **Do not inherit `@stable` in a release workflow.** It is a branch the author
   force-moves; a release built from it is not reproducible from its own YAML,
   and it is the one ref here that can change *content* under a name that looks
   version-like.
2. **Pinning `@v1`'s SHA is worse than it looks.** That tag is 11 commits and
   ~11 months behind `master` — dtolnay is *not* maintaining `v1` as a moving
   major alias the way `actions/*` do. Pinning it freezes you to Aug-2025 action
   code.
3. **Preferred: pin `4cda84d5c5c54efe2404f9d843567869ab1699d4`** — the commit
   `stable` points at today. A branch that moves does not stop the *commit* it
   currently names from being immutable. That gets current action code with a
   pin that cannot drift. Passing `toolchain:` explicitly then makes the
   `default: stable` irrelevant and the intent readable.
4. **Separate axis worth stating plainly:** pinning the *action* does not pin the
   *toolchain*. `toolchain: stable` resolves through rustup at run time, so two
   releases built from byte-identical YAML can use different rustc. That is
   precisely why SECURITY.md:105's *"the exact toolchain version is documented per
   release"* has to be satisfied by **capturing `rustc -V` during the run**, not
   by reading it off the workflow file. If the owner wants build-to-build
   determinism instead, `toolchain:` must name an exact version (e.g. `1.97.1`),
   which turns every Rust release into a deliberate repo change.

---

## 4. Secret scanning

### 4.1 Does `gitleaks/gitleaks-action` require a paid license here? — No, and the key is free anyway

**The repo owner is a User account, not an Organization:**

```
gh api repos/cipherpine/quotapane --jq '.owner.login, .owner.type' → cipherpine, User
```

From the action's own EULA (`gh api repos/gitleaks/gitleaks-action/contents/LICENSE.txt`):

> *"If you are using the Software to scan repositories owned by an Organization
> Account … then you must obtain a License Key. If you are using the Software to
> scan repositories owned by a Personal Account … then **no License Key is
> required**. License Key requirements are automatically enforced by the
> Software."*

And from the README (`…/contents/README.md`):

> *"`GITLEAKS_LICENSE` (required for organizations, not required for user
> accounts)"* · *"If you are scanning repos that belong to a personal account,
> then no license key is required."*

**So: no license key needed for `cipherpine/quotapane` today, and even for orgs
the key is free** (README: *"Do I need a free license key?"* — obtained via a
form at gitleaks.io requiring name/email/company). The premise of "paid" does not
hold in either case.

**But the licensing story is not clean, and this repo cares about licenses.**
Since v2.0.0 the action left MIT for a proprietary EULA (GitHub reports
`NOASSERTION`):

> *"Since v2.0.0 of Gitleaks-Action, the license has changed from MIT to a
> license (LICENSE.txt). Prior versions to v2.0.0 … remain under the MIT
> license."*

That EULA forbids modification and derivative works (§1.4, §2.1(d)), and gates
use on license logic enforced at runtime by the vendor. For a project whose
headline claim is a small auditable surface and whose `deny.toml` polices
licenses down to a per-crate MPL-2.0 exception, adopting a proprietary,
non-modifiable, vendor-gated CI dependency is a posture change worth deciding
deliberately — **not a mechanical pin**. Note also that the User-vs-Org
distinction is enforced by the vendor's software, so it is a behavior that could
change, not a contractual guarantee to this repo.

### 4.2 The release-binary alternative

The `gitleaks` **CLI** is a different repo with a different license:

```
gh api repos/gitleaks/gitleaks --jq '.license.spdx_id' → MIT
gh api repos/gitleaks/gitleaks/releases/latest --jq '.tag_name, .published_at'
→ v8.30.1, 2026-03-21T02:17:58Z
```

| Field | Value |
|---|---|
| Release tag | **`v8.30.1`** (newest; next-newest `v8.30.0`, 2025-11-26) |
| Asset filename | **`gitleaks_8.30.1_linux_x64.tar.gz`** |
| Asset size | 8,230,402 bytes |
| **Published SHA256** | **`551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb`** |
| Checksums file | `gitleaks_8.30.1_checksums.txt` |
| License | **MIT** |

*(linux_arm64, same release, for reference:
`e4a487ee7ccd7d3a7f7ec08657610aa3606637dab924210b3aee62570fb4b080`)*

**I verified this checksum rather than merely transcribing it** — downloaded the
asset and hashed it:

```sh
curl -sL -o gl.tar.gz https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/gitleaks_8.30.1_linux_x64.tar.gz
sha256sum gl.tar.gz
→ 551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb   # MATCH, 8230402 bytes
```

### 4.3 Which approach for a full-history scan? — Recommend the pinned binary

Both scan full history equally well; the action is a wrapper around this same
CLI, and either way `fetch-depth: 0` is what makes full history available. The
tie is broken on everything else:

| | `gitleaks-action` | Pinned release binary |
|---|---|---|
| License | Proprietary EULA, no modification | **MIT** |
| Moving parts | Action ref + wrapper + vendor license check at runtime | **One tarball + one SHA256** |
| Pin granularity | Action SHA (CLI version chosen by the wrapper) | **Exact CLI version, chosen here** |
| Failure modes | Ref drift, vendor licensing-logic change, wrapper behavior change | Asset 404 (fails loud) |
| Audit story | "Trust the wrapper and the vendor" | "This hash, or the build stops" |

**Recommendation — the release binary, and not by a narrow margin.** It replaces
a proprietary runtime-licensed dependency with an MIT binary whose exact bytes
are asserted by a hash in the repo. Verifying a pinned SHA256 before executing is
the same discipline SECURITY.md asks *users* to apply to QuotaPane's own
releases; using it in our own CI is consistent rather than novel. It also drops
one third-party action from the release surface entirely, which is the direction
the trust-boundary thesis points.

The cost is honest and small: the version no longer auto-updates, so a human
bumps the tag and hash together. For a supply-chain-sensitive tool that is a
feature, not a chore — and it is the only option where a silent upstream change
cannot alter what runs against full history.

**This is input, not a decision.** If the owner prefers the action, the
User-account path needs no key and works today.

---

## 5. Release surface

### 5.1 Exact `cargo build --release --locked` output paths

Measured on this machine, after the D3 rename, from **cargo's own machine-readable
output** (not from `ls`):

```sh
cargo build --release --locked --message-format=json | <filter reason=compiler-artifact, executable != null>
→ quotapane-cli -> C:\dev\QuotaPane\QuotaPane\target\release\quotapane-cli.exe
  quotapane     -> C:\dev\QuotaPane\QuotaPane\target\release\quotapane.exe
```

| Runner | Path (relative to `$GITHUB_WORKSPACE`) |
|---|---|
| `windows-latest` | `target\release\quotapane.exe`<br>`target\release\quotapane-cli.exe` |
| `ubuntu-latest` | `target/release/quotapane`<br>`target/release/quotapane-cli` |

The **Windows paths are measured**. The **Linux paths are derived** — Cargo omits
the executable suffix on non-Windows targets, and the binary names come from the
`[[bin]] name` fields landed in `5e5832d`, which are platform-independent. I did
not build on Linux, so the Linux rows are a derivation from a measured Windows
result plus the manifest, not an observation. Flagged rather than presented as
measured.

**Stale-artifact warning for whoever writes the upload globs:** `target/release/`
on a machine that built before the rename still contains `usage-ui.exe` and
`usage-cli.exe` from the old names — cargo does not remove them. A glob like
`target/release/*.exe` would sweep up pre-rename binaries. On a clean CI runner
this cannot happen, but it is a real hazard for any local packaging step, and it
argues for naming the two artifacts explicitly rather than globbing.

### 5.2 `rustc -V` / `cargo -V` on the runners

`windows-latest` → **Windows Server 2025**; `ubuntu-latest` → **Ubuntu 24.04**
(`gh api repos/actions/runner-images/contents/README.md`).

Preinstalled on both images (`…/contents/images/windows/Windows2025-Readme.md`,
`…/contents/images/ubuntu/Ubuntu2404-Readme.md`) — **identical on both**:

```
Rust 1.97.1 · Cargo 1.97.1 · Rustdoc 1.97.1 · Rustup 1.29.0 · Rustfmt 1.9.0
```

Current rustup `stable` channel, which is what `dtolnay/rust-toolchain@stable`
installs over the top (`curl -sL https://static.rust-lang.org/dist/channel-rust-stable.toml`):

```
[pkg.rust]  version = "1.97.1 (8bab26f4f 2026-07-14)"
[pkg.cargo] version = "0.98.0 (c980f4866 2026-06-30)"
```

So today the installed toolchain and the preinstalled one agree at **1.97.1**, and
`rustc -V` prints exactly `rustc 1.97.1 (8bab26f4f 2026-07-14)`.

For contrast, this machine is one patch behind — `rustc 1.97.0 (2d8144b78
2026-07-07)` / `cargo 1.97.0 (c980f4866 2026-06-30)` — which is itself the
argument for §3.2 point 4: **the version must be captured at build time, not
assumed.** Note the manifest's `pkg.cargo` version (`0.98.0`) is cargo's internal
crate version; `cargo -V` reports the Rust release number instead. The exact
`cargo -V` string on the runner is in [UNRESOLVED](#unresolved).

**Shape the workflow needs:** run `rustc -V` and `cargo -V` on each runner after
toolchain install and capture both into the release notes / a per-release
metadata file, per SECURITY.md:105. Because it varies by runner and by day, it
has to be recorded per artifact, not written once into a document.

### 5.3 Files a 0.1.0 → 1.0.0 bump must touch

```sh
grep -rn --exclude-dir=target --exclude-dir=.git "0\.1\.0" .
```

| File | Line | Action |
|---|---|---|
| `Cargo.toml` | 10 — `[workspace.package] version = "0.1.0"` | **Edit — the single source of truth** |
| `Cargo.lock` | 2643, 2651, 2661 — the `usage-cli` / `usage-core` / `usage-ui` entries | **Regenerated** by any cargo command; commit the result |
| `Cargo.lock` | **184 — `byteorder-lite` v0.1.0** | **DO NOT TOUCH** — unrelated third-party crate that coincidentally matches |

**Nothing else hardcodes the version.** All three crates use
`version.workspace = true`, and the only source references are derived at compile
time:

```
crates/usage-cli/src/main.rs:193   println!("quotapane-cli {}", env!("CARGO_PKG_VERSION"));
crates/usage-cli/tests/cli.rs:63   stdout.contains(env!("CARGO_PKG_VERSION"))
```

So `--version` and its test follow the bump automatically — no third place to
forget.

**Adjacent inconsistency, not part of the bump but found while proving it:**
`README.md:26` says *"Requires Rust 1.85+"* while `Cargo.toml:14` sets
`rust-version = "1.92"`. The README understates the real floor by seven minor
versions. Prompt E territory; recording it here so it is not lost.

### 5.4 Does `strip = true` interfere with provenance attestation? — No, with one ordering caveat

`Cargo.toml` sets `[profile.release] strip = true` (plus `lto = true`,
`codegen-units = 1`).

What the attestation actually binds, per the action's README:

> *"Attestations bind some subject (a named artifact along with its digest) to a
> SLSA build provenance predicate using the in-toto format."*

The digest is computed over the artifact file **at attest time**. `strip` is
applied by the linker *during* the build, so the file sitting at
`target/release/quotapane.exe` when the attest step runs is already stripped —
there are not two versions of the binary, and the attestation digests the same
bytes that ship. **No interference.**

**The caveat is ordering, not stripping.** The attestation is only meaningful if
nothing modifies the artifact between attest and publish. Any step that rewrites
the binary after attestation (re-signing in place, compression, an Authenticode
step on Windows) invalidates the digest. `cosign sign-blob` produces a *detached*
signature and does not modify the input, so cosign-then-attest and
attest-then-cosign are both safe in that respect — but a Windows code-signing
step, if one is ever added, is exactly the kind of in-place rewrite that would
break it. Build → attest → upload the same bytes.

**Limits of this answer, stated plainly:** the ordering reasoning is grounded in
the documented digest semantics quoted above, not in an observed run. I did not
execute an attestation — that needs a workflow on a runner (out of scope here),
and per §1.1 it cannot run on this repo at all while it is private on a
non-Enterprise plan. If the owner wants empirical confirmation, it has to come
after the public flip.

---

## UNRESOLVED

Recorded rather than guessed, per the method note.

1. **The GitHub plan for `cipherpine`** — decides whether build provenance (§1.1)
   works before the public flip. `gh api user --jq '.plan.name'` returned no
   readable plan under the current token's scopes. **The owner can answer this
   from account settings in seconds, and it blocks Prompt D's shape.** If the plan
   is Free/Pro/Team, attestation is impossible until Prompt G makes the repo
   public.
2. **The exact `cargo -V` string on the runners.** The runner-images README states
   "Cargo 1.97.1", but the full parenthesised hash/date suffix that `cargo -V`
   prints is not published there, and the stable channel manifest reports cargo's
   internal crate version (`0.98.0`) rather than the string the binary emits. The
   `rustc -V` string *is* resolved exactly. Resolving this needs one command on a
   runner — which the release workflow will capture anyway (§5.2).
3. **Linux release artifact paths are derived, not measured** (§5.1). Confident —
   it follows from Cargo's documented suffix behavior and the `[[bin]]` names —
   but this session built only on Windows, so it is labelled rather than asserted.
4. **Whether `dtolnay/rust-toolchain`'s `v1` tag will ever move again.** It has
   been static for ~11 months while `master` advanced 11 commits (§3.2). Whether
   that is deliberate policy or abandonment is not stated anywhere I could find,
   so the §3.2 recommendation deliberately does not depend on it.
5. **No empirical attestation run** (§5.4) — blocked by items 1 and repo privacy.

---

## Scope note

This session ran read-only apart from creating this file: no `.github/**` edit,
no YAML, no dependency change, no workflow run, no PR, and nothing touching real
credentials. The gitleaks asset was downloaded to a scratch directory outside the
repo purely to verify its checksum, and deleted afterward. The `cargo build
--release` run was a local measurement for §5.1 and produced no committed
artifact.

Everything above is input for the top tier, which owns the `.github/**` bytes.
Two items want an owner decision before those bytes are authored: **the plan
question (§1.1)** and **the gitleaks approach (§4.3)**.
