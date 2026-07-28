# Goal prompt spec: M6-RELEASE — cut v1.0.0 (Prompt F, final form)

Authored at the top tier (Cowork bridge) 2026-07-28. SUPERSEDES the
"Prompt F — v1.0.0" section of `prompts/m6-prompts-b-to-g.md`, which
predates the gate reorder, the history rewrites, and the public flip.
This file governs.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): floor tier; touches NO §4.1 path. release.yml already
exists and you do not edit it — if the release workflow needs ANY fix,
that is a STOP and a top-tier authoring pass, never an inline edit;
editing a workflow mid-release to make a release succeed is how
unreviewed bytes reach a signing job.

**CORRECTED TRANSPOSITION RULE (supersedes both rewrite specs' wording):**
filter-repo's commit-map is CUMULATIVE — `prompts/m6-sha-map-2.txt`'s old
column already holds the ORIGINAL pre-G½ SHAs. To resolve any SHA cited
in documents written before the rewrites, look it up in
**m6-sha-map-2.txt ALONE**. Do not chain through m6-sha-map.txt first;
that produces empty lookups (confirmed empirically in G phase 2).
m6-sha-map.txt remains committed as the G½ record only.

## PRECONDITIONS (mismatch = STOP and report)

P1 The repo is PUBLIC: `gh api repos/cipherpine/quotapane
   --jq .private` → `false`. If still private, STOP — attestation will
   fail and the flip is the owner's act, not yours.
P2 `main` tip is 7cf747a (CODE_OF_CONDUCT), CI green, tree clean EXCEPT:
   ```
    M prompts/m6-launchers.md
   ?? prompts/m6-release.md
   ```
P3 `git tag` is empty. Workspace version is 0.1.0 (Cargo.toml:10).
P4 DECISIONS.md marks M5a ✅ accepted and M5 frozen at M5a (D1). No
   feature work is pending anywhere — a release is not the place to
   discover a stray change.

## PHASE 0 — commit this spec

`prompts/m6-release.md` + `prompts/m6-launchers.md`:
    docs(prompts): add M6-RELEASE spec (Prompt F, final form)

## PHASE 1 — version + changelog (one commit)

- Cargo.toml `[workspace.package]` version 0.1.0 → 1.0.0. Run
  `cargo build --locked` to update Cargo.lock — ONLY the three workspace
  members' versions may move; if any third-party entry changes, STOP.
- Write `CHANGELOG.md`, Keep-a-Changelog format, one `## [1.0.0]` entry.
  Write it for a stranger: what the app does (always-on-top window,
  Claude 5h/7d, Codex 7d + per-model breakdown, headless CLI), the
  security posture (two-host compile-time allowlist, Secret<T>
  zeroize/redaction, no telemetry, no updater, read-only credentials,
  proxy opt-in), and the release-integrity story (CI-only builds, signed
  checksums, provenance). Note the 2026-07-28 history identity rewrite
  as a line item — it is part of this repo's honest record. Milestone
  codes at most in parentheses.
- Commit: `release: 1.0.0 — version bump and CHANGELOG`
- Push. CI green (7 checks) BEFORE any tag.

## PHASE 2 — release-candidate dry run. THIS IS THE REAL TEST.

- Tag `v1.0.0-rc.1` on the phase-1 commit; push the tag; let release.yml
  run end to end. If the workflow fails: STOP, report the log, hand back
  — no inline workflow edits, no re-tagging with tweaks.
- Then verify the DRAFT release AS AN OUTSIDER, pedantically, recording
  every command and its exact output:
  1. Fresh-download every asset (both archives, SHA256SUMS, .sig, .pem)
     via `gh release download v1.0.0-rc.1` into a clean directory.
  2. `sha256sum -c SHA256SUMS` (or `certutil` equivalents) — both
     archives pass.
  3. `cosign verify-blob --signature SHA256SUMS.sig --certificate
     SHA256SUMS.pem --certificate-identity-regexp
     "github.com/cipherpine/quotapane" --certificate-oidc-issuer
     https://token.actions.githubusercontent.com SHA256SUMS` → Verified.
  4. `gh attestation verify <each archive> --repo cipherpine/quotapane`
     → verified provenance, and the commit SHA in the provenance matches
     the tagged commit.
  5. Extract each archive: both binaries + LICENSE-MIT + LICENSE-APACHE
     + README.md + TOOLCHAIN.txt present; TOOLCHAIN.txt contains real
     `rustc -V` / `cargo -V` lines.
  6. Run the Windows CLI from the extracted archive:
     `quotapane-cli --help` → exit 0; `quotapane-cli --version` →
     `quotapane-cli 1.0.0`.
- Diff your transcript against README's "Verify a release" section. If
  any command or flag differs from what actually worked, fix README from
  the transcript (one commit: `docs: verify-a-release commands confirmed
  against the v1.0.0-rc.1 run`) — this closes the last deferral from
  Prompt E.
- **HARD STOP.** Report everything. Tagging v1.0.0 is the owner's call
  (§4.8): a published 1.0.0 with a broken signature is permanent.

## PHASE 3 — only on the owner's explicit go-ahead in a later turn

- Tag `v1.0.0` on the same verified commit; push; let release.yml run;
  re-verify the new draft exactly as phase 2 (all six steps — the rc
  verification does not transfer).
- Delete the rc tag and its draft release ONLY after v1.0.0 verifies.
- Hand the owner the draft-release URL. **YOU DO NOT PUBLISH.** The
  final click is the owner's, and so is the DECISIONS.md acceptance
  stamp afterward (§4.8).

## DO NOT

Publish any release; edit release.yml or any workflow; run the app
against real credentials; force-push; delete or rewrite any tag other
than the rc you created; bump any dependency; touch any §4.1 path.

## END GATE (phase 2) — STOP

Report: the phase-1 SHA; CI run; the rc tag and its workflow run; the
full outsider-verification transcript with each of the six steps'
outcomes; whether README needed correction; and your confidence that a
stranger following README verbatim reproduces your result. Then wait for
the owner's phase-3 go-ahead.
