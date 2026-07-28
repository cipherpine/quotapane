# M6-PUBLIC — full-history secret scan and exposure inventory

Produced 2026-07-28 by a floor-tier session executing `prompts/m6-public-flip.md`
(Prompt G, phase 1). **Read-only**: nothing in the repository was changed by this
scan; this file is its only output.

Scanned state: `main` = `86b6792` (phase 0's own commit), plus every other ref.
**41 commits across all refs**, 146 unique blobs, 56 distinct file paths.
Per §4.4, no candidate secret value appears anywhere in this report — findings
are recorded as path + location + key **name** + disposition.

---

## VERDICT

**No credential material was found anywhere in this repository's history.**
Both passes are clean, and they agree.

The stronger finding, which is what actually decides D7:

> **No file has ever been deleted or renamed in this repository.**
> 56 file paths have existed across all refs; 56 file paths exist in the current
> tree; the two sets are *identical*. `git log --all --diff-filter=D` and
> `--diff-filter=R` both return nothing.

So the history contains no removed file to recover. A public clone reveals
**exactly the current tree, plus the edit history of those same files**. There is
no deleted-secret problem here, because there are no deleted files at all.

What remains is therefore not a secrets question but a **disclosure** question:
three things publish that are not code, listed under "Exposure inventory" below.
None is a credential. All three are the owner's call, not a security defect.

---

## Pass A — pinned `gitleaks` binary, run locally

| | |
|---|---|
| Version | **8.30.1** — the version pinned in `.github/workflows/ci.yml` |
| Command | `gitleaks git --no-banner --redact <repo>` (identical to the CI job) |
| Result | **41 commits scanned, ~692 KB, `no leaks found`** |
| Exit status | **0** |

### Binary provenance (a deviation, verified rather than waved through)

`ci.yml` pins the **linux_x64** asset by SHA256, because CI runs on
`ubuntu-latest`. This session runs on Windows, and the only installed WSL distro
is Docker Desktop's utility image — not a usable general-purpose Linux. Rather
than skip the pin or trust an unverified download, the Windows asset was verified
through a chain anchored in `ci.yml`'s own pinned value:

1. Downloaded `gitleaks_8.30.1_linux_x64.tar.gz`; its SHA256 is
   `551f6fc8…2470eb` — **byte-identical to the pin in `ci.yml`**. This proves the
   release being drawn from is the exact one CI uses.
2. Downloaded `gitleaks_8.30.1_checksums.txt` from that same release and confirmed
   it lists that identical linux hash — which anchors the checksums file to the
   pin.
3. Verified `gitleaks_8.30.1_windows_x64.zip` against **its** entry in that
   now-anchored checksums file (`d29144de…2afc4e`), and ran that binary.

Same version, same release, same publisher, every hop checksum-verified. The
executed binary is a different *platform build* than CI's — that is the deviation,
and it is recorded here rather than hidden. Both runs agree: no leaks.

---

## Pass B — independent scan for *this project's* credential shapes

Pass A is a general-purpose scanner with generic rules. Pass B is the second
opinion it cannot give: the exact shapes this codebase parses, run over **every
blob that has ever existed on any ref** (146 unique blobs — the blob set is
complete by construction, since every version of every file is reachable from
some ref, deleted files included).

### Value-shaped patterns

| Pattern | Matching blobs | Disposition |
|---|---|---|
| `sk-ant-` (bare prefix) | 11 blobs, 17 lines | **All documentation prose.** See below. |
| `sk-` + 20 or more token-safe chars | **1 blob, 1 line** | **Synthetic test fixture.** See below. |
| `Bearer ` + 20 or more token chars | **0** | — |
| JWT shape (`eyJ…​.…`) | **0** | — |

**The `sk-ant-` hits are provably not tokens.** Every one of them matched the
bare-prefix pattern but **failed** the "prefix followed by 20+ token characters"
pattern — so by construction none is followed by token material. Their locations
confirm it: `ARCHITECTURE.md` (ADR-002's discussion of admin keys),
`SECURITY.md` (the "never ingests an org admin key" paragraph), and
`prompts/m6-prompts-b-to-g.md` (which lists `sk-ant-` as a *pattern to scan for*).
All appear inside backticks, alongside an ellipsis and the word "admin" — i.e.
`sk-ant-admin…`-style placeholders in prose about the key this project
deliberately refuses to hold.

**The single value-shaped hit is a declared fixture.**
`crates/usage-core/src/credentials/mod.rs:113`, constant **`SYNTHETIC_TOKEN`**,
inside the `#[cfg(test)]` module that begins at line 106, directly under a doc
comment reading *"A synthetic token for tests. NEVER place a real credential in
this…"*. Its value is self-describing placeholder text; it is not quoted here on
principle. **Disposition: synthetic, expected, compliant with `CLAUDE.md`'s
"test fixtures use synthetic tokens only".**

### Credential key names and credential files

Key names taken from the actual parse structs, not from memory:
`claudeAiOauth`, `accessToken`, `expiresAt` (Claude, `RawCredentials`/`RawOauth`);
`tokens`, `access_token`, `account_id` (Codex, `RawAuth`/`RawTokens`); plus
`refreshToken`, `refresh_token`, `id_token`, `OPENAI_API_KEY`.

| Check | Result |
|---|---|
| Any path in history ever named `*.credentials.json`, `auth.json`, `.env*`, `*.pem`, `*.key` | **NONE.** (The one regex hit was `crates/usage-core/src/credentials/secret.rs` — a source file.) |
| File types containing those key names | **`.rs` only** — never a `.json` document |
| Any blob that is a JSON credential *document* | **NONE.** Three blobs looked like candidates; all three are `.rs` provider sources. |

Those three candidates were examined individually
(`claude_subscription.rs`, `codex_subscription.rs`, two historical versions): the
key-name occurrences before the `#[cfg(test)]` line are **module doc comments**
describing the wire shape, and those after it are **synthetic-marked test
fixtures**. All three contain **zero** token-shaped values.

`_claude_setup/` has **never been committed on any ref** — zero paths in history,
zero log entries. This independently confirms gap-report finding L1.

`.gitignore` has covered `.credentials.json`, `auth.json`, `*.pem`, `*.key`,
`.env`, `.env.*`, and `/_claude_setup/` throughout.

---

## Exposure inventory — what a public clone retrieves

**Files that existed in history and are gone from the tip: NONE.** The inventory
below is therefore about what is *in* the tree and publishes with it, plus commit
metadata.

### 1. Commit metadata — the owner's personal e-mail address (39 commits)

All 39 human commits are authored **and** committed by
`justinparsons919 <justin.parsons919@gmail.com>`. On a public repo this address
is permanently visible via the API and the `.patch` endpoint, and is routinely
harvested. The remaining 2 commits are `dependabot[bot]`.

This is the most concrete privacy exposure in the repository, and it is **not**
fixable by editing files — it lives in commit objects. Options: accept it; switch
future commits to a GitHub `noreply` address; or rewrite history (which changes
every SHA, including the ones referenced throughout `prompts/` and `DECISIONS.md`).

### 2. `Claude-Session:` URLs in 27 commit bodies

Four distinct session URLs appear in commit trailers. They resolve only for the
authenticated account, so they are not credentials — but they are internal tooling
identifiers that will be public and are not meaningful to any reader.

### 3. `prompts/` — 18 files, 238 KB (the D4-final question)

The full development record: goal prompts, the ship program, the dependency
reviews, the pin report, and the gap report. Points the owner should weigh:

- **`m6-gap-report.md` is a catalogue of this project's own defects** — 79
  findings. Nearly all are now closed (Prompts B/D/E), but it is a precise map of
  what was recently wrong, including the test-harness blind spots.
- **`m6-prompts-b-to-g.md`, `m6-launchers.md`, `DECISIONS.md`** describe the
  autonomy charter, hard-stop conditions, and model-routing policy — i.e. how this
  repo is developed and where its review gates are.
- **Absolute local paths** (`C:\dev\QuotaPane\QuotaPane`) appear in 16 prompt
  files. They reveal a directory layout but contain no username — no `C:\Users\…`
  path appears anywhere in the tree.
- Publishing them is a *positive* signal for an auditability-first project: it
  shows the security review actually happened. That is a real argument for
  keeping them, not merely a mitigation.

### 4. Small items

- `crates/usage-core/src/providers/codex_subscription.rs:401` — a test comment
  reads *"The exact shape Justin's account returned"*. A first name in a public
  source file, tied to a real account's API response. The fixture itself is
  PII-free and a test (`per_model_windows_parse_and_carry_no_pii`) enforces that.
  Trivially reworded if the owner wants it gone.
- `.claude/` (3 agent definitions + `settings.json`, which is just
  `{"model": "opus"}`) and `.cargo/audit.toml` publish. Nothing sensitive.
- Every e-mail-shaped string in the tree is a synthetic test domain
  (`example.com`, `example.invalid`, `evil.com`). No real address in file content.

---

## Argued read: is this history safe to publish?

**Yes — on the secrets question, with high confidence.** The reasoning, not just
the verdict:

1. Two independent scanners agree on zero credential material — one general
   (gitleaks 8.30.1, the CI-pinned version, verified), one written specifically
   against this codebase's own parse structs.
2. Pass B's coverage is **complete by construction**, not by sampling: it read
   every blob reachable from every ref, which is every version of every file that
   has ever existed.
3. The usual reason a history scan finds something — *a secret committed and later
   deleted* — **cannot apply here**, because no file has ever been deleted or
   renamed. This is the single most reassuring fact in this report and it is
   independently verified three ways (path-set equality, `--diff-filter=D`,
   `--diff-filter=R`).
4. The one value-shaped match in all of history is a constant named
   `SYNTHETIC_TOKEN` in a test module, under a comment forbidding real
   credentials.
5. No credential-shaped filename has ever existed on any ref, and `.gitignore` has
   guarded those names throughout.

**Where I am less than certain, stated plainly:** both passes are pattern-based.
A credential in a shape neither pass anticipated — a bare high-entropy string with
no recognizable prefix, key name, or JWT structure — would evade both. I did not
run a generic Shannon-entropy sweep, which is the one technique that could catch
that class. Given that no file was ever deleted, that every blob's path is a
source, doc, or config file whose contents are readable and accounted for, and
that gitleaks' own high-entropy rules found nothing, I judge that residual risk
**low but not zero**.

**The disclosure question is a separate decision and is the owner's.** Nothing in
§"Exposure inventory" is a security defect; items 1 and 3 are choices about
personal information and process transparency. My read: item 1 (the e-mail
address) is the one that cannot be undone after the flip and deserves an explicit
decision *before* it, not after.

### What this means for D7

A fresh repository would buy exactly three things — dropping the commit e-mail,
the `Claude-Session` URLs, and `prompts/` history — at the cost of destroying the
verifiable development record that is arguably this project's best evidence for
its own security claims, and invalidating every commit SHA cited across
`DECISIONS.md` and `prompts/`.

**It would not remove a single secret, because there is not one to remove.**
Rename-in-place is therefore the technically indicated option; the case for a
fresh repo rests entirely on the owner's view of items 1–3, not on risk.
