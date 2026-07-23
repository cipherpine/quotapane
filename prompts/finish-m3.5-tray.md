# Goal prompt: M3.5 SYSTEM TRAY (compact)

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Phase A lands §4.2/§4.1-adjacent bytes as a **§4a
verify-and-commit** of top-tier-authored files already on disk. You author
NOTHING in them. Phase B is ordinary usage-ui work (non-boundary).

PRECONDITIONS (mismatch = STOP):
P1 main tip = 2e63007.
P2 On disk (top-tier authored — verify, don't edit):
   - crates/usage-ui/Cargo.toml contains a
     [target.'cfg(any(target_os = "windows", target_os = "macos"))'.dependencies]
     table with tray-icon = { version = "0.24", default-features = false }.
   - CONTRIBUTING.md dependency table has a `tray-icon` (+ `muda`) row.
P3 git status shows ONLY: those 2 modified files; ?? prompts/
   m3.5-tray-dependency-review.md; and possibly 7 CRLF-churned files
   (.github/workflows/ci.yml, CONTRIBUTING.md*, Cargo.toml,
   crates/usage-core/Cargo.toml, crates/usage-core/src/model/mod.rs,
   crates/usage-core/src/providers/mod.rs, deny.toml). *CONTRIBUTING.md is
   expected modified regardless (authored row).

PHASE A — §4a land the dependency:
1. If churn files present: `git diff --ignore-all-space --stat` for them must
   show only CONTRIBUTING.md/usage-ui Cargo.toml content; then
   `git checkout --` the OTHER churned files (never the 2 authored ones).
   A real hidden content change = STOP.
2. Review `git diff` of the 2 authored files: additions must match P2's
   description (tray-icon dep block + one table row) and nothing else.
3. `cargo check --workspace` (updates Cargo.lock with the 5 reviewed crates:
   tray-icon, muda, crossbeam-channel, crossbeam-utils, keyboard-types —
   any OTHER new crate = STOP).
4. Run the §3 verification bar. Commit the 2 files + Cargo.lock +
   prompts/m3.5-tray-dependency-review.md as
   "M3.5: add tray-icon dependency (top-tier reviewed) + review doc". Push;
   CI all green (esp. windows-latest) or §4.6 STOP.

PHASE B — tray implementation (edit ONLY crates/usage-ui/):
- New cfg-gated module (windows/macos): runtime-generated RGBA icon
  (simple two-bar gauge or "Q" monogram drawn in code — NO asset files, NO
  build scripts, NO png decoding).
- Create the tray on the MAIN thread after eframe init (both OSes require
  it). Forward TrayIconEvent + MenuEvent via their set_event_handler into a
  std::sync::mpsc channel drained in update() (1s repaint already exists).
- Tooltip: live summary built from the same ProviderSnapshots the window
  renders, e.g. "Claude 5h 42% | Codex 7d 3%" ("--" when unknown).
- Left-click: show + focus window (ViewportCommand::Visible/Focus).
- Menu: Show/Hide toggle, separator, Quit.
- Close button hides to tray instead of quitting (ViewportCommand::
  CancelClose then hide) WHEN tray is active; Quit exits via menu and stops
  all poller handles.
- `--no-tray` flag (hand-rolled parsing, like existing flags): disables the
  tray and restores close-to-quit. Non-tray targets (Linux) compile to
  exactly today's behavior; the flag is accepted-and-ignored there.
- Unit-test every new pure helper (tooltip formatting incl. edge cases,
  icon pixel generation invariants, flag parsing). TDD encouraged.
- Verification bar; commit "M3.5: system tray (icon, tooltip, show/hide,
  quit)"; push; record CI.

END GATE — STOP. Report: commits + CI runs (all jobs), test-count delta,
deviations, §4 touchpoints. Owner checklist (his eyes only — NEVER capture
the screen):
1. cargo run -p usage-ui -- --client-version <claude --version>
   → tray icon appears; tooltip shows both providers; left-click +
   Show/Hide + Quit work; close button hides to tray.
2. cargo run -p usage-ui -- --no-tray → old behavior.
Do NOT start the window-look polish pass or M4. Never touch §4.1 paths
beyond Phase A's verify-and-commit. Never capture the screen.
