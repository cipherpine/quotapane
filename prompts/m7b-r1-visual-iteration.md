# Goal prompt spec: M7B-R1 — visual pass, iteration round 1

Authored at the top tier 2026-07-29 from the owner's first §4.5 look:
the grid reads as noise, the text needs more weight, the whole is too
hard on the eyes, and the owner wants a plain-vs-themed option. Owner
decisions, all resolved through the top tier: grid fainter + wider;
brighten + enlarge the type first (embedding a real bold-weight font
is the round-2 escalation and is NOT yours to take); tray toggle,
persisted; Plain = the pre-M7b look.

Model: **Sonnet 5** (floor). Repo: C:\dev\QuotaPane\QuotaPane
Read CLAUDE.md, then DECISIONS.md — §4 stops override everything.
ZERO new dependencies: a config crate, a TOML crate, a dirs crate —
any of them is a STOP. std::env + std::fs cover everything below.

## PRECONDITIONS (mismatch = STOP)

P1 Tip subject is "docs(prompts): add M7B-R1 visual iteration spec";
   its parent is f4187f2. Tree clean. CI green on the tip. Workspace
   version 1.1.0.
P2 DECISIONS.md contains "M7b Cipher Pine visual pass — underway" and
   no M7b acceptance stamp. There is no DECISIONS change in this round
   and no tag; version stays 1.1.0.

## PHASE 1 — calm the theme (one commit)

- Grid: spacing 40 → 64 px, alpha 12 → 6 (both axes stay).
- Type: heading 15 → 16, body 12 → 13, small 10.5 → 11.5.
- Ink: TEXT → (222,230,225); TEXT_DIM → (158,168,162);
  TEXT_FAINT → (118,128,121). Every other const unchanged.
- The layout harness stays the arbiter: every existing width/height
  assertion at 320 px must pass. If a row overflows at body 13, step
  down by 0.5 until it fits — shrink type, never widen layout — and
  report every stepdown in the end gate.

## PHASE 2 — Theme enum + persistence (one commit)

- New `usage-ui/src/config.rs`; usage-core is untouched everywhere in
  this spec.
- `Theme::{CipherPine, Plain}`. `load()` reads a single word — `plain`
  or `cipherpine` — from `<config-dir>/quotapane/theme.cfg`; anything
  else, absent, or unreadable → CipherPine. `save()` writes the word
  and IGNORES write failures (never panic, never log). `<config-dir>`:
  `%APPDATA%` on Windows; `$XDG_CONFIG_HOME` else `$HOME/.config` on
  Linux; `$HOME/Library/Application Support` on macOS — std::env only.
  The file stores that word and nothing else, ever (§4.4 posture: this
  file must be boring).
- Plain restores the pre-M7b presentation: egui default dark visuals,
  proportional default type, no grid, no block cursor, plain titlebar
  text ("QuotaPane"), plain provider names (no "//"). Bars keep the
  SAME semantic pine/amber/cardinal mapping in both themes — severity
  is data truth, not theming; `fraction_color` stays single-source.
- Tests: config round-trip; garbage/absent/unreadable → CipherPine;
  save-then-load identity (temp dir via env override, not the real
  config dir). The layout harness keeps running under CipherPine — the
  wider mono is the binding case.

## PHASE 3 — toggle surface + README (one commit)

- Tray menu gains a theme item ("Theme: Cipher Pine" / "Theme: Plain"
  — use tray-icon's idiomatic checked/label form) between Show/Hide
  and Quit. Flipping applies live (set_style + one repaint) and
  `save()`s.
- GUI flags `--plain` / `--themed` override the config for that run
  and do NOT write it. Platforms without a tray (Linux) get the flags
  only. The CLI binary is untouched.
- README.md gains exactly this block, as a new "## Theming" section
  immediately after the "What it shows" section, verbatim:

  ## Theming

  The window ships with the Cipher Pine terminal theme. A tray-menu
  item switches between it and a plain look, live; the choice is
  remembered as a single word (`plain` or `cipherpine`) in
  `theme.cfg` under your platform's config directory
  (`%APPDATA%\quotapane\` on Windows, `~/.config/quotapane/` on
  Linux). No tray on your platform? Launch with `--plain` or
  `--themed` to pick per run. The file stores nothing but that word;
  deleting it restores the default.

- No other README change. assets/ untouched.

## VERIFY + SHIP

Full bar from `cargo clean -p usage-core`. Push. CI 7/7 green. No
Cargo.lock movement of any kind.

## END GATE — STOP

Report SHAs, CI run, test delta, any Phase 1 type stepdowns, and where
the theme state lives at runtime. Then STOP for the owner's round-2
look (§4.5). Do not self-assess the aesthetics. No DECISIONS change,
no tag, version stays 1.1.0.
