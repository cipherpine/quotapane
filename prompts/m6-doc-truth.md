# Goal prompt spec: M6-DOCS — make every claim in this repo true

Authored at the top tier (Cowork bridge) 2026-07-28, from the gap report
(`prompts/m6-gap-report.md`, 76b6421) with Prompt D landed (ea134d6). This
is **Prompt E**; it supersedes the "Prompt E" section (and its bracketed
TOP TIER placeholder) in `prompts/m6-prompts-b-to-g.md`.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): mixed, route (b). The two §4.1 documents — `SECURITY.md`
and `THREAT_MODEL.md` — were fully re-authored at the top tier and are
ALREADY ON DISK in your working tree; you verify (md5 below), diff-review,
and commit them under §4a, authoring nothing. `README.md`,
`CONTRIBUTING.md`, and `ARCHITECTURE.md` are YOURS to author in commit 1,
against the checklist below. Owner decisions folded in: D5 = there is NO
e-mail security contact (GitHub private vulnerability reporting only — the
pre-authored SECURITY.md already says so); D1 froze M5 at M5a; D6 ships
Windows+Linux binaries, macOS build-from-source; the public flip precedes
the v1.0.0 tag.

## PRECONDITIONS (mismatch = STOP and report)

P1 `main` tip is ea134d6, CI green (7 check runs incl. gitleaks), and
   `git status --porcelain` shows EXACTLY:

   ```
    M SECURITY.md
    M THREAT_MODEL.md
    M prompts/m6-launchers.md
   ?? prompts/m6-doc-truth.md
   ```

P2 md5 of the pre-authored files:

   | file | md5 |
   |---|---|
   | `SECURITY.md` | `9c76194146d3ff104710a73a073d7d57` |
   | `THREAT_MODEL.md` | `d92f6954075dceffdab2c6c645594336` |

## WHAT THE PRE-AUTHORED BYTES CHANGE (diff review, informed not ritual)

SECURITY.md: name final (no "working name"); D5 resolved — the backup
e-mail line is GONE, GitHub PVR is the only channel; invariant 1 states no
files are written at all (no config layer — G30); invariant 5 becomes "no
update mechanism exists at all" (G17); WSL clause removed (G31); network
policy describes exactly TWO allowlisted hosts, records the 2026-07-27
removal of api.github.com, and says plainly that certificate pinning is
NOT implemented (G13/G18/G33); build & release integrity now describes the
real release.yml pipeline (G04–G07) and points at README's "Verify a
release" section — which YOU create in commit 1; supply-chain describes
the real full-history gitleaks job and deliberately drops the pre-commit
claim (G01); hardening: build.rs reference removed (no build.rs exists),
"disable the update check" removed, and §3's command is now
`quotapane-cli --once --json` (G38); the invariants preamble is honest
about behavior-tests vs absence-enforcement (G25).

THREAT_MODEL.md: name final; T-S1 and the adversary table drop the pinning
claim (G15); T-T1 describes the real pipeline (G11); the repudiation line
stops claiming optional debug logs (none exist — deny.toml bans logger
backends); T-D1 drops the deferred fallback and the fictional
user-configurable cadence; T-E2 states the stronger truth (no updater at
all — G21); R1 no longer instructs "Enable pinning" (G16); R2 is honest
that the signing identity is CI itself (G12); §9's traceability table now
names REAL tests row by row, including the new end-to-end failure-path
redaction test, and is explicit about which invariants rest on absence
(G23/G24).

## EXECUTE — four commits, in order, one push

0. `prompts/m6-doc-truth.md` (this file) + `prompts/m6-launchers.md`:
       docs(prompts): add M6-DOCS spec and launcher (Prompt E)

