# Goal prompt spec: M13-RELEASE — v1.6.0 (the memory release)

Authored at the standing top tier 2026-08-05, instantiated from
prompts/release-template.md, three-leg headless protocol as executed in
M12-RELEASE (prompts/m12-release.md): a hard stop is a SESSION END; Leg
B's queue file is written only by the top tier (that act is the written
go-ahead); Leg C only after the owner confirms publication. Owner
accepted M13 + M13-R1 on 2026-08-05 (§4.5 rounds 1 and 2 complete;
owner noted a production re-look after release — record it, nothing
gates on it). Read CLAUDE.md, then DECISIONS.md — §4 overrides all.
Verification is tools/release-verify.sh verbatim; a tooling failure is
a STOP.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject "prompts: M13-RELEASE spec + launchers — v1.6.0";
   parent 1309957 "reports: M13-R1 end-gate". Tree clean. CI green on
   ed4f130 and 8cb622c lineage (all 8 required checks). Version 1.5.0
   in the workspace Cargo.toml. 386 tests.
P2 Tags exactly v1.0.0–v1.5.0; v1.5.0 is Latest. No M13 stamp in
   DECISIONS.md — created in Leg C.

## LEG A — Phase 1 (version + CHANGELOG + cleanup) and Phase 2 (rc)

Phase 1, ONE commit ("release: v1.6.0"): workspace 1.5.0 -> 1.6.0
(lock moves only the three members); `git rm
tools/m13-apply-p1-patches.py` (top-tier ruling: the report is the
audit trail, one-shot tools do not accumulate); insert into
CHANGELOG.md immediately above `## [1.5.0]` this entry VERBATIM (no foot link-reference):

## [1.6.0] - 2026-08-05

The memory release: QuotaPane learns to remember — and to speak up.

### Added

- **Quota history (opt-in).** Set `history=on` and QuotaPane keeps
  `history.jsonl` next to its config — timestamps, window labels, and usage
  percentages, nothing else. Burn-rate forecasts now survive restarts: the
  pace ring reseeds from the last two hours on launch.
- **24-hour sparklines.** With history on, each provider grows a small
  painter-drawn strip under its bars — the last day of headline usage as a
  quiet shape with a "now" dot and a `24h` tag. Silent when history is off
  or there is nothing to draw.
- **Alerts (opt-in, dep-free).** Set `alerts=on` and QuotaPane speaks when
  it matters: a banner names the worst offender
  (`alert: claude 7d at 85% >= 80% (pace)`), the tray miniature gains a
  cardinal ring, its tooltip is prefixed `ALERT — `, and the taskbar asks
  for attention once per crossing. The default `pace` mode is time-aware —
  a healthy 85% late in the week stays quiet; the same number early in the
  week alerts. `alert_mode=threshold` gives the simple version;
  `alert_at=<1-100>` sets the line (default 80). A quiet `refilled:` note
  re-arms each alert when its window resets.
- **`config.cfg`.** The one-word `theme.cfg` grows into a small key=value
  file (`theme`, `history`, `alerts`, `alert_at`, `alert_mode`) — still
  hand-parsed, still boring on purpose, still incapable of holding a
  secret. Legacy `theme.cfg` is read as a fallback, never written again.

### Changed

- SECURITY.md invariant 1 now names the two files QuotaPane may write —
  `config.cfg`, and `history.jsonl` when history is on — with the same
  machine-checked traceability as every other claim.
- `--pace-demo` now demos everything: synthetic history, sparklines, and a
  forced alert, in a window sized to show it all.

Defaults ship OFF: a fresh install writes nothing new and renders exactly
as v1.5.0 did. No JSON key changed in this release. Zero new dependencies.

§3 bar + python3 tools/check-invariants.py, commit, push, CI green on
all 8 required checks before any tag.

Phase 2: tag v1.6.0-rc.1 on the Phase 1 commit. Release run 3/3.
`tools/release-verify.sh v1.6.0-rc.1` verbatim (rc base-version
handling is in the script since 9ced94c). Content spot-check: both
shipped GUI binaries contain `alert: ` and `24h`; shipped README
carries "Theming and preferences". Write reports/m13-release-rc.md,
commit that one file, push, CI green, EXIT. No v1.6.0 tag after Leg A.

## LEG B — Phase 3 (queued ONLY by the top tier)

Confirm reports/m13-release-rc.md on main and no v1.6.0 tag (else
STOP). Tag v1.6.0 on the "release: v1.6.0" commit. Release run 3/3.
`tools/release-verify.sh v1.6.0` fresh; only after RESULT: PASS prune
the rc tag and draft. Write reports/m13-release-draft.md with the
draft URL, commit, push, CI green, EXIT. The owner publishes.

## LEG C — Phase 4 (queued ONLY after the owner confirms publication)

Verify publishedAt non-null via gh. One §4a replacement, DECISIONS.md
only, OLD/NEW extracted programmatically from THIS spec, unique before
and after:

OLD: closed by an owner live run (owner decisions 2026-08-04). Post-1.0 backlog:
NEW: closed by an owner live run (owner decisions 2026-08-04). · **M13 pace follow-ons ✅ (v1.6.0 published — owner-accepted 2026-08-05; owner will re-look at sparklines in production)**: config.cfg key=value preferences (theme.cfg migrated, read as fallback, never written again); opt-in history.jsonl (timestamps/labels/percentages only, 256 KiB keep-newest-half) reseeding the pace ring at launch; 24h painter sparklines, legibility-iterated in M13-R1 after round-1 §4.5 feedback (full-alpha stroke + fill + now-dot + `24h` tag, demo window sized to fit); dep-free time-aware alerts — banner, cardinal tray ring, `ALERT — ` tooltip, RequestUserAttention — with OS toasts declined by ADR (tray-icon exposes no balloon API; the window is always-on-top) and threshold mode as fallback for unknown-duration windows. Invariant 1 rewritten under the M11 checker in the same commit as the behavior. First slice §4.5-reviewed across two rounds fully headless (owner decisions 2026-08-04/05). Post-1.0 backlog:

Write reports/m13-release-endgate.md (full ledger incl. this commit's
CI recorded in a follow-up commit per the M13 pattern). Commit
("docs: v1.6.0 published; M13 accepted (owner)"), push, CI green, EXIT.

## DO NOT

Publish anything. Touch code, .github/, assets/, README.md, or any
§4.1 path (Leg C's DECISIONS patch excepted, verbatim; Leg A's
authorized `git rm` of tools/m13-apply-p1-patches.py excepted). Add
any dependency. Proceed past any leg boundary.
