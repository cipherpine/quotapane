# Goal prompt spec: M9-RELEASE — v1.4.0 (the security-review release)

Authored at the standing top tier 2026-07-30. The launcher paste is
the owner's acceptance of the M9 program (§4.8): eleven findings
found, verified, and remediated across M9a/M9b/M9c.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
Exact prior-release discipline, TWO HARD STOPS, both mandatory:
Phase 2 ends in a report and a WAIT; Phase 3 runs only on the top
tier's explicit written go-ahead in this session. The release-verify
standard applies in full: digest-match BOTH attestation subjects, and
every negative control asserted on its SPECIFIC error, restored
between controls, never stacked.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "docs: correct DECISIONS §1 disclosure claim; add
   M9-RELEASE spec"; parent e0cd6b0. Tree clean. CI 7/7 green on tip
   (the SHA-pinned workflow). Version 1.3.0.
P2 DECISIONS.md M9 entry present, no ✅. Tags exactly v1.0.0–v1.3.0;
   v1.3.0 is Latest.

## PHASE 1 — version + CHANGELOG (one commit)

Bump 1.3.0 → 1.4.0 (root Cargo.toml; Cargo.lock moves only the three
workspace members). Insert above [1.3.0], verbatim except <DATE> =
the commit's UTC date, re-wrapped to 80 columns, LF only:

## [1.4.0] - <DATE>

The security-review release: an owner-commissioned adversarial review
of the codebase and its claims produced eleven findings — most of
them places where the documentation said more than the code did.
Every finding was independently re-verified and every fix ships here,
each behavior change landing in the same commit as the claim it
affects. The review process itself is committed under `prompts/`.

### Security

- **`--debug-raw` now redacts by default.** Identifier fields
  (`email`, `user_id`, `account_id`, `id`) are replaced with
  `«redacted»` at any depth before printing; bodies that are not
  valid JSON are withheld. New `--debug-raw-unsafe` restores the
  byte-exact dump after one warning. Existing invocations keep
  working — safer.
- **Proxy support is now reachable, CLI-only, and fail-closed.** New
  `quotapane-cli --allow-proxy` opts a single run into the proxy
  environment after a printed warning that a TLS-inspecting proxy can
  observe the bearer token. Without it, behavior is unchanged: a set
  proxy variable (either casing) means nothing is sent and the run
  exits with an error naming the variable — now followed by a hint
  pointing at the flag. The window has no opt-in surface at all.
- **A hostile `Retry-After` can no longer stall polling.** The
  provider's hint is honored only up to the 30-minute backoff cap.
- **Expired tokens recover fast.** An auth error retries at the
  polling floor instead of escalating backoff, so a token refreshed
  via the official CLI is noticed within minutes, not up to thirty.
- **Provider timestamps are validated against the real calendar**
  (month lengths, leap years) and must carry a UTC offset; anything
  else is rejected and renders as unknown rather than as a wrong
  reset time.
- **Every CI action is now pinned to a full commit SHA**, matching
  the release workflow, and cargo-audit installs at a pinned version.

### Documentation truth

The review's larger half: claims aligned with shipped reality. The
persistence invariant now names `theme.cfg` (one word, preferences
only) instead of claiming "no files at all"; TLS validation is
correctly described as the bundled WebPKI (Mozilla) root set — the OS
trust store is not consulted, so an OS-installed interception CA is
rejected rather than trusted, and the threat model's mitigation
advice was rewritten to match; zeroization claims are scoped to
buffers QuotaPane owns; the auditable surface honestly includes the
provider parsers and the CLI raw-debug path; and text-mode CLI output
is documented as a summary (per-model rows and reset credits appear
in `--json` and the window). No JSON key changed in this release.

Commit: release: 1.4.0 — the security-review release

Full bar first from cargo clean -p usage-core (expect 286 tests).
Push; CI 7/7 green BEFORE any tag.

## PHASE 2 — rc dry run, then HARD STOP

Tag v1.4.0-rc.1 on the Phase 1 commit. Release run 3/3 green,
release.yml untouched. Six-step outsider verification in a clean
directory + six negative controls per the standard. Then HARD STOP:
report and WAIT. No v1.4.0 tag, nothing published.

## PHASE 3 — on the top tier's explicit go-ahead ONLY

Tag v1.4.0 on the verified commit. Re-run all six steps fresh against
the v1.4.0 draft. Only after clean verification, prune the rc tag and
draft. Hand back the draft URL and STOP — the owner publishes.

## PHASE 4 — after the owner confirms publication (one commit)

§4a byte-match, replace exactly once, DECISIONS.md only:
OLD: **M9 security-review remediation — underway 2026-07-30 (v1.4.0 scope)**:
NEW: **M9 security-review remediation ✅ (v1.4.0 published — owner-accepted 2026-07-30)**:
Commit: docs: v1.4.0 published; M9 accepted (owner)
Push, CI 7/7 green, STOP. Nothing further queued.

## DO NOT

Publish anything. Touch .github/, assets/, README.md, or any §4.1
path (Phase 4's DECISIONS patch is the sole exception, verbatim).
Change code. Add any dependency. Skip either stop.