1. YOURS — `README.md` (full rewrite), `CONTRIBUTING.md`,
   `ARCHITECTURE.md` (corrections). Commit:
       docs: rewrite README for v1.0 reality; correct CONTRIBUTING and
       ARCHITECTURE

   README.md, a real rewrite for a stranger arriving at a public repo:
   - No "Working name" banner, no milestone codes in the status line.
     Say what it DOES: always-on-top window; Claude 5h/7d; Codex 7d with
     a per-model breakdown toggle; headless `quotapane-cli`.
   - Accurate roadmap: M4 withdrawn (ADR-002, security grounds); v1.0 is
     the current scope; post-1.0: history/sparklines, forecast-to-limit,
     thresholds/alerts, `OtelSource`, packaging (WinGet/Homebrew/AUR).
   - "Requires Rust 1.92+" (Cargo.toml `rust-version`, floor set by
     eframe 0.35) — not 1.85.
   - Binaries are `quotapane` / `quotapane-cli`; build commands use
     `--locked`.
   - Install: download from GitHub Releases — Windows and Linux archives;
     macOS is build-from-source (D6).
   - **"Verify a release" section** — SECURITY.md points at it, so it
     must exist and be correct against release.yml: `sha256sum -c` of
     SHA256SUMS against the downloaded archive; `cosign verify-blob` of
     SHA256SUMS with `--signature SHA256SUMS.sig --certificate
     SHA256SUMS.pem --certificate-identity-regexp` for this repo's
     workflow and `--certificate-oidc-issuer
     https://token.actions.githubusercontent.com`; `gh attestation verify
     <archive> --repo cipherpine/quotapane`. Note in a comment that
     Prompt F's rc dry-run validates these commands verbatim and corrects
     them from its transcript if reality differs.
   - Update posture: there is NO auto-update and no update check (G22).
   - The DISCLAIMER's substance is untouchable: undocumented endpoints,
     own credentials only, bypasses no authentication, presents as the
     official clients via their User-Agents. Reword for flow if you like;
     soften nothing.

   CONTRIBUTING.md: the secret-scanning line (G03) now matches reality —
   full-history gitleaks in CI, no pre-commit claim; describe the six CI
   jobs; confirm the dependency table still matches the tree.

   ARCHITECTURE.md — correct every audited fiction, either to reality or
   into an explicitly-marked "Future (not implemented)" block; never leave
   a plan in present tense: G02 (secret hygiene line), G08+G09+G10
   (release pipeline now real — describe release.yml, fix the tree
   comment and the threat table row), G14 (no pinning), G19+G20 (no
   api.github.com, no update mechanism), G26–G28 (the preferences/config
   layer does not exist — mark the whole §as future design, note position
   does not persist and there is no compact mode), G29 (interactions:
   drag to move, scroll only scrolls, inline disclosure toggle, no
   right-click menu), G32 (no WSL), G34+G35 (Messages-API fallback is
   deferred, `RateLimitHeaders` is a dead variant kept as a placeholder),
   G37 (no reset-snapping; note both providers currently pin
   `Cadence::Normal`, so Fast/Slow are dormant).

2. §4a — `SECURITY.md` alone. Verify md5, read the full diff (every hunk
   must be a claim becoming accurate, the D5 resolution, or the final
   name). Commit:
       docs(security): reconcile SECURITY.md with shipped reality

3. §4a — `THREAT_MODEL.md` alone. Same discipline. Commit:
       docs(security): reconcile THREAT_MODEL.md — pinning, updater,
       traceability made honest

## TESTS

- `cargo test --workspace` still green, same count (147) — docs only; if
  a test asserts on doc content and breaks, that is information: STOP and
  report rather than editing the test.
- Mechanical link check: every relative link and every named file/section
  in all five edited documents resolves. The audit found two dangling
  pointers; the bar is zero.
- Greps that must come back empty OUTSIDE `prompts/` (historical records
  stay): `working name`, `<DOMAIN>`, `usage-cli --json`, `gitleaks.*pre-commit`,
  `Enable pinning`, `build.rs`.

## VERIFY + SHIP

Push all four commits. CI green — all 7 check runs, gitleaks included.
`git diff HEAD~4..HEAD -- .github .cargo .claude deny.toml crates/` must
be empty (this prompt touches no code and no workflow).

## END GATE — STOP

Report: four SHAs; CI; the closed-vs-deferred table — EVERY G## finding
from `prompts/m6-gap-report.md` is either closed (by B, D, or this prompt,
say which commit) or deferred with a reason. Expected deferrals, verify
rather than assume: G36 (dead `RateLimitHeaders` variant — post-1.0 code
cleanup), the dormant-cadence half of G37 (post-1.0), and final
confirmation of README's verify commands (Prompt F's rc transcript).
Then STOP. Do not start Prompt F; the flip and the history scan (Prompt G
phase 1) come first under the reordered gates, and both need the owner.
