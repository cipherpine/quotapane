# Goal prompt: M3.5 PHASE A — land tray dep + deny.toml MPL exception (§4a)

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): This session touches §4.1 protected paths (`deny.toml`) but
ONLY as a **§4a verify-and-commit** of bytes authored at the top tier
(Opus 4.8, owner-directed 2026-07-23) and already written to disk. You AUTHOR
nothing in them — you verify byte-for-byte and commit. If any protected file
would need EDITING (not just committing/restoring), STOP (§4.1).

Context: the original tray review under-counted the dependency delta; the real
delta is 9 lock entries (5 shipped + 4 phantom via tray-icon's Linux-only
`dirs`), and phantom `option-ext` is MPL-2.0. Fix (owner-approved Option A): a
narrow per-crate `deny.toml` exception. Full analysis:
prompts/m3.5-tray-dependency-review.md (corrected) and
prompts/m3.5-tray-dep-resolution-handoff.md.

PRECONDITIONS (mismatch = STOP):
P1 `main` tip = 2e63007.
P2 First: `del .git\index.lock` if a stale zero-byte lock exists (the cloud
   bridge can't delete it); if `git status` runs clean, skip.
P3 On disk, top-tier authored (verify, do NOT rewrite):
   - `deny.toml` [licenses] has `exceptions = [{ name = "option-ext",
     allow = ["MPL-2.0"] }]` with a phantom-justification comment.
   - `crates/usage-ui/Cargo.toml` has the target-gated `tray-icon` block.
   - `CONTRIBUTING.md` tray row states the 9-crate/phantom/MPL reality.
   - `prompts/m3.5-tray-dependency-review.md` shows the corrected delta.

PHASE A:
1. Restore CRLF editor-churn on the NON-authored files only (content-identical
   to HEAD — confirm `git diff --ignore-all-space --stat` for them is empty
   first; a real hidden change = STOP):
     git checkout -- .github/workflows/ci.yml Cargo.toml `
       crates/usage-core/Cargo.toml crates/usage-core/src/model/mod.rs `
       crates/usage-core/src/providers/mod.rs
   Do NOT checkout deny.toml / CONTRIBUTING.md / crates/usage-ui/Cargo.toml —
   those are the authored changes.
2. Verify the authored diffs: `git diff deny.toml` must show ONLY the added
   `exceptions` block + its comment (nothing else changed). `git diff
   CONTRIBUTING.md crates/usage-ui/Cargo.toml` must match P3. Any surprise = STOP.
3. `cargo check --workspace` — Cargo.lock gains exactly these 9: tray-icon,
   muda, crossbeam-channel, crossbeam-utils, keyboard-types, dirs, dirs-sys,
   option-ext, redox_users. Any OTHER new crate = STOP.
4. `cargo deny check` — MUST report `advisories ok, bans ok, licenses ok,
   sources ok`. If licenses still fail, STOP (the exception didn't take).
5. §3 verification bar: `cargo test --workspace --locked`,
   `cargo fmt --all --check`,
   `cargo clippy --workspace --all-targets --locked -- -D warnings`.
6. Remove the now-superseded top-tier prompt (we authored on Opus instead):
   `del prompts\m3.5-tray-dep-fix-TOPTIER.md`
7. Stage exactly: crates/usage-ui/Cargo.toml CONTRIBUTING.md deny.toml
   Cargo.lock prompts/m3.5-tray-dependency-review.md
   prompts/m3.5-tray-dep-resolution-handoff.md prompts/land-m3.5-phaseA.md
   prompts/finish-m3.5-tray.md
   Commit: "M3.5: land tray-icon dep + deny.toml MPL-2.0 exception for phantom
   option-ext (top-tier reviewed, corrected delta)"
8. `git push`. Record the commit SHA + Actions URL. All jobs green — especially
   `windows-latest` and the `cargo-deny` job. Unexplained red = §4.6 STOP.

END GATE — STOP. Report: commit SHA, CI run + all job results, the `cargo deny
check` output line, test-count delta, and any deviation. Do NOT start Phase B.

NEXT (separate floor Sonnet session): Phase B = the tray implementation, using
ONLY the Phase B section of prompts/finish-m3.5-tray.md. Its Phase A / "any
OTHER new crate = STOP" guard is now STALE — ignore it; the lock legitimately
carries the 9 tray entries and `cargo deny check` passes. Never touch §4.1
paths beyond this commit. Never capture the screen.
