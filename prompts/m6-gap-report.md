# M6-PREP — release-readiness gap report

Produced 2026-07-26 by a floor-tier (Sonnet 5) session executing
`prompts/m6-prep-audit.md`. Read-only audit: **nothing in this report was
fixed.** Every finding records the check that established it.

Tree audited at `main` = `a3cb497` (this report's parent), which is
`7e72282` + the two Phase 0 commits (`e608a58` `--debug-raw`, `a3cb497`
prompt docs). CI green on `7e72282`
(<https://github.com/cipherpine/quotapane/actions/runs/30175191233>).

## How to read this

| Field | Meaning |
|---|---|
| **Severity** | `blocks-public` — false or broken in a way a reader could rely on; must close before the visibility flip. `should-fix` — wrong but not load-bearing. `cosmetic` — tidiness. |
| **§4.1** | Which protected path a *fix* would have to touch. `—` means a floor session can fix it. |
| **Check** | The command or read that established the reality. A claim I only *suspect* is wrong is in §M, not here. |

### Counts

79 numbered entries `G01`–`G79`, of which **70 carry a severity**:

| Severity | Count |
|---|---|
| `blocks-public` | 32 |
| `should-fix` | 32 |
| `cosmetic` | 6 |
| **Severity-rated total** | **70** |

The 9 un-rated entries are: `G54` (a deliberate **negative result** — there is
only one version claim in the tree) and `G60`–`G67`, the eight user-visible
strings carrying the product name. Those eight are sub-items of the rename
rather than independent defects, so rating them separately would double-count
the rename; as a group they are `blocks-public`, since the rename gates the
public flip.

Two further findings are stated in prose rather than a table because they need
the argument: **J1** (`should-fix`) and **J2** (`cosmetic` for behavior,
`should-fix` for the traceability claim).

**Findings whose fix must touch a §4.1 protected path: 24** — 20 of them
`blocks-public`.

| Protected path | Findings |
|---|---|
| `SECURITY.md` | 16 |
| `THREAT_MODEL.md` | 8 |
| `crates/usage-core/src/egress/**` | 1 (`G18`, shared with `SECURITY.md`) |

Two further §4.1 paths need work that is not a *claim* defect and so carries no
severity row: **`.github/workflows/`** (the entire release surface, §G — a new
`release.yml` plus a `gitleaks` job) and **`deny.toml`** (one rename hit,
§F(a)).

---

## A. Doc-vs-reality

The repo's security documents are written in the present tense throughout.
That is the right voice for a shipped control and the wrong voice for a
planned one, and the gap between the two is where almost every finding here
lives.

### A1. Secret scanning (`gitleaks`) — claimed in three files, exists in none

**Check:** `.github/workflows/ci.yml` read in full. It defines exactly four
jobs — `test` (3-OS matrix), `deny`, `audit`, `no-telemetry`. No `gitleaks`
step, no secret-scanning step of any kind. `git ls-files .github/` returns
only `dependabot.yml` and `workflows/ci.yml`, so there is no second workflow
either. No pre-commit hook exists (`.git/hooks` holds only the stock
`.sample` files; nothing in the tree installs one).

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G01 | `SECURITY.md:115` | "Secret scanning (`gitleaks`) runs in CI and pre-commit; test fixtures use synthetic tokens only — no real credentials ever enter the repo." | First clause false on both halves — no CI job, no hook. Second clause true (see A11). | blocks-public | `SECURITY.md` |
| G02 | `ARCHITECTURE.md:193` | "**Secret hygiene:** `gitleaks` pre-commit + CI secret scanning; `.gitignore` covers any local credential/test fixtures; no real tokens in tests (use synthetic fixtures)." | Same false claim, echoed. The `.gitignore` and synthetic-fixture halves are true. | blocks-public | — |
| G03 | `CONTRIBUTING.md:10` | "CI runs secret scanning; don't make it earn its keep." | Same false claim, third echo. **This one is not in the ship program's F1** — F1 names only `SECURITY.md` and `ARCHITECTURE.md`. | blocks-public | — |

This is the ship program's **F1**, plus one file F1 missed.

### A2. Signed / attested releases with checksums — claimed in five places, no release pipeline exists

**Check:** `git ls-files .github/` → two files, neither a release workflow.
`gh run list` shows only the `CI` workflow has ever run. No tags exist that
would trigger one (`git tag` empty). `git grep -i cosign\|attest\|SHA256SUMS`
across the tree returns hits only inside `prompts/`. `README.md` read in full
— it has no "Verify before you run" section and no verification instructions
of any kind.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G04 | `SECURITY.md:103` | "Release artifacts are built **only in CI**, never on a maintainer's machine." | No release artifacts have ever been built anywhere. Vacuously true today, false the moment anyone builds one by hand. | blocks-public | `SECURITY.md` |
| G05 | `SECURITY.md:104` | "Artifacts are **signed** (e.g. `cosign`) and published with **build provenance / attestations**; checksums accompany every release." | No signing, no attestation, no checksums, no release. | blocks-public | `SECURITY.md` |
| G06 | `SECURITY.md:105` | "Reproducible builds are pursued where the toolchain allows; the exact toolchain version is documented per release." | No release documents a toolchain version; nothing captures one. | blocks-public | `SECURITY.md` |
| G07 | `SECURITY.md:106` | "**Verify before you run:** instructions for checking the signature/provenance and checksum are in `README.md`." | `README.md` has no such section. This is a **dangling cross-reference in a security document** — the single worst shape a false claim can take, because it sends a cautious reader looking for a control and leaves them assuming they missed it. | blocks-public | `SECURITY.md` |
| G08 | `ARCHITECTURE.md:191` | "**Releases:** built **only** in CI (never a maintainer laptop); artifacts signed (e.g. `cosign`) with **GitHub build provenance / attestations**; checksums published. This is the concrete fix for the 'is the binary really the source?' gap." | Same, echoed. | blocks-public | — |
| G09 | `ARCHITECTURE.md:82` | Repo tree listing: "`└─ .github/workflows/    # CI: build, test, audit, signed release`" | No signed-release workflow. | should-fix | — |
| G10 | `ARCHITECTURE.md:160` | Threat table: "Malicious release binary \| **Mitigated** — CI-only builds, signed artifacts + build provenance (see §7); reproducible where feasible." | The mitigation does not exist, so the threat is **unmitigated**, not mitigated. | blocks-public | — |
| G11 | `THREAT_MODEL.md:80` | "**T-T1 — Tampered release binary exfiltrates tokens.** *Mitigation:* CI-only builds, signed artifacts + provenance/attestations, published checksums, reproducible-where-feasible" | Same — a STRIDE mitigation asserted against a pipeline that does not exist. | blocks-public | `THREAT_MODEL.md` |
| G12 | `THREAT_MODEL.md:119` | R2: "A compromised maintainer account could publish a signed-but-malicious release … Signing proves *who* built it, not that the code is benign" | Presupposes signing. Today an attacker with the account publishes an **unsigned** malicious release and nothing detects it. The residual risk is understated, not merely mis-stated. | blocks-public | `THREAT_MODEL.md` |

This is the ship program's **F2**, expanded from 2 lines to 9 across 4 files.

### A3. Certificate pinning — claimed as an available option, not implemented

**Check:** `git grep -n -iE "pinn|pin_cert|certificate" -- crates/` returns
only unrelated prose ("pinned the schema", "Buttons pinned right"). The
egress module (`crates/usage-core/src/egress/mod.rs`) builds a plain
`ureq::Agent` with no custom TLS config, no root-store override, and no
pinning hook. There is no configuration surface at all (see A6).

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G13 | `SECURITY.md:96` | "TLS required; optional certificate pinning for provider hosts." | TLS is required (rustls, no native-tls). Pinning does not exist and cannot be enabled. | blocks-public | `SECURITY.md` |
| G14 | `ARCHITECTURE.md:93` | "Optional certificate pinning for provider hosts." | Same. | should-fix | — |
| G15 | `THREAT_MODEL.md:77` | "**T-S1** … *Mitigation:* TLS verification is mandatory; provider hosts **may be certificate-pinned**" | Cited as a mitigation for spoofing/MITM. Half the mitigation is absent. | blocks-public | `THREAT_MODEL.md` |
| G16 | `THREAT_MODEL.md:118` | R1 user mitigation: "**Enable pinning**; avoid untrusted networks; verify egress." | **This instructs the reader to take an action that is impossible.** A security-conscious user follows R1, finds no flag, and reasonably concludes the docs are decorative. | blocks-public | `THREAT_MODEL.md` |

### A4. The update check — invariant 5 describes a feature that does not exist

**Check:** `git grep -n -iE "updater|update_check|update-check|api\.github\.com" -- crates/`
returns exactly one hit: the allowlist entry itself,
`crates/usage-core/src/egress/mod.rs:53`. There is no updater module
(`git ls-files 'crates/**/*.rs'` lists 12 files; none is an updater), no
version-comparison code, no notification path, and no setting to turn
anything on or off.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G17 | `SECURITY.md:69` | Invariant 5: "The app never downloads and executes new code on its own. **The optional update *check* only notifies**; the user updates via their package manager or a manual, verifiable download. The update check is **off by default**." | Sentence 1 is true and is the invariant that matters. Sentences 2–3 describe an optional feature that does not exist — there is nothing to notify and nothing to be off by default. | blocks-public | `SECURITY.md` |
| G18 | `SECURITY.md:95` | Allowlist: "`api.github.com` — update **check only**, and only if the user enables it." | `api.github.com` **is** on the compile-time allowlist (`egress/mod.rs:53`) but has **no caller anywhere in the workspace**. The allowlist is one host wider than the code needs. Not exploitable on its own — reaching it still requires code that doesn't exist — but it contradicts "deny-by-default, minimal surface," and it is the first thing an auditor greps. | blocks-public | `SECURITY.md`, and a fix touches `egress/**` |
| G19 | `ARCHITECTURE.md:92` | "`api.github.com` — **update check only**, and only when the user enables it" | Same, echoed. | should-fix | — |
| G20 | `ARCHITECTURE.md:149` | "**No silent auto-update.** Update = check GitHub releases and *notify only*" | Same, echoed. | should-fix | — |
| G21 | `THREAT_MODEL.md:99` | "**T-E2** … *Mitigation:* invariant 5 — no silent auto-update; update check notifies only and is off by default." | Same. (The real posture — *no update mechanism exists at all* — is strictly stronger and should simply be stated.) | should-fix | `THREAT_MODEL.md` |
| G22 | `README.md:14` | "No silent auto-update — the optional update *check* only notifies, and it's off by default." | Same, echoed. | should-fix | — |

### A5. Invariant → test traceability — two rows claim tests that do not exist

`THREAT_MODEL.md:125-135` is the traceability table, and `SECURITY.md:63`
asserts "Each invariant below is enforced in code and backed by a test."
That makes this table the load-bearing claim of the whole security posture.

**Check:** enumerated every `#[test]` in the workspace (138 total; see §J for
the per-file tally) and read every test in `credentials/`, `egress/`, and
`poller/`.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G23 | `THREAT_MODEL.md:129` | Invariant 1 row: enforcing module "`credentials`, config layer"; test "assert no write path emits token bytes" | **No test of that description exists, and there is no config layer** (see A6). The nearest test is `credentials/mod.rs:122 loads_credential_readonly_and_redacted`, which asserts the credential file is byte-identical after loading — that is invariant **6** (read-only), and the test's own comment says so (`// Invariant 6:`). Invariant 1 (no persistence) is backed by *the absence of a write path*, not by a test. | blocks-public | `THREAT_MODEL.md` |
| G24 | `THREAT_MODEL.md:133` | Invariant 5 row: test "update-check is notify-only; disabled by default (unit test)" | No such unit test; no updater to test. | blocks-public | `THREAT_MODEL.md` |
| G25 | `SECURITY.md:63` | "Each invariant below is enforced in code and backed by a test." | False as a universal, given G23/G24. Invariants 2, 3, 6, 7 *are* genuinely test-backed (verified: `secret.rs` 5 tests, `egress/mod.rs` 8 tests incl. `non_allowlisted_host_is_rejected`, `proxy_env_without_opt_in_fails_closed`, `egress_errors_never_echo_secrets`; `credentials/mod.rs:122`). Invariant 4 is CI-enforced by the `no-telemetry` job rather than a unit test, which the table acknowledges. Invariants 1 and 5 are not. | blocks-public | `SECURITY.md` |

### A6. Configuration / preferences layer — described in detail, does not exist

**Check:** `git grep -n -iE "config_dir|APPDATA|preferences|fn save|write_config" -- crates/`
returns **nothing**. The source tree has no `config` module. `usage-ui`
parses three CLI flags and holds state in memory only; nothing is read from
or written to disk.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G26 | `ARCHITECTURE.md:172-178` | "Preferences only … JSON in the platform config dir (`%APPDATA%\QuotaPane\` / …)" followed by a five-bullet list of stored settings (window position, providers enabled, theme, update flag, proxy flag) | None of it exists. No file is written; no setting persists. | should-fix | — |
| G27 | `ARCHITECTURE.md:180` | "Schema is versioned and documented in `README.md`." | No schema, and `README.md` documents none. Second dangling cross-reference (cf. G07). | should-fix | — |
| G28 | `ARCHITECTURE.md:204` | "**Modes:** compact (thin strip) and expanded; multi-monitor aware; **position persists**." | Position does not persist (no config layer). There is no compact mode: the window is fixed at `WINDOW_WIDTH`×`WINDOW_HEIGHT` with `.with_resizable(false)` (`usage-ui/src/main.rs:1055-1058`). | should-fix | — |
| G29 | `ARCHITECTURE.md:205` | "**Interactions:** drag to move; **scroll to resize/zoom**; click a row to expand a detail popover…; right-click for settings/position/theme/minimize." | Drag-to-move works. Scroll does **not** resize — since the M5a fix, scroll drives a `ScrollArea` (`main.rs`, `DragScroll::Never`). There is no right-click menu and no settings UI. The row expansion is an inline disclosure toggle, not a popover. | should-fix | — |
| G30 | `SECURITY.md:65` | Invariant 1: "The app never writes tokens to disk. **Config files store preferences only.**" | First sentence true. Second describes config files that do not exist — harmless but currently fiction. | should-fix | `SECURITY.md` |

### A7. WSL credential support — claimed twice, not implemented

**Check:** `git grep -n -i "wsl" -- crates/` returns **nothing**.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G31 | `SECURITY.md:77` | "Sources are read-only: `~/.claude/.credentials.json`, `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`), **and optionally a credential file inside a named WSL distro**." | The first two are real. WSL is not implemented at all. | should-fix | `SECURITY.md` |
| G32 | `ARCHITECTURE.md:87` | "Resolves `~/.claude/.credentials.json`, `$CODEX_HOME/auth.json` (or `~/.codex/auth.json`), **and optionally a credential file inside a named WSL distro**." | Same. | should-fix | — |

### A8. Messages-API rate-limit fallback — deferred in code, present-tense in docs

**Check:** `crates/usage-core/src/providers/claude_subscription.rs:18-23`
states plainly: "The Messages-API rate-limit-header *fallback* named in the
spec **requires a second, verified endpoint and is deferred**." Confirmed by
`git grep -n "RateLimitHeaders" -- crates/`, which returns **exactly one
hit** — the enum variant's own declaration at `model/mod.rs:75`. The variant
is never constructed anywhere.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G33 | `SECURITY.md:93` | Allowlist rationale: "`api.anthropic.com` — subscription usage **+ Messages-API rate-limit fallback**." | The fallback is deferred. The host is correctly allowlisted for the usage endpoint; the stated second reason is not real. | should-fix | `SECURITY.md` |
| G34 | `ARCHITECTURE.md:90` | "`api.anthropic.com` (subscription usage + Messages-API rate-limit fallback)" | Same. | should-fix | — |
| G35 | `ARCHITECTURE.md:106` | "`ClaudeSubscription` — OAuth usage endpoint **+ Messages-API rate-limit-header fallback**." | Same, stated as a shipped implementation. | should-fix | — |
| G36 | `crates/usage-core/src/model/mod.rs:75` | `SnapshotSource::RateLimitHeaders` | Dead variant — never constructed. Harmless (it is the honest placeholder for deferred work) but it is public API surface for a source that cannot occur, and `model/mod.rs:20`/`:64` describe it as live. | cosmetic | — |

### A9. Poller behavior — one claimed scheduling feature is absent

**Check:** read `crates/usage-core/src/poller/mod.rs` in full. `next_delay`
takes `(cadence, consecutive_failures, retry_after)` only.
`git grep -n -iE "snap|resets_in|imminent" -- crates/usage-core/src/poller/mod.rs`
returns no scheduling hit.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G37 | `ARCHITECTURE.md:112` | "Adaptive intervals: fast (~5 min) …, normal (~7 min), slow (~20 min) when idle, **snap to imminent quota resets**, exponential backoff on `429`." | Everything except reset-snapping is real and tested (`cadence_interval` = 300/420/1200 s; `MIN_INTERVAL` 180 s; `MAX_BACKOFF` 1800 s; `retry-after` honored when longer). Reset-snapping does not exist. Also worth noting: **both providers return `Cadence::Normal` unconditionally** (`claude_subscription.rs:144`, `codex_subscription.rs:157`), so Fast and Slow are unreachable in production — the adaptivity is implemented but never exercised. | should-fix | — |

### A10. A documented hardening command that does not work

**Check:** ran it. `cargo run -q -p usage-cli -- --json` →
`error: --once is required (the only supported mode for now)`, exit **2**.
`parse_args` (`usage-cli/src/main.rs:107-110`) rejects any invocation without
`--once`.

| # | File:line | Claim (verbatim) | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G38 | `SECURITY.md:125` | Hardening guidance §3: "**Verify egress once.** Run `usage-cli --json` behind a packet capture or host-firewall allowlist and confirm the only destinations are the provider hosts." | The command exits 2 without making any request. The working form is `usage-cli --once --json`. This sits in the "You do not have to take our word for any of the above" section — the one place a skeptical reader will actually type a command, and it fails immediately. | blocks-public | `SECURITY.md` |

### A11. Claims verified TRUE (recorded so the next pass doesn't re-check them)

Not findings. Listed because a truth audit that only reports failures can't
be distinguished from one that stopped early.

- `SECURITY.md:52-57` / `ARCHITECTURE.md:138-142` — the trust boundary really
  is `credentials` + `egress` inside `usage-core`. Verified: `ureq` appears in
  no other crate's `Cargo.toml`; `usage-ui`/`usage-cli` depend on `usage-core`
  and never construct an HTTP client.
- **Invariant 2** (redaction/zeroize) — `credentials/secret.rs` (5 tests),
  `credentials/mod.rs:139-142` asserts `Debug` omits the token.
- **Invariant 3** (deny-by-default egress) — `egress/mod.rs` tests
  `non_allowlisted_host_is_rejected`, `get_refuses_non_allowlisted_host`,
  `authority_smuggling_paths_are_rejected`, `egress_errors_never_echo_secrets`.
- **Invariant 4** (no telemetry) — the `no-telemetry` CI job exists and greps
  both `Cargo.toml` deps and `*.rs` sources. Claim matches implementation.
- **Invariant 6** (read-only credentials) — `credentials/mod.rs:122` (but see
  J2 for what that test does *not* prove).
- **Invariant 7** (proxy opt-in) — `check_proxy_gate` + 3 tests; `ureq` is
  additionally configured `.proxy(None)` as defense in depth.
- `SECURITY.md:112-114` supply chain — `Cargo.lock` committed ✅,
  `cargo-deny` + `cargo-audit` CI jobs ✅, `dependabot.yml` present ✅.
- `CONTRIBUTING.md:29-34` — the three dev commands are correct and CI does run
  the same three plus deny/audit/no-telemetry on 3 OSes.
- `CONTRIBUTING.md:16-22` dependency table — matches the actual dependency set
  including the `tray-icon` row and its phantom-crate analysis.
- ADR-002 consequences — `api.openai.com` is **not** on the allowlist, and
  `ProviderId` has exactly two variants. Both match the docs.
- `SECURITY.md:84-86` / `README.md:35` User-Agent disclosure — matches code
  (`format!("claude-code/{}", …)`, `CODEX_DEFAULT_USER_AGENT`).

---

## B. Placeholders

**Check:** `git grep -nE "<DOMAIN>|TODO|TBD|fill in|placeholder|working name|XXX|FIXME"`
over the whole tree, then read each hit in context.

| # | File:line | Text | Note | Severity | §4.1 |
|---|---|---|---|---|---|
| G39 | `SECURITY.md:14` | "Backup channel: `security@<DOMAIN>` *(fill in before first release; PGP key fingerprint here if used)*" | The literal `<DOMAIN>` placeholder plus a parenthetical instruction to the maintainer, in the disclosure section of a public security policy. Blocks D5. | blocks-public | `SECURITY.md` |
| G40 | `README.md:3` | "> **Working name** — will be renamed before the first public release." | A banner that announces the repo isn't ready to be public, on a public repo. | blocks-public | — |
| G41 | `ARCHITECTURE.md:3` | "> Working name: **QuotaPane** (rename before first public release — see Open Decisions)." | Same. | blocks-public | — |
| G42 | `SECURITY.md:3` | "> Project: **QuotaPane** (working name)." | Same. | blocks-public | `SECURITY.md` |
| G43 | `THREAT_MODEL.md:3` | "> Project: **QuotaPane** (working name)." | Same. | blocks-public | `THREAT_MODEL.md` |
| G44 | `ARCHITECTURE.md:4` | "> Status: draft spec, v0.1. Intended as the seed document for a public, open-source repository." | The document has been the shipped architecture through M5a; "draft spec" understates it and "intended as the seed" is past tense in fact. | should-fix | — |

Three code hits for `placeholder`
(`usage-cli/src/main.rs:154`, `credentials/secret.rs:23`,
`usage-ui/src/main.rs:77`) are **not** findings — each uses the word to
describe intended runtime behavior (the default `--client-version` string,
the `«redacted»` text, a default value), not unfinished work. Checked each.

---

## C. Stale roadmap / status

| # | File:line | Claim | Reality (`DECISIONS.md` §2) | Severity | §4.1 |
|---|---|---|---|---|---|
| G45 | `README.md:4` | "Status: **M0 — trust boundary & scaffolding.** Not yet useful on a desk; the security core ships before the features do, on purpose." | Four milestones stale. M0–M3.5 are ✅ accepted, the look pass is ✅ accepted, M5a is implemented and awaiting visual acceptance. The app is a working two-provider always-on-top window with a system tray and per-model breakdown. "Not yet useful on a desk" is flatly false. | blocks-public | — |
| G46 | `README.md:31` | Roadmap: "→ **M4** opt-in official billing APIs → **M5** history/forecasts → **M6** signed releases + packaging" | M4 was **withdrawn** by ADR-002 on security grounds (would require ingesting an org-admin key). Advertising it as live scope contradicts `ARCHITECTURE.md:42`, `DECISIONS.md:14`, and `SECURITY.md:79`, all of which say it is out of scope — and it advertises exactly the capability the project's thesis rejects. | blocks-public | — |
| G47 | `README.md:6` | "Single Rust binary, no web layer, no telemetry, no auto-update." | The workspace produces **two** binaries (`usage-ui`, `usage-cli`). "no auto-update" is true. | cosmetic | — |
| G48 | `README.md` (whole file) | No mention of: the system tray (M3.5, accepted), the per-model breakdown (M5a), multi-provider support being live, or that Linux is window-only (no tray, per the `tray-icon` gating in `usage-ui/Cargo.toml`). | Features shipped and accepted but undocumented. | should-fix | — |
| G49 | `ARCHITECTURE.md:188` | "**License:** MIT or Apache-2.0 (Open Decision)." | Settled in `DECISIONS.md:13` — dual MIT OR Apache-2.0 — and already reflected in `Cargo.toml` and both LICENSE files. Not an open decision. | should-fix | — |
| G50 | `ARCHITECTURE.md:224-231` | "## 10. Open decisions (resolve in Cowork)" listing 6 items, of which #1 (UI), #3 (undocumented endpoints), #4 (name & license), #5 (platform priority) are shown unresolved | #1, #3, #5 and the license half of #4 are all settled in `DECISIONS.md` §1. Only the **name** is genuinely still open. | should-fix | — |
| G51 | `ARCHITECTURE.md:81` | Repo tree lists "`├─ LICENSE`" | Actual files are `LICENSE-MIT` and `LICENSE-APACHE`; there is no `LICENSE`. | cosmetic | — |
| G52 | `ARCHITECTURE.md:63` | Repo tree root shown as "`quotapane/`" | Cosmetic, but it is a naming-surface hit that a rename must catch (see F). | cosmetic | — |

---

## D. Version claims

**Check:** `Cargo.toml:14` — `rust-version = "1.92" # floor set by eframe 0.35 (M2)`.
`Cargo.toml:10` — `version = "0.1.0"`.

| # | File:line | Claim | Reality | Severity | §4.1 |
|---|---|---|---|---|---|
| G53 | `README.md:27` | "Requires Rust 1.85+." | The workspace pins `rust-version = "1.92"`. A user on 1.85 gets a hard cargo error, not a warning. Off by seven minor versions. | should-fix | — |
| G54 | — | No other document states a toolchain version. `CONTRIBUTING.md` states none (checked); `ci.yml` uses `dtolnay/rust-toolchain@stable` and pins nothing. | Recorded as a **negative result**: `README.md:27` is the only version claim in the tree, so it is the only one to fix. | — | — |

---

## E. CLI surface

**Check:** ran `cargo run -q -p usage-cli -- --help` and read
`parse_args` in `crates/usage-cli/src/main.rs:79-121` and
`crates/usage-ui/src/main.rs:85-115`.

Actual `usage-cli` flags: `--once` (**required**), `--json`, `--provider
claude|codex|all`, `--client-version <VER>`, `--debug-raw`.
Actual `usage-ui` flags: `--client-version <VER>`, `--codex-user-agent <UA>`,
`--no-tray`.

| # | Finding | Detail | Severity | §4.1 |
|---|---|---|---|---|
| G55 | **`--help` is not a recognized flag.** | `usage-cli --help` prints `error: unrecognized argument: --help` and exits **2**. It does print the usage line (as the error path for *any* bad argument), so the user gets the information — but via an error, with a failing exit code. `--version` likewise does not exist. For a tool whose README will tell people to download a signed binary and run it, `--help` failing is the first interaction most users will have. | should-fix | — |
| G56 | **`--debug-raw` is not documented anywhere outside code.** | It appears in the usage line and in module docs, but in no `.md` file. Phase 0 of this session (`e608a58`) just extended it from Codex-only to both providers — **so there is no stale doc describing it as Codex-specific**, because there was never a doc describing it at all. (The audit prompt anticipated a stale doc here; the real gap is absence, not staleness.) | should-fix | — |
| G57 | **`usage-ui`'s three flags are documented nowhere.** | `--no-tray`, `--codex-user-agent`, `--client-version`. `git grep -- "--no-tray" -- '*.md'` outside `prompts/` returns nothing. `--no-tray` is the documented escape hatch for a tray that fails to create, and a Linux user has no way to learn it exists. | should-fix | — |
| G58 | **`--codex-user-agent` already exists, but `DECISIONS.md` §2 lists it as future M5 work.** | `DECISIONS.md:24` lists "a Codex User-Agent flag in the CLI" among M5 items still to come. It is **implemented and tested in `usage-ui`** (`main.rs:99`, tests at `:1141`, `:1147`, `:1153`) and **absent from `usage-cli`**. So the roadmap item is half-done in the wrong binary. This matters for D1 (see §L3). | should-fix | — |
| G59 | **No document lists the CLI surface at all.** | `README.md` mentions `usage-cli --json` only inside the security bullet list; `ARCHITECTURE.md:72` calls it "headless `--once` / `--json` mode". Neither enumerates flags. A release with no `--help` and no documented flags has no discoverable interface. | should-fix | — |

---

## F. Naming surface

**Check:** `git grep -n -i "quotapane"` over all tracked files, then the same
for crate names against `.github/`.

**Total: 58 occurrences across 32 tracked files** (excluding `prompts/`:
**44 occurrences across 21 files**).

### F(a) — §4.1 protected paths → rename **commit 2**, §4a verify-and-commit

| File | Lines | Count |
|---|---|---|
| `SECURITY.md` | 3, 5, 79, 84, 133 | 5 |
| `THREAT_MODEL.md` | 3, 11, 107 | 3 |
| `deny.toml` | 89 | 1 |
| `.claude/agents/implementer.md` | 7 | 1 |
| `.claude/agents/mechanical.md` | 7 | 1 |
| `.claude/agents/security-reviewer.md` | 8 | 1 |
| **Subtotal** | | **12** |

### F(b) — everything else → rename **commit 1**, floor-authorable

| File | Lines | Count |
|---|---|---|
| `crates/usage-ui/src/main.rs` | 1, 132, 374, 550, 562, 621, 692, 1063, 1077 | 9 |
| `crates/usage-core/src/providers/claude_subscription.rs` | 10, 12, 413, 439 | 4 |
| `crates/usage-core/src/providers/codex_subscription.rs` | 608, 626 | 2 |
| `crates/usage-core/src/lib.rs` | 3 | 1 |
| `crates/usage-core/src/egress/mod.rs` | 28 | 1 |
| `crates/usage-core/src/credentials/mod.rs` | 117 | 1 |
| `crates/usage-core/src/poller/mod.rs` | 133 | 1 |
| `crates/usage-core/src/providers/mod.rs` | 40 | 1 |
| `crates/usage-cli/src/main.rs` | 1 | 1 |
| `crates/usage-core/Cargo.toml` | 3 | 1 |
| `crates/usage-ui/Cargo.toml` | 3 | 1 |
| `crates/usage-cli/Cargo.toml` | 3 | 1 |
| `Cargo.toml` | 13 (repo URL) | 1 |
| `ARCHITECTURE.md` | 1, 3, 42, 63, 172 | 5 |
| `README.md` | 1, 35 | 2 |
| `DECISIONS.md` | 13, 15 | 2 |
| `CLAUDE.md` | 1 | 1 |
| `LICENSE-MIT` | 3 (copyright line) | 1 |
| **Subtotal** | | **36** |

Plus 14 occurrences across 8 files in `prompts/` (historical records of what
was asked; most describe the repo *path* `C:\dev\QuotaPane\QuotaPane` rather
than the product, and should NOT be rewritten — they would become false
records of what the prompt said).

### F(c) — USER-VISIBLE strings (each must change; each is separately verifiable)

| # | Location | String | Where the user sees it |
|---|---|---|---|
| G60 | `usage-ui/src/main.rs:1063` | `"QuotaPane"` — first arg to `eframe::run_native` | OS window title / taskbar entry |
| G61 | `usage-ui/src/main.rs:621` | `egui::RichText::new("QuotaPane")` | The app name rendered in the custom slim titlebar (visually accepted 2026-07-24 — **changing it needs a new §4.5 visual check**) |
| G62 | `usage-ui/src/main.rs:374` | `const INITIAL_TOOLTIP: &str = "QuotaPane"` | System tray hover tooltip before the first snapshot arrives |
| G63 | `usage-core/src/poller/mod.rs:133` | `.name("quotapane-poller")` | OS thread name — visible in debuggers, crash dumps, Process Explorer |
| G64 | binary filenames | `usage-ui.exe`, `usage-cli.exe` | What the user downloads and types. **Needs the `[[bin]]` rename (D3).** |
| G65 | `Cargo.toml:13` | `repository = "https://github.com/cipherpine/quotapane"` | Rendered by cargo tooling; also the repo URL itself |
| G66 | `LICENSE-MIT:3` | `Copyright (c) 2026 The QuotaPane Contributors` | The license text every distributor reproduces |
| G67 | all three `crates/*/Cargo.toml:3` | `description = "QuotaPane …"` | `cargo metadata`, and any future crates.io page |

Tray **menu items** carry no product name — they are `"Show/Hide"` and
`"Quit"` (`usage-ui/src/main.rs:399-400`). No CLI output carries the name
either: the usage line says `usage-cli`, which changes only via `[[bin]]`.

Test fixtures using a `quotapane-` temp-file prefix
(`credentials/mod.rs:117`, `claude_subscription.rs:413,439`,
`codex_subscription.rs:608,626`) are not user-visible but collide across
concurrent runs of a renamed and un-renamed build; rename them for hygiene.

### F(d) — `.github/workflows/ci.yml` — CONFIRMED clean

`git grep -n -iE "quotapane|usage-core|usage-cli|usage-ui" -- .github/`
returns **zero hits**. The workflow references no product name and no crate
name — it drives everything through `cargo … --workspace`. **The ship
program's claim on this point is correct: `ci.yml` survives the rename
untouched.** `.github/dependabot.yml` is likewise clean.

---

## G. Release surface — what does not exist yet

Nothing release-related exists. Enumerated so a later prompt has a checklist.

| # | Missing | Detail | Severity |
|---|---|---|---|
| G68 | Release workflow | No `.github/workflows/release.yml`. No tag trigger. `git tag` is empty — no release has ever been attempted. | blocks-public |
| G69 | Artifact naming convention | Undecided. Nothing in the tree names an artifact. | should-fix |
| G70 | Per-OS release build steps | CI builds debug + test on 3 OSes; nothing runs `cargo build --release --locked` anywhere. `[profile.release]` is configured (`strip`, `lto`, `codegen-units = 1`) but never exercised in CI. **`strip = true` interacts with build-provenance attestation and should be checked before D-phase authoring.** | should-fix |
| G71 | Checksum generation | None. | blocks-public |
| G72 | Signing | No `cosign`, no key, no keyless/OIDC config. Requires `id-token: write`, which `ci.yml`'s top-level `permissions: contents: read` does not grant. | blocks-public |
| G73 | Attestation | No `actions/attest-build-provenance`. Requires `attestations: write`. | blocks-public |
| G74 | `CHANGELOG.md` | Absent. A 1.0.0 with no changelog covering M0–M5a. | should-fix |
| G75 | Toolchain capture | `SECURITY.md:105` promises a per-release toolchain version; nothing records `rustc -V`. | blocks-public |

### Version-bump points (the precise list — `0.1.0` → `1.0.0`)

**Check:** `git grep -n "0\.1\.0"` over the whole tree.

| File:line | What | Action |
|---|---|---|
| `Cargo.toml:10` | `[workspace.package] version = "0.1.0"` | **The only source of truth.** All three crates inherit via `version.workspace = true` (verified in each crate's `Cargo.toml`). |
| `Cargo.lock:184, 2643, 2651, 2661` | the three workspace members + one dep coincidentally at 0.1.0 | Regenerated by `cargo build --locked`; do not hand-edit. Note line 184 is a **third-party** crate that is also at 0.1.0 — do not sweep it. |
| `prompts/m6-ship-program.md:297` | prose describing the bump | Historical record; leave. |

**No other file hardcodes a version.** In particular no `.rs` file embeds one,
and `usage-cli`'s `DEFAULT_CLIENT_VERSION = "0.0.0"` is the *Claude Code*
client version sent as a User-Agent, unrelated to the product version — do
not sweep it during the bump.

---

## H. Public-repo hygiene — present / absent

**Check:** filesystem test per path plus `git ls-files`.

| File | State |
|---|---|
| `CHANGELOG.md` | **ABSENT** |
| `CODE_OF_CONDUCT.md` | **ABSENT** |
| `.github/ISSUE_TEMPLATE/` | **ABSENT** |
| `.github/PULL_REQUEST_TEMPLATE.md` | **ABSENT** |
| `LICENSE` (singular) | **ABSENT** — by design; `LICENSE-MIT` + `LICENSE-APACHE` present (but see G51) |
| `.gitattributes` | present (`* text=auto eol=lf`) |
| `.gitignore` | present, and correctly covers `.credentials.json`, `auth.json`, `*.pem`, `*.key`, `.env*` |
| `.github/dependabot.yml` | present |
| `SECURITY.md` / `CONTRIBUTING.md` / `ARCHITECTURE.md` / `THREAT_MODEL.md` | present |

**Created nothing**, per the prompt.

---

## I. Files in the tree that are working material, not product

**Check:** `git ls-files` + `du -b`, and `git log --all -- <path>` for history.

| Path | Tracked? | Size | Note |
|---|---|---|---|
| `prompts/` | **yes — 11 files, 89 KB** | `m5a-per-model-breakdown.md` 23 KB, `m6-ship-program.md` 18 KB, `m6-prep-audit.md` 11 KB, `m3.5-tray-dep-resolution-handoff.md` 7.7 KB, `m3.5-tray-dependency-review.md` 7.5 KB, `finish-m3.5-phaseB.md` 4.3 KB, `land-m3.5-phaseA.md` 4.0 KB, `window-look-pass.md` 3.9 KB, `finish-m3.5-tray.md` 3.9 KB, `land-charter-amendment.md` 3.2 KB, `finish-m3.md` 2.9 KB (+ this report) | Goal prompts and reviews. In the tree **and in history**. |
| `_claude_setup/` | **NO — gitignored** | 1 file (`cargo/audit.toml`) | `.gitignore:21` has `/_claude_setup/`, and `git log --all -- _claude_setup` is **empty**: it has never been committed. See §L1. |
| `_to_delete/` | **NO — empty dir** | 0 bytes, 0 files | Untracked leftover from a prior session (git does not track empty directories, so it is invisible to `git status`). Harmless; removable. |
| `target/` | NO — gitignored | — | Build output. |
| `.claude/` | **yes — 4 files** | 3 agent definitions + `settings.json` | D4 recommends keeping these public. |

**Recommending nothing** — the disposition is the owner's (D4/D7).

---

## J. Test-harness blind spots

The prompt asked me to be harder on this section than on the docs, because it
audits my own prior work. Method: enumerated all 138 tests
(`usage-ui` 64, `usage-cli` 20, `codex_subscription` 19,
`claude_subscription` 10, `egress` 8, `poller` 6, `secret` 5, `time` 3,
`credentials` 3 — sums to 138, matching `cargo test --workspace`), then read
every test in the trust-boundary crates and every assertion matching a weak
shape (`is_empty`, `contains`, `is_some`, `is_ok`).

### J1 — `failures_are_forwarded_as_non_secret_messages` cannot fail for the reason it names — **the worst one here**

`crates/usage-core/src/poller/mod.rs:322-337`. The test name asserts a
**security** property: that failure messages carry no secret. The only
assertion on the message is:

```rust
assert!(!message.is_empty());
```

That checks the message is non-empty. It never checks the message lacks token
material. And the fixture cannot reach the interesting case: `FailingProvider`
(`:308-320`) returns a hard-coded `ProviderError::UnexpectedPayload`, a unit
variant that **structurally cannot carry a token**. So the test would pass
identically if the poller formatted the raw credential into every failure
string, because no code path in the fixture ever has a credential to leak.

Both failure modes the prompt described, in one test: a weak assertion where a
value check was intended, **and** a fixture that cannot reach the branch it
claims to cover. It is one of only six poller tests, and it is the one
carrying an invariant-2 name. `egress_errors_never_echo_secrets`
(`egress/mod.rs:370`) is the test that does this properly — J1 should look
like that. Severity: **should-fix** (the underlying behavior is very likely
correct; the *test* proves nothing).

### J2 — `loads_credential_readonly_and_redacted` proves "unmodified", not "read-only"

`crates/usage-core/src/credentials/mod.rs:122-148`. The invariant-6 assertion
is `assert_eq!(before, after)` — the file's bytes are unchanged after loading.
That is a real check and it would catch an accidental write. It does **not**
prove the file was *opened* read-only: an implementation that opened with
write permission and never wrote would pass. Since invariant 6 is worded
"Credential files are **opened** read-only," the test is one notch weaker than
the claim it backs. Severity: **cosmetic** for behavior, **should-fix** for
the traceability claim in `THREAT_MODEL.md:134`.

### J3 — the M5a layout harness is sound; I checked it adversarially

The `egui::__run_test_ui` blind spot that let the clipped row ship is
genuinely closed. `lay_out` (`usage-ui/src/main.rs:1230-1260`) uses a default
`egui::Context` (real fonts), sizes the raw input to the real
`WINDOW_WIDTH`×`WINDOW_HEIGHT`, runs **two** frames so the font atlas and
id-keyed state settle, and — the part that matters — **measures**
`available_width` from `ui.max_rect().width()` rather than hard-coding it, so
the assertions self-calibrate. `single_line_layout_would_not_fit_which_is_why_rows_stack`
(`:1304`) is a counterfactual that fails if the fix is reverted, which is the
thing that makes the other four assertions non-vacuous. `__run_test_ui`
appears nowhere in the crate (`git grep` — only the comment explaining why
not). **No finding.** Recorded because "I checked and it holds" is the useful
output when the answer is negative.

### J4 — assertions checked and cleared

Every remaining weak-shape assertion was read in context and is fine:

- `claude_subscription.rs:370,378-379`, `codex_subscription.rs:428-429,500,509`
  — `assert!(…is_empty())` on `windows`/`per_model` is the **intended value**
  (degrade-don't-crash on malformed payloads), not a placeholder.
- `codex_subscription.rs:459` — `assert!(json.contains(pii), "fixture lost its
  PII")` is an explicit **anti-vacuity guard**: it proves the fixture really
  carries the PII before asserting the parsed struct drops it. This is the
  pattern J1 is missing.
- `credentials/mod.rs:141-142` — asserts both that `Debug` **lacks** the token
  and **contains** "redacted". Two-sided; correct.
- `usage-cli/src/main.rs:441-462`, `usage-ui/src/main.rs:1454-1455` — string
  `contains` on JSON/label output where the string *is* the contract.

### J5 — coverage gaps (absence of tests, not weak tests)

| Area | Gap |
|---|---|
| Invariant 1 (no persistence) | No test asserts the app writes nothing. Backed only by absence of a write path. (= G23) |
| Invariant 5 (no auto-update) | No test; no code. (= G24) |
| `--debug-raw` end-to-end | The two new tests (`e608a58`) cover the expired-token and missing-file paths. No test covers a **successful** `debug_raw_body`, so the `"status: {}\n{}"` format string is unpinned. |
| `usage-cli` arg parsing | `--debug-raw` + `--json` now emits a stderr note (`e608a58`); no test asserts that note is emitted. |

---

## K. Known accepted drift

Recorded so these live in the record rather than only in code comments.

| # | Item | Why accepted | Severity |
|---|---|---|---|
| G76 | The `120.0` progress-bar width literal is duplicated in `render_window_row` and `render_per_model_row` (`usage-ui/src/main.rs`) rather than extracted to a shared constant. | Extracting it would mean editing `render_window_row`, which is part of the **visually accepted** window look pass (2026-07-24). The M5a-fix commit (`7e72282`) chose duplication over touching accepted code, and put a comment in each function noting they must stay in step. Deliberate; the risk is that a future width change updates one and not the other. | cosmetic |
| G77 | Both providers hard-code `Cadence::Normal`, so `Cadence::Fast`/`Slow` and the whole adaptive-interval mechanism are unreachable in production (tested in isolation only). | Deliberate conservatism toward undocumented endpoints — polite polling beats adaptive polling when the endpoint is not ours. But it means `ARCHITECTURE.md:112`'s "adaptive intervals" describes a capability no shipped code path selects. (= G37) | should-fix |
| G78 | `SnapshotSource::RateLimitHeaders` is a public enum variant that is never constructed. | Honest placeholder for the deferred Messages-API fallback (`claude_subscription.rs:18-23`). (= G36) | cosmetic |
| G79 | Linux is window-only: no tray, because `tray-icon` is `cfg`-gated to Windows/macOS to avoid the unmaintained gtk-rs 0.18 + libappindicator chain. | Top-tier dependency review, documented in `CONTRIBUTING.md:22` and `usage-ui/Cargo.toml`. Accepted. **Not documented in `README.md`** — a Linux user finds out by running it. (= G48) | should-fix |

---

## L. Where this report disagrees with `prompts/m6-ship-program.md`

The prompt calls this the most valuable section, since the program document
was authored without running these checks. Three real disagreements and one
correction.

### L1 — `_claude_setup` is **not in the repo**, so D4 and G6 have less to do than stated

`m6-ship-program.md:136-151` (D4) says "The repo carries … `prompts/` … and a
`_claude_setup` directory," and recommends "Move `prompts/` and
`_claude_setup` out of the public tree."

**Check:** `.gitignore:20-21` contains `# Local Claude staging folder (not
part of the project)` / `/_claude_setup/`. `git log --all --oneline --
_claude_setup` returns **empty** — it has never been committed on any ref. It
holds a single file, `cargo/audit.toml`.

So half of D4's move is a no-op: `_claude_setup` is already outside the tree
and outside history. **Only `prompts/` is actually in the repo**, and — the
part that matters for D4 — `prompts/` is in **history**, so removing it from
the tip does not remove it from a public repo. That is precisely the
interaction the Prompt G preamble warns about, and it now has a confirmed
input: the fresh-repo option in D7 is the *only* way D4-as-written holds.

### L2 — D2's naming-surface count materially undercounts the crate sources

`m6-ship-program.md:116-119` measures the rename as: "9 hits in
`usage-ui/src/main.rs`, 5 each in `SECURITY.md` and `ARCHITECTURE.md`, 3 in
`THREAT_MODEL.md`, 2 each in `README.md`/`DECISIONS.md`, **1 each across the
remaining crates**, `LICENSE-MIT`, `deny.toml`, `CLAUDE.md`, and the
`.claude/agents/*` files."

Every figure is exact **except the last**. Verified against
`git grep -n -i quotapane`:

- `usage-ui/src/main.rs` = 9 ✅ · `SECURITY.md` = 5 ✅ · `ARCHITECTURE.md` = 5 ✅
  · `THREAT_MODEL.md` = 3 ✅ · `README.md` = 2 ✅ · `DECISIONS.md` = 2 ✅
  · `LICENSE-MIT` = 1 ✅ · `deny.toml` = 1 ✅ · `CLAUDE.md` = 1 ✅
  · `.claude/agents/*` = 1 each ✅
- **"1 each across the remaining crates" — actually `usage-core` has 12**
  (spread over 7 files + its `Cargo.toml`, incl. **4** in
  `claude_subscription.rs` and **2** in `codex_subscription.rs`), `usage-cli`
  has 2, and `usage-ui/Cargo.toml` adds 1. Plus `Cargo.toml:13`.

Not a contradiction of the plan, but Prompt B works from this list, and a
rename that treats `usage-core` as "one hit" will miss eleven. §F(b) above is
the corrected list.

### L3 — D1 lists the Codex User-Agent flag as unshipped; it is already shipped in the GUI

`m6-ship-program.md:100-101` (D1) lists "the Codex User-Agent CLI flag" among
M5 items that could be cut, and `DECISIONS.md:24` likewise lists it as
pending.

**Check:** `--codex-user-agent` is implemented in **`usage-ui`**
(`main.rs:99`) with three passing tests (`:1141`, `:1147`, `:1153`). It is
**not** in `usage-cli`.

So the item is half-shipped, in the binary the roadmap didn't name. Under D1
("M5a is all of M5"), the honest position is that this flag ships in v1.0 for
the GUI and is absent from the CLI — which needs saying in the CHANGELOG and
means `DECISIONS.md` §2's M5 list is wrong today, not merely frozen.

### L4 — F1 is right but names two files; there are three

`m6-ship-program.md:65-70` (F1) names `SECURITY.md` and `ARCHITECTURE.md` §7
as carrying the false gitleaks claim. `CONTRIBUTING.md:10` carries it too
("CI runs secret scanning"). Three files need editing, not two. (= G03)

### L5 — agreements worth recording

- F1's "`ci.yml` has four jobs … none of them is gitleaks" — **confirmed
  exactly** (`test`, `deny`, `audit`, `no-telemetry`).
- D2's "`ci.yml` contains no product or crate name, so the workflow survives a
  rename untouched" — **confirmed**, zero hits (§F(d)).
- F3's three README defects — **all three confirmed** (G45, G46, G53).
- F2's "no release workflow, no signing, no attestation, no such README
  section" — **all four confirmed**, and it is worse than stated: the claim
  is echoed across four files and understates residual risk R2 (G12).

---

## M. Suspicions — NOT verified, listed separately per the method note

Each of these is a thing I noticed and could not establish with a check I
actually ran. They are **not** findings.

1. **`strip = true` vs build provenance.** `[profile.release] strip = true`
   removes symbols; I did not establish whether that interferes with
   `actions/attest-build-provenance` subject digests (it should not — the
   attestation covers the artifact bytes as built — but I did not verify it,
   and Prompt C is explicitly tasked with resolving this).
2. **Whether `api.github.com` on the allowlist is reachable in practice.** I
   established it has no caller. I did not audit whether some transitive path
   could construct an `Egress` request to it. Reading `egress::get`, every
   call site passes a `const HOST` from a provider module, so I believe not —
   but "I believe not" is a suspicion.
3. **Whether any `prompts/` file contains material the owner would not want
   public** beyond the working-notes concern in D4. I read `m6-ship-program.md`
   and `m6-prep-audit.md` in full for this session's purposes; I did not read
   the other nine end to end for disclosure-sensitivity.
4. **macOS behavior.** CI builds and tests it, but no one has run the window or
   the tray on macOS. `SECURITY.md`/`README.md` make no macOS-specific claim,
   so there is nothing to falsify — but "best-effort" is doing quiet work.
5. **Whether the 90-day embargo / 72-hour acknowledgement commitments in
   `SECURITY.md:23-25` are ones the owner intends to honor.** Not a factual
   gap — a commitment. Flagging because a public security policy that promises
   a 72-hour acknowledgement from a volunteer project is a promise someone
   will eventually measure.
