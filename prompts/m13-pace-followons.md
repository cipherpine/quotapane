# M13 — pace follow-ons: config.cfg, disk history, sparklines, dep-free alerts (ships as v1.6.0)

Authored at the top tier 2026-08-04. Owner decisions (2026-08-04, this
slice's ADR): notifications are DEP-FREE (tray alert variant + tooltip
prefix + in-window banner + eframe RequestUserAttention; OS toasts
declined — tray-icon exposes no balloon API and reaching Shell_NotifyIcon
would need a new crate or unsafe plumbing against tray-icon's internal
handle, a poor trade for an always-on-top window); alert default is
TIME-AWARE; theme.cfg evolves into config.cfg (the "design conversation"
config.rs's own header demands). Read DECISIONS.md first — §3, §4, §4a,
§4.4, §4.5, §4.7 govern. Headless under the dispatcher; a hard stop or
conflict is a report + session end.

## Boundaries

- Zero new dependencies (serde/serde_json already in usage-core may be
  used for history lines; NO config/TOML/dirs/notification crate).
- No JSON key in `--json` changes. CLI behavior untouched.
- §4.1 exception, M9b-pattern: P1 carries TWO byte-exact patches I
  authored below (SECURITY.md, invariants.manifest). Apply under full
  §4a discipline (OLD unique before, NEW unique after, extracted from
  this spec's bytes, never retyped). No other §4.1 bytes may change.
- No version bump, no CHANGELOG — M13-RELEASE cuts v1.6.0.
- End-gate report to reports/m13-endgate.md. After CI, the slice STOPS
  for the owner's visual pass (§4.5) — never self-accept visuals.
- All byte-pinned strings below are exact (— is U+2014).

## P1 — config.cfg (one commit)

`crates/usage-ui/src/config.rs` grows from one word to a small,
hand-parsed key=value file, `config.cfg`, same directory as theme.cfg:

- Grammar: one `key=value` per line; `#`-prefixed lines and blank lines
  ignored; unknown keys ignored (forward compat); keys and values
  trimmed, ASCII-lowercased. No quoting, no sections, no escapes.
- Keys (all optional; defaults apply): `theme` = cipherpine|plain
  (default cipherpine) · `history` = on|off (default off) · `alerts` =
  on|off (default off) · `alert_at` = integer 1..=100 (default 80;
  out-of-range or garbage -> 80) · `alert_mode` = pace|threshold
  (default pace).
- Migration: if config.cfg is absent, read legacy theme.cfg for the
  theme (existing parser). Saving (tray theme toggle, or any future
  setter) writes config.cfg with ALL current values and a two-line
  `#` header naming the file's scope; theme.cfg is never written again
  and never deleted. Every failure path degrades to defaults, silently
  (existing philosophy — a preference is never worth a dialog).
- Rewrite the module doc comment honestly: the file is still boring on
  purpose, still credential-incapable; the one-word era and the
  migration are recorded.
- Unit tests: defaults on absent/garbage file; each key parses; unknown
  keys ignored; out-of-range alert_at -> 80; migration reads theme.cfg;
  save writes all keys; round-trip.

Same-commit doc truth (the same-change rule):
- SECURITY.md patch (§4a, byte-exact):
OLD: Since v1.2.0 the app writes exactly one non-credential file: `theme.cfg`, a single word (`plain` or `cipherpine`) recording the theme choice under the platform config directory (see the README's Theming section). Preferences only, never secrets; every other setting is a CLI flag held in memory.
NEW: Since v1.6.0 the app writes at most two non-credential files under the platform config directory: `config.cfg`, a handful of key=value preference lines (theme, history, alerts — see the README), and, only when `history=on`, `history.jsonl` — timestamps, window labels, and usage percentages, nothing else. The legacy one-word `theme.cfg` is still read as a fallback but never written. Preferences and percentages only, never secrets; every other setting is a CLI flag held in memory.
- invariants.manifest patch (§4a, byte-exact):
OLD: invariant 1: No credential persistence — no code path writes a token; the only file ever written is theme.cfg (one word, preferences only).
NEW: invariant 1: No credential persistence — no code path writes a token; the only files ever written are config.cfg (key=value preferences) and, when history=on, history.jsonl (timestamps, labels, and percentages only).
- README Theming section and ARCHITECTURE.md's "one file, one word"
  paragraph updated to the same truth (floor-authored prose, accurate).
- python3 tools/check-invariants.py must pass in the SAME commit.

Commit: `ui: config.cfg — key=value preferences with theme.cfg migration`

## P2 — disk history + pace rehydration (one commit)

New module `crates/usage-core/src/history.rs` (NOT a protected path):
- Entry: unix seconds, provider id string, window label, used fraction,
  duration_secs (nullable). One compact JSON object per line. Contains
  timestamps, labels, and numbers ONLY — constructing an entry from
  anything credential-shaped must be impossible by type.
- Pure core: encode/decode line, retention decision, rehydration
  filter — all clock-free, exhaustively unit-tested (garbage lines are
  skipped, never fatal; truncated final line tolerated).
- I/O shell: append-on-poll (headline windows only, per successful
  snapshot, only when history=on); file `history.jsonl` next to
  config.cfg. Retention: if file exceeds 256 KiB at startup, rewrite
  keeping the newest half (deterministic, tested at the boundary).
- Rehydration: on startup with history=on, entries newer than the pace
  trail (7200 s) seed each window's PaceRing so forecasts survive
  restart. Ring semantics unchanged — seeding uses the existing insert
  path; `usage_core::pace` itself is UNTOUCHED.
- history=off: file never created, never read, never deleted.
- `--pace-demo` also fabricates ~24 h of synthetic history in memory
  (no disk write) so sparklines demo without network or real data.

Commit: `core,ui: opt-in disk history; forecasts survive restart`

## P3 — sparklines (one commit)

Per provider, in the collapsed view, directly under that provider's
headline bars: one painter-drawn polyline strip, height 12 px, full bar
width, showing the headline window's used fraction over the last 24 h.
- Drawn only when history=on AND >= 2 points exist in the last 24 h;
  otherwise the pane renders exactly as it does today (calm is silent).
- Style: TEXT_DIM stroke 1 px at alpha 140, no axes, no grid, no
  labels, no fill. Both themes identical (a sparkline is information,
  not styling — same rule as the pace tick).
- Pure layout fn mapping (t, used) points to strip coordinates,
  unit-tested (empty, single point, out-of-window points clipped,
  y clamped 0..1).

Commit: `ui: 24h sparkline strip per provider (history-fed)`

## P4 — dep-free alerts, time-aware default (one commit)

Pure decision fn (usage-ui, near the pace helpers), unit-tested:
- alerts=off -> never. For each window with known used fraction:
  candidate when used*100 >= alert_at. In `pace` mode (default) a
  candidate fires only if used_fraction > elapsed_fraction; windows
  with unknown duration fall back to threshold semantics (fail-safe).
  In `threshold` mode every candidate fires.
- Debounce: one alert per (provider, window) per crossing; re-arms when
  the window's used fraction falls back below alert_at (reset/refill).
- Refill notice: when a window that previously fired drops below
  alert_at, show the quiet refill line once (no attention request).

Surfaces, all dep-free:
- In-window banner (CARDINAL, small, above the freshness footer),
  worst offender only, byte-exact:
    alert: <provider> <label> at <PCT>% >= <N>% (<mode>)
  Refill line (TEXT_DIM), byte-exact:
    refilled: <provider> <label> back under <N>%
- Tray: alert variant of the live miniature — a 1 px CARDINAL border
  ring painted around the existing icon (painter/rasterizer, no
  assets); tooltip prefixed exactly `ALERT — ` while any alert is
  active. Both revert when no alert is active.
- Taskbar: on each new alert, send
  `ViewportCommand::RequestUserAttention(Informational)` once (eframe
  built-in; no-op where unsupported).
- `--pace-demo` forces one alert active so all three surfaces can be
  seen and screenshotted without real data.

Commit: `ui: time-aware quota alerts — banner, tray ring, attention (dep-free)`

## The bar (§3), every phase

cargo fmt --all --check · cargo clippy --workspace --all-targets
--locked -- -D warnings · cargo test --workspace --locked · python3
tools/check-invariants.py. All green before each commit.

## End gate

Push all four commits, CI green on all 8 required checks, write the
full report to reports/m13-endgate.md (SHAs, test delta, §4a proof for
the two P1 patches, verbatim-grep proof of every byte-pinned string,
diff-stat proof no §4.1 path changed beyond the two authorized patches,
zero dependency changes), commit it (`reports: M13 end-gate`), push, CI
green, EXIT. Then the slice waits on TWO owner gates: the §4.5 visual
pass (owner runs `--pace-demo` and looks at sparkline + alert surfaces)
and acceptance. No version bump, no tag — M13-RELEASE cuts v1.6.0 after
both.

## DO NOT

Add any dependency. Touch egress/credentials/poller/pace math. Change
any `--json` key. Alter §4.1 bytes beyond the two authorized patches.
Self-accept visuals. Proceed past the end gate.
