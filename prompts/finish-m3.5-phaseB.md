# Goal prompt: M3.5 PHASE B — system tray implementation

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): Phase B is `usage-ui`-only, NON-boundary. No §4.1 path is
touched; the tray dependency + deny.toml exception already landed in Phase A
(commit d45e0fc). If any task here would touch a §4.1 path, STOP — it doesn't.

PRECONDITIONS (mismatch = STOP and report):
P1 `main` tip = d45e0fc ("M3.5: land tray-icon dep + deny.toml MPL-2.0
   exception…"). If a stale 0-byte `.git\index.lock` exists, `del` it first.
P2 `crates/usage-ui/Cargo.toml` already has the target-gated
   `[target.'cfg(any(target_os="windows", target_os="macos"))'.dependencies]
   tray-icon = { version = "0.24", default-features = false }`.
P3 `cargo deny check` passes (advisories/bans/licenses/sources ok) and the lock
   carries the 9 tray crates. Do NOT re-litigate the dependency; it's reviewed.
P4 Read the existing usage-ui main/window code first; match its style, its
   existing flag parsing (`--client-version`, `--codex-user-agent`), its 1s
   repaint, and how it drains poller `Update`s. Reuse, don't reinvent.

SCOPE — edit ONLY `crates/usage-ui/`. Hand-rolled parsing; NO new dependencies.

Build the tray (behind `cfg(any(target_os="windows", target_os="macos"))`):
- **Icon:** generate a small RGBA icon in code (e.g. 32×32: a two-segment gauge
  or a "Q" monogram drawn by filling pixels). NO asset files, NO build script,
  NO png/image decode path. Feed it to `tray_icon::Icon::from_rgba`.
- **Creation thread:** create the `TrayIcon` on the MAIN thread after eframe
  init (both Windows and macOS require the tray on the main/event-loop thread).
  Do it in the eframe app setup/creation closure, holding the handle in the App
  so it lives as long as the window.
- **Events:** register `TrayIconEvent::set_event_handler` and
  `MenuEvent::set_event_handler`, forwarding each into a `std::sync::mpsc`
  channel that `update()` drains every frame. Optionally call
  `ctx.request_repaint()` from the handler so clicks feel instant rather than
  waiting for the 1s tick.
- **Tooltip:** live one-line summary built from the SAME `ProviderSnapshot`s the
  window renders — e.g. `Claude 5h 42% | Codex 7d 3%`; show `--` for an unknown
  or absent provider. Update it as snapshots arrive.
- **Left-click:** show + focus the window (`ViewportCommand::Visible(true)` then
  focus).
- **Menu:** `Show/Hide` (toggles window visibility), a separator, and `Quit`.
  Quit exits cleanly, stopping ALL poller handles (match existing on_exit).
- **Close-to-tray:** when the tray is active, the window close button HIDES to
  tray instead of quitting — intercept the close request
  (`ViewportCommand::CancelClose`) and hide. Quit (menu) is the real exit.
- **`--no-tray` flag:** hand-rolled like the existing flags. When set, skip tray
  creation and keep today's close-to-quit behavior. On Linux (no tray compiled)
  the flag is accepted and ignored; Linux behavior is exactly today's.

TESTS: unit-test every new PURE helper — tooltip formatting (incl. unknown/
absent, both providers, one provider), icon pixel generation invariants
(dimensions, non-empty, expected corner/center pixels), and `--no-tray` flag
parsing. TDD encouraged. GUI/tray glue itself needn't be unit-tested.

VERIFY + SHIP: run the DECISIONS §3 bar — `cargo test --workspace --locked`,
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked
-- -D warnings`. Commit "M3.5: system tray (icon, tooltip, show/hide, quit)";
push; record the commit SHA + Actions URL; all jobs green (esp. windows-latest)
or §4.6 STOP.

END GATE — STOP. Report commits + CI (all jobs), test-count delta, deviations,
§4 touchpoints. Then the owner's checklist (HIS eyes only — NEVER capture the
screen):
1. `cargo run -p usage-ui -- --client-version <claude --version>` → tray icon
   appears; tooltip shows both providers; left-click shows/raises the window;
   Show/Hide and Quit work; the close button hides to tray.
2. `cargo run -p usage-ui -- --no-tray` → today's behavior (close quits, no
   tray).
Do NOT start the Usage-window look pass or M4. Never capture the screen.
