# Goal prompt spec: M12-RELEASE — v1.5.0 (the automation release)

Authored at the standing top tier 2026-08-04, instantiated from
prompts/release-template.md (first use). The Leg-A queue file is the
owner's acceptance of M12 (§4.8), verified at the top tier against the
device and the live CI API on 2026-08-04.

Model: floor. Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.

**Headless decomposition.** This release runs as THREE dispatched
sessions (Leg A / B / C below). A hard stop is a SESSION END: finish
the leg, write its report in-tree, push, and exit. You never proceed
past a leg boundary; the next leg's queue file is written by the top
tier (Leg B — that act IS the written go-ahead) or after the owner's
publish confirmation (Leg C). If your queue file's leg does not match
the repository state (e.g. Leg B queued but no rc verification report
exists), STOP.

Verification is tools/release-verify.sh — run it verbatim and paste
its full output into the leg report; a tooling failure (not a
verification failure) is a STOP, not license to improvise.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "prompts: M12-RELEASE spec + launchers — v1.5.0";
   parent 812ac32 "reports: M12 end-gate". Tree clean. CI green on
   812ac32 (all 8 required checks; the spec commit is prompts-only).
   Version 1.4.1 in the workspace Cargo.toml. 321 tests.
P2 Tags exactly v1.0.0–v1.4.1; v1.4.1 is Latest. No M12 stamp in
   DECISIONS.md yet — the stamp is created in Leg C.

## LEG A — Phase 1 (version + CHANGELOG) and Phase 2 (rc dry run)

Phase 1: workspace version 1.4.1 -> 1.5.0. Cargo.lock may move ONLY
the three workspace members. Insert into CHANGELOG.md, immediately
above the `## [1.4.1]` heading, this entry VERBATIM (no
link-reference line at the foot):

## [1.5.0] - 2026-08-04

The automation release: the CLI becomes a quota gate for scripts and
agents.

### Added

- **`--fail-at <N>`.** After polling, if any reported window — headline or
  per-model — has reached N% used, the CLI prints one stderr line naming the
  worst offender (`fail-at: claude 5h at 92% >= 90%`) and exits with code 3.
  The gate deliberately covers every window the providers report: a per-model
  quota can block your run just as surely as a headline one. Scripts wanting
  narrower semantics can filter `--json` themselves.
- **`--watch <SECS>`.** A second mode alongside `--once`: poll on an
  interval (minimum 180 seconds — the same floor the window's own poller
  honors). Text mode prints each cycle under an RFC 3339 timestamp
  separator; with `--json`, output is NDJSON — one compact line per cycle,
  readable as a stream. Combined with `--fail-at`, the first tripping cycle
  exits 3.
- **Documented exit codes** in `--help`: 0 success (with `--fail-at`: all
  windows under the threshold), 1 provider or credential error, 2 usage
  error, 3 gate tripped.
- **A written JSON stability contract** (`docs/cli-json.md`): every `--json`
  key documented with type and nullability, plus the policy — keys are never
  renamed or removed within a major version; new keys may be added in any
  release and are announced here; consumers must ignore keys they do not
  recognize.

### Changed

- `--once` is no longer described as the only mode; the usage line reads
  `(--once | --watch <SECS>)`. `--once --json` output is byte-for-byte
  unchanged; `--watch --json` differs only in whitespace.

No JSON key was added, removed, or renamed in this release. Zero new
dependencies.

§3 bar (incl. python3 tools/check-invariants.py), commit
("release: v1.5.0"), push, CI green on all 8 required checks before
any tag.

Phase 2: tag v1.5.0-rc.1 on the Phase 1 commit. Release run 3/3
green, release.yml untouched. Then in Git Bash:

    tools/release-verify.sh v1.5.0-rc.1

Content spot-check: both shipped quotapane-cli binaries contain
`fail-at: ` and the `exit codes:` help block; the shipped README
carries the Verify section unchanged.

Write the full Leg-A report (preconditions table, Phase 1 facts,
complete release-verify output, spot-check evidence) to
reports/m12-release-rc.md, commit exactly that file
("reports: M12-RELEASE Leg A — rc verified"), push, CI green, EXIT.
No v1.5.0 tag exists after Leg A. HARD STOP = session end.

## LEG B — Phase 3 (only ever queued by the top tier)

Re-verify preconditions: reports/m12-release-rc.md exists on main and
the v1.5.0 tag does NOT exist. Tag v1.5.0 on the Phase-1 commit
(rc-verified; name it by its subject "release: v1.5.0"). Release run
3/3. Then:

    tools/release-verify.sh v1.5.0

Only after RESULT: PASS, prune the rc tag and rc draft. Write the
Leg-B report (fresh verify output, draft URL, digests) to
reports/m12-release-draft.md, commit ("reports: M12-RELEASE Leg B —
draft verified"), push, CI green, EXIT. The draft URL is in the
report; the owner publishes (pasting the release body — the
CHANGELOG entry plus the standard verify footer — before clicking).

## LEG C — Phase 4 (only queued after the owner confirms publication)

Verify publishedAt is non-null via gh. Then two §4a replacements,
DECISIONS.md only, OLD/NEW extracted programmatically from THIS
spec's bytes, each unique before and after:

Patch A:
OLD: the top tier's Phase-2→3 go-ahead (owner decisions 2026-08-03). Post-1.0 backlog:
NEW: the top tier's Phase-2→3 go-ahead (owner decisions 2026-08-03). · **M12 CLI automation ✅ (v1.5.0 published — owner-accepted 2026-08-04)**: `--fail-at <N>` (exit 3; the gate covers every reported window incl. per-model buckets — proven live by the owner tripping on a per-model quota the text summary doesn't even display), `--watch <SECS>` (180s floor, RFC 3339 separators, NDJSON), documented exit codes, docs/cli-json.md stability contract; zero new deps, no JSON key changed. First slice executed end to end under the M11d headless dispatcher (owner touch: one trust dialog and pushes); the floor mutation-tested its own tests 8/8, found and closed two of its own escapes, and honestly flagged the one untested hop (exit-3 wiring), closed by an owner live run (owner decisions 2026-08-04). Post-1.0 backlog:

Write the final ledger report to reports/m12-release-endgate.md.
Commit both ("docs: v1.5.0 published; M12 accepted (owner)"), push,
CI green on all 8 required checks, EXIT. Nothing further queued.

## DO NOT

Publish anything. Touch code, .github/, assets/, README.md, or any
§4.1 path (Leg C's DECISIONS patch is the sole exception, verbatim).
Add any dependency. Proceed past any leg boundary.
