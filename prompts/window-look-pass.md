# Goal prompt: USAGE-WINDOW LOOK PASS (+ LF normalization)

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything below.

TIER NOTE (§6): All work here is `usage-ui`-only + one repo-root
`.gitattributes`, all NON-boundary. No §4.1 path is touched. `.gitattributes`
is top-tier-authored and already on disk — Phase A is a verify-and-commit of
it, you author nothing in it.

PRECONDITIONS (mismatch = STOP):
P1 `main` tip = 52c73c6 (M3.5 Phase B). If a stale 0-byte `.git\index.lock`
   exists, `del` it first.
P2 `.gitattributes` exists at repo root with `* text=auto eol=lf` and a comment.

PHASE A — end the CRLF churn:
1. `git add --renormalize .` then `git status`. Expected: `.gitattributes` new,
   and the recurring churn files (ci.yml, CONTRIBUTING.md, Cargo.toml,
   usage-core/Cargo.toml, model/mod.rs, providers/mod.rs, deny.toml) should now
   drop out of `status` because git normalizes them to their committed LF form.
   If renormalize stages a REAL content change on any file, STOP and report.
2. Commit just `.gitattributes` (+ any renormalized index entries, which should
   be none): "chore: normalize line endings to LF via .gitattributes". Push.
   CI green. This is docs/config-only; CI stays green.

PHASE B — the look pass (edit ONLY `crates/usage-ui/`; no new dependencies):
Read the existing window/render code FIRST and match its style, theme, color
thresholds (green / amber ≥80% / red ≥95% / gray unknown), staleness handling,
and flag parsing. Reuse helpers; don't reinvent.

1. **Per-bar detail.** On each provider's usage bar, add:
   - the numeric percent, e.g. `42%` (from `used_fraction`; `--` when unknown),
   - a short reset countdown, e.g. `resets in 3h 12m` (from `resets_in_secs`;
     omit / `--` when unknown). Format compactly: `<1m`, `12m`, `3h 12m`,
     `5d 4h`. Keep it readable in the small always-on-top window.
2. **Slim custom titlebar.** Add a thin (~24px) top strip matching the dark
   theme: app name (`QuotaPane`) on the left; on the right two small buttons —
   **minimize-to-tray** (hides the window: `ViewportCommand::Visible(false)`)
   and **close**. The close button behaves exactly like the OS close already
   does: hide-to-tray when the tray is active, quit when `--no-tray`. The strip
   is the drag handle (`ViewportCommand::StartDrag` on drag) — preserve
   move-the-window; keep existing drag elsewhere if present. Use egui built-in
   glyphs/shapes (e.g. `–` and `✕`); NO image assets, NO new deps.
   - On Linux (tray cfg'd out) the titlebar still renders; its close button
     quits (today's behavior) and minimize hides the window.
3. Keep the 1s repaint, staleness treatment, and absent-provider quiet line.

TESTS: unit-test the new PURE helpers — percent formatting (fraction→`42%`,
`None`→`--`, clamp 0–100) and countdown formatting (seconds→`3h 12m` incl.
0 / <60s / exact-hour / multi-day / `None`). Titlebar/button GUI glue needn't
be unit-tested.

VERIFY + SHIP: §3 bar — `cargo test --workspace --locked`,
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked
-- -D warnings`. Commit "usage-ui: window look pass — percent + reset
countdown, slim titlebar (min/close)"; push; record commit SHA + Actions URL;
all jobs green (esp. windows-latest) or §4.6 STOP.

END GATE — STOP. Report commits + CI (all jobs), test-count delta, deviations,
§4 touchpoints. Owner checklist (HIS eyes only — NEVER capture the screen):
1. `cargo run -p usage-ui -- --client-version <claude --version>` → each bar
   shows percent + reset countdown; the slim titlebar shows the name and the
   minimize/close buttons; minimize hides to tray; close hides to tray; drag
   the titlebar moves the window.
2. `cargo run -p usage-ui -- --no-tray` → close button quits (no tray).
Do NOT start M4. Never capture the screen.
