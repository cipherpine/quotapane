//! QuotaPane desktop window — M3.5 milestone (multi-provider + system tray).
//!
//! Pure render: this crate receives non-secret `ProviderSnapshot` values over
//! per-provider channels and draws them. It never touches credentials or the
//! network directly — `usage_core::poller` and the `Egress`/`UsageProvider`
//! types it wires together do that, entirely inside the trust boundary crate.
//!
//! M3 runs both subscription providers (Claude + Codex), each in its own
//! poller thread and its own titled section. Staleness and last-failure are
//! tracked **per provider**, so one provider being signed out or erroring
//! never disturbs the other. A provider whose credential file is absent is
//! rendered as a single quiet "not signed in" line rather than a red banner.
//!
//! M3.5 adds a system tray on Windows (primary) and macOS: a runtime-drawn
//! icon, a live tooltip built from the same snapshots the window renders, a
//! left-click that raises the window, and a Show/Hide + Quit menu. When the
//! tray is active the window's close request hides to tray instead of quitting;
//! `--no-tray` restores the classic close-to-quit behavior. Linux has no tray
//! (window-only) and compiles to exactly the pre-M3.5 behavior.
//!
//! M5a adds a per-provider disclosure toggle for the per-model rows a snapshot
//! carries in `per_model` (Claude's model-scoped `limits` entries, Codex's
//! `additional_rate_limits`). It is collapsed by default and its state is per
//! pane, so the providers expand independently. The toggle is suppressed when
//! a provider reports no per-model data. This is presentational only: the rows
//! use the same renderer as the headline rows and no label is ever parsed.
//!
//! M7a narrows what the disclosure shows: an untouched bucket (0%, or usage the
//! provider did not report) is hidden, and the toggle disappears when that
//! leaves nothing to show. Providers list every model on the plan, not just the
//! ones in use, so those rows are noise in a 320px window. The filter is
//! **display-only** — `quotapane-cli --json` still emits every bucket the
//! provider sent, pinned by a test in `usage-cli`.
//!
//! M7a2 adds one dim line for a provider's reset credits (`resets available:
//! N`), between the headline rows and the per-model toggle. Only Codex reports
//! them, so the line is absent for Claude and that pane is untouched.
//!
//! M8 adds pace. Every bar whose window has a known duration gets a 1px
//! elapsed-time tick, so fill-versus-tick reads as spending slower or faster
//! than the clock. Per provider, an in-memory `PaceRing` per headline window is
//! fed on each poll and fitted by `usage_core::pace`; when a window is projected
//! to fill before it resets, one line says so — amber, cardinal inside six
//! hours. Nothing at risk means no line at all, so a healthy window looks
//! exactly as it did before. All of it recomputes on poll events only, and
//! `--pace-demo` renders a fixed synthetic scenario (no polling, no network) so
//! the feature can be reviewed without waiting hours for one to occur.
//!
//! M13 follows the pace work on: preferences, memory, and a way to be told.
//! `config.cfg` replaces the one-word `theme.cfg` (see [`config`]); with
//! `history=on` each poll's headline readings are appended to `history.jsonl`
//! and replayed into the pace rings at startup, so a forecast survives a
//! restart, and the day's readings draw a 16px sparkline — filled body under a
//! full-strength line, a bright dot on the newest reading, a `24h` tag — under
//! each provider's bars. With `alerts=on` a window over `alert_at` — and, in
//! the default `pace` mode, also being spent faster than it elapses — raises
//! one banner, rings the tray icon, prefixes the tray tooltip, and asks the
//! taskbar for attention once. All of it is off by default and dep-free: no
//! notification crate, and every surface one the app already owned.
//! `--pace-demo` fabricates a day of history in memory, forces the alert
//! settings to on-at-the-defaults, and opens a taller window sized to the
//! whole scenario, so all of it can be seen at once without real data — and,
//! having no history path, it writes none of that day to disk.
//!
//! M15 gives the titlebar a second view. `usage // agents` switches the pane
//! between the quota bars and a list of the Claude Code / Codex CLI sessions
//! running on this machine, read by [`usage_core::agents`] — content-free by
//! construction: a state dot, `project · branch · id8`, and an age. The scan
//! runs only while that view is showing, on switch and every [`SCAN_EVERY`]
//! after, so a window left on `usage` touches no session log at all.
//! `--agents-demo` opens straight onto a synthetic fixture set covering every
//! state, the `--pace-demo` idiom, and it never looks at a real root.

use eframe::egui;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use usage_core::agents::{self, AgentSession, AgentState, SessionRoots};
use usage_core::egress::Egress;
use usage_core::history::{self, HistoryEntry};
use usage_core::model::{ProviderId, ProviderSnapshot, QuotaWindow, ResetCredits};
use usage_core::pace::{self, Burn, PaceRing, PaceSample};
use usage_core::poller::{self, PollerHandle, Update};
use usage_core::providers::{
    ClaudeSubscription, CodexSubscription, UsageProvider, CODEX_DEFAULT_USER_AGENT,
};

mod config;
mod icon;

use config::{AlertMode, Config, Theme, DEFAULT_ALERT_AT};

/// Sent when `--client-version` is omitted. Mirrors `usage-cli`'s default —
/// real Claude Code versions avoid the provider's aggressively rate-limited
/// fallback bucket (see `claude_subscription` module docs in usage-core).
const DEFAULT_CLIENT_VERSION: &str = "0.0.0";

/// A snapshot is considered stale once it's this old without a fresh poll.
///
/// M7b lowered this from 15 minutes to 10: the stale treatment is now a
/// whole-line CARDINAL flip rather than a single amber word, so it earns being
/// reached sooner.
///
/// The **outer** of the two freshness thresholds — see [`AGING_AFTER`], which
/// is always the nearer one. This one keeps every meaning it has ever had:
/// [`is_stale`] and its callers, including M10's pace-line mute, are untouched
/// by M14.
const STALE_AFTER: Duration = Duration::from_secs(600);

/// A snapshot is considered *aging* once it's this old — the inner of the two
/// freshness thresholds, always shorter than [`STALE_AFTER`] (M14).
///
/// Aging is not a problem, it is a warning that one is approaching: the poll
/// cadence means a healthy pane refreshes well inside five minutes, so five
/// minutes without one is the first honest sign that something is wrong.
/// Nothing but the freshness dot's colour reads this — it deliberately does
/// **not** gate the forecast or anything else `is_stale` gates.
const AGING_AFTER: Duration = Duration::from_secs(300);

// Aging comes before stale, and the dot's three states depend on it: were the
// order reversed the amber band would be empty and the dot would jump from
// green to cardinal. A compile-time fact, so it is checked at compile time.
const _: () = assert!(AGING_AFTER.as_secs() < STALE_AFTER.as_secs());

// --------------------------------------------------------------------------
// Cipher Pine palette (M7b). Every colour in the window comes from here — a
// literal `Color32::from_rgb` anywhere else in this file is a regression.
// --------------------------------------------------------------------------

/// Window fill. `#0a0f0d`
const GROUND: egui::Color32 = egui::Color32::from_rgb(10, 15, 13);
/// Titlebar fill, and the trough behind a quota bar. `#0e100f`
const PANEL: egui::Color32 = egui::Color32::from_rgb(14, 16, 15);
/// Borders and the blueprint grid's base hue. `#1e2422`
const HAIRLINE: egui::Color32 = egui::Color32::from_rgb(30, 36, 34);
/// Primary text. `#cdd6d1`
const TEXT: egui::Color32 = egui::Color32::from_rgb(222, 230, 225);
/// Labels and reset countdowns. `#8a938e`
const TEXT_DIM: egui::Color32 = egui::Color32::from_rgb(158, 168, 162);
/// The quietest ink in the window: the resize grip at rest. `#5c665f`
const TEXT_FAINT: egui::Color32 = egui::Color32::from_rgb(118, 128, 121);
/// Healthy bar fill. `#2d7a4f`
const PINE: egui::Color32 = egui::Color32::from_rgb(45, 122, 79);
/// The "operational" freshness dot. `#3fae6a`
const OPER_GREEN: egui::Color32 = egui::Color32::from_rgb(63, 174, 106);
/// Caution bar fill. `#d9a13b`
const AMBER: egui::Color32 = egui::Color32::from_rgb(217, 161, 59);
/// Prompt, cursor, critical fill, and every stale/error line. `#c41e3a`
const CARDINAL: egui::Color32 = egui::Color32::from_rgb(196, 30, 58);

/// The pre-M7b titlebar slate, restored under [`Theme::Plain`].
const PLAIN_TITLEBAR_BG: egui::Color32 = egui::Color32::from_rgb(24, 27, 33);

/// Slim custom titlebar — the window is borderless (`with_decorations(false)`),
/// so it draws its own. ~24px tall.
const TITLEBAR_HEIGHT: f32 = 24.0;

/// The titlebar frame's left inset: the prompt does not start flush.
const TITLEBAR_MARGIN_LEFT: i8 = 8;
/// The titlebar frame's right inset — small, because the close button's own
/// hover square already supplies the visual gap.
///
/// Named (M15) so the width budget the switcher has to live inside is derived
/// from the same numbers the frame uses rather than copied beside them.
const TITLEBAR_MARGIN_RIGHT: i8 = 2;

/// Status-cursor blink period. Steps, not a fade — a terminal cursor is on or
/// off.
const CURSOR_BLINK_PERIOD: Duration = Duration::from_millis(1100);
/// Block-cursor size at titlebar scale.
const CURSOR_SIZE: egui::Vec2 = egui::vec2(7.0, 13.0);
/// Gap between the prompt text and the block cursor.
const CURSOR_GAP: f32 = 3.0;

/// Blueprint grid pitch, both axes.
const GRID_PITCH: f32 = 64.0;
/// Grid line alpha, out of 255. Texture, not noise — high enough to read as
/// deliberate under the content, low enough never to compete with it.
const GRID_ALPHA: u8 = 6;

/// Caption beside the per-model disclosure triangle.
const PER_MODEL_CAPTION: &str = "per-model";

/// The freshness dot's glyph — the same `•` the pre-M14 footer's status dot
/// used, kept so the mark a user already reads did not change, only its place.
const FRESHNESS_DOT: &str = "•";

// --------------------------------------------------------------------------
// M15: the agents view.
// --------------------------------------------------------------------------

/// How often the agents list rescans **while its view is showing**.
///
/// Two seconds. A scan is a `read_dir` plus a `stat` per file plus two bounded
/// reads of the handful inside the lookback, and the thing it reports — is that
/// session still working — changes on the order of seconds. Slower would make
/// the pane lag the terminal beside it; faster would spend I/O on a question
/// whose answer has not moved.
const SCAN_EVERY: Duration = Duration::from_secs(2);

/// The view switcher's two words, and the separator between them.
///
/// `usage // agents` in the M7b terminal voice: the `//` is the same comment
/// mark the provider headers wear, so the switcher reads as part of the same
/// language rather than as a tab bar borrowed from somewhere else.
const VIEW_LABEL_USAGE: &str = "usage";
/// The other half of the switcher — see [`VIEW_LABEL_USAGE`].
const VIEW_LABEL_AGENTS: &str = "agents";
/// The switcher's separator, spaced so each word keeps its own click target.
const VIEW_SEPARATOR: &str = " // ";

/// What the agents view says when nothing has run today.
///
/// Names the lookback, because "nothing here" and "nothing here *in the last
/// day*" are different claims and only the second one is true.
const NO_AGENTS_LINE: &str = "// no agent sessions in the last 24h";

/// The mark on a row the log flagged as a subagent (`isSidechain`).
const SUBAGENT_PREFIX: &str = "· sub";

/// Between the parts of a row's identity: `project · branch · id8`.
const AGENT_ROW_SEPARATOR: &str = " · ";

/// Which of the two panes the window is showing (M15).
///
/// Deliberately **not** persisted: a view is where you are looking right now,
/// not a preference, and a window that reopened on `agents` would start
/// scanning session logs before its owner asked to see them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum View {
    /// The quota bars — everything through M14.
    #[default]
    Usage,
    /// The local agent sessions.
    Agents,
}

/// Whether this frame should rescan the session logs.
///
/// The **whole** scanning policy, as one pure decision, so the promise "no
/// scanning at all while the usage view shows" is a thing a test can hold
/// rather than a thing to be read off a call site. [`QuotaPaneApp::refresh_agents`]
/// is the only caller and the only place [`agents::scan`] is named, and it asks
/// this first.
///
/// `since_last` is `None` before the first scan of a viewing — which is what
/// makes switching to the view scan immediately rather than up to
/// [`SCAN_EVERY`] later.
fn should_scan(view: View, agents_demo: bool, since_last: Option<Duration>) -> bool {
    // The demo's rows are a fixture built at startup. Scanning would replace
    // them with the reviewer's own sessions, which is precisely the thing a
    // demo flag exists to avoid.
    if view != View::Agents || agents_demo {
        return false;
    }
    match since_last {
        None => true,
        Some(elapsed) => elapsed >= SCAN_EVERY,
    }
}

/// The window's inner **width**, and it is fixed forever: every row in this
/// file is laid out against 320px, and the layout harness asserts against it.
/// The viewport is built with the same value as its minimum *and* its maximum
/// width, so even an OS-provided borderless edge-drag cannot move it.
const WINDOW_WIDTH: f32 = 320.0;

/// The inner height a window opens at when nothing is stored — the height the
/// window was hard-fixed at from M0 through v1.6.0.
///
/// Since M14 the height is the user's: the bottom grip resizes it and
/// `config.cfg`'s `height` key remembers it. This constant is what an install
/// that never touches the grip gets, so that window is byte-for-byte the one
/// every visual acceptance to date was given against.
const WINDOW_HEIGHT: f32 = 240.0;

/// The smallest inner height the OS will let the user drag to (M14).
///
/// Preserves the titlebar plus one provider header — enough that the window is
/// always recognisably itself and always has a grip to grab. Without a floor,
/// winit would happily let a drag collapse the window to a zero-height sliver
/// with no way back short of editing the config file.
const MIN_WINDOW_HEIGHT: f32 = 160.0;

/// The largest inner height the OS will let the user drag to (M14).
///
/// **Finite on purpose.** The natural expression of "no ceiling" is
/// `f32::INFINITY`, and egui guards against exactly that on the
/// `ViewportCommand::MaxInnerSize` path — but the *builder* path hands the
/// value straight to winit, where `LogicalSize::to_physical` saturates
/// infinity to `u32::MAX` and Windows' `WM_GETMINMAXINFO` handler then casts
/// it to `-1`, producing a maximum track size of a few pixels: a window that
/// cannot be resized at all, which is the opposite of this milestone. A finite
/// ceiling beyond any real monitor's work area says the same thing safely, and
/// is the same number the config parser accepts up to.
const MAX_WINDOW_HEIGHT: f32 = 4096.0;

// The stored bounds and the viewport's bounds are the same bounds. Welded at
// compile time so a change to one is a build failure rather than a window that
// can be dragged to a size it cannot remember.
const _: () = {
    assert!(MIN_WINDOW_HEIGHT == config::MIN_HEIGHT as f32);
    assert!(MAX_WINDOW_HEIGHT == config::MAX_HEIGHT as f32);
    assert!(WINDOW_HEIGHT == config::DEFAULT_HEIGHT as f32);
    assert!(MIN_WINDOW_HEIGHT < WINDOW_HEIGHT);
};

/// Height of the bottom resize grip strip (M14).
///
/// Chrome, exactly like [`TITLEBAR_HEIGHT`]: it is carved out of the window
/// before the `CentralPanel` is shown, so it never overlaps the scrollable
/// content it sits under.
const GRIP_HEIGHT: f32 = 8.0;

/// The grip's mark: a right-aligned quadrant fill, the terminal voice's answer
/// to a resize handle.
const GRIP_GLYPH: char = '▞';

/// The inner height `--pace-demo` asks for instead of [`WINDOW_HEIGHT`]
/// (M13-R1). **Demo only** — the production window is untouched.
///
/// The demo is the review path, and it shows every state at once: two panes,
/// both with a 24 h strip, one raising an alert banner, one carrying a
/// reset-credits line and a per-model disclosure. That is more than the
/// 240px pane a real subscriber usually sees, and reviewing it through a
/// scroll bar means reviewing it in pieces.
///
/// Derived, not chosen. `the_demo_scenario_fits_the_window` measures the demo
/// body through the same layout harness the width assertions use, puts it
/// through [`snapped_height`] — the very function the grip's double-click
/// uses — and fails if this constant is under that or more than one row over
/// it. As measured 2026-08-06, after M14 added the 8px grip strip to the
/// chrome and took the per-provider freshness footer out of the body:
///
/// | theme        | titlebar | panel chrome | grip | demo body | needed  |
/// |--------------|----------|--------------|------|-----------|---------|
/// | Cipher Pine  | 24.0     | 16.0         | 8.0  | 281.8125  | 329.8125|
/// | Plain        | 24.0     | 16.0         | 8.0  | 269.625   | 317.625 |
///
/// Cipher Pine binds — its monospace is taller per row, the same reason it
/// binds the width assertions — so this is its 329.8125 rounded up to a whole
/// pixel. Plain then opens with ~12px of slack, which is the correct direction
/// to be wrong in: a little empty ground, never a scroll bar.
const DEMO_WINDOW_HEIGHT: f32 = 330.0;

/// The inner height the window asks the OS for at startup.
///
/// Two inputs so each answer has exactly one source. `--pace-demo` gets
/// [`DEMO_WINDOW_HEIGHT`] and ignores `saved_height` entirely — the demo is a
/// fixture, not a session, and it neither reads nor writes the user's height.
/// Every other launch gets whatever the config file remembered, which is
/// [`WINDOW_HEIGHT`] until the user first drags the grip.
///
/// `saved_height` arrives already range-checked by `config::height`; the clamp
/// here is the second lock on the same door, so no path can ask the OS for a
/// window outside the bounds the viewport declares.
fn initial_inner_height(pace_demo: bool, saved_height: u32) -> f32 {
    if pace_demo {
        DEMO_WINDOW_HEIGHT
    } else {
        (saved_height as f32).clamp(MIN_WINDOW_HEIGHT, MAX_WINDOW_HEIGHT)
    }
}

/// The inner height that exactly fits `content_height` — what the grip's
/// double-click snaps to (M14).
///
/// Everything the content does *not* get is added here: the titlebar above it,
/// the grip strip below it, and the `CentralPanel`'s own margins around it.
/// `panel_chrome_height` is measured by the caller from the live style rather
/// than assumed, exactly as the layout harness measures it, so a margin change
/// moves the snap without anyone having to remember to update a number.
///
/// Only the floor is clamped. egui already clamps a requested inner size to the
/// monitor's work area, and fighting it here would mean this file guessing at a
/// monitor size it cannot see.
fn snapped_height(content_height: f32, panel_chrome_height: f32) -> f32 {
    (TITLEBAR_HEIGHT + panel_chrome_height + content_height + GRIP_HEIGHT).max(MIN_WINDOW_HEIGHT)
}

/// The viewport command the grip's double-click sends.
///
/// The width component is [`WINDOW_WIDTH`], literally and unconditionally, and
/// this is the **only** place in the window that ever names an inner size at
/// runtime — `no_resize_path_can_emit_a_width_other_than_the_fixed_one` scans
/// the source to keep it that way.
fn snap_command(content_height: f32, panel_chrome_height: f32) -> egui::ViewportCommand {
    egui::ViewportCommand::InnerSize(egui::vec2(
        WINDOW_WIDTH,
        snapped_height(content_height, panel_chrome_height),
    ))
}

/// How long an observed window height must hold still before it is written to
/// disk (M14).
///
/// A resize is driven by the OS, not by egui, so there is no "drag ended"
/// event to hang the save on — the window simply repaints at a new size, many
/// times a second, for as long as the user holds the edge. Writing on each of
/// those frames would be dozens of `config.cfg` rewrites for one decision, so
/// the height is written once the dragging stops.
const HEIGHT_SETTLE: Duration = Duration::from_millis(500);

/// What to do about the height observed this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeightAction {
    /// The window is at the stored height: nothing to remember.
    Nothing,
    /// A different height, but not yet still enough to be the user's answer.
    Wait,
    /// Settled on a new height — write it.
    Write,
}

/// Decide whether a newly observed height should be persisted.
///
/// Pure, so the debounce is unit-testable without a window: `pending` is the
/// height seen on a previous frame and how long ago it was first seen.
fn height_action(saved: u32, current: u32, pending: Option<(u32, Duration)>) -> HeightAction {
    if current == saved {
        return HeightAction::Nothing;
    }
    match pending {
        Some((height, elapsed)) if height == current && elapsed >= HEIGHT_SETTLE => {
            HeightAction::Write
        }
        _ => HeightAction::Wait,
    }
}

/// How far a per-model row's bar line is inset under its model label.
const PER_MODEL_ROW_INDENT: f32 = 8.0;

/// Quota bar width. Shared by the headline and per-model rows so a gauge stays
/// comparable at a glance wherever it appears.
const BAR_WIDTH: f32 = 120.0;
/// Corner rounding on a quota bar and its border.
const BAR_ROUNDING: u8 = 3;

/// Alpha of the elapsed-time pace tick, out of 255 (M8).
///
/// Present enough to read against a filled bar, short of full opacity so it
/// never competes with the fill edge it sits beside.
const PACE_TICK_ALPHA: u8 = 200;

/// Under this many seconds to projected exhaustion the at-risk line goes
/// CARDINAL rather than AMBER (6 h).
///
/// A working session, not a round number: inside six hours the forecast is
/// something to act on now, and beyond it something to merely know.
const PACE_CARDINAL_UNDER_SECS: u64 = 21_600;

/// How far back the in-memory history trail — and the sparkline drawn from it —
/// reaches (24 h).
///
/// A day is the span where "am I burning faster than usual" is answerable at a
/// glance: it covers several 5h windows and a meaningful slice of a weekly one.
/// Longer would compress the interesting part of the line into a few pixels of
/// a 120px strip.
const HISTORY_WINDOW_SECS: u64 = 86_400;

/// Height of the history sparkline strip, in px (M13; raised in M13-R1).
///
/// Sixteen. Twelve shipped first and the owner's round-1 verdict on it was
/// "hard to tell what it is and what it means": a day whose fraction moves by
/// a third moves four pixels at that height, which reads as a wobble rather
/// than a trend. A third more vertical range for the same horizontal day, and
/// still cheaper than the text row it replaces the need for.
const SPARK_HEIGHT: f32 = 16.0;

/// Sparkline stroke width, in px (M13-R1; was 1.0).
///
/// A hairline is what a 1px line renders as on a HiDPI panel — technically
/// present, visually a smudge. One and a half is the narrowest width that
/// still resolves as a drawn line rather than an artifact.
const SPARK_STROKE_WIDTH: f32 = 1.5;

/// Alpha of the fill under the sparkline curve, out of 255 (M13-R1).
///
/// Eighteen is deliberately near the floor: the fill's job is to give the
/// line a *body* so the eye reads a shape, not to carry any value of its own.
/// Any darker and the strip would compete with the quota bars directly above
/// it, which are the thing the pane is actually about.
const SPARK_FILL_ALPHA: u8 = 18;

/// Radius of the "now" dot at the strip's newest reading, in px (M13-R1).
///
/// The one bright mark on the strip, and the reason the line reads as flowing
/// toward the present rather than as an anonymous squiggle.
const SPARK_NOW_RADIUS: f32 = 2.5;

/// The strip's self-describing tag, above its right edge (M13-R1).
///
/// The strip was unlabelled in M13 on the theory that a 120px day has no room
/// for an axis. It has room for three characters, and three characters are the
/// difference between a line and a line that says what its width means.
const SPARK_TAG: &str = "24h";

/// The current unix second.
///
/// The one clock read in the history path, and it lives here rather than in
/// `usage_core::history` on purpose: that module stays clock-free so its
/// retention and rehydration cutoffs are testable at any moment (the same rule
/// `usage_core::pace` follows). Zero on a system clock before the epoch, which
/// simply reads as "nothing in the file is recent".
fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct Args {
    /// Claude Code client version → `User-Agent: claude-code/<ver>`.
    client_version: String,
    /// True when `--client-version` was omitted (drives the throttle note).
    client_version_defaulted: bool,
    /// The `User-Agent` sent by the Codex provider. Defaults to the verified
    /// Codex CLI default ([`CODEX_DEFAULT_USER_AGENT`]) — a correct value, not
    /// a placeholder, so its default needs no warning.
    codex_user_agent: String,
    /// Disable the system tray, restoring close-to-quit. On platforms without a
    /// tray (Linux) the flag is accepted and ignored.
    no_tray: bool,
    /// Theme for this run only, from `--plain` / `--themed`. `None` means use
    /// the saved preference. Never written back to disk: a flag is a choice
    /// about one launch, not a new default.
    theme_override: Option<Theme>,
    /// Render the fixed synthetic pace scenario instead of polling (M8).
    ///
    /// Nothing is constructed that could reach the network: no provider, no
    /// [`Egress`], no poller thread. The pace markers only become visible after
    /// hours of real usage, so this is how they get reviewed and screenshotted.
    pace_demo: bool,
    /// Open on the agents view with a synthetic session list instead of the
    /// real one (M15).
    ///
    /// The same bargain `--pace-demo` strikes, for the same reason: the states
    /// worth reviewing (a subagent, an idle session, one that finished hours
    /// ago) cannot be conjured on demand, and the alternative to a fixture is
    /// pointing a reviewer's window at his own transcripts. Nothing under this
    /// flag reads a session root at all.
    agents_demo: bool,
}

fn parse_args<I: IntoIterator<Item = String>>(argv: I) -> Result<Args, String> {
    let mut client_version: Option<String> = None;
    let mut codex_user_agent: Option<String> = None;
    let mut no_tray = false;
    let mut theme_override: Option<Theme> = None;
    let mut pace_demo = false;
    let mut agents_demo = false;

    let mut iter = argv.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--client-version" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--client-version requires a value".to_string())?;
                client_version = Some(value);
            }
            // Named to match `CodexSubscription::new`'s `user_agent` parameter.
            "--codex-user-agent" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--codex-user-agent requires a value".to_string())?;
                codex_user_agent = Some(value);
            }
            // Boolean flag: disable the tray (accepted-and-ignored on Linux).
            "--no-tray" => {
                no_tray = true;
            }
            // Theme for this run only. The later flag wins if both are given,
            // which is the ordinary shell convention.
            "--plain" => {
                theme_override = Some(Theme::Plain);
            }
            "--themed" => {
                theme_override = Some(Theme::CipherPine);
            }
            // Synthetic pace scenario; no polling, no network at all.
            "--pace-demo" => {
                pace_demo = true;
            }
            // Synthetic agent sessions; no session root is read at all.
            "--agents-demo" => {
                agents_demo = true;
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
    }

    let client_version_defaulted = client_version.is_none();
    Ok(Args {
        client_version: client_version.unwrap_or_else(|| DEFAULT_CLIENT_VERSION.to_string()),
        client_version_defaulted,
        codex_user_agent: codex_user_agent.unwrap_or_else(|| CODEX_DEFAULT_USER_AGENT.to_string()),
        no_tray,
        theme_override,
        pace_demo,
        agents_demo,
    })
}

/// Human title for a provider's section.
fn provider_label(id: ProviderId) -> &'static str {
    match id {
        ProviderId::ClaudeSubscription => "Claude",
        ProviderId::CodexSubscription => "Codex",
    }
}

/// The quiet one-liner shown when a provider's credential file is absent —
/// points the user at the official CLI that signs them in (invariant 6:
/// QuotaPane never writes credentials itself).
fn not_signed_in_line(id: ProviderId) -> &'static str {
    match id {
        ProviderId::ClaudeSubscription => "Claude: not signed in — run `claude` to sign in",
        ProviderId::CodexSubscription => "Codex: not signed in — run `codex login`",
    }
}

/// The line shown when the stored token has expired — the most-seen error state
/// in the product, so it names the exact command rather than the architecture.
///
/// Same shape and placement as [`not_signed_in_line`], and the same division of
/// labour: refreshing happens in the provider's own CLI (invariant 6), and the
/// poller keeps retrying, so the user is told both halves — what to run, and
/// that QuotaPane picks it up by itself.
fn token_expired_line(id: ProviderId) -> &'static str {
    match id {
        ProviderId::ClaudeSubscription => "token expired — start any claude session (even `claude -p hi`) to refresh it. QuotaPane recovers on its own within ~3 min.",
        ProviderId::CodexSubscription => "token expired — run `codex login` to refresh it. QuotaPane recovers on its own within ~3 min.",
    }
}

/// Whether a provider failure message indicates an absent credential file (as
/// opposed to a genuine error worth a red banner). The credential loader
/// reports a missing file as `credential file not found: <path>` (see
/// `usage_core::credentials::CredentialError::NotFound`); no other
/// `ProviderError` variant's message contains this phrase.
fn is_absent_credentials(message: &str) -> bool {
    message.contains("not found")
}

/// Whether a provider failure message is an expired OAuth token.
///
/// `OAuth token expired` is a stable marker on `ProviderError`'s `Display`
/// (documented as such on the impl in `usage_core::providers`), matched the
/// same way [`is_absent_credentials`] matches its own. A pin test asserts the
/// core's real message classifies here, so a reworded error cannot silently
/// demote this pane to a raw banner.
fn is_token_expired(message: &str) -> bool {
    message.starts_with("OAuth token expired")
}

/// How a provider's latest failure should be presented.
#[derive(Debug, PartialEq, Eq)]
enum FailureDisplay {
    /// No failure recorded.
    NoFailure,
    /// Credentials absent — show the quiet "not signed in" line, not an error.
    NotSignedIn,
    /// Token expired — show the per-provider refresh instruction, not the raw
    /// poller message.
    TokenExpired,
    /// A genuine failure — show a red banner with the message.
    Banner,
}

/// Per-provider state selection: decide how to present the latest failure.
fn classify_failure(failure: Option<&str>) -> FailureDisplay {
    match failure {
        None => FailureDisplay::NoFailure,
        Some(msg) if is_absent_credentials(msg) => FailureDisplay::NotSignedIn,
        Some(msg) if is_token_expired(msg) => FailureDisplay::TokenExpired,
        Some(_) => FailureDisplay::Banner,
    }
}

/// Format a used-fraction as a whole-number percent for the bar label:
/// `Some(0.42)` → `"42%"`, `None` → `"--"`. Clamped to 0–100 so a fraction that
/// (transiently) exceeds 1.0 never renders as `"101%"`.
fn format_percent(fraction: Option<f64>) -> String {
    match fraction {
        None => "--".to_string(),
        Some(f) => {
            let pct = (f * 100.0).round().clamp(0.0, 100.0) as i64;
            format!("{pct}%")
        }
    }
}

/// Format a reset countdown compactly for the small always-on-top window:
/// `None` → `"--"`, under a minute → `"<1m"`, under an hour → `"12m"`, under a
/// day → `"3h 12m"` (incl. `"2h 0m"` exactly on the hour), else days+hours →
/// `"5d 4h"`.
fn format_reset(remaining: Option<u64>) -> String {
    let Some(secs) = remaining else {
        return "--".to_string();
    };
    if secs < 60 {
        "<1m".to_string()
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    } else {
        format!("{}d {}h", secs / 86_400, (secs % 86_400) / 3600)
    }
}

/// Format a "last updated" age, e.g. "5s ago", "2m ago".
fn format_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s ago")
    } else {
        format!("{}m ago", secs / 60)
    }
}

/// Whether a snapshot this old should be flagged stale in the UI.
fn is_stale(age: Duration) -> bool {
    age >= STALE_AFTER
}

/// Everything the freshness dot says about a pane's last successful poll: how
/// long ago it was, and when it was (M14).
///
/// Both are carried rather than the second derived from the first at the point
/// of use, so the tooltip's producer is pure and its bytes are pinnable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Freshness {
    /// Time since the poll landed.
    age: Duration,
    /// The Unix second the poll landed on.
    polled_at_unix: u64,
}

/// The freshness dot's colour: green fresh, amber aging, cardinal stale (M14).
///
/// Shared by both themes, for the reason [`fraction_color`] is: how old the
/// data is, is data truth rather than decoration, and a Plain window that
/// disagreed with a Cipher Pine one about whether a pane had gone quiet would
/// be reporting the look rather than the pane. Plain already wore this AMBER
/// on its pre-M7b stale footer.
fn freshness_color(age: Duration) -> egui::Color32 {
    if age >= STALE_AFTER {
        CARDINAL
    } else if age >= AGING_AFTER {
        AMBER
    } else {
        OPER_GREEN
    }
}

/// The freshness dot's hover text — the exact age the footer used to print,
/// plus the wall clock the dot alone cannot carry (M14).
///
/// `updated 5s ago at 09:14:22 UTC`, and past [`STALE_AFTER`]
/// `updated 12m ago at 09:14:22 UTC — stale`.
///
/// **The clock is UTC, and says so.** The spec asked for local time, and that
/// is the better tooltip; it is also not available here. `std` exposes no
/// timezone, the milestone's hard limit is zero new dependencies, and the only
/// dependency-free route to a local offset is per-platform FFI — raw `unsafe`
/// in the window crate of a project whose thesis is a small auditable surface,
/// for a hover string. Rendering a UTC clock *as if* it were local would be the
/// one thing this codebase refuses more firmly than an ugly label: a number
/// that is confidently wrong by the reader's own offset, exactly the failure
/// `providers::time` rejects a bare local timestamp to avoid, and exactly why
/// `quotapane-cli --watch` stamps `Z`. So the suffix is the honest version of
/// the same information, and swapping it for a local clock later is this one
/// function. Flagged for the owner in the M14 end-gate report.
/// Pair an age with the wall-clock second it counts back to.
///
/// The pane stores an [`Instant`], which is monotonic and has no wall clock in
/// it, so the poll's clock time is reconstructed here rather than stored a
/// second time. Impure by nature (it reads the clock), which is exactly why it
/// is one line, and why [`freshness_tooltip`] takes the result instead of
/// calling this itself.
fn freshness_at_now(age: Duration) -> Freshness {
    Freshness {
        age,
        polled_at_unix: now_unix_secs().saturating_sub(age.as_secs()),
    }
}

fn freshness_tooltip(freshness: Freshness) -> String {
    let second_of_day = freshness.polled_at_unix % 86_400;
    let (hour, minute, second) = (
        second_of_day / 3600,
        (second_of_day % 3600) / 60,
        second_of_day % 60,
    );
    let mut text = format!(
        "updated {} at {hour:02}:{minute:02}:{second:02} UTC",
        format_age(freshness.age.as_secs())
    );
    if is_stale(freshness.age) {
        text.push_str(" — stale");
    }
    text
}

/// Whether the at-risk pace line should be drawn for data of this age.
///
/// A forecast is an extrapolation from the last couple of hours; extrapolating
/// from data the freshness dot is simultaneously calling stale is not a
/// forecast, it is misinformation, and it is stated with more confidence than
/// the rest of the pane. So the line goes quiet on exactly [`is_stale`]'s
/// boundary — one
/// predicate, deliberately adjacent to it, so the two cannot drift apart.
///
/// An unknown age keeps drawing, consistent with how `None` is treated
/// elsewhere: the pane declines to assert staleness it cannot measure.
///
/// Rendering only. The ring is still fed and the forecast still computed and
/// stored; when a fresh poll lands, the line simply reappears.
fn show_pace_warning(age: Option<Duration>) -> bool {
    !age.is_some_and(is_stale)
}

/// Bar color for a quota window's used fraction: pine/amber/cardinal by
/// severity threshold, or gray when the fraction is unknown.
///
/// The thresholds are lower than the pre-M7b ones (0.80 / 0.95): a quota half
/// spent is worth noticing, and the palette has a caution colour that reads
/// calmly enough to use at that point.
fn fraction_color(fraction: Option<f64>) -> egui::Color32 {
    match fraction {
        // Unknown keeps the existing neutral treatment — an unknown fraction is
        // not a severity, and colouring it would assert something we don't know.
        None => egui::Color32::GRAY,
        Some(f) if f >= 0.80 => CARDINAL,
        Some(f) if f >= 0.50 => AMBER,
        Some(_) => PINE,
    }
}

/// Install a theme on a context.
///
/// Called from the eframe creation closure, from the tray toggle, **and** from
/// the test layout harness, so every width/height assertion measures the real
/// shipped type rather than egui's proportional default. That shared call is
/// the whole point: mono is wider per character, and a harness on the default
/// font would have cheerfully passed a layout that clips in the real window.
///
/// [`Theme::Plain`] restores egui's own dark defaults wholesale — it is the
/// pre-M7b look, so the honest way to produce it is to install nothing rather
/// than to hand-tune a second palette that would drift.
fn install_theme(ctx: &egui::Context, theme: Theme) {
    if theme == Theme::Plain {
        ctx.all_styles_mut(|style| *style = egui::Style::default());
        ctx.set_theme(egui::ThemePreference::Dark);
        return;
    }

    use egui::{FontFamily, FontId, TextStyle};

    // Everything is egui's built-in monospace — no font asset, no new crate.
    // Sizes were arbitrated by the layout harness, not chosen by eye.
    let text_styles: std::collections::BTreeMap<TextStyle, FontId> = [
        (TextStyle::Heading, FontId::new(16.0, FontFamily::Monospace)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Monospace)),
        (
            TextStyle::Monospace,
            FontId::new(13.0, FontFamily::Monospace),
        ),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Monospace)),
        (TextStyle::Small, FontId::new(11.5, FontFamily::Monospace)),
    ]
    .into();

    // Applied to *both* the dark and light styles. The window pins itself to
    // dark below, but a stray light-themed popup inheriting egui's defaults
    // would break the look, and `all_styles_mut` costs nothing to prevent it.
    ctx.all_styles_mut(|style| {
        style.text_styles = text_styles.clone();

        let v = &mut style.visuals;
        v.panel_fill = GROUND;
        v.window_fill = GROUND;
        // `extreme_bg_color` is what `ProgressBar` paints its trough with.
        v.extreme_bg_color = PANEL;
        // Applies wherever text sets no explicit colour; an explicit
        // `RichText::color` still wins, which is how the accents survive.
        v.override_text_color = Some(TEXT);
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, HAIRLINE);
        v.window_stroke = egui::Stroke::new(1.0, HAIRLINE);
    });

    ctx.set_theme(egui::ThemePreference::Dark);
}

// --------------------------------------------------------------------------
// Theme-aware ink. Under CipherPine these are the palette consts; under Plain
// they defer to egui's own visuals, because an explicit `RichText::color`
// overrides the style and would otherwise drag palette colours into the plain
// look. Bar fills are deliberately NOT here — `fraction_color` is shared by
// both themes, since severity is data truth rather than decoration.
// --------------------------------------------------------------------------

fn text_color(ui: &egui::Ui, theme: Theme) -> egui::Color32 {
    match theme {
        Theme::CipherPine => TEXT,
        Theme::Plain => ui.visuals().text_color(),
    }
}

fn dim_color(ui: &egui::Ui, theme: Theme) -> egui::Color32 {
    match theme {
        Theme::CipherPine => TEXT_DIM,
        Theme::Plain => ui.visuals().weak_text_color(),
    }
}

/// The status cursor's state for one frame: `(visible, needs_repaint)`.
///
/// The cursor is a **status indicator, not decoration**: solid means idle and
/// healthy, blinking means the app is working or the data has gone stale.
///
/// `needs_repaint` is the load-bearing half. A solid cursor returns `false`, so
/// an idle healthy window schedules no repaint on the cursor's account and
/// costs nothing while it sits on screen. Only a blinking cursor asks to be
/// woken. Pure, so all three states are unit-testable without a window.
fn cursor_phase(blinking: bool, elapsed: Duration) -> (bool, bool) {
    if !blinking {
        return (true, false);
    }
    let period = CURSOR_BLINK_PERIOD.as_millis();
    let phase = elapsed.as_millis() % period;
    // On for the first half of the period, off for the second — steps.
    (phase * 2 < period, true)
}

/// How long until the blinking cursor next flips, so a repaint can be
/// scheduled exactly at the boundary instead of polling at some safe-but-chatty
/// interval.
fn cursor_next_toggle(elapsed: Duration) -> Duration {
    let half = CURSOR_BLINK_PERIOD.as_millis() / 2;
    let into_half = elapsed.as_millis() % half;
    Duration::from_millis((half - into_half) as u64)
}

/// Whether the status cursor should blink: any provider still awaiting its
/// first poll, or any provider's data gone stale.
///
/// "A poll is in flight" is read as *awaiting the first snapshot* — the poller
/// reports only `Snapshot` and `Failure`, with no in-flight event, so this is
/// the honest signal available without inventing one. A pane that has a
/// snapshot polls again on its own cadence without the window knowing.
fn cursor_should_blink(panes: &[ProviderPane]) -> bool {
    panes.iter().any(|pane| {
        pane_wants_blink(
            pane.handle.is_some(),
            pane.latest_snapshot.is_some(),
            pane.latest_failure.is_some(),
            pane.snapshot_received_at.map(|t| t.elapsed()),
        )
    })
}

/// Whether one pane's state warrants a blinking cursor.
///
/// Split out from [`cursor_should_blink`] as a pure predicate over plain facts:
/// a live `PollerHandle` cannot be constructed in a test without spawning a
/// real poller thread, so testing through `ProviderPane` could only ever
/// exercise the no-handle cases and would quietly assert nothing about the
/// in-flight one.
fn pane_wants_blink(
    has_poller: bool,
    has_snapshot: bool,
    has_failure: bool,
    age: Option<Duration>,
) -> bool {
    let awaiting_first_poll = has_poller && !has_snapshot && !has_failure;
    let stale = age.is_some_and(is_stale);
    awaiting_first_poll || stale
}

/// Paint the block cursor, always allocating its space so the prompt does not
/// jitter as the cursor blinks.
fn render_cursor(ui: &mut egui::Ui, visible: bool) {
    ui.add_space(CURSOR_GAP);
    let (rect, _) = ui.allocate_exact_size(CURSOR_SIZE, egui::Sense::hover());
    if visible {
        ui.painter().rect_filled(rect, 0.0, CARDINAL);
    }
}

/// The blueprint grid: hairline-thin PINE rules every [`GRID_PITCH`] px on both
/// axes, painted under the content.
///
/// Drawn with the painter rather than a background image for the same reason
/// the disclosure triangle is painted: no asset, no decoder, no dependency.
fn paint_grid(ui: &egui::Ui, rect: egui::Rect) {
    let color = egui::Color32::from_rgba_unmultiplied(PINE.r(), PINE.g(), PINE.b(), GRID_ALPHA);
    let stroke = egui::Stroke::new(1.0, color);
    let painter = ui.painter();

    let mut x = rect.left();
    while x <= rect.right() {
        painter.vline(x, rect.y_range(), stroke);
        x += GRID_PITCH;
    }
    let mut y = rect.top();
    while y <= rect.bottom() {
        painter.hline(rect.x_range(), y, stroke);
        y += GRID_PITCH;
    }
}

// --------------------------------------------------------------------------
// System tray (Windows + macOS only) — see the CONTRIBUTING.md / deny.toml
// rationale. Everything below the pure helpers is gated to the tray targets;
// Linux compiles to exactly the pre-M3.5 window-only behavior.
// --------------------------------------------------------------------------

// --------------------------------------------------------------------------
// `--pace-demo`: the fixed synthetic scenario (M8).
//
// The pace markers are slow features — a real at-risk line needs a couple of
// hours of rising usage to appear, and the interesting case (a forecast that
// beats the reset) may not happen on a given day at all. Waiting for one is not
// a review process, and it is not a way to take a screenshot either. So the
// scenario is written down.
//
// Two rules make it trustworthy: it fabricates *snapshots*, then feeds them
// through the same `ingest_pace` a real poll uses (so what renders is the
// shipping code, not a mock of it), and it constructs no provider, no `Egress`
// and no poller, so demo mode cannot reach the network even by accident.
// --------------------------------------------------------------------------

/// The demo series' base observation time — a fixed epoch second, never the
/// clock. Determinism is the requirement: two runs must render identically, or a
/// screenshot means nothing and a regression hides in the noise.
const DEMO_BASE_UNIX_SECS: u64 = 1_785_000_000;
/// Snapshots in each provider's synthetic series.
const DEMO_STEPS: u64 = 4;
/// Spacing between them, in seconds — 20 min, so the series spans an hour and
/// clears `pace::MIN_SPAN_SECS` with room to spare.
const DEMO_STEP_SECS: u64 = 1_200;

/// One window in the demo scenario: where it ends up, and where it came from.
struct DemoWindow {
    /// Headline label, as a provider would report it.
    label: &'static str,
    /// The window's length.
    duration_secs: u64,
    /// Countdown at the **final** snapshot. With `duration_secs` this fixes the
    /// tick's position, so each entry chooses what the reviewer sees.
    final_resets_in_secs: u64,
    /// Used fraction at the first snapshot.
    from_fraction: f64,
    /// Used fraction at the final snapshot. The gap is the burn rate the
    /// forecast will find.
    to_fraction: f64,
}

/// Expand demo windows into the series of snapshots that would have produced
/// them: [`DEMO_STEPS`] snapshots [`DEMO_STEP_SECS`] apart, usage rising
/// linearly and countdowns ticking down.
fn demo_series(
    provider: ProviderId,
    windows: &[DemoWindow],
    per_model: &[QuotaWindow],
    reset_credits: Option<ResetCredits>,
) -> Vec<ProviderSnapshot> {
    let last_step = DEMO_STEPS - 1;
    (0..DEMO_STEPS)
        .map(|step| {
            let elapsed = step * DEMO_STEP_SECS;
            ProviderSnapshot {
                provider,
                taken_at_unix_secs: DEMO_BASE_UNIX_SECS + elapsed,
                windows: windows
                    .iter()
                    .map(|w| {
                        let progress = step as f64 / last_step as f64;
                        QuotaWindow {
                            label: w.label.to_string(),
                            used_fraction: Some(
                                w.from_fraction + (w.to_fraction - w.from_fraction) * progress,
                            ),
                            // Counts down to the final value, so the tick walks
                            // rightward across the series as a real one does.
                            resets_in_secs: Some(
                                w.final_resets_in_secs + (last_step - step) * DEMO_STEP_SECS,
                            ),
                            duration_secs: Some(w.duration_secs),
                        }
                    })
                    .collect(),
                per_model: per_model.to_vec(),
                reset_credits,
                source: usage_core::model::SnapshotSource::UsageEndpoint,
            }
        })
        .collect()
}

/// Spacing between fabricated history points, in seconds (30 min) — 48 of them
/// cover the sparkline's full 24 h.
const DEMO_HISTORY_STEP_SECS: u64 = 1_800;
/// How far the fabricated history reaches back from the series' first
/// snapshot — [`HISTORY_WINDOW_SECS`] minus the hour the series itself spans.
const DEMO_HISTORY_STEPS: u64 =
    (HISTORY_WINDOW_SECS - DEMO_STEPS * DEMO_STEP_SECS).div_ceil(DEMO_HISTORY_STEP_SECS);
/// How much lower the fabricated day starts than the series does.
const DEMO_HISTORY_CLIMB: f64 = 0.35;

/// Fabricate the day of history behind the demo's snapshots (M13).
///
/// The sparkline needs 24 h of readings and the demo series is one hour long,
/// so the missing 23 are written down here — rising toward the series' opening
/// fraction with a small repeating wobble, because a perfectly straight line
/// would read as a rendering bug rather than as usage.
///
/// Deterministic, like everything else in the demo: a fixed base second, a
/// fixed step, and an arithmetic wobble rather than a random one. Two runs draw
/// the same strip, or a screenshot means nothing.
///
/// In memory only — [`ProviderPane::demo`] gives these panes no history path,
/// so not one fabricated reading can reach the user's real log.
fn demo_history(provider: ProviderId, windows: &[DemoWindow]) -> Vec<HistoryEntry> {
    let mut entries = Vec::new();
    for step in 0..DEMO_HISTORY_STEPS {
        // Counted back from the first real snapshot, so the fabricated day ends
        // exactly where the series begins.
        let at = DEMO_BASE_UNIX_SECS - (DEMO_HISTORY_STEPS - step) * DEMO_HISTORY_STEP_SECS;
        let progress = step as f64 / (DEMO_HISTORY_STEPS - 1) as f64;
        for window in windows {
            let base = window.from_fraction - DEMO_HISTORY_CLIMB * (1.0 - progress);
            // A period that does not divide the step count, so the wobble never
            // lines up into a visible stripe.
            let wobble = 0.012 * ((step % 7) as f64 - 3.0);
            entries.push(HistoryEntry {
                at_unix_secs: at,
                provider,
                label: window.label.to_string(),
                used_fraction: (base + wobble).clamp(0.0, 1.0),
                duration_secs: Some(window.duration_secs),
            });
        }
    }
    entries
}

/// The preferences `--pace-demo` runs under: the user's, with the three alert
/// settings forced to on-at-the-defaults (M13).
///
/// Forcing all three, not just `alerts`, is the point. The demo exists so the
/// alert surfaces can be seen and screenshotted without waiting for a real
/// quota to fill; a reviewer whose own `config.cfg` says `alert_at=95` would
/// otherwise get a demo with no alert in it and no hint why. The theme is left
/// alone, because that is a look and the reviewer chose it.
///
/// The scenario's Codex `5h` window — 80% spent at 75% elapsed — is what makes
/// exactly one alert fire under these settings, through the ordinary rules
/// (`the_demo_raises_exactly_one_alert` pins it).
fn demo_config(base: Config) -> Config {
    Config {
        alerts: true,
        alert_at: DEFAULT_ALERT_AT,
        alert_mode: AlertMode::Pace,
        ..base
    }
}

/// The scenario, one entry per provider. Deliberately covers every state the
/// feature can be in, so one look at the window reviews all of them:
///
/// - **Claude `5h`** — 20% elapsed, 12% spent and barely moving: fill well left
///   of the tick, comfortably under budget, and *no* warning. The silent case.
/// - **Claude `7d`** — half elapsed, 55% spent and climbing: fills in ~9 h,
///   before the reset 3.5 days out. AMBER, since 9 h is beyond the six-hour
///   cardinal threshold. The pane's warning is this window and not the 5h one,
///   which is the "sooner wins" selection doing its job.
/// - **Codex `5h`** — 75% elapsed, 80% spent and climbing hard: fills in ~1 h
///   against a 1h15m reset. CARDINAL.
/// - **Codex `7d`** — 90% elapsed, 30% spent, flat: the strongest "under
///   budget" read on the screen, and silent despite being the pane's other
///   headline window.
/// - **Codex per-model** — a used bucket and an untouched one, so the
///   disclosure toggle appears with a ticked row behind it (and M7a's filter
///   still hides the untouched one).
/// - **Codex reset credits** — one available, as the real account reports.
fn demo_panes(config: &Config) -> Vec<ProviderPane> {
    let claude_windows = [
        DemoWindow {
            label: "5h",
            duration_secs: 18_000,
            final_resets_in_secs: 14_400,
            from_fraction: 0.10,
            to_fraction: 0.12,
        },
        DemoWindow {
            label: "7d",
            duration_secs: 604_800,
            final_resets_in_secs: 302_400,
            from_fraction: 0.50,
            to_fraction: 0.55,
        },
    ];
    let claude = demo_series(ProviderId::ClaudeSubscription, &claude_windows, &[], None);

    let codex_windows = [
        DemoWindow {
            label: "5h",
            duration_secs: 18_000,
            final_resets_in_secs: 4_500,
            from_fraction: 0.60,
            to_fraction: 0.80,
        },
        DemoWindow {
            label: "7d",
            duration_secs: 604_800,
            final_resets_in_secs: 60_480,
            from_fraction: 0.30,
            to_fraction: 0.30,
        },
    ];
    let codex = demo_series(
        ProviderId::CodexSubscription,
        &codex_windows,
        &[
            QuotaWindow {
                label: "GPT-5.3-Codex-Max".to_string(),
                used_fraction: Some(0.42),
                resets_in_secs: Some(302_400),
                duration_secs: Some(604_800),
            },
            QuotaWindow {
                label: "GPT-5.3-Codex-Spark".to_string(),
                used_fraction: Some(0.0),
                resets_in_secs: Some(302_400),
                duration_secs: Some(604_800),
            },
        ],
        // The real Codex account reports one reset credit, so the demo does
        // too. The demo exists to show the owner what they will actually see,
        // and omitting a line the live pane carries would make the review
        // easier than reality — including its effect on the height budget
        // (see `the_demo_scenario_fits_the_window`).
        Some(ResetCredits {
            available: 1,
            applicable_now: Some(0),
        }),
    );

    vec![
        ProviderPane::demo(
            ProviderId::ClaudeSubscription,
            claude,
            demo_history(ProviderId::ClaudeSubscription, &claude_windows),
            config,
        ),
        ProviderPane::demo(
            ProviderId::CodexSubscription,
            codex,
            demo_history(ProviderId::CodexSubscription, &codex_windows),
            config,
        ),
    ]
}

/// The synthetic session list `--agents-demo` shows (M15).
///
/// Four rows, chosen so one look reviews the whole feature: every state, both
/// providers, a subagent, a row with no branch, and an age in each of
/// [`format_age`]'s two shapes.
///
/// - **Claude, working** — `QuotaPane · main`, seconds old: the green dot.
/// - **Claude, working, subagent** — the same project, marked `· sub`, which is
///   what a fan-out looks like from outside.
/// - **Codex, idle** — a session open on a branch, quiet for a quarter of an
///   hour: amber.
/// - **Codex, recent** — finished hours ago, and carrying *no* branch, so the
///   row proves the separator collapses rather than leaving a gap.
///
/// Ages are frozen at the moment the window opens, exactly as `--pace-demo`'s
/// numbers are: the demo is a fixture, not a clock.
fn demo_agents(now: SystemTime) -> Vec<AgentSession> {
    let row = |provider, short_id: &str, project: &str, branch: Option<&str>, secs, is_subagent| {
        let age = Duration::from_secs(secs);
        AgentSession {
            provider,
            short_id: short_id.to_string(),
            project: project.to_string(),
            branch: branch.map(str::to_string),
            state: agents::state_for_age(age).unwrap_or(AgentState::Recent),
            last_write: now - age,
            age,
            is_subagent,
        }
    };
    vec![
        row(
            ProviderId::ClaudeSubscription,
            "a1b2c3d4",
            "QuotaPane",
            Some("main"),
            12,
            false,
        ),
        row(
            ProviderId::ClaudeSubscription,
            "7e5c91f0",
            "QuotaPane",
            Some("main"),
            41,
            true,
        ),
        row(
            ProviderId::CodexSubscription,
            "9f8e7d6c",
            "payments-api",
            Some("feat/idempotency"),
            14 * 60,
            false,
        ),
        row(
            ProviderId::CodexSubscription,
            "4b2a10de",
            "notes",
            None,
            3 * 3600 + 600,
            false,
        ),
    ]
}

/// The OS window title. Demo mode says so, since the pane is showing numbers
/// — or sessions — that describe nothing real.
fn window_title(demo: bool) -> &'static str {
    if demo {
        "QuotaPane — demo"
    } else {
        "QuotaPane"
    }
}

/// A tray/menu interaction, forwarded from the OS event handlers into the app.
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMessage {
    /// Left-click: show and focus the window.
    ShowAndFocus,
    /// "Show/Hide" menu item: flip the window's visibility.
    ToggleShowHide,
    /// "Theme: …" menu item: switch between Cipher Pine and Plain.
    ToggleTheme,
    /// "Quit" menu item: exit the app for real.
    Quit,
}

/// The window that best represents a provider at a glance: the one closest to
/// its limit (highest used fraction). Ties and unknown fractions resolve to the
/// earliest such window. `None` when the snapshot has no windows.
///
/// Not tray-gated since M13: the sparkline draws this same window's history, so
/// the strip, the tray miniature and the tooltip all speak about one window per
/// provider rather than each picking its own.
fn representative_window(snapshot: &ProviderSnapshot) -> Option<&QuotaWindow> {
    let mut best: Option<&QuotaWindow> = None;
    for window in &snapshot.windows {
        let better = match best {
            None => true,
            Some(current) => {
                window.used_fraction.unwrap_or(-1.0) > current.used_fraction.unwrap_or(-1.0)
            }
        };
        if better {
            best = Some(window);
        }
    }
    best
}

/// One provider's tray line, e.g. `"Claude 5h 42%"`, or `"Claude --"` when no
/// usable snapshot or percentage is available.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn provider_tray_summary(label: &str, snapshot: Option<&ProviderSnapshot>) -> String {
    if let Some(snapshot) = snapshot {
        if let Some(window) = representative_window(snapshot) {
            if let Some(fraction) = window.used_fraction {
                let pct = (fraction * 100.0).round().clamp(0.0, 100.0) as i64;
                let window_label = &window.label;
                return format!("{label} {window_label} {pct}%");
            }
        }
    }
    format!("{label} --")
}

/// The whole tray tooltip: each provider's line joined by `" | "`, e.g.
/// `"Claude 5h 42% | Codex 7d 3%"`.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn tray_tooltip(entries: &[(&str, Option<&ProviderSnapshot>)]) -> String {
    entries
        .iter()
        .map(|(label, snapshot)| provider_tray_summary(label, *snapshot))
        .collect::<Vec<_>>()
        .join(" | ")
}

/// Prefixed to the tray tooltip while any provider is in alert (M13).
///
/// A tooltip is the one alert surface that has to survive being read out of
/// context — hovering the tray says nothing about which pane is which — so the
/// word comes first, before the per-provider summary. The dash is U+2014, as
/// everywhere else in the product's copy.
///
/// The tooltip is a tray surface, so on a platform without a tray (Linux) this
/// is written and never read. Kept compiled everywhere rather than `cfg`-ed
/// out, so the test that pins its exact bytes runs on every CI target — the
/// same treatment `QuotaPaneApp::theme_overridden` gets, and for the same
/// reason: it is code for a platform this build does not have, not dead code.
#[cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]
const ALERT_TOOLTIP_PREFIX: &str = "ALERT — ";

/// Side length (px) of the square icon generated at runtime.
///
/// Not tray-gated: since M7b the same mark is also the window/taskbar icon,
/// which every platform sets — Linux included.
const ICON_SIZE: u32 = 32;

/// Runtime tray-icon integration. Owns the live tray handle and the receiver
/// its OS event handlers feed. Windows + macOS only.
#[cfg(any(target_os = "windows", target_os = "macos"))]
mod tray {
    use eframe::egui;
    use std::sync::mpsc::{self, Receiver};
    use std::sync::{Arc, Mutex};
    use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
    use tray_icon::{
        Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    };

    use super::{Theme, TrayMessage};

    const INITIAL_TOOLTIP: &str = "QuotaPane";

    /// A live tray icon. Dropping it removes the icon, so it lives as long as
    /// the app; the owned menu (and its items) live with it.
    pub struct Tray {
        icon: TrayIcon,
        rx: Receiver<TrayMessage>,
        /// Whether the window is currently shown (the tray toggles this).
        pub visible: bool,
        /// Kept so the label can follow the active theme.
        theme_item: MenuItem,
        last_tooltip: String,
        /// The RGBA bytes currently on screen, so an unchanged render skips
        /// the platform call entirely.
        last_icon: Vec<u8>,
    }

    impl Tray {
        /// Build the tray on the calling (main) thread — both Windows and macOS
        /// require it there. Returns `None` if the OS refuses to create it, in
        /// which case the app falls back to close-to-quit. Registers the
        /// process-wide tray/menu event handlers, which forward into an mpsc
        /// channel drained each frame.
        pub fn create(ctx: &egui::Context, theme: Theme) -> Option<Tray> {
            let (tx, rx) = mpsc::channel::<TrayMessage>();
            // The event handlers must be `Send + Sync`; a bare `Sender` is not
            // `Sync`, so guard it behind a mutex.
            let tx = Arc::new(Mutex::new(tx));

            let menu = Menu::new();
            let show_hide = MenuItem::new("Show/Hide", true, None);
            // Label names the *current* theme, and is relabelled on every
            // switch — the same shape as the Show/Hide item beside it.
            let theme_item = MenuItem::new(theme.menu_label(), true, None);
            let quit = MenuItem::new("Quit", true, None);
            menu.append_items(&[
                &show_hide,
                &theme_item,
                &PredefinedMenuItem::separator(),
                &quit,
            ])
            .ok()?;
            let show_hide_id = show_hide.id().clone();
            let theme_id = theme_item.id().clone();
            let quit_id = quit.id().clone();

            {
                let tx = Arc::clone(&tx);
                let ctx = ctx.clone();
                MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                    let msg = if event.id == show_hide_id {
                        TrayMessage::ToggleShowHide
                    } else if event.id == theme_id {
                        TrayMessage::ToggleTheme
                    } else if event.id == quit_id {
                        TrayMessage::Quit
                    } else {
                        return;
                    };
                    if let Ok(tx) = tx.lock() {
                        let _ = tx.send(msg);
                    }
                    // Wake the event loop so the click feels instant rather than
                    // waiting for the 1s tick.
                    ctx.request_repaint();
                }));
            }

            {
                let tx = Arc::clone(&tx);
                let ctx = ctx.clone();
                TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Ok(tx) = tx.lock() {
                            let _ = tx.send(TrayMessage::ShowAndFocus);
                        }
                        ctx.request_repaint();
                    }
                }));
            }

            // Start from the neutral mark; `set_icon_if_changed` swaps in a
            // live one as soon as the first snapshots land.
            let initial = crate::icon::render_icon(None, None, super::ICON_SIZE);
            let icon = Icon::from_rgba(initial.clone(), super::ICON_SIZE, super::ICON_SIZE).ok()?;
            let icon = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                // Left-click raises the window; the menu is a right-click.
                .with_menu_on_left_click(false)
                .with_tooltip(INITIAL_TOOLTIP)
                .with_icon(icon)
                .build()
                .ok()?;

            Some(Tray {
                icon,
                rx,
                visible: true,
                theme_item,
                last_tooltip: INITIAL_TOOLTIP.to_string(),
                last_icon: initial,
            })
        }

        /// Take every tray/menu event queued since the last frame.
        pub fn take_messages(&self) -> Vec<TrayMessage> {
            self.rx.try_iter().collect()
        }

        /// Update the OS tooltip only when the text actually changed, to avoid
        /// hammering the platform API every frame.
        pub fn set_tooltip_if_changed(&mut self, tooltip: &str) {
            if tooltip != self.last_tooltip {
                let _ = self.icon.set_tooltip(Some(tooltip));
                self.last_tooltip = tooltip.to_string();
            }
        }

        /// Relabel the theme item after a switch.
        pub fn set_theme_label(&self, theme: Theme) {
            self.theme_item.set_text(theme.menu_label());
        }

        /// Swap in a freshly rendered mark, but only when its bytes differ from
        /// what is already displayed.
        ///
        /// The icon is re-rendered every frame (pure arithmetic, cheap), so
        /// without this guard the app would hand the OS an identical bitmap
        /// dozens of times a second. Usage moves slowly; the bytes almost
        /// always match.
        pub fn set_icon_if_changed(&mut self, rgba: Vec<u8>) {
            if rgba == self.last_icon {
                return;
            }
            if let Ok(icon) = Icon::from_rgba(rgba.clone(), super::ICON_SIZE, super::ICON_SIZE) {
                let _ = self.icon.set_icon(Some(icon));
                self.last_icon = rgba;
            }
        }
    }
}

/// Live state for one provider's section. Owns its poller handle and the
/// latest non-secret update; never touches credentials. Independent of every
/// other pane — its staleness and failure are its own.
struct ProviderPane {
    id: ProviderId,
    handle: Option<PollerHandle>,
    latest_snapshot: Option<ProviderSnapshot>,
    snapshot_received_at: Option<Instant>,
    latest_failure: Option<String>,
    /// Set when the poller could not even be started (e.g. no home directory).
    startup_error: Option<String>,
    /// Whether this provider's per-model rows are disclosed. Per pane, not
    /// global: Claude and Codex expand and collapse independently. Starts
    /// collapsed, so the window opens at its established size.
    expanded: bool,
    /// One sample trail per **headline** window, keyed by its label (M8).
    ///
    /// A `Vec` of pairs rather than a map: a provider reports one or two
    /// headline windows, and at that size a linear scan is both faster and
    /// easier to read than hashing. Per-model rows deliberately get no ring in
    /// this slice — they get the elapsed tick, but a forecast per model row
    /// would be several more lines in a 320px pane for a fact the headline
    /// windows already carry.
    ///
    /// Seeded from `history.jsonl` at startup when history is on (M13), so a
    /// restart resumes the trail instead of going quiet for two hours.
    rings: Vec<(String, PaceRing)>,
    /// The last [`HISTORY_WINDOW_SECS`] of readings for this provider, oldest
    /// first — what the sparkline is drawn from.
    ///
    /// Empty unless history is on, or `--pace-demo` fabricated one. Held
    /// separately from [`Self::rings`] because the two answer different
    /// questions over different spans: a ring is the last two hours, fitted;
    /// this is the last day, drawn.
    history: Vec<HistoryEntry>,
    /// Whether this pane maintains [`Self::history`] at all.
    history_enabled: bool,
    /// Where readings are appended. `None` means write nothing — history is
    /// off, or this is a demo pane, whose fabricated numbers must never reach
    /// the user's real log.
    history_path: Option<PathBuf>,
    /// Labels of this pane's headline windows currently in alert (M13).
    ///
    /// This **is** the debounce: a label enters once, on the crossing, and
    /// leaves only when the window falls back under the threshold. A `Vec`
    /// rather than a set for the same reason [`Self::rings`] is one — a
    /// provider reports one or two headline windows.
    alerts_firing: Vec<String>,
    /// The banner for the worst window currently in alert, already formatted.
    ///
    /// Stored rather than derived at render time, on the same repaint
    /// discipline as [`Self::pace_warning`]: the window repaints about once a
    /// second, and none of the inputs can have changed since the last poll.
    /// Doubles as the "is this pane alerting" flag the tray reads.
    alert_line: Option<String>,
    /// The refill line, shown for the one poll cycle after a window drops back
    /// under the threshold, then cleared. There is no ongoing state to report —
    /// a quota that is fine is reported by the pane looking normal.
    refill_line: Option<String>,
    /// This provider's at-risk line, or `None` for "nothing to say".
    ///
    /// Recomputed **only** when a snapshot arrives, and stored rather than
    /// derived at render time. That is the repaint discipline: the window
    /// already repaints about once a second for the freshness dot, and running a
    /// least-squares fit on each of those frames would be arithmetic in a hot
    /// loop to produce a value that cannot have changed since the last poll.
    pace_warning: Option<PaceWarning>,
}

impl ProviderPane {
    /// Build a pane from an optional provider. `None` means the credential
    /// path could not be resolved at all (no home directory), which is a
    /// startup error rather than a per-poll failure.
    fn new<P: UsageProvider + Send + 'static>(id: ProviderId, provider: Option<P>) -> Self {
        let (handle, startup_error) = match provider {
            // Each provider gets its own egress instance; it moves into the
            // poller thread.
            //
            // `false` is unconditional and deliberate (SECURITY.md invariant
            // 7): the window has no proxy opt-in surface at all, and must not
            // grow one — there is no flag, no setting, and no code path that
            // can reach `Egress::new(true)` from here. Under a proxy
            // environment the chokepoint therefore refuses to send and the
            // pane shows the error, which is the intended outcome: a
            // TLS-inspecting proxy can read the bearer token, and consenting
            // to that is a per-run decision made deliberately at a command
            // line (`quotapane-cli --allow-proxy`), not a checkbox in an
            // always-on window that a user ticks once and forgets.
            Some(p) => (Some(poller::spawn(p, Egress::new(false))), None),
            None => (
                None,
                Some("could not resolve a home directory for the credentials path".to_string()),
            ),
        };
        ProviderPane {
            id,
            handle,
            latest_snapshot: None,
            snapshot_received_at: None,
            latest_failure: None,
            startup_error,
            expanded: false,
            rings: Vec::new(),
            history: Vec::new(),
            history_enabled: false,
            history_path: None,
            alerts_firing: Vec::new(),
            alert_line: None,
            refill_line: None,
            pace_warning: None,
        }
    }

    /// A pane with no provider and no poller, replaying a fixed series of
    /// snapshots — the `--pace-demo` path.
    ///
    /// The series goes through [`Self::ingest_pace`], the same function a real
    /// poll uses, so what the demo shows is the shipping pace code applied to
    /// synthetic numbers rather than a parallel mock of it. `handle` is `None`,
    /// so no poller thread exists, no [`Egress`] is ever constructed, and this
    /// pane cannot make a request even in principle.
    ///
    /// `history` is the fabricated 24 h trail behind the sparkline. It is held
    /// in memory only: `history_path` stays `None`, so a demo run cannot write
    /// a single invented reading into the user's real `history.jsonl`.
    fn demo(
        id: ProviderId,
        series: Vec<ProviderSnapshot>,
        history: Vec<HistoryEntry>,
        config: &Config,
    ) -> Self {
        let mut pane = ProviderPane {
            id,
            handle: None,
            latest_snapshot: None,
            snapshot_received_at: None,
            latest_failure: None,
            startup_error: None,
            expanded: false,
            rings: Vec::new(),
            history,
            history_enabled: true,
            history_path: None,
            alerts_firing: Vec::new(),
            alert_line: None,
            refill_line: None,
            pace_warning: None,
        };
        for snapshot in series {
            pane.ingest_pace(&snapshot);
            // The same calls a real poll makes, so the demo's last hour joins
            // the fabricated day through the shipping code rather than beside
            // it, and the alert surfaces are raised by the alert rules rather
            // than switched on for the screenshot. With no `history_path` this
            // touches memory only.
            pane.record_history(&snapshot);
            pane.update_alerts(&snapshot, config);
            pane.latest_snapshot = Some(snapshot);
        }
        // The dot's age is the one thing the demo does not fake: it counts
        // from now, so the dot starts green, ages to amber and goes stale on
        // schedule exactly as a live one would.
        pane.snapshot_received_at = Some(Instant::now());
        pane
    }

    /// Turn history on for this pane: keep the day's readings for the
    /// sparkline, replay the pace trail, and start appending to `path` (M13).
    ///
    /// `entries` is the whole decoded log — both providers' lines — so the
    /// caller reads and retains the file once rather than once per pane.
    ///
    /// What survives a restart is the **trail**, not the forecast: a
    /// [`PaceWarning`] needs the reset countdown, which history deliberately
    /// does not store, so the line reappears with the first poll rather than
    /// before it. Ahead of M13 that first poll produced nothing at all and the
    /// pane stayed silent for the two hours it took to refill a ring.
    fn enable_history(&mut self, path: PathBuf, entries: &[HistoryEntry], now_unix_secs: u64) {
        self.history_enabled = true;
        self.history_path = Some(path);

        let mine: Vec<HistoryEntry> = entries
            .iter()
            .filter(|entry| entry.provider == self.id)
            .cloned()
            .collect();

        // Seed each window's ring through `observe`, the same door a live poll
        // uses — so a reset inside the replayed span still clears the trail
        // (the used-fraction signal works without a countdown, and the
        // countdown signal being unavailable can only fail toward keeping
        // samples, never toward inventing a reset).
        for entry in history::rehydratable(&mine, now_unix_secs) {
            let index = match self
                .rings
                .iter()
                .position(|(label, _)| label == &entry.label)
            {
                Some(index) => index,
                None => {
                    self.rings.push((entry.label.clone(), PaceRing::new()));
                    self.rings.len() - 1
                }
            };
            self.rings[index].1.observe(
                PaceSample {
                    at_unix_secs: entry.at_unix_secs,
                    used_fraction: entry.used_fraction,
                },
                None,
                entry.duration_secs,
            );
        }

        self.history = history::within(&mine, now_unix_secs, HISTORY_WINDOW_SECS);
    }

    /// Record a fresh snapshot's headline readings: append them to the log (when
    /// one is configured) and keep the in-memory day trail current.
    ///
    /// A write failure is swallowed, exactly as a preference write is: a full
    /// disk is not worth an error dialog in an always-on-top window, and the
    /// only thing lost is a sparkline's memory.
    fn record_history(&mut self, snapshot: &ProviderSnapshot) {
        if !self.history_enabled {
            return;
        }
        let entries = history::entries_for_snapshot(snapshot);
        if entries.is_empty() {
            return;
        }
        if let Some(path) = &self.history_path {
            let _ = history::append(path, &entries);
        }
        self.history.extend(entries);
        // Pruned against the newest reading rather than the wall clock, so the
        // trail this pane holds is exactly the span the sparkline draws.
        let newest = self
            .history
            .iter()
            .map(|entry| entry.at_unix_secs)
            .max()
            .unwrap_or(0);
        self.history = history::within(&self.history, newest, HISTORY_WINDOW_SECS);
    }

    /// Fold a fresh snapshot's headline windows into their sample trails, then
    /// recompute this pane's at-risk line (M8).
    ///
    /// The sample's timestamp is the provider's own `taken_at_unix_secs`, not
    /// the moment this code ran: the pace math has to be anchored to when the
    /// numbers were true. That is also why no clock is read here — the snapshot
    /// carries the only time that matters.
    fn ingest_pace(&mut self, snapshot: &ProviderSnapshot) {
        let now = snapshot.taken_at_unix_secs;
        let mut candidates: Vec<(String, Burn, Option<u64>)> = Vec::new();

        for window in &snapshot.windows {
            // Without a usage fraction there is nothing to sample. The window
            // still renders; it just contributes no pace.
            let Some(used_fraction) = window.used_fraction else {
                continue;
            };

            // An index rather than a held reference, so the miss branch can push
            // without the lookup's borrow still being alive.
            let index = match self
                .rings
                .iter()
                .position(|(label, _)| label == &window.label)
            {
                Some(index) => index,
                None => {
                    self.rings.push((window.label.clone(), PaceRing::new()));
                    self.rings.len() - 1
                }
            };
            let ring = &mut self.rings[index].1;

            // `observe` clears the trail itself when these facts say the window
            // reset — the UI does not second-guess it.
            ring.observe(
                PaceSample {
                    at_unix_secs: now,
                    used_fraction,
                },
                window.resets_in_secs,
                window.duration_secs,
            );

            if let Some(burn) = pace::estimate(ring.samples(), now) {
                candidates.push((window.label.clone(), burn, window.resets_in_secs));
            }
        }

        self.pace_warning = select_pace_warning(&candidates);
    }

    /// Fold a fresh snapshot into this pane's alert state (M13).
    ///
    /// Returns `true` when a **new** alert was raised, which is the one event
    /// that earns a taskbar attention request — a window that was already
    /// alerting last poll and still is asks for nothing.
    ///
    /// Runs on the poll event, like the pace fit beside it, and leaves the two
    /// rendered lines formatted and ready.
    fn update_alerts(&mut self, snapshot: &ProviderSnapshot, config: &Config) -> bool {
        // A label the provider has stopped reporting cannot refill, and would
        // otherwise sit in the firing list forever suppressing a later crossing.
        self.alerts_firing
            .retain(|label| snapshot.windows.iter().any(|w| &w.label == label));

        let mut raised = false;
        let mut refilled: Option<String> = None;

        for window in &snapshot.windows {
            let already_firing = self.alerts_firing.iter().any(|l| l == &window.label);
            match alert_action(
                config,
                window.used_fraction,
                elapsed_fraction(window),
                already_firing,
            ) {
                AlertAction::Fire => {
                    self.alerts_firing.push(window.label.clone());
                    raised = true;
                }
                AlertAction::Refill => {
                    self.alerts_firing.retain(|l| l != &window.label);
                    refilled = Some(window.label.clone());
                }
                AlertAction::Quiet => {}
            }
        }

        // Worst offender only. Two alert lines in a 320px pane would make the
        // reader compare them instead of act — the same choice the at-risk line
        // makes (see `select_pace_warning`).
        self.alert_line = snapshot
            .windows
            .iter()
            .filter(|w| self.alerts_firing.iter().any(|l| l == &w.label))
            .filter_map(|w| w.used_fraction.map(|used| (w, used)))
            .max_by(|(_, a), (_, b)| a.total_cmp(b))
            .map(|(window, used)| {
                alert_banner_line(
                    self.id,
                    &window.label,
                    used,
                    config.alert_at,
                    config.alert_mode,
                )
            });

        // Cleared and re-set every poll, so the notice shows once and the pane
        // then goes back to looking ordinary.
        self.refill_line =
            refilled.map(|label| alert_refill_line(self.id, &label, config.alert_at));

        raised
    }

    /// Fold one poller update into this pane's state.
    ///
    /// Returns `true` when it raised a new alert.
    ///
    /// Split out from [`Self::drain`] as a function over a plain `Update`, for
    /// the same reason [`pane_wants_blink`] is split out of
    /// [`cursor_should_blink`]: a live [`PollerHandle`] cannot be built in a
    /// test without spawning a real poller thread, so everything reachable only
    /// through the channel would otherwise be asserted by nothing.
    fn apply_update(&mut self, update: Update, config: &Config) -> bool {
        match update {
            Update::Snapshot(snapshot) => {
                // Pace first: the trail is fed from the arriving snapshot, and
                // the at-risk line is recomputed here — on the poll event —
                // rather than on every frame that renders it.
                self.ingest_pace(&snapshot);
                // Then the day trail and, when it is on, the disk log.
                self.record_history(&snapshot);
                let raised = self.update_alerts(&snapshot, config);
                self.latest_snapshot = Some(snapshot);
                self.snapshot_received_at = Some(Instant::now());
                // A fresh success supersedes any prior failure.
                self.latest_failure = None;
                raised
            }
            Update::Failure { message, .. } => {
                self.latest_failure = Some(message);
                false
            }
        }
    }

    /// Drain this pane's channel, folding every pending update into its state.
    ///
    /// Returns `true` when a new alert was raised by one of the updates.
    fn drain(&mut self, config: &Config) -> bool {
        let mut raised = false;
        let Some(handle) = &self.handle else {
            return raised;
        };
        // Collected before the loop so the channel borrow ends: folding a
        // snapshot in now takes `&mut self` (the pace trails). `try_iter` drains
        // only what is already queued — normally nothing, or one poll — so this
        // holds no more than the loop would have.
        let updates: Vec<Update> = handle.updates().try_iter().collect();
        for update in updates {
            raised |= self.apply_update(update, config);
        }
        raised
    }

    /// Stop the poller thread, if one is running.
    fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}

/// Draggable, always-on-top window showing live quota for every provider.
struct QuotaPaneApp {
    panes: Vec<ProviderPane>,
    /// Whether close-to-tray is in effect. False on Linux, when `--no-tray` is
    /// given, or if tray creation failed — in all of which close quits.
    tray_active: bool,
    /// Set once the user picks "Quit" from the tray, so the close interceptor
    /// lets the real exit through.
    quitting: bool,
    /// The persisted preferences, as loaded at startup and mutated by the tray
    /// toggle. One field on the app: `config.theme` is the single value every
    /// render path branches on, and a save writes the whole struct back, so a
    /// toggle can never drop a preference it did not touch.
    config: Config,
    /// True when `--plain`/`--themed` picked the theme for this run, so the
    /// flag is not written back to disk.
    ///
    /// Read by [`Self::persist_preferences`], the app's single writer: a save
    /// triggered by anything else (a grip resize) must not smuggle a run-only
    /// look into the file on its way past.
    theme_overridden: bool,
    /// The theme as the file names it, captured before `--plain`/`--themed`
    /// was applied. Written back verbatim whenever `theme_overridden` is set.
    disk_theme: Theme,
    /// True under `--pace-demo`, so the titlebar can say the pane is synthetic.
    pace_demo: bool,
    /// Which pane the switcher is showing (M15). Runtime state, never saved.
    view: View,
    /// True under `--agents-demo`: [`agents`] is a fixture and no scan may run.
    agents_demo: bool,
    /// Where a scan looks. [`SessionRoots::default`] — nowhere — under
    /// `--agents-demo`, so the demo cannot reach a real log even by mistake.
    agent_roots: SessionRoots,
    /// The most recent scan's rows, or the demo fixture.
    agents: Vec<AgentSession>,
    /// When the last scan ran. `None` means "not since this view was opened",
    /// which is what makes a switch scan on the very next frame.
    last_agent_scan: Option<Instant>,
    /// A height seen on an earlier frame that has not settled yet: the height
    /// itself and when it was first observed (M14). `None` means the window is
    /// sitting at the height already stored.
    pending_height: Option<(u32, Instant)>,
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    tray: Option<tray::Tray>,
}

impl QuotaPaneApp {
    /// Write the preferences to disk. The app's **single** writer.
    ///
    /// Two things it refuses to do, both of which a naive `config::save` at
    /// each call site would get wrong:
    ///
    /// - **Never under `--pace-demo`.** `demo_config` forces alerts on for the
    ///   fixture; persisting that would hand a real user settings they never
    ///   chose, and a demo height they never asked for.
    /// - **Never a run-only theme.** `--plain`/`--themed` picks a look for one
    ///   run, so the stored word is written back untouched.
    fn persist_preferences(&self) {
        if self.pace_demo {
            return;
        }
        let mut on_disk = self.config;
        if self.theme_overridden {
            on_disk.theme = self.disk_theme;
        }
        config::save(&on_disk);
    }

    /// Rescan the local session logs, if this frame is one that may (M15).
    ///
    /// **The only place in this crate that names [`agents::scan`]**, and the
    /// gate is [`should_scan`], which answers `false` for every frame the usage
    /// view is showing and for every frame at all under `--agents-demo`. A
    /// window sitting on `usage` therefore does not read a session log, does
    /// not `read_dir` a session root, and does not so much as `stat` one.
    fn refresh_agents(&mut self) {
        if !should_scan(
            self.view,
            self.agents_demo,
            self.last_agent_scan.map(|at| at.elapsed()),
        ) {
            return;
        }
        self.last_agent_scan = Some(Instant::now());
        self.agents = agents::scan(&self.agent_roots, SystemTime::now());
    }

    /// Switch views, and arm the next frame to scan when the destination is the
    /// agents pane.
    fn switch_to(&mut self, view: View) {
        if self.view == view {
            return;
        }
        self.view = view;
        // Clearing the stamp is what "a scan on switch" means: the list a user
        // is shown is gathered after they ask for it, never before.
        self.last_agent_scan = None;
    }

    /// Notice that the user has resized the window, and remember the new height
    /// once they stop (M14).
    ///
    /// There is no drag-ended event to hang this on: `BeginResize` hands the
    /// drag to the OS, which simply resizes the window under egui. So the size
    /// is *observed* each frame and written once it has held still for
    /// [`HEIGHT_SETTLE`] — see [`height_action`] for the decision itself.
    fn track_height(&mut self, ctx: &egui::Context) {
        // The demo neither reads nor writes the saved height.
        if self.pace_demo {
            return;
        }
        let current = ctx
            .viewport_rect()
            .height()
            .round()
            .clamp(MIN_WINDOW_HEIGHT, MAX_WINDOW_HEIGHT) as u32;
        let pending = self
            .pending_height
            .map(|(height, since)| (height, since.elapsed()));
        match height_action(self.config.height, current, pending) {
            HeightAction::Nothing => self.pending_height = None,
            HeightAction::Wait => {
                if self.pending_height.map(|(height, _)| height) != Some(current) {
                    self.pending_height = Some((current, Instant::now()));
                }
                // The 1s tick in `logic` would get here eventually; this makes
                // the write land promptly after the user lets go.
                ctx.request_repaint_after(HEIGHT_SETTLE);
            }
            HeightAction::Write => {
                self.config.height = current;
                self.pending_height = None;
                self.persist_preferences();
            }
        }
    }

    /// Draw the bottom resize grip: an 8px strip carrying a right-aligned
    /// [`GRIP_GLYPH`], faint at rest and full-strength under the pointer.
    ///
    /// Returns `true` when it was double-clicked, which the caller turns into a
    /// snap-to-fit *after* the content has been laid out — the fit is measured
    /// from this frame's content, not last frame's.
    ///
    /// Dragging it does not resize the window here: it asks the OS to take over
    /// the drag ([`egui::ViewportCommand::BeginResize`] southwards), which is
    /// what makes the resize feel native and keeps the width pinned by the
    /// viewport's own min/max rather than by arithmetic in this file.
    fn render_grip(&self, root_ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
        let theme = self.config.theme;
        let mut snap = false;

        // The pane's own fill, so the strip reads as the bottom of the window
        // rather than as a band of a different colour under it. Taken from the
        // installed visuals, so Plain gets Plain's panel and Cipher Pine gets
        // GROUND without either being named here.
        let frame = egui::Frame::new()
            .fill(root_ui.visuals().panel_fill)
            .inner_margin(egui::Margin {
                left: 0,
                right: 4,
                top: 0,
                bottom: 0,
            });

        egui::Panel::bottom("resize_grip")
            .exact_size(GRIP_HEIGHT)
            .resizable(false)
            .show_separator_line(false)
            .frame(frame)
            .show(root_ui, |ui| {
                let rect = ui.max_rect();
                let response = ui.interact(
                    rect,
                    ui.id().with("resize_grip"),
                    egui::Sense::click_and_drag(),
                );

                let ink = match (theme, response.hovered()) {
                    (Theme::CipherPine, false) => TEXT_FAINT,
                    (Theme::CipherPine, true) => TEXT,
                    (Theme::Plain, false) => ui.visuals().weak_text_color(),
                    (Theme::Plain, true) => ui.visuals().text_color(),
                };
                ui.painter().text(
                    rect.right_center(),
                    egui::Align2::RIGHT_CENTER,
                    GRIP_GLYPH,
                    egui::FontId::monospace(GRIP_HEIGHT),
                    ink,
                );

                if response.hovered() {
                    ctx.set_cursor_icon(egui::CursorIcon::ResizeVertical);
                }
                if response.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(
                        egui::ResizeDirection::South,
                    ));
                }
                if response.double_clicked() {
                    snap = true;
                }
            });

        snap
    }

    /// Hide the window (minimize-to-tray). On Windows/macOS the app lives on in
    /// the tray; on Linux (no tray) it simply hides. Keeps the tray's toggle
    /// state in sync so a later Show/Hide behaves correctly.
    fn hide_window(&mut self, ctx: &egui::Context) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        if let Some(tray) = self.tray.as_mut() {
            tray.visible = false;
        }
    }

    /// Draw the slim custom titlebar: app name on the left, minimize + close on
    /// the right, and the strip itself as a window-drag handle. Rendered on
    /// every platform (Linux included). The buttons only *record* intent inside
    /// the panel closure; the app acts on it after the closure returns, so no
    /// `&mut self` is borrowed across the egui closure.
    fn render_titlebar(&mut self, root_ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut minimize = false;
        let mut close = false;
        let mut switch_to = None;

        let theme = self.config.theme;
        // Either demo marks the prompt: M13's reason for the marker was that a
        // decoration-less window shows its OS title only in the taskbar, so a
        // pane of invented data has to say so where the data is. A pane of
        // invented *sessions* is the same claim.
        let demo = self.pace_demo || self.agents_demo;
        let view = self.view;

        // Status cursor: computed before the panel closure so the pane borrow
        // ends before the closure takes `&mut self` state. Plain has no cursor
        // at all, so it never asks for a repaint either.
        let blinking = theme == Theme::CipherPine && cursor_should_blink(&self.panes);
        let elapsed = Duration::from_secs_f64(ctx.input(|i| i.time).max(0.0));
        let (cursor_visible, cursor_needs_repaint) = cursor_phase(blinking, elapsed);

        let titlebar_fill = match theme {
            Theme::CipherPine => PANEL,
            Theme::Plain => PLAIN_TITLEBAR_BG,
        };
        let frame = egui::Frame::new()
            .fill(titlebar_fill)
            .inner_margin(egui::Margin {
                left: TITLEBAR_MARGIN_LEFT,
                right: TITLEBAR_MARGIN_RIGHT,
                top: 0,
                bottom: 0,
            });

        egui::Panel::top("titlebar")
            .exact_size(TITLEBAR_HEIGHT)
            .resizable(false)
            .show_separator_line(false)
            .frame(frame)
            .show(root_ui, |ui| {
                // Whole-strip drag handle. Added first so the buttons drawn
                // afterwards sit on top and keep their own clicks; dragging the
                // empty strip (or the app name) moves the decoration-less window.
                let strip = ui.interact(
                    ui.max_rect(),
                    ui.id().with("titlebar_drag"),
                    egui::Sense::click_and_drag(),
                );
                if strip.drag_started() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

                // Buttons pinned right (close is right-most); the app name fills
                // the remaining space on the left.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if titlebar_button(ui, draw_close_glyph).clicked() {
                        close = true;
                    }
                    if titlebar_button(ui, draw_minimize_glyph).clicked() {
                        minimize = true;
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        render_prompt(ui, theme, demo);
                        if theme == Theme::CipherPine {
                            render_cursor(ui, cursor_visible);
                        }
                        // Drawn after the strip's drag handle, so its labels
                        // keep their own clicks (the same ordering the buttons
                        // above rely on).
                        switch_to = render_view_switcher(ui, theme, view);
                    });
                });
            });

        // Only a blinking cursor schedules a repaint; a solid one costs
        // nothing. Scheduled at the exact next flip rather than on a safe
        // interval, so blinking is crisp without extra wake-ups.
        if cursor_needs_repaint {
            ctx.request_repaint_after(cursor_next_toggle(elapsed));
        }

        // The 1px HAIRLINE rule under the strip. Painted rather than using
        // egui's separator line so it spans the full width with no margin.
        if theme == Theme::CipherPine {
            let bar = root_ui.max_rect();
            root_ui.painter().hline(
                bar.x_range(),
                bar.top() + TITLEBAR_HEIGHT,
                egui::Stroke::new(1.0, HAIRLINE),
            );
        }

        if let Some(view) = switch_to {
            self.switch_to(view);
        }
        if minimize {
            self.hide_window(ctx);
        }
        if close {
            // Exactly the OS-close path: the `logic()` interceptor hides to tray
            // when the tray is active, or lets the close through (quit) under
            // `--no-tray` / on Linux.
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// Refresh the tooltip and drain queued tray/menu events, acting on each.
    /// Runs from `logic`, so it keeps working even while the window is hidden.
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    fn service_tray(&mut self, ctx: &egui::Context) {
        // Any pane alerting puts the tray into its alert variant: the ringed
        // icon and the prefixed tooltip. Both revert by themselves the moment
        // no pane is alerting — there is no separate "clear" path to forget.
        let alerting = self.panes.iter().any(|pane| pane.alert_line.is_some());

        // Build the tooltip from the same snapshots the window renders.
        let tooltip = {
            let entries: Vec<(&str, Option<&ProviderSnapshot>)> = self
                .panes
                .iter()
                .map(|pane| (provider_label(pane.id), pane.latest_snapshot.as_ref()))
                .collect();
            let tooltip = tray_tooltip(&entries);
            if alerting {
                format!("{ALERT_TOOLTIP_PREFIX}{tooltip}")
            } else {
                tooltip
            }
        };

        // The live miniature: the tray mark carries each provider's
        // representative headline fraction, so the tray and the window report
        // the same thing. Rendered from the same snapshots as the tooltip.
        let rgba = {
            let fraction = |id: ProviderId| {
                self.panes
                    .iter()
                    .find(|pane| pane.id == id)
                    .and_then(|pane| pane.latest_snapshot.as_ref())
                    .and_then(representative_window)
                    .and_then(|window| window.used_fraction)
                    .map(|f| f as f32)
            };
            icon::render_icon_with_alert(
                fraction(ProviderId::ClaudeSubscription),
                fraction(ProviderId::CodexSubscription),
                ICON_SIZE,
                alerting,
            )
        };

        // Update the tooltip and collect events without holding the tray borrow
        // across the app mutations below.
        let messages = match self.tray.as_mut() {
            Some(tray) => {
                tray.set_tooltip_if_changed(&tooltip);
                tray.set_icon_if_changed(rgba);
                tray.take_messages()
            }
            None => return,
        };

        for message in messages {
            match message {
                TrayMessage::ShowAndFocus => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                    if let Some(tray) = self.tray.as_mut() {
                        tray.visible = true;
                    }
                }
                TrayMessage::ToggleShowHide => {
                    if let Some(tray) = self.tray.as_mut() {
                        tray.visible = !tray.visible;
                        let visible = tray.visible;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(visible));
                        if visible {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                        }
                    }
                }
                TrayMessage::ToggleTheme => {
                    self.config.theme = self.config.theme.toggled();
                    // Live switch: restyle the context and repaint once.
                    install_theme(ctx, self.config.theme);
                    ctx.request_repaint();
                    if let Some(tray) = self.tray.as_mut() {
                        tray.set_theme_label(self.config.theme);
                    }
                    // A `--plain`/`--themed` launch picked the theme for this
                    // run only, so a toggle during it must not rewrite the
                    // stored default — `persist_preferences` is what knows
                    // that. Everything else in `config` came off disk
                    // unchanged, so writing the whole struct back is a no-op for
                    // every key but the theme.
                    self.persist_preferences();
                }
                TrayMessage::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }
}

impl eframe::App for QuotaPaneApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Fold in every pending poller update. Runs even while hidden to tray,
        // so the tooltip stays live and Show/Quit keep responding.
        let mut alert_raised = false;
        for pane in &mut self.panes {
            alert_raised |= pane.drain(&self.config);
        }

        // The third alert surface, and the only one that reaches outside the
        // app: one attention request per new crossing, never per poll and never
        // per frame. `Informational` is the gentle variant — on Windows it
        // flashes the taskbar button rather than forcing the window forward,
        // which is the right volume for "your quota is nearly gone" in a
        // window the user chose to keep on top. A no-op where unsupported.
        if alert_raised {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        }

        #[cfg(any(target_os = "windows", target_os = "macos"))]
        self.service_tray(ctx);

        // Close-to-tray: when the tray is active, a close request hides the
        // window instead of quitting. Always compiled; on platforms without a
        // tray `tray_active` is always false, so close quits exactly as before.
        if self.tray_active && !self.quitting && ctx.input(|i| i.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            if let Some(tray) = self.tray.as_mut() {
                tray.visible = false;
            }
        }

        // The agents list, and only while its view is showing — see
        // [`Self::refresh_agents`] and [`should_scan`].
        self.refresh_agents();
        if self.view == View::Agents && !self.agents_demo {
            ctx.request_repaint_after(SCAN_EVERY);
        }

        // Keep polling for updates (and tray events) about once a second, even
        // when the window is hidden.
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let theme = self.config.theme;

        // A resize the OS performed is noticed here, before anything draws, so
        // a settled height is written the moment the user lets go of the edge.
        self.track_height(&ctx);

        // Slim custom titlebar first (a top panel): app name + minimize/close,
        // and a window-drag handle. Takes its ~24px; the CentralPanel fills the
        // rest below it.
        self.render_titlebar(ui, &ctx);

        // The grip, second: also chrome, also carved out before the content, so
        // the 8px it takes is never 8px the ScrollArea thinks it has.
        let snap_requested = self.render_grip(ui, &ctx);

        // The `CentralPanel`'s own frame, measured from the live style — the
        // snap has to add back everything the content is not given, and the
        // layout harness measures it exactly this way.
        let panel_chrome_height = egui::Frame::central_panel(ui.style())
            .total_margin()
            .sum()
            .y;
        let mut content_height = 0.0;

        // The root `Ui` eframe hands to `App::ui` has no margin or background
        // (see `eframe::App::ui` docs) — a `CentralPanel` supplies both.
        egui::CentralPanel::default().show(ui, |ui| {
            // Whole-panel drag handle: any click-drag not consumed by a widget
            // above it moves the (decoration-less) window.
            let bg_rect = ui.max_rect();
            let bg_response = ui.interact(
                bg_rect,
                ui.id().with("background_drag"),
                egui::Sense::drag(),
            );
            if bg_response.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            // Blueprint grid, painted before any content so it sits
            // underneath. Cipher Pine only — Plain is the pre-M7b look.
            if theme == Theme::CipherPine {
                paint_grid(ui, bg_rect);
            }

            // Vertical safety net. The window opens at 240px and the user is
            // free to leave it there, so content that outgrows it still needs
            // somewhere to go: expanding several per-model rows (two lines
            // each), or Claude also reporting per-model windows, would push the
            // last row out of the window with no way to reach it. This converts
            // that silent truncation into a scroll — and since M14 the grip's
            // double-click is the other answer, resizing the window to the
            // content instead of scrolling the content inside the window.
            //
            // Deliberately invisible until needed: egui shows no scroll bar
            // while the content fits, which is every state accepted so far, so
            // this changes nothing the owner has already signed off on.
            //
            // `DragScroll::Never` matters — the default is `OnTouch`, and on a
            // touch-capable Windows machine that would turn a drag on the pane
            // background into a scroll, stealing the only gesture that moves
            // this decoration-less window. Wheel and scroll bar stay enabled.
            let view = self.view;
            content_height = egui::ScrollArea::vertical()
                .scroll_source(egui::containers::scroll_area::ScrollSource {
                    drag: egui::containers::scroll_area::DragScroll::Never,
                    ..Default::default()
                })
                .show(ui, |ui| match view {
                    View::Usage => render_panes(ui, &mut self.panes, theme),
                    View::Agents => render_agents(ui, &self.agents, theme),
                })
                .content_size
                .y;
        });

        // Snap-to-fit, acted on after the layout so the fit is measured from
        // the content this frame actually produced. The width component is
        // [`WINDOW_WIDTH`] and nothing else — see [`snap_command`].
        if snap_requested {
            ctx.send_viewport_cmd(snap_command(content_height, panel_chrome_height));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Stop every poller thread, not just the first.
        for pane in &mut self.panes {
            pane.stop();
        }
    }
}

/// The titlebar's shell prompt: `> quotapane`, the caret in CARDINAL.
///
/// Two labels with horizontal item spacing zeroed rather than one string:
/// egui colours a `RichText` as a unit, and the caret is the only cardinal
/// part. The leading space lives in the second label so the caret keeps tight
/// bounds — which matters once the status cursor sits after the name.
/// Under either demo flag the prompt carries a `demo` marker in AMBER, in both
/// themes: this window is decoration-less, so its OS title shows only in the
/// taskbar, and a pane full of invented numbers — or invented sessions — has
/// to say so where they are.
fn render_prompt(ui: &mut egui::Ui, theme: Theme, demo: bool) {
    if theme == Theme::Plain {
        // The pre-M7b titlebar: just the product name.
        ui.label(egui::RichText::new("QuotaPane").strong());
        if demo {
            ui.label(egui::RichText::new("demo").small().color(AMBER));
        }
        return;
    }
    ui.spacing_mut().item_spacing.x = 0.0;
    ui.label(egui::RichText::new(">").color(CARDINAL).strong());
    ui.label(egui::RichText::new(" quotapane").color(TEXT));
    if demo {
        ui.label(egui::RichText::new(" demo").color(AMBER));
    }
}

/// A provider section header: `// CLAUDE` — comment slashes CARDINAL, name
/// uppercase TEXT_DIM — with the freshness dot pinned to its right edge.
///
/// egui has no letter-spacing control, so plain uppercase mono is the
/// approximation the spec accepts rather than faking tracking with padding.
///
/// The dot is here, and not on a line of its own, because that is the density
/// win of M14: `updated Ns ago` cost a full text row per provider to say what a
/// coloured dot says at a glance, and the exact seconds — which nobody reads
/// every second — moved to hover. `freshness` is `None` before the first poll
/// lands, and the header then renders exactly as it always did.
///
/// Right-to-left outer layout with a left-to-right inner one: the shipped
/// titlebar idiom, and the way to pin one widget to the right edge while the
/// rest fills from the left.
fn render_provider_header(
    ui: &mut egui::Ui,
    id: ProviderId,
    theme: Theme,
    freshness: Option<Freshness>,
) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(freshness) = freshness {
                render_freshness_dot(ui, freshness);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                if theme == Theme::Plain {
                    // The pre-M7b heading: the provider's name, title-cased.
                    ui.heading(provider_label(id));
                    return;
                }
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(egui::RichText::new("// ").small().color(CARDINAL));
                ui.label(
                    egui::RichText::new(provider_label(id).to_uppercase())
                        .small()
                        .color(TEXT_DIM),
                );
            });
        });
    });
}

/// The freshness dot itself: one small glyph, coloured by age, whose hover
/// carries the age and the wall clock the row no longer spends a line on.
///
/// The same `•` the footer's status dot used since M7b, so the mark a user
/// already reads as "this pane's data" is the mark that survived; only its
/// position and what it says on hover are new. Identical in both themes —
/// [`freshness_color`] is shared, and no theme branch reaches this far.
fn render_freshness_dot(ui: &mut egui::Ui, freshness: Freshness) {
    ui.label(
        egui::RichText::new(FRESHNESS_DOT)
            .small()
            .color(freshness_color(freshness.age)),
    )
    .on_hover_text(freshness_tooltip(freshness));
}

/// The whole scrollable body: every provider's pane, ruled apart.
///
/// Extracted from `App::ui` so the layout harness can measure the *window's*
/// content rather than one pane at a time — which is what
/// [`DEMO_WINDOW_HEIGHT`] is derived from. A test that summed pane heights
/// would silently miss the separators and the spacing between them.
fn render_panes(ui: &mut egui::Ui, panes: &mut [ProviderPane], theme: Theme) {
    for (i, pane) in panes.iter_mut().enumerate() {
        if i > 0 {
            ui.separator();
        }
        render_pane(ui, pane, theme);
    }
}

/// Render one provider's titled section.
fn render_pane(ui: &mut egui::Ui, pane: &mut ProviderPane, theme: Theme) {
    // Measured once, before anything draws: the header's dot and the pace
    // line's stale gate are then answering from the same reading rather than
    // from two `elapsed()` calls a few microseconds apart.
    let age = pane.snapshot_received_at.map(|t| t.elapsed());
    render_provider_header(ui, pane.id, theme, age.map(freshness_at_now));

    if let Some(err) = &pane.startup_error {
        ui.colored_label(CARDINAL, err);
        return;
    }

    // The disclosure toggle only *records* intent while the snapshot is
    // borrowed; the flip happens after, mirroring the titlebar buttons.
    let mut toggled = false;
    if let Some(snapshot) = &pane.latest_snapshot {
        // The strip follows the same window the tray miniature does, so the two
        // never describe different quotas. Selected here rather than stored
        // because it is a filter over a few dozen readings, not the kind of
        // arithmetic (a least-squares fit) that earns being cached off the poll.
        // `history` is empty unless history is on, so this is a no-op by default.
        let spark = representative_window(snapshot)
            .map(|window| spark_series(&pane.history, &window.label))
            .unwrap_or_default();
        toggled = render_windows(
            ui,
            snapshot,
            age,
            pane.expanded,
            theme,
            PaneLines {
                pace: pane.pace_warning.as_ref(),
                spark: &spark,
                alert: pane.alert_line.as_deref(),
                refill: pane.refill_line.as_deref(),
            },
        );
    }
    if toggled {
        pane.expanded = !pane.expanded;
    }

    match classify_failure(pane.latest_failure.as_deref()) {
        FailureDisplay::NoFailure => {
            if pane.latest_snapshot.is_none() {
                ui.label("waiting for first poll…");
            }
        }
        FailureDisplay::NotSignedIn => {
            // Quiet, non-alarming: absent credentials are expected when a user
            // only uses one provider. The other pane is unaffected.
            ui.colored_label(ui.visuals().weak_text_color(), not_signed_in_line(pane.id));
        }
        FailureDisplay::TokenExpired => {
            // The instruction replaces the raw poller message rather than
            // joining it: two lines saying the same thing, one of them in
            // architecture terms, is what made the recovery non-obvious.
            ui.colored_label(CARDINAL, token_expired_line(pane.id));
        }
        FailureDisplay::Banner => {
            if let Some(message) = &pane.latest_failure {
                ui.colored_label(CARDINAL, message);
            }
        }
    }
}

// --------------------------------------------------------------------------
// M15: the agents view.
// --------------------------------------------------------------------------

/// The titlebar's view switcher: `usage // agents`, the current view in [`TEXT`]
/// and the other in [`TEXT_FAINT`]. Returns the view the user clicked, if any.
///
/// Two separately-sensing labels rather than one string, for the reason
/// [`render_prompt`] uses two: egui colours a `RichText` as a unit, and here the
/// two halves also need their own hit targets. The separator between them
/// senses nothing at all, so the gap belongs to the titlebar's drag handle.
///
/// **Click, never drag.** `selectable(false)` is doing real work here, not
/// tidying: a selectable label — egui's default, and what every other label in
/// this window is — ORs `Sense::click_and_drag` into whatever it was given, so
/// a press-and-drag starting on the word would be swallowed as a text
/// selection. Turned off, the labels sense [`egui::Sense::click`] and nothing
/// else, and a drag falls through to the whole-strip handle installed before
/// them and moves the window — the interaction a decoration-less window cannot
/// afford to lose to a tab bar.
fn render_view_switcher(ui: &mut egui::Ui, theme: Theme, current: View) -> Option<View> {
    let mut clicked = None;
    ui.spacing_mut().item_spacing.x = 0.0;

    // Read off the style once, before anything borrows `ui` mutably.
    let (here, elsewhere) = match theme {
        Theme::CipherPine => (TEXT, TEXT_FAINT),
        Theme::Plain => (ui.visuals().text_color(), ui.visuals().weak_text_color()),
    };
    let ink = |view: View| if view == current { here } else { elsewhere };

    for (view, label, leading) in [
        (View::Usage, VIEW_LABEL_USAGE, true),
        (View::Agents, VIEW_LABEL_AGENTS, false),
    ] {
        if leading {
            // One space of breathing room after the prompt's cursor, so the
            // switcher is not read as part of the command line.
            ui.label(egui::RichText::new(" ").small());
        } else {
            ui.label(egui::RichText::new(VIEW_SEPARATOR).small().color(HAIRLINE));
        }
        let response = ui.add(
            egui::Label::new(egui::RichText::new(label).small().color(ink(view)))
                .selectable(false)
                .sense(egui::Sense::click()),
        );
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if response.clicked() {
            clicked = Some(view);
        }
    }
    clicked
}

/// The dot beside an agent row, in the M14 freshness palette's own terms.
///
/// Deliberately rhymed rather than merely reused: green means "this is live",
/// amber "this has gone quiet", faint "this is history" in both views, so a
/// reader who has learned the usage pane's dot has already learned this one.
/// Shared by both themes for the reason [`freshness_color`] is — it reports a
/// fact, not a look.
fn agent_state_color(state: AgentState) -> egui::Color32 {
    match state {
        AgentState::Working => OPER_GREEN,
        AgentState::Idle => AMBER,
        AgentState::Recent => TEXT_FAINT,
    }
}

/// A row's identity: `project · branch · id8`, with any part the scan could not
/// name simply absent rather than left as an empty gap.
fn agent_identity(session: &AgentSession) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(3);
    if !session.project.is_empty() {
        parts.push(&session.project);
    }
    if let Some(branch) = &session.branch {
        parts.push(branch);
    }
    if !session.short_id.is_empty() {
        parts.push(&session.short_id);
    }
    parts.join(AGENT_ROW_SEPARATOR)
}

/// One session: state dot, identity, and a right-aligned age.
///
/// The age is placed first under a right-to-left layout — the titlebar and
/// provider-header idiom — so the identity gets whatever width is left and
/// elides into it. Elision matters here in a way it does not elsewhere in this
/// window: a branch name is user-supplied text of no bounded length, and the
/// alternative to eliding it is a row that overflows a 320px pane.
fn render_agent_row(ui: &mut egui::Ui, session: &AgentSession, theme: Theme) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let age_ink = match theme {
                Theme::CipherPine => TEXT_DIM,
                Theme::Plain => ui.visuals().weak_text_color(),
            };
            ui.label(
                egui::RichText::new(format_age(session.age.as_secs()))
                    .small()
                    .color(age_ink),
            );
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new(FRESHNESS_DOT)
                        .small()
                        .color(agent_state_color(session.state)),
                );
                if session.is_subagent {
                    ui.label(
                        egui::RichText::new(SUBAGENT_PREFIX)
                            .small()
                            .color(TEXT_FAINT),
                    );
                }
                let identity_ink = match theme {
                    Theme::CipherPine => TEXT,
                    Theme::Plain => ui.visuals().text_color(),
                };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(agent_identity(session))
                            .small()
                            .color(identity_ink),
                    )
                    .truncate(),
                );
            });
        });
    });
}

/// The whole agents pane: each provider's sessions under the header style the
/// usage pane already uses, or one quiet line when nothing has run today.
///
/// A provider with no sessions gets no header at all. Two headers over two
/// empty lists would spend a third of a 240px window saying nothing twice.
fn render_agents(ui: &mut egui::Ui, sessions: &[AgentSession], theme: Theme) {
    if sessions.is_empty() {
        let ink = match theme {
            Theme::CipherPine => TEXT_FAINT,
            Theme::Plain => ui.visuals().weak_text_color(),
        };
        ui.label(egui::RichText::new(NO_AGENTS_LINE).small().color(ink));
        return;
    }

    let mut drawn = 0;
    for id in [
        ProviderId::ClaudeSubscription,
        ProviderId::CodexSubscription,
    ] {
        let rows: Vec<&AgentSession> = sessions.iter().filter(|s| s.provider == id).collect();
        if rows.is_empty() {
            continue;
        }
        if drawn > 0 {
            ui.separator();
        }
        drawn += 1;
        // No freshness dot: that dot reports a *poll's* age, and this view
        // polls nothing. Each row carries its own age instead.
        render_provider_header(ui, id, theme, None);
        for session in rows {
            render_agent_row(ui, session, theme);
        }
    }
}

/// The lines a pane hands [`render_windows`] beyond the snapshot itself:
/// everything computed on a poll event and merely drawn on a frame.
///
/// A struct rather than four more parameters — the renderer already takes the
/// snapshot, the age, the disclosure state and the theme, and a sixth, seventh
/// and eighth positional `Option` would be a call site nobody could read.
#[derive(Default)]
struct PaneLines<'a> {
    /// The at-risk forecast (M8), or `None` for "nothing to say".
    pace: Option<&'a PaceWarning>,
    /// The 24 h history series behind the sparkline (M13); empty draws nothing.
    spark: &'a [(u64, f64)],
    /// The alert banner for the worst window in alert (M13).
    alert: Option<&'a str>,
    /// The one-shot refill notice (M13).
    refill: Option<&'a str>,
}

/// Render a snapshot's quota bars, the per-model disclosure, and its
/// freshness/staleness line.
///
/// `expanded` is the pane's current disclosure state. Returns `true` when the
/// user clicked the toggle this frame; the caller owns the flip.
///
/// `pace` is the pane's at-risk line, already selected and already computed off
/// a poll event ([`ProviderPane::ingest_pace`]) — this function only draws it,
/// and only while the snapshot is fresh enough to extrapolate from
/// ([`show_pace_warning`]).
fn render_windows(
    ui: &mut egui::Ui,
    snapshot: &ProviderSnapshot,
    age: Option<Duration>,
    expanded: bool,
    theme: Theme,
    lines: PaneLines,
) -> bool {
    for window in &snapshot.windows {
        render_window_row(ui, window, theme);
    }

    // The 24 h history strip, directly under the bars it is about. Draws only
    // when history is on and there are at least two readings in the day;
    // otherwise it takes no space at all (M13).
    render_sparkline(ui, lines.spark);

    // The at-risk forecast, directly under the bars it is about — one line at
    // most, and absent entirely when nothing is at risk. Calm is silent: a pane
    // that is fine looks exactly as it did before this milestone. Stale is
    // silent too (see `show_pace_warning`): the header's dot already says the
    // data is dead, and a projection drawn off dead data would contradict it.
    if let Some(warning) = lines.pace.filter(|_| show_pace_warning(age)) {
        ui.label(
            egui::RichText::new(pace_warning_line(warning))
                .small()
                .color(pace_warning_color(warning.exhaust_in_secs)),
        );
    }

    // Reset credits, between the headline rows and the per-model disclosure.
    // Absent entirely when the provider has no such concept, so the Claude
    // pane renders exactly as it did before this line existed.
    if let Some(credits) = snapshot.reset_credits {
        ui.label(
            egui::RichText::new(reset_credits_line(&credits))
                .small()
                .color(dim_color(ui, theme)),
        );
    }

    // Per-model disclosure, under the headline rows.
    // Suppressed entirely when there is nothing to disclose — no affordance
    // that opens onto an empty list. "Nothing to disclose" counts *visible*
    // rows, so a provider whose per-model buckets are all untouched shows no
    // toggle at all rather than one that opens onto blank space.
    let mut toggled = false;
    let visible: Vec<&QuotaWindow> = snapshot
        .per_model
        .iter()
        .filter(|w| per_model_row_is_visible(w))
        .collect();
    if !visible.is_empty() {
        toggled = disclosure_toggle(ui, expanded, PER_MODEL_CAPTION).clicked();
        if expanded {
            // Salted with the provider so the two panes' indent regions get
            // distinct ids.
            ui.indent(("per_model", snapshot.provider), |ui| {
                for window in visible {
                    render_per_model_row(ui, window, theme);
                }
            });
        }
    }

    // The alert surfaces, the last rows in the pane (M13). Both
    // are absent by default — alerts are opt-in, and a pane with nothing to
    // alert about renders exactly as it did before. The banner is CARDINAL
    // because it is the same class of thing as the stale line; the refill
    // notice is TEXT_DIM because good news does not get to shout.
    if let Some(line) = lines.alert {
        ui.label(egui::RichText::new(line).small().color(CARDINAL));
    }
    if let Some(line) = lines.refill {
        ui.label(
            egui::RichText::new(line)
                .small()
                .color(dim_color(ui, theme)),
        );
    }

    toggled
}

/// The reset-credits line, e.g. `resets available: 1`.
///
/// Shows the owned count only. `applicable_now` is `0` except while the
/// account is actually rate-limited, so surfacing it here would park a
/// permanent, meaningless "0" in a 320px window; it stays in the snapshot for
/// `--json` consumers, who can tell the two counts apart.
fn reset_credits_line(credits: &ResetCredits) -> String {
    format!("resets available: {}", credits.available)
}

/// Whether a per-model row earns its space in the window.
///
/// Hidden when usage is `0.0` or unknown. Both providers enumerate every model
/// bucket on the plan, so a subscriber who has never touched one still gets a
/// row for it — two lines of "0% · resets in 7d" in a window whose width is
/// fixed at 320px. Anything actually used, however little, still shows.
///
/// Display-only, and deliberately not pushed down into `usage-core`: the
/// snapshot stays the full truth for `--json` and any future consumer. A
/// `usage-cli` test pins that zero-usage buckets remain in the JSON, so this
/// filter cannot quietly grow into the data.
fn per_model_row_is_visible(window: &QuotaWindow) -> bool {
    window.used_fraction.is_some_and(|fraction| fraction > 0.0)
}

/// One provider's pace warning: which of its headline windows is projected to
/// fill before it resets, and in how long.
///
/// Exactly one per provider, or none. Two lines competing for attention in a
/// 320px pane would make the reader compare them instead of act, so the sooner
/// one speaks and the other stays quiet (see [`select_pace_warning`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PaceWarning {
    /// The headline window's label, verbatim from the snapshot.
    label: String,
    /// Seconds until that window is projected to reach 100%.
    exhaust_in_secs: u64,
}

/// Pick the at-risk headline window that runs out soonest.
///
/// Pure, over `(label, burn, resets_in_secs)` triples the caller has already
/// estimated — so the choice is testable without a poller, a window, or a clock.
/// A window is a candidate only if [`pace::at_risk`] holds for it: burning, and
/// projected to fill *before* its own reset. Everything else is silence, which
/// is the point — a calm pane says nothing rather than reassuring the user in a
/// line they then have to read on every glance.
fn select_pace_warning(candidates: &[(String, Burn, Option<u64>)]) -> Option<PaceWarning> {
    candidates
        .iter()
        .filter(|(_, burn, resets_in)| pace::at_risk(burn, *resets_in))
        // `at_risk` is only true when `exhaust_in_secs` is `Some`, so this drops
        // nothing that survived the filter — it unwraps the fact.
        .filter_map(|(label, burn, _)| burn.exhaust_in_secs.map(|secs| (label, secs)))
        .min_by_key(|(_, exhaust_in_secs)| *exhaust_in_secs)
        .map(|(label, exhaust_in_secs)| PaceWarning {
            label: label.clone(),
            exhaust_in_secs,
        })
}

/// The at-risk line's text, e.g. `at this pace: 7d full in ~9h 0m`.
///
/// "at this pace" and the `~` are load-bearing: this is an extrapolation from
/// the last couple of hours, not a countdown, and the wording must not let it be
/// mistaken for one. The duration goes through [`format_reset`], so it reads in
/// the same units as the reset countdown beside it.
fn pace_warning_line(warning: &PaceWarning) -> String {
    format!(
        "at this pace: {} full in ~{}",
        warning.label,
        format_reset(Some(warning.exhaust_in_secs))
    )
}

/// The at-risk line's colour: AMBER, or CARDINAL inside
/// [`PACE_CARDINAL_UNDER_SECS`].
///
/// Takes no [`Theme`], like [`pace_tick_color`] and for the same reason: how
/// urgent a forecast is does not depend on which look is installed. Both colours
/// already appear in both themes (AMBER is Plain's stale marker).
fn pace_warning_color(exhaust_in_secs: u64) -> egui::Color32 {
    if exhaust_in_secs < PACE_CARDINAL_UNDER_SECS {
        CARDINAL
    } else {
        AMBER
    }
}

/// The `(observed_at, used_fraction)` series the sparkline draws: this pane's
/// history for one window label, oldest first.
///
/// Pure over the trail the pane already holds, so the strip cannot show a
/// window the bars above it are not about.
fn spark_series(history: &[HistoryEntry], label: &str) -> Vec<(u64, f64)> {
    let mut series: Vec<(u64, f64)> = history
        .iter()
        .filter(|entry| entry.label == label)
        .map(|entry| (entry.at_unix_secs, entry.used_fraction))
        .collect();
    // A polyline over unsorted points draws a zigzag, and nothing upstream
    // promises order: the trail is a replayed file followed by live appends.
    series.sort_by_key(|(at, _)| *at);
    series
}

/// The readings inside the strip's window: everything from the last
/// [`HISTORY_WINDOW_SECS`] up to `now_unix_secs`, oldest first.
///
/// `now` is the newest reading rather than the wall clock, so the line always
/// ends at the strip's right edge and two runs of the same data draw the same
/// picture. Anything older than the window, and anything stamped after `now`,
/// is clipped out rather than squeezed into an edge pixel where it would read
/// as a spike that never happened.
fn spark_window(series: &[(u64, f64)], now_unix_secs: u64) -> Vec<(u64, f64)> {
    series
        .iter()
        .copied()
        .filter(|(at, _)| *at <= now_unix_secs && now_unix_secs - at <= HISTORY_WINDOW_SECS)
        .collect()
}

/// Map the in-window readings onto strip coordinates inside `rect`.
///
/// Time runs left (a day ago) to right (now); a full window is the top edge and
/// an empty one the bottom, matching the bars above, which fill rightward from
/// an empty trough. The fraction is clamped to `0..=1` — a provider that
/// transiently reports over 100% must not draw a line above the strip.
///
/// Pure, and the whole of the strip's geometry: everything the painter does
/// with the result is `Shape::line`.
fn spark_points(series: &[(u64, f64)], now_unix_secs: u64, rect: egui::Rect) -> Vec<egui::Pos2> {
    spark_window(series, now_unix_secs)
        .into_iter()
        .map(|(at, used)| {
            let age = (now_unix_secs - at) as f32 / HISTORY_WINDOW_SECS as f32;
            let x = rect.right() - rect.width() * age;
            let y = rect.bottom() - rect.height() * used.clamp(0.0, 1.0) as f32;
            egui::pos2(x, y)
        })
        .collect()
}

/// The sparkline's stroke colour: TEXT_DIM at full alpha, in both themes.
///
/// M13 drew this at alpha 140 so the strip would sit *behind* the pace tick.
/// The owner's round-1 verdict retired that ordering: a mark nobody can
/// identify is not deferring to anything, it is just faint. The hierarchy is
/// now carried by hue — TEXT_DIM for the day, TEXT for the [`SPARK_NOW_RADIUS`]
/// dot on the present — rather than by transparency.
///
/// Takes no [`Theme`], for the same reason [`pace_tick_color`] does not: a
/// history strip is information, and how a day of usage looks does not depend
/// on which look is installed.
fn spark_color() -> egui::Color32 {
    TEXT_DIM
}

/// The fill under the sparkline curve: [`spark_color`] at [`SPARK_FILL_ALPHA`].
fn spark_fill_color() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(
        TEXT_DIM.r(),
        TEXT_DIM.g(),
        TEXT_DIM.b(),
        SPARK_FILL_ALPHA,
    )
}

/// The area between the curve and `baseline`, as a triangle mesh (M13-R1).
///
/// A mesh rather than a filled path because the region under a usage curve is
/// concave in general, and egui's polygon fill is a *convex* one — a concave
/// series would fill across its own bays. Two triangles per segment, each
/// spanning one pair of adjacent readings down to the baseline, is exactly the
/// region under the polyline for any series, convex or not.
///
/// Sharing the vertices between adjacent segments matters as much as the
/// triangles do: a per-segment quad would double-blend its seams, and at alpha
/// 18 that would draw a faint vertical stripe at every reading — the "wobble
/// never lines up into a visible stripe" rule the demo history already follows.
///
/// Pure, like [`spark_points`]: everything the painter does with the result is
/// `Shape::mesh`.
fn spark_fill_mesh(points: &[egui::Pos2], baseline: f32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    if points.len() < 2 {
        return mesh;
    }
    let color = spark_fill_color();
    for point in points {
        mesh.colored_vertex(*point, color);
        mesh.colored_vertex(egui::pos2(point.x, baseline), color);
    }
    // Vertices come in (curve, baseline) pairs, so segment `i` owns 2i..2i+4.
    for segment in 0..points.len() as u32 - 1 {
        let (top, bottom) = (2 * segment, 2 * segment + 1);
        let (next_top, next_bottom) = (top + 2, bottom + 2);
        mesh.add_triangle(top, bottom, next_bottom);
        mesh.add_triangle(top, next_bottom, next_top);
    }
    mesh
}

/// Draw the 24 h history strip, or nothing at all.
///
/// Nothing is the common case and the important one: with `history=off`, or
/// with fewer than two readings in the window, this returns **before**
/// allocating, so a pane without history has exactly the layout it had before
/// M13 — no strip, no gap, no reserved space. Calm is silent, as with the
/// at-risk line.
///
/// One reading is not a line, and drawing a single dot would assert a trend
/// from one number.
///
/// M13-R1 gives the strip the three things that make it read as an instrument
/// rather than a scribble: a filled body under a full-strength line, a bright
/// dot on the newest reading, and a [`SPARK_TAG`] naming the span it covers.
/// The tag and the strip are one unit with no row spacing between them, so the
/// label belongs to the line rather than to the bars above it.
fn render_sparkline(ui: &mut egui::Ui, series: &[(u64, f64)]) {
    let Some(now) = series.iter().map(|(at, _)| *at).max() else {
        return;
    };
    if spark_window(series, now).len() < 2 {
        return;
    }
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;

        // `24h`, right-aligned over the strip's right edge — the end the line
        // runs *to*, and the end the eye starts from.
        let font = egui::TextStyle::Small.resolve(ui.style());
        let (tag_rect, _) = ui.allocate_exact_size(
            egui::vec2(BAR_WIDTH, ui.text_style_height(&egui::TextStyle::Small)),
            egui::Sense::hover(),
        );
        ui.painter().text(
            tag_rect.right_top(),
            egui::Align2::RIGHT_TOP,
            SPARK_TAG,
            font,
            TEXT_FAINT,
        );

        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(BAR_WIDTH, SPARK_HEIGHT), egui::Sense::hover());
        let points = spark_points(series, now, rect);
        // The newest reading is the last point: `spark_window` keeps the series
        // oldest-first and `now` is its maximum.
        let newest = points.last().copied();

        // Body first, then the line over its own top edge, then the anchor —
        // painted back to front so nothing is half-covered by what follows.
        ui.painter()
            .add(egui::Shape::mesh(spark_fill_mesh(&points, rect.bottom())));
        ui.painter().add(egui::Shape::line(
            points,
            egui::Stroke::new(SPARK_STROKE_WIDTH, spark_color()),
        ));
        if let Some(newest) = newest {
            ui.painter().circle_filled(newest, SPARK_NOW_RADIUS, TEXT);
        }
    });
}

/// The elapsed-time pace tick's colour: TEXT_DIM at [`PACE_TICK_ALPHA`].
///
/// Takes no [`Theme`] on purpose, and is the one mark in the window that does
/// not: the tick is **information**, not styling. Where a bar's fill sits
/// relative to how much of the window has elapsed is the same fact in both
/// looks, so it is drawn the same way in both. Themed marks branch on the
/// theme; facts do not.
///
/// A function rather than a `const` because the premultiplied conversion
/// `from_rgba_unmultiplied` performs is not available in a const context.
fn pace_tick_color() -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(TEXT_DIM.r(), TEXT_DIM.g(), TEXT_DIM.b(), PACE_TICK_ALPHA)
}

// --------------------------------------------------------------------------
// Quota alerts (M13). Dep-free by owner decision: no notification crate, no
// Shell_NotifyIcon plumbing. The three surfaces are ones the app already owns —
// a line in its own window, its own tray icon and tooltip, and eframe's
// built-in taskbar attention request.
//
// The decision itself is one pure function over four facts, so every rule
// below is testable without a window, a tray, or a clock.
// --------------------------------------------------------------------------

/// What one window's numbers ask the app to do this poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertAction {
    /// Nothing to say — the overwhelming majority of polls.
    Quiet,
    /// A fresh crossing: raise the alert on every surface.
    Fire,
    /// A window that had fired has fallen back under the threshold.
    Refill,
}

/// Whether this window should raise, clear, or say nothing — the whole alert
/// policy, in one place.
///
/// `already_firing` is the debounce: an alert fires **once per crossing**, not
/// once per poll, so a window sitting at 92% for six hours interrupts a user
/// exactly once. It re-arms only by falling back under the threshold, which is
/// what a reset or a refill looks like from here.
///
/// In [`AlertMode::Pace`] — the default — crossing the threshold is not enough
/// on its own: the window must also be spent *ahead of its own clock*, because
/// a weekly quota 85% used six days in is on budget and saying otherwise is
/// noise. A window whose provider never stated a duration has no clock to
/// compare against, and falls back to threshold semantics: an alert that might
/// not have been needed beats silence about a quota that is nearly gone.
///
/// An unknown used fraction says nothing at all. There is no reading to judge,
/// and inventing one would be the one way this function could lie.
fn alert_action(
    config: &Config,
    used_fraction: Option<f64>,
    elapsed_fraction: Option<f64>,
    already_firing: bool,
) -> AlertAction {
    if !config.alerts {
        return AlertAction::Quiet;
    }
    // A non-finite reading is no more a reading than an absent one: it cannot
    // be compared with a threshold, and letting it fall through would make the
    // answer depend on which side of a NaN comparison happened to be written.
    let Some(used) = used_fraction.filter(|f| f.is_finite()) else {
        return AlertAction::Quiet;
    };
    if used * 100.0 < config.alert_at as f64 {
        return if already_firing {
            AlertAction::Refill
        } else {
            AlertAction::Quiet
        };
    }
    if already_firing {
        return AlertAction::Quiet;
    }
    match config.alert_mode {
        AlertMode::Threshold => AlertAction::Fire,
        AlertMode::Pace => match elapsed_fraction {
            Some(elapsed) if used <= elapsed => AlertAction::Quiet,
            _ => AlertAction::Fire,
        },
    }
}

/// The in-window banner line, byte-exact:
/// `alert: Codex 5h at 80% >= 80% (pace)`.
///
/// Names the provider even though it renders inside that provider's own pane:
/// the same words go in a bug report or a message to someone else, where the
/// pane it sat in is not part of the quote. The mode is the word from
/// `config.cfg`, so the line names the setting a reader would edit.
fn alert_banner_line(
    provider: ProviderId,
    label: &str,
    used_fraction: f64,
    alert_at: u8,
    alert_mode: AlertMode,
) -> String {
    format!(
        "alert: {} {label} at {} >= {alert_at}% ({})",
        provider_label(provider),
        format_percent(Some(used_fraction)),
        alert_mode.as_word(),
    )
}

/// The refill line, byte-exact: `refilled: Codex 5h back under 80%`.
///
/// Deliberately quiet — TEXT_DIM, and no attention request. Getting a quota
/// back is good news, and good news that steals focus is a bug.
fn alert_refill_line(provider: ProviderId, label: &str, alert_at: u8) -> String {
    format!(
        "refilled: {} {label} back under {alert_at}%",
        provider_label(provider)
    )
}

/// How far through its window the account is: `1 - resets_in / duration`,
/// clamped to `0..=1`. `None` when the provider did not state both, or stated a
/// zero-length window (the one value that would divide).
///
/// One definition, shared by the pace tick and the alert decision, so what the
/// tick draws and what an alert compares against cannot drift apart.
fn elapsed_fraction(window: &QuotaWindow) -> Option<f64> {
    let duration = window.duration_secs?;
    let resets_in = window.resets_in_secs?;
    if duration == 0 {
        return None;
    }
    // Clamped because a countdown longer than the window (clock skew, a
    // provider's own rounding) must not push the elapsed fraction negative.
    Some(1.0 - (resets_in as f64 / duration as f64).clamp(0.0, 1.0))
}

/// Where the elapsed-time tick belongs inside a bar `bar_width` px wide: the
/// offset, in px, from the bar's left edge. `None` when the bar earns no tick.
///
/// `elapsed_fraction = 1 - resets_in / duration`, clamped to `0..=1`, times the
/// width. Reading the result: fill to the **left** of the tick means the quota
/// is being spent slower than the window is passing; fill **past** it means
/// faster.
///
/// All three inputs are required, and each absence is a real case rather than a
/// defensive check:
/// - no `used_fraction` — there is no fill to compare the tick against, so the
///   mark would be a line on an empty trough saying nothing;
/// - no `duration_secs` — the provider never said how long this window is (an
///   unverified Claude limit kind, a Codex row without `limit_window_seconds`),
///   so "how far through it" has no answer;
/// - no `resets_in_secs` — same, from the other end.
///
/// A zero duration is `None` too: it is the one value that would divide, and a
/// window of no length has no elapsed fraction to show. Guarding here rather
/// than laundering it in the provider keeps the snapshot honest about what the
/// endpoint said (see `codex_subscription::to_quota_window`).
///
/// The clamped ratio itself comes from [`elapsed_fraction`], shared with the
/// alert decision (M13) so the mark on the bar and the rule behind an alert are
/// the same arithmetic rather than two copies of it.
fn pace_tick_x(window: &QuotaWindow, bar_width: f32) -> Option<f32> {
    window.used_fraction?;
    Some(bar_width * elapsed_fraction(window)? as f32)
}

/// Add a quota bar, outline its trough with a HAIRLINE border, and mark how far
/// through the window the account is (M8).
///
/// `ProgressBar` paints its trough from `visuals.extreme_bg_color` (PANEL) but
/// exposes no stroke, so the border is painted over the returned rect. One
/// helper for both row renderers, so the headline and per-model bars cannot
/// drift apart — and so every bar in the window gets the pace tick from one
/// place, headline and per-model alike.
///
/// The tick is **painted**, not allocated: it consumes no layout space, so no
/// row changes height or width by having one. Pinned by
/// `the_pace_tick_changes_no_row_geometry`.
fn add_quota_bar(ui: &mut egui::Ui, window: &QuotaWindow) {
    let fraction = window.used_fraction;
    let response = ui.add(
        egui::ProgressBar::new(fraction.unwrap_or(0.0) as f32)
            .desired_width(BAR_WIDTH)
            .corner_radius(BAR_ROUNDING)
            .fill(fraction_color(fraction))
            .text(format_percent(fraction)),
    );
    ui.painter().rect_stroke(
        response.rect,
        BAR_ROUNDING,
        egui::Stroke::new(1.0, HAIRLINE),
        egui::StrokeKind::Inside,
    );

    // Measured off the rect the bar actually got, not off BAR_WIDTH, so the
    // tick stays aligned if a row ever gives the bar less room than it asked
    // for. Painted last so it sits over both the fill and the border.
    if let Some(offset) = pace_tick_x(window, response.rect.width()) {
        ui.painter().vline(
            response.rect.left() + offset,
            response.rect.y_range(),
            egui::Stroke::new(1.0, pace_tick_color()),
        );
    }
}

fn render_window_row(ui: &mut egui::Ui, window: &QuotaWindow, theme: Theme) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&window.label).color(text_color(ui, theme)));
        // The numeric percent rides on the bar itself; `--` when unknown (an
        // unknown fraction also draws an empty gray bar).
        add_quota_bar(ui, window);
        // Compact reset countdown, e.g. "resets in 3h 12m"; `--` when unknown.
        ui.label(
            egui::RichText::new(format!("resets in {}", format_reset(window.resets_in_secs)))
                .color(dim_color(ui, theme)),
        );
    });
}

/// Render one per-model row as **two lines**: the model's label, then the bar
/// and reset countdown inset beneath it.
///
/// Separate from [`render_window_row`] on purpose, and that function is left
/// untouched: the headline rows are visually accepted and must keep rendering
/// exactly as they do. Their single-line layout works because a headline label
/// is two to seven characters (`5h`, `7d`); a provider's model name on that
/// same line (`GPT-5.3-Codex-Spark`, indented under the disclosure) wanted
/// roughly 355px inside a fixed 320px window and pushed the reset countdown
/// off the right edge.
///
/// Stacking is **length-independent**: it holds for any model name, however
/// long. Widening the label column instead would only move the cliff, and
/// model names trend longer over time.
///
/// The label is small + dim so the two lines read as one subordinate row
/// rather than as two unrelated ones. The bar goes through the shared
/// [`add_quota_bar`], so a per-model gauge stays comparable at a glance with a
/// headline gauge — width, rounding, border and fill mapping all in step.
fn render_per_model_row(ui: &mut egui::Ui, window: &QuotaWindow, theme: Theme) {
    let dim = dim_color(ui, theme);
    ui.label(
        egui::RichText::new(window.label.as_str())
            .small()
            .color(dim),
    );
    ui.horizontal(|ui| {
        ui.add_space(PER_MODEL_ROW_INDENT);
        add_quota_bar(ui, window);
        ui.label(
            egui::RichText::new(format!("resets in {}", format_reset(window.resets_in_secs)))
                .small()
                .color(dim),
        );
    });
}

/// Paint the disclosure triangle into `rect`: pointing right (▸) when
/// collapsed, down (▾) when expanded.
///
/// Painted rather than set as a `▸`/`▾` text glyph for the same reason the
/// titlebar's ✕ and – are: it cannot depend on font coverage, so there is no
/// tofu risk on any platform.
fn draw_disclosure_triangle(
    painter: &egui::Painter,
    rect: egui::Rect,
    expanded: bool,
    color: egui::Color32,
) {
    let points = if expanded {
        vec![rect.left_top(), rect.right_top(), rect.center_bottom()]
    } else {
        vec![rect.left_top(), rect.right_center(), rect.left_bottom()]
    };
    painter.add(egui::Shape::convex_polygon(
        points,
        color,
        egui::Stroke::NONE,
    ));
}

/// The per-model disclosure control: triangle plus caption, as one clickable
/// row. Both halves are clickable so the target is not a 9px triangle.
fn disclosure_toggle(ui: &mut egui::Ui, expanded: bool, caption: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let (rect, triangle) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::click());
        // Copy the color out so the &Style borrow ends before painting.
        let color = ui.style().interact(&triangle).fg_stroke.color;
        // Nudge down to sit on the caption's baseline rather than its cap line.
        draw_disclosure_triangle(
            ui.painter(),
            rect.translate(egui::vec2(0.0, 1.0)),
            expanded,
            color,
        );

        let weak = ui.visuals().weak_text_color();
        let label = ui.add(
            egui::Label::new(egui::RichText::new(caption).small().color(weak))
                .sense(egui::Sense::click()),
        );
        (triangle | label).on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// Paint a close "✕" into `rect` with `stroke`. Painted (not a font glyph),
/// mirroring egui's own window close button, so it never depends on font
/// coverage — no tofu risk.
fn draw_close_glyph(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    painter.line_segment([rect.left_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.right_top(), rect.left_bottom()], stroke);
}

/// Paint a minimize "–" (one horizontal bar) into `rect` with `stroke`.
fn draw_minimize_glyph(painter: &egui::Painter, rect: egui::Rect, stroke: egui::Stroke) {
    let y = rect.center().y;
    painter.line_segment(
        [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
        stroke,
    );
}

/// A small square titlebar control filling the strip's height. Paints `draw`
/// with the interaction's foreground stroke over a subtle hover background, and
/// returns the click response.
fn titlebar_button(
    ui: &mut egui::Ui,
    draw: impl FnOnce(&egui::Painter, egui::Rect, egui::Stroke),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(TITLEBAR_HEIGHT, TITLEBAR_HEIGHT),
        egui::Sense::click(),
    );
    // Copy the (Copy) visual fields out so the &Style borrow ends before we paint.
    let (stroke, hover_fill) = {
        let visuals = ui.style().interact(&response);
        (visuals.fg_stroke, visuals.weak_bg_fill)
    };
    if response.hovered() {
        ui.painter().rect_filled(rect, 2.0, hover_fill);
    }
    // Inset the glyph so it reads as a small icon, not edge-to-edge.
    let glyph = rect.shrink(rect.height() * 0.33);
    draw(ui.painter(), glyph, stroke);
    response
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: quotapane [--client-version <VER>] [--codex-user-agent <UA>] [--no-tray]\n\
                 \x20                [--plain | --themed] [--pace-demo] [--agents-demo]"
            );
            return ExitCode::from(2);
        }
    };

    // The throttle note is advice about polling, and demo mode does not poll —
    // it would be advice about a request that is never made.
    if args.client_version_defaulted && !args.pace_demo {
        eprintln!(
            "note: no --client-version given; using \"{DEFAULT_CLIENT_VERSION}\" — pass a real claude-code version to avoid provider throttling"
        );
    }
    if args.pace_demo {
        eprintln!(
            "note: --pace-demo — the window shows a fixed synthetic scenario. No polling, no credentials read, no network requests."
        );
    }
    if args.agents_demo {
        eprintln!(
            "note: --agents-demo — the agents view shows a fixed synthetic session list. No session log is read."
        );
    }

    // `--no-tray` disables the tray; on platforms without one (Linux) the flag
    // is accepted and ignored. Reading it via `cfg!` keeps the field live on
    // every platform and yields today's behavior wherever there is no tray.
    let tray_active = !args.no_tray && cfg!(any(target_os = "windows", target_os = "macos"));
    let theme_override = args.theme_override;
    let pace_demo = args.pace_demo;
    let agents_demo = args.agents_demo;

    // The demo opens *on* the agents view — a fixture nobody is looking at
    // reviews nothing — while every ordinary launch opens on usage.
    let view = if agents_demo {
        View::Agents
    } else {
        View::default()
    };
    // A fixture built once, at launch; and under the flag the roots are
    // nowhere, so no code path can reach a real session log even if the scan
    // gate were wrong.
    let (agent_roots, agents) = if agents_demo {
        (SessionRoots::default(), demo_agents(SystemTime::now()))
    } else {
        (SessionRoots::from_env(), Vec::new())
    };

    // One pane per provider. A missing home directory becomes a per-pane
    // startup error; a missing credential file surfaces later as a quiet
    // "not signed in" line — neither aborts the window or the other provider.
    //
    // Under `--pace-demo` the providers are never constructed at all. This
    // branch is what makes "no network" structural rather than promised:
    // `ProviderPane::new` is where a poller thread would be spawned and where an
    // `Egress` would be handed to it.
    // Preferences are read here, before the window, because `history=on` and
    // the alert settings both have to be answered before the first poll can be
    // folded in.
    let mut config = config::load();
    // The look the file names, before any flag touches it — what a save
    // triggered by something other than the tray toggle must write back.
    let disk_theme = config.theme;
    // A run-only flag beats the saved preference; every other preference comes
    // from the file only, since there is no flag for them.
    if let Some(theme) = theme_override {
        config.theme = theme;
    }
    if pace_demo {
        config = demo_config(config);
    }

    let mut panes = if pace_demo {
        demo_panes(&config)
    } else {
        vec![
            ProviderPane::new(
                ProviderId::ClaudeSubscription,
                ClaudeSubscription::with_default_path(args.client_version),
            ),
            ProviderPane::new(
                ProviderId::CodexSubscription,
                CodexSubscription::with_default_path(args.codex_user_agent),
            ),
        ]
    };

    // Demo panes carry their own fabricated trail and must never touch the real
    // log, so this block is skipped under `--pace-demo`.
    if config.history && !pace_demo {
        if let Some(path) = config::config_dir().map(|dir| dir.join(history::FILE_NAME)) {
            // Retention first, so a log that outgrew the cap is trimmed before
            // it is read — the one moment the whole file is in memory.
            let _ = history::apply_retention(&path);
            let entries = history::read(&path);
            let now = now_unix_secs();
            for pane in &mut panes {
                pane.enable_history(path.clone(), &entries, now);
            }
        }
    }

    // The window/taskbar icon is the neutral mark — no usage yet at startup,
    // and a taskbar entry is not the place for a live gauge. The tray is.
    let window_icon = egui::IconData {
        rgba: icon::render_icon(None, None, ICON_SIZE),
        width: ICON_SIZE,
        height: ICON_SIZE,
    };

    // Resizable in one axis only (M14). The min and max inner sizes carry the
    // *same* width, which is what pins it: winit enforces both, so the
    // OS-provided edge-drag a borderless resizable window gets on Windows can
    // only ever move the bottom edge. The window has no left/right handle to
    // fight over, and this file never computes a width but [`WINDOW_WIDTH`].
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, initial_inner_height(pace_demo, config.height)])
            .with_decorations(false)
            .with_resizable(true)
            .with_min_inner_size([WINDOW_WIDTH, MIN_WINDOW_HEIGHT])
            .with_max_inner_size([WINDOW_WIDTH, MAX_WINDOW_HEIGHT])
            .with_icon(window_icon)
            .with_always_on_top(),
        ..Default::default()
    };

    let result = eframe::run_native(
        window_title(pace_demo || agents_demo),
        native_options,
        Box::new(move |_cc| {
            // `config` was resolved above — the window cannot be the one to read
            // it, because `history=on` had to be answered before the panes were
            // built. `theme_overridden` records that a flag, not the file, chose
            // the look, so a tray toggle during this run does not rewrite it.
            let theme_overridden = theme_override.is_some();
            let theme = config.theme;

            // Installed before the first frame. The test layout harness
            // installs the same theme, so its width assertions measure the
            // type this window actually renders.
            install_theme(&_cc.egui_ctx, theme);

            // Create the tray on the main thread, now that eframe/winit is up.
            // If creation fails, fall back to close-to-quit.
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            let (tray, tray_active) = if tray_active {
                let tray = tray::Tray::create(&_cc.egui_ctx, theme);
                let active = tray.is_some();
                (tray, active)
            } else {
                (None, false)
            };

            let app = QuotaPaneApp {
                panes,
                tray_active,
                quitting: false,
                config,
                theme_overridden,
                disk_theme,
                pace_demo,
                view,
                agents_demo,
                agent_roots,
                agents,
                last_agent_scan: None,
                pending_height: None,
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                tray,
            };
            Ok(Box::new(app))
        }),
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The demo panes exactly as `--pace-demo` builds them: default
    /// preferences put through `demo_config`, so alerts are on at the
    /// defaults. Every demo test measures the shipped scenario, not a variant.
    fn demo_test_panes() -> Vec<ProviderPane> {
        demo_panes(&demo_config(Config::default()))
    }

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // --- M9b: the window has no proxy opt-in surface (SECURITY.md inv. 7) ---

    /// The window's egress is constructed proxy-off unconditionally, and that
    /// is an invariant, not a default: there must be no flag, no setting, and
    /// no code path by which the GUI can build a proxy-enabled chokepoint.
    ///
    /// Scans this binary's own source, because the claim is about what the
    /// source does *not* contain — a runtime test cannot observe an absent
    /// feature. If someone adds a GUI proxy toggle, this fails and sends the
    /// change back to the top tier, where a SECURITY.md invariant belongs.
    ///
    /// Comment lines are stripped first: the assertion is about code paths,
    /// and the call site's own comment necessarily *discusses* the thing that
    /// must not exist.
    #[test]
    fn the_window_exposes_no_proxy_opt_in() {
        const SRC: &str = include_str!("main.rs");
        let code: String = SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            !code.contains("--allow-proxy"),
            "the window must not grow a proxy opt-in flag (SECURITY.md invariant 7)"
        );
        assert!(
            !code.contains("Egress::new(true)"),
            "the window must never construct a proxy-enabled egress"
        );
        assert_eq!(
            code.matches("Egress::new(").count(),
            1,
            "exactly one egress construction in the window"
        );
        assert!(
            code.contains("Egress::new(false)"),
            "the window's egress must be constructed proxy-off, literally and unconditionally"
        );
    }

    // --- parse_args: Claude flag (unchanged behavior) ---

    #[test]
    fn no_args_defaults_client_version() {
        let parsed = parse_args(args(&[])).unwrap();
        assert_eq!(parsed.client_version, DEFAULT_CLIENT_VERSION);
        assert!(parsed.client_version_defaulted);
    }

    #[test]
    fn client_version_flag_overrides_default() {
        let parsed = parse_args(args(&["--client-version", "1.2.3"])).unwrap();
        assert_eq!(parsed.client_version, "1.2.3");
        assert!(!parsed.client_version_defaulted);
    }

    #[test]
    fn client_version_without_value_is_an_error() {
        assert!(parse_args(args(&["--client-version"])).is_err());
    }

    #[test]
    fn unrecognized_flag_is_an_error() {
        assert!(parse_args(args(&["--bogus"])).is_err());
    }

    // --- parse_args: Codex UA flag (new) ---

    #[test]
    fn no_args_defaults_codex_user_agent_to_verified_default() {
        let parsed = parse_args(args(&[])).unwrap();
        assert_eq!(parsed.codex_user_agent, CODEX_DEFAULT_USER_AGENT);
    }

    #[test]
    fn codex_user_agent_flag_overrides_default() {
        let parsed = parse_args(args(&["--codex-user-agent", "codex-cli/9.9.9"])).unwrap();
        assert_eq!(parsed.codex_user_agent, "codex-cli/9.9.9");
    }

    #[test]
    fn codex_user_agent_without_value_is_an_error() {
        assert!(parse_args(args(&["--codex-user-agent"])).is_err());
    }

    #[test]
    fn both_flags_can_be_combined_in_any_order() {
        let parsed = parse_args(args(&[
            "--codex-user-agent",
            "ua-x",
            "--client-version",
            "4.5.6",
        ]))
        .unwrap();
        assert_eq!(parsed.client_version, "4.5.6");
        assert!(!parsed.client_version_defaulted);
        assert_eq!(parsed.codex_user_agent, "ua-x");
    }

    // --- parse_args: --no-tray flag (new) ---

    #[test]
    fn no_tray_defaults_to_false() {
        let parsed = parse_args(args(&[])).unwrap();
        assert!(!parsed.no_tray);
    }

    #[test]
    fn no_tray_flag_sets_true() {
        let parsed = parse_args(args(&["--no-tray"])).unwrap();
        assert!(parsed.no_tray);
    }

    #[test]
    fn no_tray_combines_with_other_flags() {
        let parsed = parse_args(args(&[
            "--no-tray",
            "--client-version",
            "1.2.3",
            "--codex-user-agent",
            "ua-y",
        ]))
        .unwrap();
        assert!(parsed.no_tray);
        assert_eq!(parsed.client_version, "1.2.3");
        assert_eq!(parsed.codex_user_agent, "ua-y");
    }

    // --- M7b-r1: theme flags ---

    #[test]
    fn no_theme_flag_leaves_the_saved_preference_alone() {
        assert_eq!(parse_args(args(&[])).unwrap().theme_override, None);
    }

    #[test]
    fn plain_and_themed_flags_are_recognised() {
        assert_eq!(
            parse_args(args(&["--plain"])).unwrap().theme_override,
            Some(Theme::Plain)
        );
        assert_eq!(
            parse_args(args(&["--themed"])).unwrap().theme_override,
            Some(Theme::CipherPine)
        );
    }

    #[test]
    fn the_later_theme_flag_wins() {
        // Ordinary shell convention, rather than erroring on a harmless
        // duplicate.
        assert_eq!(
            parse_args(args(&["--themed", "--plain"]))
                .unwrap()
                .theme_override,
            Some(Theme::Plain)
        );
        assert_eq!(
            parse_args(args(&["--plain", "--themed"]))
                .unwrap()
                .theme_override,
            Some(Theme::CipherPine)
        );
    }

    #[test]
    fn theme_flags_combine_with_the_others() {
        let parsed =
            parse_args(args(&["--plain", "--no-tray", "--client-version", "1.2.3"])).unwrap();
        assert_eq!(parsed.theme_override, Some(Theme::Plain));
        assert!(parsed.no_tray);
        assert_eq!(parsed.client_version, "1.2.3");
    }

    // --- provider_label: label mapping ---

    #[test]
    fn provider_labels_map_to_titles() {
        assert_eq!(provider_label(ProviderId::ClaudeSubscription), "Claude");
        assert_eq!(provider_label(ProviderId::CodexSubscription), "Codex");
    }

    use usage_core::model::SnapshotSource;

    // --- layout: the fixed window is the whole budget (M5a-fix) ---
    //
    // These exist because the clipped per-model row shipped with a full green
    // test suite: every M5a test asserted on parsed fixtures, and a fixture is
    // never laid out, so nothing measured whether a row actually fits. These
    // do measure.

    /// What a layout wanted, versus what the window actually offers.
    struct Laid {
        /// Width the content occupied.
        width: f32,
        /// Height the content occupied.
        height: f32,
        /// Width available inside the real window's panel — measured, not
        /// assumed, so the assertions self-calibrate if a margin changes.
        available_width: f32,
        /// Vertical space the `CentralPanel`'s own frame costs, on top of the
        /// content — measured for the same reason `available_width` is.
        panel_chrome_height: f32,
    }

    /// Lay `add_contents` out in a headless replica of the real window: same
    /// fixed size, same `CentralPanel`, same `ScrollArea`.
    ///
    /// Deliberately **not** `egui::__run_test_ui`, which installs empty fonts
    /// to save CPU. Under empty fonts every string measures ~0 wide, so a
    /// width assertion made through it would pass no matter how far a row
    /// overflowed — it would recreate exactly the blind spot that let this bug
    /// ship. A default `Context` keeps egui's real fonts and real text
    /// extents.
    fn lay_out(add_contents: impl FnMut(&mut egui::Ui)) -> Laid {
        // Cipher Pine is the binding case: its monospace is wider per
        // character than Plain's proportional default. Proven by
        // `plain_is_never_wider_than_cipher_pine`, not assumed.
        lay_out_themed(Theme::CipherPine, add_contents)
    }

    fn lay_out_themed(theme: Theme, add_contents: impl FnMut(&mut egui::Ui)) -> Laid {
        lay_out_sized(theme, WINDOW_HEIGHT, add_contents)
    }

    /// [`lay_out_themed`] against a window of `screen_height` rather than the
    /// production [`WINDOW_HEIGHT`] — how the demo's own taller window is
    /// measured. Also returns the central panel's chrome, so a caller sizing a
    /// window can add it rather than guess it.
    fn lay_out_sized(
        theme: Theme,
        screen_height: f32,
        mut add_contents: impl FnMut(&mut egui::Ui),
    ) -> Laid {
        let ctx = egui::Context::default();
        // The shipped theme, not egui's default. A harness without this would
        // happily pass a row that clips in the real window — precisely the
        // blind spot these tests exist to close.
        install_theme(&ctx, theme);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WINDOW_WIDTH, screen_height),
            )),
            ..Default::default()
        };
        let (mut width, mut height, mut available_width) = (0.0, 0.0, 0.0);
        let mut panel_chrome_height = 0.0;
        // Two frames: the first warms the font atlas and the id-keyed state
        // the indent and scroll area allocate, so the second measures a
        // settled layout.
        for _ in 0..2 {
            let _ = ctx.run_ui(input.clone(), |ui| {
                // The panel's own margins, read from the style the window
                // installed rather than assumed — the same self-calibration
                // `available_width` gets.
                panel_chrome_height = egui::Frame::central_panel(ui.style())
                    .total_margin()
                    .sum()
                    .y;
                egui::CentralPanel::default().show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        available_width = ui.max_rect().width();
                        add_contents(ui);
                        width = ui.min_rect().width();
                        height = ui.min_rect().height();
                    });
                });
            });
        }
        Laid {
            width,
            height,
            available_width,
            panel_chrome_height,
        }
    }

    /// A per-model window with a realistic provider model name.
    fn model_window(label: &str) -> QuotaWindow {
        QuotaWindow {
            label: label.to_string(),
            used_fraction: Some(0.42),
            // 5d 4h — the widest reset string the formatter produces.
            resets_in_secs: Some(446_400),
            // A weekly window, so these rows also carry a pace tick (M8) —
            // which is deliberate: every layout assertion below then measures
            // the tick's presence too, and would catch it allocating space.
            duration_secs: Some(604_800),
        }
    }

    #[test]
    fn per_model_row_fits_the_window() {
        // The exact label from the Codex fixture that clipped in the window.
        let laid = lay_out(|ui| {
            ui.indent("t", |ui| {
                render_per_model_row(ui, &model_window("GPT-5.3-Codex-Spark"), Theme::CipherPine)
            });
        });
        assert!(
            laid.width <= laid.available_width,
            "per-model row wanted {}px inside {}px",
            laid.width,
            laid.available_width
        );
    }

    #[test]
    fn per_model_row_fits_for_any_label_length() {
        // The point of stacking: length-independence. Model names trend
        // longer, and a fix that merely bought some pixels would fail here.
        let absurd = "Claude-Opus-5-20260501-Extended-Thinking-Preview";
        let laid = lay_out(|ui| {
            ui.indent("t", |ui| {
                render_per_model_row(ui, &model_window(absurd), Theme::CipherPine)
            });
        });
        assert!(
            laid.width <= laid.available_width,
            "long label wanted {}px inside {}px",
            laid.width,
            laid.available_width
        );
    }

    #[test]
    fn single_line_layout_would_not_fit_which_is_why_rows_stack() {
        // Pins the defect itself. `render_window_row` is correct for the
        // headline labels it serves, but a model name on that one line
        // overflows — this is the counterfactual that justifies
        // `render_per_model_row` existing at all, and it fails if anyone
        // "simplifies" the two-line row back to one line.
        let laid = lay_out(|ui| {
            ui.indent("t", |ui| {
                render_window_row(ui, &model_window("GPT-5.3-Codex-Spark"), Theme::CipherPine)
            });
        });
        assert!(
            laid.width > laid.available_width,
            "expected the single-line layout to overflow; it wanted {}px inside {}px",
            laid.width,
            laid.available_width
        );
    }

    #[test]
    fn headline_rows_still_fit() {
        // `render_window_row` is untouched; guard that it stays fitting for
        // the labels it actually serves.
        for label in ["5h", "7d"] {
            let laid = lay_out(|ui| {
                render_window_row(
                    ui,
                    &QuotaWindow {
                        label: label.to_string(),
                        used_fraction: Some(0.33),
                        resets_in_secs: Some(446_400),
                        duration_secs: Some(604_800),
                    },
                    Theme::CipherPine,
                )
            });
            assert!(
                laid.width <= laid.available_width,
                "headline row {label} wanted {}px inside {}px",
                laid.width,
                laid.available_width
            );
        }
    }

    #[test]
    fn expanded_pane_fits_the_window_width() {
        // Integration guard, and the strongest available proof that the
        // expanded branch calls `render_per_model_row`: if it still called
        // `render_window_row`, these labels would overflow exactly as
        // `single_line_layout_would_not_fit_which_is_why_rows_stack` shows.
        let snapshot = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 0,
            windows: vec![
                QuotaWindow {
                    label: "5h".to_string(),
                    used_fraction: Some(0.25),
                    resets_in_secs: Some(3600),
                    duration_secs: Some(18_000),
                },
                QuotaWindow {
                    label: "7d".to_string(),
                    used_fraction: Some(0.18),
                    resets_in_secs: Some(86_400),
                    duration_secs: Some(604_800),
                },
            ],
            per_model: vec![
                model_window("GPT-5.3-Codex-Spark"),
                model_window("GPT-5.3-Codex-Max"),
            ],
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        };
        let laid = lay_out(|ui| {
            render_windows(
                ui,
                &snapshot,
                Some(Duration::from_secs(30)),
                true,
                Theme::CipherPine,
                PaneLines {
                    pace: None,
                    spark: &[],
                    alert: None,
                    refill: None,
                },
            );
        });
        assert!(
            laid.width <= laid.available_width,
            "expanded pane wanted {}px inside {}px",
            laid.width,
            laid.available_width
        );
    }

    #[test]
    fn several_expanded_models_outgrow_the_window_height() {
        // Why the `ScrollArea` is there, measured rather than asserted.
        //
        // Two-line rows cost ~34px each, and the central panel can never be
        // taller than the window minus the titlebar (less its own margins
        // still). One provider expanding six models already exceeds that, and
        // the real window splits this budget across *two* panes, so the true
        // threshold is lower again. Without the scroll area, overflow silently
        // eats the last row with no way to reach it.
        let usable_height = WINDOW_HEIGHT - TITLEBAR_HEIGHT;
        let snapshot = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 0,
            windows: vec![QuotaWindow {
                label: "5h".to_string(),
                used_fraction: Some(0.25),
                resets_in_secs: Some(3600),
                duration_secs: Some(18_000),
            }],
            per_model: (0..6)
                .map(|i| model_window(&format!("GPT-5.3-Codex-Variant-{i}")))
                .collect(),
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        };
        let laid = lay_out(|ui| {
            render_windows(
                ui,
                &snapshot,
                Some(Duration::from_secs(30)),
                true,
                Theme::CipherPine,
                PaneLines {
                    pace: None,
                    spark: &[],
                    alert: None,
                    refill: None,
                },
            );
        });
        assert!(
            laid.height > usable_height,
            "expected {}px of content to exceed the {usable_height}px the panel can have",
            laid.height
        );
        // Width still holds even in the overflowing case.
        assert!(laid.width <= laid.available_width);
    }

    #[test]
    fn per_model_rows_use_the_dedicated_two_line_renderer() {
        // The signature the expanded branch depends on.
        let _: fn(&mut egui::Ui, &QuotaWindow, Theme) = render_per_model_row;
    }

    // --- M7a: untouched buckets are hidden ---

    /// A per-model window with an explicit usage fraction.
    fn model_window_at(label: &str, used_fraction: Option<f64>) -> QuotaWindow {
        QuotaWindow {
            used_fraction,
            ..model_window(label)
        }
    }

    /// A one-headline-window snapshot carrying `per_model`, so the layouts
    /// below differ only in their per-model content.
    fn per_model_snapshot(per_model: Vec<QuotaWindow>) -> ProviderSnapshot {
        ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 0,
            windows: vec![QuotaWindow {
                label: "5h".to_string(),
                used_fraction: Some(0.25),
                resets_in_secs: Some(3600),
                duration_secs: Some(18_000),
            }],
            per_model,
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        }
    }

    fn lay_out_pane(snapshot: &ProviderSnapshot, expanded: bool) -> Laid {
        lay_out(|ui| {
            render_windows(
                ui,
                snapshot,
                Some(Duration::from_secs(30)),
                expanded,
                Theme::CipherPine,
                PaneLines {
                    pace: None,
                    spark: &[],
                    alert: None,
                    refill: None,
                },
            );
        })
    }

    #[test]
    fn only_touched_buckets_are_visible() {
        assert!(per_model_row_is_visible(&model_window_at(
            "Barely",
            Some(0.01)
        )));
        assert!(per_model_row_is_visible(&model_window_at(
            "Full",
            Some(1.0)
        )));
        assert!(!per_model_row_is_visible(&model_window_at(
            "Untouched",
            Some(0.0)
        )));
        assert!(!per_model_row_is_visible(&model_window_at("Unknown", None)));
    }

    #[test]
    fn all_untouched_per_model_renders_no_toggle() {
        // The Codex case that prompted this: every listed bucket at 0%. The
        // pane must lay out exactly as if it carried no per-model data at all
        // — equal height means no toggle row was allocated.
        let untouched = per_model_snapshot(vec![
            model_window_at("GPT-5.3-Codex-Spark", Some(0.0)),
            model_window_at("GPT-5.3-Codex-Mini", None),
        ]);
        let empty = per_model_snapshot(vec![]);

        // Both disclosure states: neither may reveal an affordance.
        for expanded in [false, true] {
            let with = lay_out_pane(&untouched, expanded);
            let without = lay_out_pane(&empty, expanded);
            assert_eq!(
                with.height, without.height,
                "all-untouched pane (expanded={expanded}) took {}px vs {}px with no per-model data",
                with.height, without.height
            );
        }
    }

    #[test]
    fn mixed_buckets_show_only_the_used_rows() {
        // One used bucket among untouched ones lays out exactly like a
        // snapshot carrying that bucket alone — the 0%/unknown rows are gone.
        let mixed = per_model_snapshot(vec![
            model_window_at("GPT-5.3-Codex-Spark", Some(0.0)),
            model_window_at("GPT-5.3-Codex-Max", Some(0.42)),
            model_window_at("GPT-5.3-Codex-Mini", None),
        ]);
        let used_only = per_model_snapshot(vec![model_window_at("GPT-5.3-Codex-Max", Some(0.42))]);

        let mixed_laid = lay_out_pane(&mixed, true);
        let used_laid = lay_out_pane(&used_only, true);
        assert_eq!(
            mixed_laid.height, used_laid.height,
            "mixed pane took {}px vs {}px for the used bucket alone",
            mixed_laid.height, used_laid.height
        );

        // Guard against a vacuous comparison: one visible row still earns the
        // toggle, so this must be taller than the no-per-model pane.
        let empty_laid = lay_out_pane(&per_model_snapshot(vec![]), true);
        assert!(
            mixed_laid.height > empty_laid.height,
            "expected a visible row to render; got {}px vs {}px",
            mixed_laid.height,
            empty_laid.height
        );

        // Width still holds with the filter in place.
        assert!(mixed_laid.width <= mixed_laid.available_width);
    }

    #[test]
    fn hidden_rows_stay_in_the_snapshot() {
        // The filter is display-only. The snapshot the CLI serializes is
        // untouched by rendering — pinned end-to-end by the `usage-cli` test
        // `zero_usage_per_model_buckets_stay_in_json`.
        let snapshot = per_model_snapshot(vec![
            model_window_at("GPT-5.3-Codex-Spark", Some(0.0)),
            model_window_at("GPT-5.3-Codex-Max", Some(0.42)),
        ]);
        let _ = lay_out_pane(&snapshot, true);
        assert_eq!(snapshot.per_model.len(), 2);
        assert_eq!(snapshot.per_model[0].label, "GPT-5.3-Codex-Spark");
        assert_eq!(snapshot.per_model[0].used_fraction, Some(0.0));
    }

    // --- M8: the elapsed-time pace tick ---

    /// A headline window with an explicit countdown and duration.
    fn paced_window(resets_in_secs: Option<u64>, duration_secs: Option<u64>) -> QuotaWindow {
        QuotaWindow {
            label: "5h".to_string(),
            used_fraction: Some(0.40),
            resets_in_secs,
            duration_secs,
        }
    }

    #[test]
    fn pace_tick_sits_at_the_elapsed_fraction() {
        let width = 120.0;
        // Nothing elapsed: the whole window still to run → hard left.
        assert_eq!(
            pace_tick_x(&paced_window(Some(18_000), Some(18_000)), width),
            Some(0.0)
        );
        // Half elapsed → half way across.
        assert_eq!(
            pace_tick_x(&paced_window(Some(9_000), Some(18_000)), width),
            Some(60.0)
        );
        // Fully elapsed (resetting now) → hard right.
        assert_eq!(
            pace_tick_x(&paced_window(Some(0), Some(18_000)), width),
            Some(120.0)
        );
        // A quarter left → three quarters across, and the arithmetic scales
        // with whatever width the bar actually got.
        assert_eq!(
            pace_tick_x(&paced_window(Some(151_200), Some(604_800)), 80.0),
            Some(60.0)
        );
    }

    #[test]
    fn pace_tick_needs_all_three_facts() {
        // Each absence is a real provider case, not a defensive check.
        assert_eq!(pace_tick_x(&paced_window(None, Some(18_000)), 120.0), None);
        assert_eq!(pace_tick_x(&paced_window(Some(9_000), None), 120.0), None);
        assert_eq!(pace_tick_x(&paced_window(None, None), 120.0), None);

        // No fill to compare the tick against → no tick, even with both times.
        let no_fraction = QuotaWindow {
            used_fraction: None,
            ..paced_window(Some(9_000), Some(18_000))
        };
        assert_eq!(pace_tick_x(&no_fraction, 120.0), None);

        // A zero-length window is the one value that would divide.
        assert_eq!(pace_tick_x(&paced_window(Some(0), Some(0)), 120.0), None);
    }

    #[test]
    fn pace_tick_clamps_a_countdown_longer_than_its_window() {
        // Clock skew or a provider's own rounding can report more time left
        // than the window is long. Clamped to "nothing elapsed" rather than
        // pushed off the left edge.
        assert_eq!(
            pace_tick_x(&paced_window(Some(20_000), Some(18_000)), 120.0),
            Some(0.0)
        );
    }

    #[test]
    fn pace_tick_is_the_same_colour_in_both_themes() {
        // The tick is information, not theming: `pace_tick_color` takes no
        // Theme at all, which this asserts by type. It is TEXT_DIM at alpha
        // 200 — dimmed, never invisible and never opaque.
        let _: fn() -> egui::Color32 = pace_tick_color;
        assert_eq!(pace_tick_color().a(), PACE_TICK_ALPHA);
        assert_eq!(PACE_TICK_ALPHA, 200);
        assert_ne!(pace_tick_color(), egui::Color32::TRANSPARENT);
    }

    #[test]
    fn the_pace_tick_changes_no_row_geometry() {
        // The tick is painted, not allocated. A row that carries one must lay
        // out to exactly the size of the same row without one — in both themes
        // and for both row renderers, at the real 320px window width.
        let with = paced_window(Some(9_000), Some(18_000));
        let without = QuotaWindow {
            duration_secs: None,
            ..with.clone()
        };
        // Guard against a vacuous comparison: the first really does earn a
        // tick and the second really does not.
        assert!(pace_tick_x(&with, BAR_WIDTH).is_some());
        assert!(pace_tick_x(&without, BAR_WIDTH).is_none());

        for theme in [Theme::CipherPine, Theme::Plain] {
            let headline_with = lay_out_themed(theme, |ui| render_window_row(ui, &with, theme));
            let headline_without =
                lay_out_themed(theme, |ui| render_window_row(ui, &without, theme));
            assert_eq!(
                (headline_with.height, headline_with.width),
                (headline_without.height, headline_without.width),
                "{theme:?} headline row changed geometry: {}x{} vs {}x{}",
                headline_with.width,
                headline_with.height,
                headline_without.width,
                headline_without.height
            );

            let model_with = lay_out_themed(theme, |ui| {
                ui.indent("t", |ui| render_per_model_row(ui, &with, theme));
            });
            let model_without = lay_out_themed(theme, |ui| {
                ui.indent("t", |ui| render_per_model_row(ui, &without, theme));
            });
            assert_eq!(
                (model_with.height, model_with.width),
                (model_without.height, model_without.width),
                "{theme:?} per-model row changed geometry"
            );

            // And the ticked row still fits the window it has to live in.
            assert!(
                headline_with.width <= headline_with.available_width,
                "{theme:?} ticked headline row wanted {}px inside {}px",
                headline_with.width,
                headline_with.available_width
            );
        }
    }

    #[test]
    fn every_bar_gets_a_tick_through_the_one_shared_helper() {
        // Headline and per-model rows both draw through `add_quota_bar`, so the
        // tick cannot appear on one kind of bar and not the other. The shared
        // signature is what guarantees it.
        let _: fn(&mut egui::Ui, &QuotaWindow) = add_quota_bar;

        // And a whole pane with ticked windows lays out unchanged from the same
        // pane with the durations stripped — the integration form of
        // `the_pace_tick_changes_no_row_geometry`.
        let ticked = per_model_snapshot(vec![model_window_at("GPT-5.3-Codex-Max", Some(0.42))]);
        let unticked = ProviderSnapshot {
            windows: ticked
                .windows
                .iter()
                .map(|w| QuotaWindow {
                    duration_secs: None,
                    ..w.clone()
                })
                .collect(),
            per_model: ticked
                .per_model
                .iter()
                .map(|w| QuotaWindow {
                    duration_secs: None,
                    ..w.clone()
                })
                .collect(),
            ..ticked.clone()
        };
        assert!(ticked.windows[0].duration_secs.is_some());
        assert!(ticked.per_model[0].duration_secs.is_some());

        let a = lay_out_pane(&ticked, true);
        let b = lay_out_pane(&unticked, true);
        assert_eq!((a.height, a.width), (b.height, b.width));
    }

    // --- M8: the at-risk line ---

    fn burn(per_hour: f64, exhaust_in_secs: Option<u64>) -> Burn {
        Burn {
            per_hour,
            exhaust_in_secs,
        }
    }

    fn warning(label: &str, exhaust_in_secs: u64) -> PaceWarning {
        PaceWarning {
            label: label.to_string(),
            exhaust_in_secs,
        }
    }

    #[test]
    fn the_sooner_at_risk_window_wins() {
        // Both headline windows are at risk; only the one that runs out first
        // gets the line. Two competing lines in a 320px pane would make the
        // reader compare instead of act.
        let candidates = vec![
            ("7d".to_string(), burn(0.05, Some(32_400)), Some(302_400)),
            ("5h".to_string(), burn(0.20, Some(6_000)), Some(9_000)),
        ];
        assert_eq!(select_pace_warning(&candidates), Some(warning("5h", 6_000)));

        // The choice is about the numbers, not the order they arrive in.
        let mut reversed = candidates.clone();
        reversed.reverse();
        assert_eq!(select_pace_warning(&reversed), Some(warning("5h", 6_000)));
    }

    #[test]
    fn a_window_that_resets_before_it_fills_is_not_a_candidate() {
        // The soonest exhaustion overall, but its window resets first — so it is
        // not at risk and must not be selected. The 7d window, which really is
        // at risk, gets the line instead. Selecting on exhaustion alone (the
        // easy mistake) would fail here.
        let candidates = vec![
            ("5h".to_string(), burn(0.20, Some(6_000)), Some(3_000)),
            ("7d".to_string(), burn(0.05, Some(32_400)), Some(302_400)),
        ];
        assert_eq!(
            select_pace_warning(&candidates),
            Some(warning("7d", 32_400))
        );
    }

    #[test]
    fn nothing_at_risk_says_nothing() {
        // Calm is silent — no line, not a reassuring one.
        assert_eq!(select_pace_warning(&[]), None);

        let calm = vec![
            // Burning, but the window resets long before it fills.
            ("5h".to_string(), burn(0.02, Some(158_400)), Some(14_400)),
            // Not burning at all.
            ("7d".to_string(), burn(0.0, None), Some(302_400)),
            // Burning, but with no visible deadline to beat.
            ("primary".to_string(), burn(0.30, Some(600)), None),
        ];
        assert_eq!(select_pace_warning(&calm), None);
    }

    #[test]
    fn the_at_risk_line_reads_as_an_extrapolation() {
        // "at this pace" and the `~` matter: this is a projection from the last
        // couple of hours and must not be mistaken for a countdown.
        assert_eq!(
            pace_warning_line(&warning("7d", 32_400)),
            "at this pace: 7d full in ~9h 0m"
        );
        assert_eq!(
            pace_warning_line(&warning("5h", 3_600)),
            "at this pace: 5h full in ~1h 0m"
        );
        // Under a minute, and days — the same units as the reset countdown
        // beside it, because it goes through the same formatter.
        assert_eq!(
            pace_warning_line(&warning("5h", 30)),
            "at this pace: 5h full in ~<1m"
        );
        assert_eq!(
            pace_warning_line(&warning("7d", 446_400)),
            "at this pace: 7d full in ~5d 4h"
        );
        // A provider's own label rides verbatim, unparsed, as everywhere else.
        assert_eq!(
            pace_warning_line(&warning("GPT-5.3-Codex-Max", 5_400)),
            "at this pace: GPT-5.3-Codex-Max full in ~1h 30m"
        );
    }

    #[test]
    fn the_at_risk_line_turns_cardinal_inside_six_hours() {
        assert_eq!(PACE_CARDINAL_UNDER_SECS, 21_600);
        assert_eq!(pace_warning_color(0), CARDINAL);
        assert_eq!(pace_warning_color(PACE_CARDINAL_UNDER_SECS - 1), CARDINAL);
        // Exactly six hours out is the calmer read — the boundary is amber's.
        assert_eq!(pace_warning_color(PACE_CARDINAL_UNDER_SECS), AMBER);
        assert_eq!(pace_warning_color(446_400), AMBER);
        // Neither colour is theme-dependent; the signature says so.
        let _: fn(u64) -> egui::Color32 = pace_warning_color;
    }

    #[test]
    fn the_at_risk_line_costs_a_row_only_when_there_is_one() {
        let snapshot = per_model_snapshot(vec![]);
        let render = |pace: Option<&PaceWarning>| {
            lay_out(|ui| {
                render_windows(
                    ui,
                    &snapshot,
                    Some(Duration::from_secs(30)),
                    false,
                    Theme::CipherPine,
                    PaneLines {
                        pace,
                        spark: &[],
                        alert: None,
                        refill: None,
                    },
                );
            })
        };
        let silent = render(None);
        let warned = render(Some(&warning("5h", 6_000)));

        // Both directions at once: `None` adds nothing, `Some` adds a line.
        assert!(
            warned.height > silent.height,
            "the at-risk line rendered nothing: {}px vs {}px",
            warned.height,
            silent.height
        );
        // And it fits the fixed window — including the longest label a provider
        // could plausibly hand it.
        let long = render(Some(&warning("Claude-Opus-5-Extended-Thinking", 6_000)));
        assert!(
            long.width <= long.available_width,
            "a long at-risk label wanted {}px inside {}px",
            long.width,
            long.available_width
        );
    }

    // --- M10: the at-risk line goes quiet on stale data ---

    #[test]
    fn the_at_risk_line_is_suppressed_exactly_when_the_dot_says_stale() {
        // Fresh forecasts.
        assert!(show_pace_warning(Some(Duration::from_secs(1))));
        assert!(show_pace_warning(Some(
            STALE_AFTER - Duration::from_secs(1)
        )));
        // The boundary is `is_stale`'s own, not a second opinion about it —
        // asserted against the predicate so moving STALE_AFTER moves both.
        assert_eq!(show_pace_warning(Some(STALE_AFTER)), !is_stale(STALE_AFTER));
        assert!(!show_pace_warning(Some(STALE_AFTER)));
        // Well past it.
        assert!(!show_pace_warning(Some(STALE_AFTER * 10)));
        // Unknown age keeps drawing, as everywhere else in the pane.
        assert!(show_pace_warning(None));
    }

    #[test]
    fn stale_data_draws_no_at_risk_line() {
        // The helper being right is not the same as it being wired in: this
        // measures the rendered pane, so a gate that got factored out but never
        // called would fail here.
        let snapshot = per_model_snapshot(vec![]);
        let render = |age: Duration, pace: Option<&PaceWarning>| {
            lay_out(|ui| {
                render_windows(
                    ui,
                    &snapshot,
                    Some(age),
                    false,
                    Theme::CipherPine,
                    PaneLines {
                        pace,
                        spark: &[],
                        alert: None,
                        refill: None,
                    },
                );
            })
        };
        let at_risk = warning("5h", 6_000);

        // Stale: the warned pane is exactly the silent pane — no line, no gap.
        let stale = STALE_AFTER + Duration::from_secs(1);
        assert_eq!(
            render(stale, Some(&at_risk)).height,
            render(stale, None).height,
            "a stale pane drew the at-risk line anyway"
        );

        // Fresh: unchanged from M8 — the line is still there. Both directions
        // together, so this cannot pass by suppressing the line everywhere.
        let fresh = Duration::from_secs(30);
        assert!(
            render(fresh, Some(&at_risk)).height > render(fresh, None).height,
            "a fresh pane stopped drawing the at-risk line"
        );
    }

    // --- M8: rings fed per poll ---

    #[test]
    fn one_ring_per_headline_window_fed_from_every_snapshot() {
        // Per-model rows deliberately get no ring in this slice, so a snapshot
        // carrying them must still leave exactly one ring per headline window.
        let series = demo_series(
            ProviderId::CodexSubscription,
            &[
                DemoWindow {
                    label: "5h",
                    duration_secs: 18_000,
                    final_resets_in_secs: 9_000,
                    from_fraction: 0.20,
                    to_fraction: 0.30,
                },
                DemoWindow {
                    label: "7d",
                    duration_secs: 604_800,
                    final_resets_in_secs: 302_400,
                    from_fraction: 0.10,
                    to_fraction: 0.12,
                },
            ],
            &[model_window_at("GPT-5.3-Codex-Max", Some(0.42))],
            None,
        );
        let pane = ProviderPane::demo(
            ProviderId::CodexSubscription,
            series,
            Vec::new(),
            &Config::default(),
        );

        let labels: Vec<&str> = pane.rings.iter().map(|(l, _)| l.as_str()).collect();
        assert_eq!(labels, vec!["5h", "7d"]);
        for (label, ring) in &pane.rings {
            assert_eq!(
                ring.len(),
                DEMO_STEPS as usize,
                "ring {label} missed a snapshot"
            );
        }
    }

    #[test]
    fn a_window_without_a_usage_fraction_gets_no_ring() {
        // Nothing to sample. The row still renders; it just contributes no pace.
        let mut series = demo_series(
            ProviderId::ClaudeSubscription,
            &[DemoWindow {
                label: "5h",
                duration_secs: 18_000,
                final_resets_in_secs: 9_000,
                from_fraction: 0.20,
                to_fraction: 0.30,
            }],
            &[],
            None,
        );
        for snapshot in &mut series {
            snapshot.windows[0].used_fraction = None;
        }
        let pane = ProviderPane::demo(
            ProviderId::ClaudeSubscription,
            series,
            Vec::new(),
            &Config::default(),
        );
        assert!(pane.rings.is_empty());
        assert!(pane.pace_warning.is_none());
    }

    #[test]
    fn a_detected_reset_clears_the_trail_and_silences_the_line() {
        // A window climbing toward full, then rolling over. The pre-reset trail
        // describes a window that no longer exists, so the forecast must go
        // quiet rather than extrapolate across the discontinuity.
        let mut series = demo_series(
            ProviderId::CodexSubscription,
            &[DemoWindow {
                label: "5h",
                duration_secs: 18_000,
                final_resets_in_secs: 3_600,
                from_fraction: 0.70,
                to_fraction: 0.90,
            }],
            &[],
            None,
        );
        let before = ProviderPane::demo(
            ProviderId::CodexSubscription,
            series.clone(),
            Vec::new(),
            &Config::default(),
        );
        assert!(
            before.pace_warning.is_some(),
            "the pre-reset pane must be warning, or this test proves nothing"
        );

        // The rollover: usage back to nothing, countdown restored.
        let mut rolled_over = series.last().expect("a series").clone();
        rolled_over.taken_at_unix_secs += 600;
        rolled_over.windows[0].used_fraction = Some(0.0);
        rolled_over.windows[0].resets_in_secs = Some(18_000);
        series.push(rolled_over);

        let after = ProviderPane::demo(
            ProviderId::CodexSubscription,
            series,
            Vec::new(),
            &Config::default(),
        );
        assert_eq!(after.rings[0].1.len(), 1, "the old trail must be gone");
        assert!(
            after.pace_warning.is_none(),
            "one post-reset sample cannot support a forecast"
        );
    }

    // --- M13: disk history and pace rehydration ---

    /// A pane in exactly the state `ProviderPane::new` leaves a live provider
    /// in — no poller, no history. Built by hand because `new` spawns a real
    /// poller thread, which a unit test must not do.
    fn bare_pane(id: ProviderId) -> ProviderPane {
        ProviderPane {
            id,
            handle: None,
            latest_snapshot: None,
            snapshot_received_at: None,
            latest_failure: None,
            startup_error: None,
            expanded: false,
            rings: Vec::new(),
            history: Vec::new(),
            history_enabled: false,
            history_path: None,
            alerts_firing: Vec::new(),
            alert_line: None,
            refill_line: None,
            pace_warning: None,
        }
    }

    /// A private temp path for one test; removed when the guard drops.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(tag: &str) -> TempDir {
            let mut dir = std::env::temp_dir();
            dir.push(format!("quotapane-ui-hist-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            TempDir(dir)
        }

        fn file(&self) -> PathBuf {
            self.0.join(history::FILE_NAME)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn history_entry(provider: ProviderId, at: u64, label: &str, used: f64) -> HistoryEntry {
        HistoryEntry {
            at_unix_secs: at,
            provider,
            label: label.to_string(),
            used_fraction: used,
            duration_secs: Some(18_000),
        }
    }

    /// Six readings of a 5h window climbing 0.60 → 0.85 over the hour ending
    /// ten minutes before `now`.
    fn climbing_trail(provider: ProviderId, now: u64) -> Vec<HistoryEntry> {
        (0..6)
            .map(|i| {
                history_entry(
                    provider,
                    now - 3_600 + i * 600,
                    "5h",
                    0.60 + 0.05 * i as f64,
                )
            })
            .collect()
    }

    /// The snapshot that would land at `now`, continuing that climb.
    fn climbing_snapshot(provider: ProviderId, now: u64) -> ProviderSnapshot {
        ProviderSnapshot {
            provider,
            taken_at_unix_secs: now,
            windows: vec![QuotaWindow {
                label: "5h".to_string(),
                used_fraction: Some(0.90),
                resets_in_secs: Some(1_800),
                duration_secs: Some(18_000),
            }],
            per_model: Vec::new(),
            reset_credits: None,
            source: usage_core::model::SnapshotSource::UsageEndpoint,
        }
    }

    #[test]
    fn a_forecast_survives_a_restart() {
        // The whole point of P2, stated as a before/after: the same first poll
        // says nothing to a cold pane and produces a forecast on a rehydrated
        // one, because the trail behind it came back off disk.
        let now = DEMO_BASE_UNIX_SECS;
        let snapshot = climbing_snapshot(ProviderId::ClaudeSubscription, now);

        let mut cold = bare_pane(ProviderId::ClaudeSubscription);
        cold.ingest_pace(&snapshot);
        assert!(
            cold.pace_warning.is_none(),
            "one sample cannot support a forecast — the M8 behaviour"
        );

        let tmp = TempDir::new("restart");
        let mut warm = bare_pane(ProviderId::ClaudeSubscription);
        warm.enable_history(
            tmp.file(),
            &climbing_trail(ProviderId::ClaudeSubscription, now),
            now,
        );
        assert_eq!(warm.rings.len(), 1, "the ring should have been seeded");
        assert_eq!(warm.rings[0].1.len(), 6);
        warm.ingest_pace(&snapshot);
        let warning = warm.pace_warning.expect("the replayed trail should fit");
        assert_eq!(warning.label, "5h");
        assert!(
            warning.exhaust_in_secs < 1_800,
            "0.10 left at 0.30/h fills inside the half hour to reset, got {}",
            warning.exhaust_in_secs
        );
    }

    #[test]
    fn rehydration_takes_only_this_provider_and_only_the_trail() {
        let now = DEMO_BASE_UNIX_SECS;
        let mut entries = climbing_trail(ProviderId::ClaudeSubscription, now);
        // The other provider's lines, and one older than the pace trail.
        entries.push(history_entry(
            ProviderId::CodexSubscription,
            now - 300,
            "5h",
            0.4,
        ));
        entries.push(history_entry(
            ProviderId::ClaudeSubscription,
            now - pace::TRAIL_SECS - 1,
            "5h",
            0.1,
        ));
        // And one from the future, which no clock-forward run should replay.
        entries.push(history_entry(
            ProviderId::ClaudeSubscription,
            now + 60,
            "5h",
            0.99,
        ));

        let tmp = TempDir::new("filter");
        let mut pane = bare_pane(ProviderId::ClaudeSubscription);
        pane.enable_history(tmp.file(), &entries, now);

        assert_eq!(pane.rings.len(), 1);
        assert_eq!(
            pane.rings[0].1.len(),
            6,
            "only this provider's in-trail readings seed the ring"
        );
        // The day trail keeps this provider's readings inside 24 h — which
        // includes the pre-trail one the ring dropped, since the sparkline
        // draws a longer span than the fit uses.
        assert_eq!(pane.history.len(), 7);
        assert!(pane
            .history
            .iter()
            .all(|e| e.provider == ProviderId::ClaudeSubscription));
    }

    #[test]
    fn a_reset_inside_the_replayed_span_still_clears_the_trail() {
        // Rehydration goes through `PaceRing::observe`, not a private back
        // door, so the used-fraction reset signal works on replayed samples
        // even though history stores no countdown to check.
        let now = DEMO_BASE_UNIX_SECS;
        let mut entries = climbing_trail(ProviderId::ClaudeSubscription, now);
        entries.push(history_entry(
            ProviderId::ClaudeSubscription,
            now - 300,
            "5h",
            0.02,
        ));

        let tmp = TempDir::new("reset");
        let mut pane = bare_pane(ProviderId::ClaudeSubscription);
        pane.enable_history(tmp.file(), &entries, now);
        assert_eq!(
            pane.rings[0].1.len(),
            1,
            "the pre-reset trail must not survive replay"
        );
    }

    #[test]
    fn history_off_records_nothing_anywhere() {
        let now = DEMO_BASE_UNIX_SECS;
        let mut pane = bare_pane(ProviderId::ClaudeSubscription);
        pane.record_history(&climbing_snapshot(ProviderId::ClaudeSubscription, now));
        assert!(pane.history.is_empty(), "history=off must keep no trail");
        assert!(pane.history_path.is_none(), "and must name no file");
    }

    #[test]
    fn recording_appends_to_disk_and_survives_a_reopen() {
        let now = DEMO_BASE_UNIX_SECS;
        let tmp = TempDir::new("append");
        let path = tmp.file();

        let mut pane = bare_pane(ProviderId::CodexSubscription);
        pane.enable_history(path.clone(), &[], now);
        assert!(!path.exists(), "enabling history must not create the file");

        pane.record_history(&climbing_snapshot(ProviderId::CodexSubscription, now));
        pane.record_history(&climbing_snapshot(ProviderId::CodexSubscription, now + 600));
        assert_eq!(
            pane.history.len(),
            2,
            "the day trail tracks what was written"
        );

        // What a later run would read back.
        let reread = history::read(&path);
        assert_eq!(reread.len(), 2);
        assert!(reread
            .iter()
            .all(|e| e.provider == ProviderId::CodexSubscription));
        assert_eq!(reread[0].at_unix_secs, now);
        assert_eq!(reread[1].at_unix_secs, now + 600);
        assert_eq!(reread[1].used_fraction, 0.90);
    }

    #[test]
    fn the_day_trail_forgets_readings_older_than_a_day() {
        let now = DEMO_BASE_UNIX_SECS;
        let tmp = TempDir::new("prune");
        let mut pane = bare_pane(ProviderId::ClaudeSubscription);
        pane.enable_history(
            tmp.file(),
            &[history_entry(
                ProviderId::ClaudeSubscription,
                now,
                "5h",
                0.5,
            )],
            now,
        );
        assert_eq!(pane.history.len(), 1);

        // A poll a day and a minute later pushes the old reading out.
        pane.record_history(&climbing_snapshot(
            ProviderId::ClaudeSubscription,
            now + HISTORY_WINDOW_SECS + 60,
        ));
        assert_eq!(pane.history.len(), 1);
        assert_eq!(pane.history[0].at_unix_secs, now + HISTORY_WINDOW_SECS + 60);
    }

    #[test]
    fn the_demo_fabricates_a_day_of_history_and_writes_none_of_it() {
        for pane in demo_test_panes() {
            assert!(
                pane.history_path.is_none(),
                "a demo pane must never name a real log file"
            );
            assert!(
                pane.history_enabled,
                "but it must still have a trail to draw"
            );

            let times: Vec<u64> = pane.history.iter().map(|e| e.at_unix_secs).collect();
            let (first, last) = (
                *times.iter().min().expect("history"),
                *times.iter().max().expect("history"),
            );
            assert_eq!(
                last,
                DEMO_BASE_UNIX_SECS + (DEMO_STEPS - 1) * DEMO_STEP_SECS,
                "the trail must end where the bars do"
            );
            assert_eq!(
                last - first,
                HISTORY_WINDOW_SECS,
                "the trail must cover the sparkline's full span"
            );
            // Two labels per point, and every reading a real percentage.
            for entry in &pane.history {
                assert!(
                    (0.0..=1.0).contains(&entry.used_fraction),
                    "{entry:?} is not a percentage"
                );
                assert!(entry.label == "5h" || entry.label == "7d", "{entry:?}");
            }
            assert!(
                pane.history.len() >= 2 * 24,
                "{} points",
                pane.history.len()
            );
        }
    }

    #[test]
    fn the_demo_history_is_deterministic() {
        // Same rule as the rest of the demo: two runs must draw the same strip,
        // or a screenshot proves nothing.
        let first: Vec<Vec<HistoryEntry>> =
            demo_test_panes().into_iter().map(|p| p.history).collect();
        let second: Vec<Vec<HistoryEntry>> =
            demo_test_panes().into_iter().map(|p| p.history).collect();
        assert_eq!(first, second);
    }

    // --- M13: the 24 h sparkline strip ---

    /// A strip the size the pane allocates, at a convenient origin.
    fn strip_rect() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(10.0, 100.0), egui::vec2(BAR_WIDTH, SPARK_HEIGHT))
    }

    #[test]
    fn the_strip_runs_a_day_left_to_right_and_fills_upward() {
        let now = DEMO_BASE_UNIX_SECS;
        let rect = strip_rect();
        let points = spark_points(
            &[
                (now - HISTORY_WINDOW_SECS, 0.0),
                (now - HISTORY_WINDOW_SECS / 2, 0.5),
                (now, 1.0),
            ],
            now,
            rect,
        );
        assert_eq!(points.len(), 3);
        // A day ago is the left edge, now is the right edge.
        assert!((points[0].x - rect.left()).abs() < 0.01, "{:?}", points[0]);
        assert!(
            (points[1].x - rect.center().x).abs() < 0.01,
            "{:?}",
            points[1]
        );
        assert!((points[2].x - rect.right()).abs() < 0.01, "{:?}", points[2]);
        // Empty is the bottom edge, full is the top — the same direction the
        // bars above fill in.
        assert!(
            (points[0].y - rect.bottom()).abs() < 0.01,
            "{:?}",
            points[0]
        );
        assert!(
            (points[1].y - rect.center().y).abs() < 0.01,
            "{:?}",
            points[1]
        );
        assert!((points[2].y - rect.top()).abs() < 0.01, "{:?}", points[2]);
    }

    #[test]
    fn the_strip_clips_what_is_outside_its_day() {
        let now = DEMO_BASE_UNIX_SECS;
        let series = [
            (now - HISTORY_WINDOW_SECS - 1, 0.9), // yesterday's yesterday
            (now - HISTORY_WINDOW_SECS, 0.1),     // the edge is inside
            (now, 0.2),
            (now + 1, 0.99), // the future is not a reading
        ];
        assert_eq!(spark_window(&series, now).len(), 2);
        let points = spark_points(&series, now, strip_rect());
        assert_eq!(points.len(), 2);
        assert!(points.iter().all(|p| strip_rect().x_range().contains(p.x)));
    }

    #[test]
    fn a_fraction_outside_zero_to_one_is_clamped_into_the_strip() {
        let now = DEMO_BASE_UNIX_SECS;
        let rect = strip_rect();
        let points = spark_points(&[(now - 600, -0.5), (now, 1.7)], now, rect);
        assert!(
            (points[0].y - rect.bottom()).abs() < 0.01,
            "{:?}",
            points[0]
        );
        assert!((points[1].y - rect.top()).abs() < 0.01, "{:?}", points[1]);
    }

    #[test]
    fn an_empty_series_maps_to_no_points() {
        let now = DEMO_BASE_UNIX_SECS;
        assert!(spark_points(&[], now, strip_rect()).is_empty());
        assert!(spark_window(&[], now).is_empty());
    }

    #[test]
    fn the_series_is_this_windows_readings_in_time_order() {
        let now = DEMO_BASE_UNIX_SECS;
        let history = vec![
            history_entry(ProviderId::ClaudeSubscription, now, "5h", 0.3),
            history_entry(ProviderId::ClaudeSubscription, now - 600, "5h", 0.2),
            history_entry(ProviderId::ClaudeSubscription, now - 300, "7d", 0.9),
        ];
        assert_eq!(
            spark_series(&history, "5h"),
            vec![(now - 600, 0.2), (now, 0.3)],
            "only this window's readings, oldest first"
        );
        assert_eq!(spark_series(&history, "nothing"), vec![]);
    }

    #[test]
    fn one_reading_or_none_draws_no_strip_and_costs_no_space() {
        let now = DEMO_BASE_UNIX_SECS;
        let bare = lay_out(|ui| render_sparkline(ui, &[])).height;
        let single = lay_out(|ui| render_sparkline(ui, &[(now, 0.5)])).height;
        let pair = lay_out(|ui| render_sparkline(ui, &[(now - 600, 0.4), (now, 0.5)])).height;

        assert_eq!(
            bare, single,
            "one reading is not a line and must take no space"
        );
        assert!(
            pair >= single + SPARK_HEIGHT,
            "two readings should have drawn a {SPARK_HEIGHT}px strip: {single} -> {pair}"
        );
    }

    #[test]
    fn a_pane_without_history_renders_exactly_as_it_did_before() {
        // The silence guarantee: history=off is the default, and a default pane
        // must be pixel-identical to the accepted M8 layout.
        let now = DEMO_BASE_UNIX_SECS;
        let snapshot = climbing_snapshot(ProviderId::ClaudeSubscription, now);
        for theme in [Theme::CipherPine, Theme::Plain] {
            let off = lay_out_themed(theme, |ui| {
                render_windows(
                    ui,
                    &snapshot,
                    None,
                    false,
                    theme,
                    PaneLines {
                        pace: None,
                        spark: &[],
                        alert: None,
                        refill: None,
                    },
                );
            });
            let on = lay_out_themed(theme, |ui| {
                render_windows(
                    ui,
                    &snapshot,
                    None,
                    false,
                    theme,
                    PaneLines {
                        pace: None,
                        spark: &[(now - 600, 0.4), (now, 0.5)],
                        alert: None,
                        refill: None,
                    },
                );
            });
            assert!(
                on.height > off.height,
                "{theme:?}: the strip should cost its own height"
            );
            assert_eq!(
                on.width, off.width,
                "{theme:?}: the strip must not widen the pane"
            );
        }
    }

    #[test]
    fn the_strip_is_the_same_colour_in_both_themes() {
        // A history strip is information, not styling — the same rule the pace
        // tick follows.
        assert_eq!(spark_color(), TEXT_DIM);
        assert_eq!(spark_color().a(), 255, "the line draws at full strength");
        assert_eq!(
            spark_fill_color(),
            egui::Color32::from_rgba_unmultiplied(
                TEXT_DIM.r(),
                TEXT_DIM.g(),
                TEXT_DIM.b(),
                SPARK_FILL_ALPHA,
            )
        );
        assert_eq!(SPARK_FILL_ALPHA, 18);
    }

    #[test]
    fn the_strips_hierarchy_is_hue_not_transparency() {
        // M13-R1 replaced M13's "the strip is dimmer than the pace tick" rule,
        // which produced a mark the owner could not identify. Everything the
        // strip draws is now opaque, and the ranking is TEXT (now) over
        // TEXT_DIM (the day) over the near-invisible fill — the same ladder the
        // rest of the pane's text uses. A const block because these are consts:
        // a runtime `assert!` over them is a compile-time fact deferred.
        const { assert!(SPARK_FILL_ALPHA < PACE_TICK_ALPHA) };
        fn luma(c: egui::Color32) -> u32 {
            c.r() as u32 + c.g() as u32 + c.b() as u32
        }
        assert!(
            luma(TEXT) > luma(TEXT_DIM),
            "the now dot must be brighter than the line it terminates"
        );
        // And the tag is the faintest — it names the axis, it is not the data.
        assert!(
            luma(TEXT_FAINT) < luma(TEXT_DIM),
            "the tag must sit under the line it labels"
        );
    }

    #[test]
    fn the_strip_is_drawn_at_the_sizes_the_spec_names() {
        // Byte-and-number pins for the round-2 look. These are the four values
        // the owner's verdict turned on, and a "tidy-up" that quietly restores
        // any of them puts the strip back to the state that was rejected.
        assert_eq!(SPARK_HEIGHT, 16.0);
        assert_eq!(SPARK_STROKE_WIDTH, 1.5);
        assert_eq!(SPARK_NOW_RADIUS, 2.5);
        assert_eq!(SPARK_TAG, "24h");
    }

    #[test]
    fn the_fill_spans_every_segment_down_to_the_baseline() {
        let rect = strip_rect();
        let points = vec![
            egui::pos2(rect.left(), rect.center().y),
            egui::pos2(rect.center().x, rect.top()),
            egui::pos2(rect.right(), rect.center().y),
        ];
        let mesh = spark_fill_mesh(&points, rect.bottom());
        assert!(mesh.is_valid(), "every index must address a vertex");
        // One curve vertex and one baseline vertex per reading…
        assert_eq!(mesh.vertices.len(), points.len() * 2);
        // …and two triangles per segment, sharing the vertices between them:
        // a per-segment quad would double-blend its seams into visible stripes.
        assert_eq!(mesh.indices.len(), (points.len() - 1) * 6);

        for (i, point) in points.iter().enumerate() {
            assert_eq!(mesh.vertices[i * 2].pos, *point);
            assert_eq!(
                mesh.vertices[i * 2 + 1].pos,
                egui::pos2(point.x, rect.bottom()),
                "the fill must reach the strip's floor, not the curve's low point"
            );
        }
        assert!(
            mesh.vertices.iter().all(|v| v.color == spark_fill_color()),
            "the whole body is one flat wash — no gradient to read a value off"
        );
        // The concave case is why this is a mesh and not a convex polygon: a
        // convex fill over these points would flood across the valley.
        let valley = vec![
            egui::pos2(rect.left(), rect.top()),
            egui::pos2(rect.center().x, rect.bottom()),
            egui::pos2(rect.right(), rect.top()),
        ];
        assert!(spark_fill_mesh(&valley, rect.bottom()).is_valid());
    }

    #[test]
    fn a_fill_needs_a_segment() {
        // The same "one reading is not a line" rule the strip itself follows —
        // and `render_sparkline` never gets here with fewer than two, so this
        // is the belt to that braces.
        assert!(spark_fill_mesh(&[], 0.0).is_empty());
        assert!(spark_fill_mesh(&[egui::pos2(1.0, 2.0)], 0.0).is_empty());
    }

    /// Every shape a render actually emitted, flattened out of egui's nesting.
    ///
    /// The strip's marks are *painted*, not laid out, so no width or height
    /// assertion can see them — the M13 strip would have passed every layout
    /// test in this file while drawing nothing at all. This is how the round-2
    /// spec gets checked against what reaches the screen rather than against
    /// the constants feeding it.
    fn painted_shapes(add_contents: impl FnMut(&mut egui::Ui)) -> Vec<egui::Shape> {
        painted_shapes_themed(Theme::CipherPine, add_contents)
    }

    /// [`painted_shapes`] against a chosen theme — how the M14 dot is checked
    /// in Plain as well, since it is deliberately the same dot in both.
    fn painted_shapes_themed(
        theme: Theme,
        mut add_contents: impl FnMut(&mut egui::Ui),
    ) -> Vec<egui::Shape> {
        let ctx = egui::Context::default();
        install_theme(&ctx, theme);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WINDOW_WIDTH, WINDOW_HEIGHT),
            )),
            ..Default::default()
        };
        // Two frames, for the reason `lay_out_sized` runs two: the first warms
        // the font atlas, and a galley laid out against a cold one measures
        // nothing.
        let _warm = ctx.run_ui(input.clone(), &mut add_contents);
        let output = ctx.run_ui(input, &mut add_contents);

        let mut flat = Vec::new();
        for clipped in output.shapes {
            flatten_shape(clipped.shape, &mut flat);
        }
        flat
    }

    /// egui nests shapes; every assertion here wants them flat.
    fn flatten_shape(shape: egui::Shape, out: &mut Vec<egui::Shape>) {
        match shape {
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    flatten_shape(shape, out);
                }
            }
            other => out.push(other),
        }
    }

    #[test]
    fn the_strip_paints_a_body_a_line_a_now_dot_and_its_tag() {
        // Eight half-hourly readings climbing gently — enough segments that a
        // fill spanning only the first or last one would show up here. The
        // oldest is the lowest, and deliberately well clear of empty: a series
        // that touched 0.0 could not tell the strip's floor from the curve's.
        const LOWEST: f64 = 0.2;
        let now = DEMO_BASE_UNIX_SECS;
        let series: Vec<(u64, f64)> = (0..8)
            .map(|i| (now - (7 - i) * 1_800, LOWEST + 0.05 * i as f64))
            .collect();
        let shapes = painted_shapes(|ui| render_sparkline(ui, &series));

        // The body: one mesh, one flat wash, reaching every reading.
        let meshes: Vec<_> = shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Mesh(mesh) => Some(mesh),
                _ => None,
            })
            .collect();
        assert_eq!(meshes.len(), 1, "expected exactly one filled body");
        assert_eq!(meshes[0].vertices.len(), series.len() * 2);
        assert!(meshes[0]
            .vertices
            .iter()
            .all(|v| v.color == spark_fill_color()));

        // The line: one open path, at the spec's width and full-strength ink.
        let paths: Vec<_> = shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Path(path) => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 1, "expected exactly one history line");
        assert_eq!(paths[0].stroke.width, SPARK_STROKE_WIDTH);
        let egui::epaint::ColorMode::Solid(ink) = paths[0].stroke.color else {
            panic!("the strip's line must be one solid colour");
        };
        assert_eq!(ink, spark_color());
        assert_eq!(paths[0].points.len(), series.len());
        assert!(!paths[0].closed, "a day of history is not a loop");

        // The body belongs to *this* line, and hangs from it to the strip's
        // own floor. Checked here rather than only in the mesh's own unit test,
        // because the floor is chosen at the call site: a body stopping at the
        // curve's low point would leave the trough hollow and pass every
        // assertion above.
        let curve: Vec<egui::Pos2> = meshes[0]
            .vertices
            .iter()
            .step_by(2)
            .map(|v| v.pos)
            .collect();
        assert_eq!(curve, paths[0].points, "the body must hang from the line");
        let floor = meshes[0].vertices[1].pos.y;
        assert!(
            meshes[0]
                .vertices
                .iter()
                .skip(1)
                .step_by(2)
                .all(|v| v.pos.y == floor),
            "the body has one flat floor, not a per-segment one"
        );
        let lowest = paths[0].points[0].y;
        assert!(
            (floor - lowest - SPARK_HEIGHT * LOWEST as f32).abs() < 0.01,
            "the floor sits {}px under the lowest reading; the strip's own floor \
             is {}px under it",
            floor - lowest,
            SPARK_HEIGHT * LOWEST as f32
        );

        // The anchor: one dot, on the newest reading — which is the line's
        // right-hand end, not merely somewhere near it.
        let circles: Vec<_> = shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Circle(circle) => Some(circle),
                _ => None,
            })
            .collect();
        assert_eq!(circles.len(), 1, "one anchor, and it is on the present");
        assert_eq!(circles[0].radius, SPARK_NOW_RADIUS);
        assert_eq!(circles[0].fill, TEXT);
        assert_eq!(
            circles[0].center,
            *paths[0].points.last().expect("a line has ends")
        );

        // The tag: the byte-exact string, faint, right-aligned over the same
        // edge the dot sits on rather than floating loose in the pane.
        let texts: Vec<_> = shapes
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(text) => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 1, "the strip labels itself exactly once");
        assert_eq!(texts[0].galley.text(), SPARK_TAG);
        assert_eq!(texts[0].fallback_color, TEXT_FAINT);
        let tag_right = texts[0].pos.x + texts[0].galley.size().x;
        assert!(
            (tag_right - circles[0].center.x).abs() < 1.0,
            "the {SPARK_TAG} tag ends at {tag_right}, not over the strip's right \
             edge at {}",
            circles[0].center.x
        );
        assert!(
            texts[0].pos.y < circles[0].center.y,
            "the tag belongs above the strip, not inside it"
        );
    }

    #[test]
    fn the_strip_costs_its_tag_row_on_top_of_its_height() {
        // The tag is a real row, not paint inside the strip: it has to be, or
        // it would sit on top of the line's left half. Pinned because the demo
        // window's height is derived from measurements that include it.
        let now = DEMO_BASE_UNIX_SECS;
        let bare = lay_out(|ui| render_sparkline(ui, &[])).height;
        let drawn = lay_out(|ui| render_sparkline(ui, &[(now - 600, 0.4), (now, 0.5)])).height;
        assert!(
            drawn > bare + SPARK_HEIGHT,
            "the strip drew {}px of extra height, which is the strip alone — \
             the {SPARK_TAG} tag row is missing",
            drawn - bare
        );
    }

    #[test]
    fn the_demo_draws_a_strip_for_every_pane() {
        // The review path: `--pace-demo` must show the feature, not an empty
        // space where it would be.
        for pane in demo_test_panes() {
            let snapshot = pane.latest_snapshot.as_ref().expect("a demo snapshot");
            let label = representative_window(snapshot)
                .expect("a representative window")
                .label
                .clone();
            let series = spark_series(&pane.history, &label);
            let now = series.iter().map(|(at, _)| *at).max().expect("readings");
            assert!(
                spark_window(&series, now).len() >= 2,
                "{label} had {} readings in the day",
                spark_window(&series, now).len()
            );
        }
    }

    // --- M13: quota alerts ---

    fn alert_config(alert_at: u8, alert_mode: AlertMode) -> Config {
        Config {
            alerts: true,
            alert_at,
            alert_mode,
            ..Config::default()
        }
    }

    /// A headline window at `used`, `elapsed` of the way through its 5 h span.
    fn alerting_window(label: &str, used: f64, elapsed: f64) -> QuotaWindow {
        let duration = 18_000u64;
        QuotaWindow {
            label: label.to_string(),
            used_fraction: Some(used),
            resets_in_secs: Some(((1.0 - elapsed) * duration as f64) as u64),
            duration_secs: Some(duration),
        }
    }

    fn alerting_snapshot(provider: ProviderId, windows: Vec<QuotaWindow>) -> ProviderSnapshot {
        ProviderSnapshot {
            provider,
            taken_at_unix_secs: DEMO_BASE_UNIX_SECS,
            windows,
            per_model: Vec::new(),
            reset_credits: None,
            source: usage_core::model::SnapshotSource::UsageEndpoint,
        }
    }

    #[test]
    fn alerts_off_is_silent_no_matter_the_numbers() {
        let off = Config::default();
        assert!(!off.alerts, "alerts must be opt-in");
        for used in [0.0, 0.5, 0.8, 1.0] {
            for firing in [false, true] {
                assert_eq!(
                    alert_action(&off, Some(used), Some(0.0), firing),
                    AlertAction::Quiet,
                    "used={used} firing={firing}"
                );
            }
        }
    }

    #[test]
    fn an_unknown_or_impossible_reading_says_nothing() {
        let config = alert_config(80, AlertMode::Threshold);
        for used in [None, Some(f64::NAN), Some(f64::INFINITY)] {
            assert_eq!(
                alert_action(&config, used, Some(0.0), false),
                AlertAction::Quiet,
                "{used:?} is not a reading to judge"
            );
        }
    }

    #[test]
    fn threshold_mode_fires_on_every_crossing_regardless_of_the_clock() {
        let config = alert_config(80, AlertMode::Threshold);
        // Exactly at the threshold counts as crossing it.
        assert_eq!(
            alert_action(&config, Some(0.80), Some(0.99), false),
            AlertAction::Fire,
            "80% of an almost-elapsed window still fires in threshold mode"
        );
        assert_eq!(
            alert_action(&config, Some(0.79), Some(0.0), false),
            AlertAction::Quiet
        );
    }

    #[test]
    fn pace_mode_only_speaks_when_spending_beats_the_clock() {
        let config = alert_config(80, AlertMode::Pace);
        // 85% spent one day into a week: over the threshold, and burning hard.
        assert_eq!(
            alert_action(&config, Some(0.85), Some(0.14), false),
            AlertAction::Fire
        );
        // 85% spent six days into a week: over the threshold, but on budget.
        assert_eq!(
            alert_action(&config, Some(0.85), Some(0.86), false),
            AlertAction::Quiet,
            "a window spent slower than it elapses is not news"
        );
        // Dead level is not ahead of the clock.
        assert_eq!(
            alert_action(&config, Some(0.85), Some(0.85), false),
            AlertAction::Quiet
        );
    }

    #[test]
    fn an_unknown_duration_falls_back_to_threshold_semantics() {
        // Fail-safe: with no clock to compare against, an alert that might not
        // have been needed beats silence about a quota that is nearly gone.
        let config = alert_config(80, AlertMode::Pace);
        assert_eq!(
            alert_action(&config, Some(0.9), None, false),
            AlertAction::Fire
        );
        // And that is exactly what a window with no stated duration produces.
        let window = QuotaWindow {
            label: "5h".to_string(),
            used_fraction: Some(0.9),
            resets_in_secs: Some(3_600),
            duration_secs: None,
        };
        assert_eq!(elapsed_fraction(&window), None);
    }

    #[test]
    fn an_alert_fires_once_per_crossing_and_re_arms_on_refill() {
        let config = alert_config(80, AlertMode::Threshold);
        // First crossing speaks.
        assert_eq!(
            alert_action(&config, Some(0.9), None, false),
            AlertAction::Fire
        );
        // Still over: silent, however many polls land.
        assert_eq!(
            alert_action(&config, Some(0.95), None, true),
            AlertAction::Quiet
        );
        assert_eq!(
            alert_action(&config, Some(1.0), None, true),
            AlertAction::Quiet
        );
        // Back under: the refill notice, and the alert is re-armed.
        assert_eq!(
            alert_action(&config, Some(0.1), None, true),
            AlertAction::Refill
        );
        // A window that never fired does not announce a refill it never lost.
        assert_eq!(
            alert_action(&config, Some(0.1), None, false),
            AlertAction::Quiet
        );
        // And the next crossing speaks again.
        assert_eq!(
            alert_action(&config, Some(0.9), None, false),
            AlertAction::Fire
        );
    }

    #[test]
    fn the_threshold_is_the_one_from_the_config() {
        for alert_at in [1u8, 50, 95, 100] {
            let config = alert_config(alert_at, AlertMode::Threshold);
            let just_under = (alert_at as f64 - 0.5) / 100.0;
            assert_eq!(
                alert_action(&config, Some(alert_at as f64 / 100.0), None, false),
                AlertAction::Fire,
                "alert_at={alert_at} should fire at exactly {alert_at}%"
            );
            assert_eq!(
                alert_action(&config, Some(just_under), None, false),
                AlertAction::Quiet,
                "alert_at={alert_at} should not fire just under it"
            );
        }
    }

    #[test]
    fn the_banner_and_refill_lines_are_byte_exact() {
        assert_eq!(
            alert_banner_line(
                ProviderId::CodexSubscription,
                "5h",
                0.823,
                80,
                AlertMode::Pace
            ),
            "alert: Codex 5h at 82% >= 80% (pace)"
        );
        assert_eq!(
            alert_banner_line(
                ProviderId::ClaudeSubscription,
                "7d",
                0.95,
                90,
                AlertMode::Threshold
            ),
            "alert: Claude 7d at 95% >= 90% (threshold)"
        );
        assert_eq!(
            alert_refill_line(ProviderId::CodexSubscription, "5h", 80),
            "refilled: Codex 5h back under 80%"
        );
        assert_eq!(
            alert_refill_line(ProviderId::ClaudeSubscription, "weekly", 42),
            "refilled: Claude weekly back under 42%"
        );
    }

    #[test]
    fn the_pane_banners_the_worst_offender_only() {
        let config = alert_config(80, AlertMode::Threshold);
        let mut pane = bare_pane(ProviderId::CodexSubscription);
        let snapshot = alerting_snapshot(
            ProviderId::CodexSubscription,
            vec![
                alerting_window("5h", 0.85, 0.1),
                alerting_window("7d", 0.97, 0.1),
                alerting_window("odd", 0.10, 0.1),
            ],
        );
        assert!(pane.update_alerts(&snapshot, &config), "a new alert");
        assert_eq!(pane.alerts_firing, vec!["5h".to_string(), "7d".to_string()]);
        assert_eq!(
            pane.alert_line.as_deref(),
            Some("alert: Codex 7d at 97% >= 80% (threshold)"),
            "two alerts must not both take a row"
        );
        assert_eq!(pane.refill_line, None);
    }

    #[test]
    fn only_a_new_crossing_asks_for_attention() {
        let config = alert_config(80, AlertMode::Threshold);
        let mut pane = bare_pane(ProviderId::ClaudeSubscription);
        let over = alerting_snapshot(
            ProviderId::ClaudeSubscription,
            vec![alerting_window("5h", 0.85, 0.1)],
        );
        assert!(pane.update_alerts(&over, &config), "the crossing");
        assert!(
            !pane.update_alerts(&over, &config),
            "still over is not a new crossing"
        );
        assert!(pane.alert_line.is_some(), "but the banner stays up");

        // The refill: the line appears, the banner clears, and nothing asks for
        // attention — good news does not steal focus.
        let under = alerting_snapshot(
            ProviderId::ClaudeSubscription,
            vec![alerting_window("5h", 0.05, 0.9)],
        );
        assert!(
            !pane.update_alerts(&under, &config),
            "a refill is not an alert"
        );
        assert_eq!(
            pane.refill_line.as_deref(),
            Some("refilled: Claude 5h back under 80%")
        );
        assert_eq!(pane.alert_line, None);
        assert!(pane.alerts_firing.is_empty(), "and the alert is re-armed");

        // The notice shows once: the next poll clears it.
        assert!(!pane.update_alerts(&under, &config));
        assert_eq!(pane.refill_line, None);
    }

    #[test]
    fn a_window_the_provider_stops_reporting_does_not_stay_armed() {
        let config = alert_config(80, AlertMode::Threshold);
        let mut pane = bare_pane(ProviderId::CodexSubscription);
        let with_window = alerting_snapshot(
            ProviderId::CodexSubscription,
            vec![alerting_window("5h", 0.85, 0.1)],
        );
        assert!(pane.update_alerts(&with_window, &config));

        // The provider drops the window entirely, then brings it back over the
        // threshold. Without pruning, the stale firing entry would suppress the
        // second crossing forever.
        let without = alerting_snapshot(ProviderId::CodexSubscription, vec![]);
        assert!(!pane.update_alerts(&without, &config));
        assert!(pane.alerts_firing.is_empty());
        assert_eq!(pane.alert_line, None);
        assert!(
            pane.update_alerts(&with_window, &config),
            "the window coming back over the threshold is a fresh crossing"
        );
    }

    #[test]
    fn the_alert_lines_cost_a_row_only_when_there_is_one() {
        let snapshot = alerting_snapshot(
            ProviderId::CodexSubscription,
            vec![alerting_window("5h", 0.85, 0.1)],
        );
        let render = |alert: Option<&str>, refill: Option<&str>| {
            lay_out(|ui| {
                render_windows(
                    ui,
                    &snapshot,
                    None,
                    false,
                    Theme::CipherPine,
                    PaneLines {
                        alert,
                        refill,
                        ..Default::default()
                    },
                );
            })
            .height
        };
        let quiet = render(None, None);
        let banner = render(Some("alert: Codex 5h at 85% >= 80% (pace)"), None);
        let refill = render(None, Some("refilled: Codex 5h back under 80%"));
        assert!(
            banner > quiet,
            "the banner should cost a row: {quiet} -> {banner}"
        );
        assert!(refill > quiet, "so should the refill line");
        assert!(
            render(Some("alert: a"), Some("refilled: b")) > banner,
            "both at once cost both rows"
        );
    }

    #[test]
    fn the_tray_alert_variant_rings_the_icon_and_prefixes_the_tooltip() {
        assert_eq!(ALERT_TOOLTIP_PREFIX, "ALERT \u{2014} ");

        let calm = icon::render_icon_with_alert(Some(0.9), Some(0.2), ICON_SIZE, false);
        let alarmed = icon::render_icon_with_alert(Some(0.9), Some(0.2), ICON_SIZE, true);
        assert_eq!(
            calm,
            icon::render_icon(Some(0.9), Some(0.2), ICON_SIZE),
            "the calm variant must be byte-identical to the accepted mark"
        );
        assert_ne!(calm, alarmed, "the alert variant must look different");

        let n = ICON_SIZE as usize;
        let px = |buf: &[u8], x: usize, y: usize| {
            let i = (y * n + x) * 4;
            [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
        };
        // The top edge, mid-span: ring in the alert variant, tile otherwise.
        assert_eq!(px(&alarmed, n / 2, 0), CARDINAL.to_array());
        assert_eq!(px(&calm, n / 2, 0), GROUND.to_array());
        // One pixel in is already past the ring — it is a hairline, not a border.
        assert_eq!(px(&alarmed, n / 2, 1), GROUND.to_array());
        // The corners stay transparent: the ring follows the rounded tile.
        assert_eq!(px(&alarmed, 0, 0), [0, 0, 0, 0]);
        // And the mark itself is untouched — same bars, same dots.
        for (x, y) in [(n / 2, n / 2), (8, 16), (8, 21)] {
            assert_eq!(px(&alarmed, x, y), px(&calm, x, y), "pixel ({x},{y}) moved");
        }
    }

    #[test]
    fn folding_an_update_reports_the_new_alert_to_its_caller() {
        // The channel-side wiring: a snapshot arriving from the poller has to
        // carry the "raise attention" answer back out of the pane, and this is
        // the only place that hop is asserted — `drain` itself needs a live
        // poller thread, which a unit test must not spawn.
        let config = alert_config(80, AlertMode::Threshold);
        let mut pane = bare_pane(ProviderId::CodexSubscription);
        let over = alerting_snapshot(
            ProviderId::CodexSubscription,
            vec![alerting_window("5h", 0.85, 0.1)],
        );

        assert!(
            pane.apply_update(Update::Snapshot(over.clone()), &config),
            "the crossing must reach the caller"
        );
        assert!(pane.latest_snapshot.is_some(), "and the snapshot must land");
        assert!(
            !pane.apply_update(Update::Snapshot(over), &config),
            "a repeat is not a new crossing"
        );

        // A failure is never an alert, and never clears the last snapshot.
        assert!(!pane.apply_update(
            Update::Failure {
                provider: ProviderId::CodexSubscription,
                message: "boom".to_string(),
            },
            &config
        ));
        assert_eq!(pane.latest_failure.as_deref(), Some("boom"));
        assert!(
            pane.alert_line.is_some(),
            "the banner survives a failed poll"
        );
    }

    #[test]
    fn the_demo_raises_exactly_one_alert() {
        // `--pace-demo` must show the alert surfaces without real data, and it
        // does it through the ordinary rules rather than a switch: the Codex 5h
        // window is 80% spent at 75% elapsed, so it is the only one both over
        // the threshold and ahead of its clock.
        let panes = demo_test_panes();
        let banners: Vec<&str> = panes
            .iter()
            .filter_map(|pane| pane.alert_line.as_deref())
            .collect();
        assert_eq!(banners, vec!["alert: Codex 5h at 80% >= 80% (pace)"]);
        assert!(
            panes.iter().all(|pane| pane.refill_line.is_none()),
            "nothing refilled in the scenario"
        );
    }

    #[test]
    fn the_demo_forces_the_alert_settings_but_leaves_the_look_alone() {
        // A reviewer whose own config says alert_at=95, alerts=off must still
        // see the feature — and must still see their own theme.
        let theirs = Config {
            theme: Theme::Plain,
            height: 612,
            history: true,
            alerts: false,
            alert_at: 95,
            alert_mode: AlertMode::Threshold,
        };
        let forced = demo_config(theirs);
        assert!(forced.alerts);
        assert_eq!(forced.alert_at, DEFAULT_ALERT_AT);
        assert_eq!(forced.alert_mode, AlertMode::Pace);
        assert_eq!(forced.theme, Theme::Plain, "the look is the reviewer's");
        assert!(forced.history, "and so is everything else");
        assert_eq!(
            forced.height, 612,
            "demo_config carries the height through untouched — the demo's own \
             window comes from initial_inner_height, not from the config"
        );
    }

    // --- M8: --pace-demo ---

    #[test]
    fn pace_demo_flag_defaults_off_and_is_recognized_anywhere() {
        assert!(!parse_args(args(&[])).unwrap().pace_demo);
        assert!(parse_args(args(&["--pace-demo"])).unwrap().pace_demo);
        let parsed = parse_args(args(&["--plain", "--pace-demo", "--no-tray"])).unwrap();
        assert!(parsed.pace_demo);
        assert!(parsed.no_tray);
        assert_eq!(parsed.theme_override, Some(Theme::Plain));
    }

    #[test]
    fn demo_mode_names_itself_in_the_window_title() {
        assert_eq!(window_title(false), "QuotaPane");
        assert!(window_title(true).contains("demo"));
    }

    #[test]
    fn demo_panes_own_no_poller_and_therefore_no_egress() {
        // The structural half of "no network in demo mode": a pane with no
        // handle has no poller thread, and `ProviderPane::new` — the only place
        // an `Egress` is constructed — was never called.
        for pane in demo_test_panes() {
            assert!(pane.handle.is_none(), "a demo pane must not own a poller");
            assert!(pane.startup_error.is_none());
            assert!(pane.latest_failure.is_none());
            assert!(pane.latest_snapshot.is_some(), "a demo pane must have data");
        }
    }

    #[test]
    fn the_demo_scenario_is_deterministic() {
        // Two runs must be identical, or a screenshot means nothing and a
        // regression hides in the noise. Nothing in the scenario reads a clock:
        // the observation times come from the fixed base.
        let first = demo_test_panes();
        let second = demo_test_panes();
        assert_eq!(first.len(), second.len());
        for (a, b) in first.iter().zip(&second) {
            assert_eq!(a.id, b.id);
            assert_eq!(a.latest_snapshot, b.latest_snapshot);
            assert_eq!(a.pace_warning, b.pace_warning);
        }

        let final_step = DEMO_BASE_UNIX_SECS + (DEMO_STEPS - 1) * DEMO_STEP_SECS;
        for pane in &first {
            assert_eq!(
                pane.latest_snapshot
                    .as_ref()
                    .expect("a snapshot")
                    .taken_at_unix_secs,
                final_step
            );
        }
    }

    #[test]
    fn the_demo_scenario_exercises_both_warning_colours() {
        // The point of the flag: one pane amber, one cardinal — reviewable in
        // one glance instead of after a day of real usage.
        let panes = demo_test_panes();
        let claude = &panes[0];
        let codex = &panes[1];

        // Claude: the weekly window fills in ~9 h, before its 3.5-day reset.
        // Beyond the six-hour threshold, so AMBER. The 5h window is *not* the
        // warning even though it is the shorter window — it is under budget.
        let amber = claude.pace_warning.as_ref().expect("Claude must warn");
        assert_eq!(amber.label, "7d");
        assert!(
            (amber.exhaust_in_secs as i64 - 32_400).abs() <= 2,
            "expected ~32400s, got {}",
            amber.exhaust_in_secs
        );
        assert_eq!(pace_warning_color(amber.exhaust_in_secs), AMBER);
        assert_eq!(
            pace_warning_line(amber),
            "at this pace: 7d full in ~9h 0m",
            "the amber line the owner reviews"
        );

        // Codex: the session window fills in ~1 h against a 1h15m reset. Inside
        // six hours, so CARDINAL. Its weekly window is flat, and silent.
        let cardinal = codex.pace_warning.as_ref().expect("Codex must warn");
        assert_eq!(cardinal.label, "5h");
        assert!(
            (cardinal.exhaust_in_secs as i64 - 3_600).abs() <= 2,
            "expected ~3600s, got {}",
            cardinal.exhaust_in_secs
        );
        assert_eq!(pace_warning_color(cardinal.exhaust_in_secs), CARDINAL);
        assert_eq!(
            pace_warning_line(cardinal),
            "at this pace: 5h full in ~1h 0m",
            "the cardinal line the owner reviews"
        );
    }

    #[test]
    fn the_demo_scenario_puts_a_tick_on_every_bar() {
        // Ticks are the other half of what the flag exists to show, so every
        // demo bar must earn one — and at spread-out positions, not bunched at
        // one end.
        let panes = demo_test_panes();
        let mut offsets = Vec::new();
        for pane in &panes {
            let snapshot = pane.latest_snapshot.as_ref().expect("a snapshot");
            for window in snapshot.windows.iter().chain(&snapshot.per_model) {
                let offset = pace_tick_x(window, BAR_WIDTH)
                    .unwrap_or_else(|| panic!("no tick for {}", window.label));
                offsets.push(offset);
            }
        }
        // Headline ticks at 20%, 50%, 75% and 90% of the bar, plus the
        // per-model rows.
        assert!(offsets.len() >= 4);
        let min = offsets.iter().cloned().fold(f32::MAX, f32::min);
        let max = offsets.iter().cloned().fold(f32::MIN, f32::max);
        assert!(
            min < BAR_WIDTH * 0.3 && max > BAR_WIDTH * 0.7,
            "tick positions are bunched: {offsets:?}"
        );
    }

    /// What the demo's window has to be, per theme, for the whole scenario to
    /// render unscrolled: the body as the harness measures it, plus the chrome
    /// the body sits inside.
    ///
    /// The body is measured through `render_panes` — the same function
    /// `App::ui` calls — so the separator between the two panes and the spacing
    /// around it are in the number rather than estimated around it. Measured at
    /// [`DEMO_WINDOW_HEIGHT`] rather than [`WINDOW_HEIGHT`], because that is
    /// the window the demo actually opens.
    fn demo_window_height_needed(theme: Theme) -> f32 {
        let mut panes = demo_test_panes();
        for pane in &mut panes {
            // The state `--pace-demo` launches in. Expanding a disclosure is a
            // click, and the ScrollArea remains the escape hatch for it (the
            // expanded-state cutoff is accepted and queued post-1.0).
            pane.expanded = false;
        }
        let laid = lay_out_sized(theme, DEMO_WINDOW_HEIGHT, |ui| {
            render_panes(ui, &mut panes, theme)
        });
        assert!(
            laid.width <= laid.available_width,
            "{theme:?} demo body wanted {}px inside {}px",
            laid.width,
            laid.available_width
        );
        // Through the shipped snap function, not a retyped sum (M14): the
        // demo's constant and the grip's double-click are then answering the
        // same question with the same arithmetic, and a change to one is a
        // failure in the other.
        snapped_height(laid.height, laid.panel_chrome_height)
    }

    #[test]
    fn the_demo_scenario_fits_the_window() {
        // This *is* the review path: if the demo clips, the owner reviews a
        // clipped window and learns nothing about the feature. Through M13 this
        // test *recorded* an overflow — the demo wanted more than the 240px
        // window and every row was merely reachable by scrolling. M13-R1 gives
        // the demo its own taller window instead, so the claim becomes an
        // equality: DEMO_WINDOW_HEIGHT is what the scenario measures, not a
        // number someone liked.
        //
        // Both directions are asserted. Under-size and the owner reviews
        // through a scroll bar again; over-size by more than a row and the
        // constant has stopped being derived from anything.
        let one_row = 24.0; // a bar row with its spacing
        for theme in [Theme::CipherPine, Theme::Plain] {
            let needed = demo_window_height_needed(theme);
            assert!(
                DEMO_WINDOW_HEIGHT >= needed,
                "{theme:?} demo needs {needed}px of window and --pace-demo asks \
                 for {DEMO_WINDOW_HEIGHT}px — the scenario would open scrolled"
            );
            assert!(
                DEMO_WINDOW_HEIGHT <= needed + one_row,
                "{theme:?} demo needs {needed}px but --pace-demo asks for \
                 {DEMO_WINDOW_HEIGHT}px — more than a row of window nothing fills"
            );
        }
    }

    #[test]
    fn only_the_demo_gets_the_taller_window() {
        // The production default is untouched by M13-R1, and that is the whole
        // safety of point 4: every visual acceptance to date was given against
        // a 320x240 window, and a synthetic review scenario does not get to
        // resize the app real subscribers run.
        assert_eq!(
            initial_inner_height(false, config::DEFAULT_HEIGHT),
            WINDOW_HEIGHT
        );
        assert_eq!(WINDOW_HEIGHT, 240.0);
        assert_eq!(
            initial_inner_height(true, config::DEFAULT_HEIGHT),
            DEMO_WINDOW_HEIGHT
        );
        // The demo window exists to be taller than the default one. A const
        // block, because both sides are consts and a runtime `assert!` over
        // them is a compile-time fact deferred.
        const { assert!(DEMO_WINDOW_HEIGHT > WINDOW_HEIGHT) };
    }

    // --- M14 Phase 1: the height is the user's ---

    #[test]
    fn a_saved_height_is_what_the_window_opens_at() {
        for saved in [config::MIN_HEIGHT, 240, 612, config::MAX_HEIGHT] {
            assert_eq!(
                initial_inner_height(false, saved),
                saved as f32,
                "a stored height={saved} must be the height asked for"
            );
        }
    }

    #[test]
    fn the_demo_ignores_the_saved_height_entirely() {
        // The demo is a fixture, not a session: whatever the user's window is,
        // `--pace-demo` opens the scenario's own window.
        for saved in [config::MIN_HEIGHT, 240, 612, config::MAX_HEIGHT] {
            assert_eq!(initial_inner_height(true, saved), DEMO_WINDOW_HEIGHT);
        }
    }

    #[test]
    fn the_opening_height_is_clamped_to_the_viewport_bounds() {
        // `config::height` rejects out-of-range values already; this is the
        // second lock, so no path can ask the OS for a size the viewport's own
        // min/max would refuse.
        assert_eq!(initial_inner_height(false, 0), MIN_WINDOW_HEIGHT);
        assert_eq!(initial_inner_height(false, 1), MIN_WINDOW_HEIGHT);
        assert_eq!(initial_inner_height(false, u32::MAX), MAX_WINDOW_HEIGHT);
    }

    #[test]
    fn snapping_adds_exactly_the_chrome_the_content_does_not_get() {
        // Titlebar above, grip below, panel margins around — and nothing else.
        assert_eq!(
            snapped_height(300.0, 16.0),
            TITLEBAR_HEIGHT + 16.0 + 300.0 + GRIP_HEIGHT
        );
        // A boundary mutant that drops the grip from the sum lands here.
        assert_eq!(snapped_height(300.0, 16.0), 348.0);
    }

    #[test]
    fn snapping_never_goes_under_the_minimum() {
        // An empty window (no provider signed in, nothing to draw) must not
        // snap to a sliver with no grip left to grab.
        assert_eq!(snapped_height(0.0, 0.0), MIN_WINDOW_HEIGHT);
        assert_eq!(snapped_height(1.0, 1.0), MIN_WINDOW_HEIGHT);
        // One pixel of content over the floor and the clamp lets go.
        let just_over = MIN_WINDOW_HEIGHT - TITLEBAR_HEIGHT - GRIP_HEIGHT + 1.0;
        assert_eq!(snapped_height(just_over, 0.0), MIN_WINDOW_HEIGHT + 1.0);
    }

    #[test]
    fn the_snap_command_can_only_name_the_fixed_width() {
        for (content, chrome) in [(0.0, 0.0), (300.0, 16.0), (5000.0, 16.0)] {
            match snap_command(content, chrome) {
                egui::ViewportCommand::InnerSize(size) => {
                    assert_eq!(
                        size.x, WINDOW_WIDTH,
                        "the snap must never move the width, content={content}"
                    );
                    assert_eq!(size.y, snapped_height(content, chrome));
                }
                other => panic!("snap must be an InnerSize command, got {other:?}"),
            }
        }
    }

    /// The width claim, made against the source rather than a value: a runtime
    /// test can only check the paths it calls, and the claim is that **no**
    /// path exists that names another width. Same technique, and same reason,
    /// as `the_window_exposes_no_proxy_opt_in`.
    #[test]
    fn no_resize_path_can_emit_a_width_other_than_the_fixed_one() {
        const SRC: &str = include_str!("main.rs");
        let code: String = SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");

        // Exactly one place names an inner size at runtime, and it is
        // `snap_command`.
        assert_eq!(
            code.matches("ViewportCommand::InnerSize").count(),
            1,
            "exactly one runtime inner-size command in the window"
        );
        // Every viewport size the window declares carries WINDOW_WIDTH.
        for call in [
            "with_inner_size([WINDOW_WIDTH,",
            "with_min_inner_size([WINDOW_WIDTH,",
            "with_max_inner_size([WINDOW_WIDTH,",
        ] {
            assert!(
                code.contains(call),
                "the viewport must declare `{call}` — the width is pinned by \
                 the builder, not by arithmetic"
            );
        }
        // And nothing may re-enable a free-floating window by dropping the
        // width bounds again.
        assert!(
            !code.contains("with_resizable(false)"),
            "M14 makes the window resizable; the width is pinned by min/max \
             inner size instead"
        );
    }

    /// The grip's mark has to be a mark, not a replacement box.
    ///
    /// Measured, because the answer is not obvious and it is not the same in
    /// both bundled fonts: Hack (the monospace family) has U+259E, and
    /// Ubuntu-Light (the proportional family egui's Plain default uses) does
    /// not — nor do egui's emoji fallbacks. That is exactly why `render_grip`
    /// paints with an explicit monospace `FontId` in **both** themes rather
    /// than inheriting the style's family, and this test is what keeps that
    /// from being quietly undone.
    ///
    /// `Fonts::has_glyph` cannot answer this: for the monospace family its
    /// "is this the replacement face" check compares Hack against itself and
    /// reports `false` for every character, ASCII included. So the comparison
    /// is made where the truth actually shows up — the atlas rectangle a laid
    /// out glyph points at. A character no face owns is drawn as the
    /// replacement glyph, so it lands on the replacement's rectangle; a
    /// character Hack owns lands somewhere else.
    #[test]
    fn the_grip_glyph_is_a_glyph_and_not_a_replacement_box() {
        let ctx = egui::Context::default();
        install_theme(&ctx, Theme::CipherPine);
        // Fonts do not exist until a frame has run.
        let _ = ctx.run_ui(egui::RawInput::default(), |_| {});

        let font = egui::FontId::monospace(GRIP_HEIGHT);
        let uv_rect = |c: char| {
            let galley = ctx.fonts_mut(|fonts| {
                fonts.layout_no_wrap(c.to_string(), font.clone(), egui::Color32::WHITE)
            });
            galley.rows[0].glyphs[0].uv_rect
        };

        // U+E000 is Private Use: no bundled font can claim it, so whatever it
        // renders as *is* the replacement box.
        let replacement = uv_rect('\u{E000}');
        assert_ne!(
            uv_rect(GRIP_GLYPH),
            replacement,
            "{GRIP_GLYPH:?} is not in the monospace family — the grip would \
             draw a replacement box"
        );
        // The probe itself has to be honest: a character that *is* present
        // must not compare equal to the replacement either way round.
        assert_ne!(uv_rect('='), replacement, "the probe is not discriminating");
    }

    /// The grip's mark is painted in the monospace family whatever the theme,
    /// because the proportional one does not have it (see the test above).
    #[test]
    fn the_grip_paints_its_mark_in_the_monospace_family() {
        const SRC: &str = include_str!("main.rs");
        let code: String = SRC[..SRC
            .find("#[cfg(test)]")
            .expect("test module marker not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        let grip = code
            .split("fn render_grip")
            .nth(1)
            .expect("render_grip not found");
        assert!(
            grip.contains("FontId::monospace("),
            "the grip must name the monospace family explicitly — Plain's \
             proportional default has no U+259E to draw"
        );
    }

    #[test]
    fn the_grip_is_carved_out_of_the_window_not_out_of_the_content() {
        // Chrome, like the titlebar: the height it costs is the height the
        // snap adds back, so a window snapped to fit shows every row *and* a
        // grip to grab.
        const { assert!(GRIP_HEIGHT > 0.0) };
        // Well clear of the MIN_WINDOW_HEIGHT clamp, which would otherwise
        // flatten both sides of the subtraction to the floor.
        let content = 300.0;
        assert_eq!(
            snapped_height(content, 0.0) - snapped_height(content - GRIP_HEIGHT, 0.0),
            GRIP_HEIGHT,
            "the grip's strip must be part of the window, not part of the body"
        );
    }

    #[test]
    fn a_height_is_written_only_once_the_user_stops_dragging() {
        let settled = HEIGHT_SETTLE;
        let mid_drag = HEIGHT_SETTLE - Duration::from_millis(1);

        // Sitting at the stored height: nothing to do, ever.
        assert_eq!(height_action(240, 240, None), HeightAction::Nothing);
        assert_eq!(
            height_action(240, 240, Some((240, settled))),
            HeightAction::Nothing
        );

        // A height seen for the first time is never written on sight.
        assert_eq!(height_action(240, 320, None), HeightAction::Wait);

        // Still moving: each new size restarts the clock.
        assert_eq!(
            height_action(240, 320, Some((300, settled))),
            HeightAction::Wait,
            "a different pending height must not hand its settled clock over"
        );

        // Held, but not long enough.
        assert_eq!(
            height_action(240, 320, Some((320, mid_drag))),
            HeightAction::Wait
        );

        // Held long enough — exactly on the boundary, and past it.
        assert_eq!(
            height_action(240, 320, Some((320, settled))),
            HeightAction::Write
        );
        assert_eq!(
            height_action(240, 320, Some((320, settled * 4))),
            HeightAction::Write
        );
    }

    // --- M7a2: the reset-credits line ---

    /// The M7a2 snapshot: one headline window, no per-model rows, and the
    /// given reset credits — so these layouts differ only in that field.
    fn credits_snapshot(reset_credits: Option<ResetCredits>) -> ProviderSnapshot {
        ProviderSnapshot {
            reset_credits,
            ..per_model_snapshot(vec![])
        }
    }

    // --- M14 Phase 2: the freshness dot ---

    /// A poll at 09:14:22 UTC, chosen so every field of the clock is distinct
    /// and none is zero — a formatter that transposed two of them, or dropped
    /// a zero pad, has nowhere to hide.
    const POLL_UNIX: u64 = 1_754_471_662;

    #[test]
    fn aging_comes_before_stale() {
        const { assert!(AGING_AFTER.as_secs() < STALE_AFTER.as_secs()) };
        // And the band between them is not a rounding artefact.
        assert!(STALE_AFTER - AGING_AFTER >= Duration::from_secs(60));
    }

    #[test]
    fn the_dot_goes_green_amber_cardinal_on_the_two_thresholds() {
        let second = Duration::from_secs(1);
        // Every case named against the consts, never against 300 or 600: the
        // thresholds are allowed to move, the *table* is not.
        assert_eq!(freshness_color(Duration::ZERO), OPER_GREEN);
        assert_eq!(freshness_color(AGING_AFTER - second), OPER_GREEN);
        assert_eq!(freshness_color(AGING_AFTER), AMBER);
        assert_eq!(freshness_color(STALE_AFTER - second), AMBER);
        assert_eq!(freshness_color(STALE_AFTER), CARDINAL);
        assert_eq!(freshness_color(STALE_AFTER + second), CARDINAL);
        // Far past stale stays stale rather than wrapping into another state.
        assert_eq!(freshness_color(STALE_AFTER * 100), CARDINAL);
    }

    #[test]
    fn the_dots_cardinal_is_exactly_the_stale_predicate() {
        // M10 welded the pace-line mute to `is_stale`. The dot must agree with
        // it at every age, or the pane would show a green dot beside a muted
        // forecast — two surfaces disagreeing about the same fact.
        for age in [
            Duration::ZERO,
            AGING_AFTER,
            STALE_AFTER - Duration::from_secs(1),
            STALE_AFTER,
            STALE_AFTER + Duration::from_secs(1),
        ] {
            assert_eq!(
                freshness_color(age) == CARDINAL,
                is_stale(age),
                "the dot and `is_stale` disagree at {age:?}"
            );
        }
    }

    #[test]
    fn the_tooltip_is_byte_exact_fresh_and_stale() {
        // Fresh: age straight from `format_age`, then the wall clock.
        assert_eq!(
            freshness_tooltip(Freshness {
                age: Duration::from_secs(5),
                polled_at_unix: POLL_UNIX,
            }),
            "updated 5s ago at 09:14:22 UTC"
        );
        // Aging is not stale — the amber dot changes no bytes here.
        assert_eq!(
            freshness_tooltip(Freshness {
                age: AGING_AFTER,
                polled_at_unix: POLL_UNIX,
            }),
            "updated 5m ago at 09:14:22 UTC"
        );
        // Stale appends the marker, em dash and all, after the clock.
        assert_eq!(
            freshness_tooltip(Freshness {
                age: STALE_AFTER,
                polled_at_unix: POLL_UNIX,
            }),
            "updated 10m ago at 09:14:22 UTC — stale"
        );
        // The clock is zero-padded on every field, and midnight is 00:00:00
        // rather than 24:00:00.
        assert_eq!(
            freshness_tooltip(Freshness {
                age: Duration::from_secs(1),
                polled_at_unix: 1_754_438_401,
            }),
            "updated 1s ago at 00:00:01 UTC"
        );
        // And the last second of the day is 23:59:59, not a rollover.
        assert_eq!(
            freshness_tooltip(Freshness {
                age: Duration::from_secs(1),
                polled_at_unix: 1_754_438_399,
            }),
            "updated 1s ago at 23:59:59 UTC"
        );
    }

    #[test]
    fn the_tooltips_age_is_format_ages_output_unchanged() {
        // No second opinion about how an age reads: the tooltip embeds the
        // same string the pane has printed since M2.
        for secs in [0, 1, 59, 60, 61, 600, 3_600, 86_399] {
            let text = freshness_tooltip(Freshness {
                age: Duration::from_secs(secs),
                polled_at_unix: POLL_UNIX,
            });
            assert!(
                text.starts_with(&format!("updated {} at ", format_age(secs))),
                "{text:?} does not open with format_age({secs})"
            );
        }
    }

    #[test]
    fn the_poll_clock_counts_back_from_the_age() {
        // `freshness_at_now` is the only impure part of the dot, and all it
        // does is subtract: a poll N seconds ago happened N seconds before now.
        let now = now_unix_secs();
        let fresh = freshness_at_now(Duration::from_secs(30));
        assert!(
            (now.saturating_sub(30)..=now_unix_secs().saturating_sub(30))
                .contains(&fresh.polled_at_unix),
            "polled_at_unix {} is not 30s behind now",
            fresh.polled_at_unix
        );
        assert_eq!(fresh.age, Duration::from_secs(30));
        // An age longer than the epoch saturates rather than panicking.
        assert_eq!(
            freshness_at_now(Duration::from_secs(u64::MAX)).polled_at_unix,
            0
        );
    }

    /// A pane with one headline window and a known age, laid out whole.
    fn dot_pane(age: Option<Duration>) -> ProviderPane {
        let mut pane = bare_pane(ProviderId::ClaudeSubscription);
        pane.latest_snapshot = Some(per_model_snapshot(vec![]));
        pane.snapshot_received_at = age.map(|age| Instant::now() - age);
        pane
    }

    /// Every string a render actually painted.
    fn painted_text(theme: Theme, mut add_contents: impl FnMut(&mut egui::Ui)) -> Vec<String> {
        painted_shapes_themed(theme, &mut add_contents)
            .into_iter()
            .filter_map(|shape| match shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_pane_prints_no_age_row_at_any_age_in_either_theme() {
        // The density win, asserted where it happens: the words are gone from
        // the pane entirely, not merely moved down or shortened.
        for theme in [Theme::CipherPine, Theme::Plain] {
            for age in [Duration::ZERO, AGING_AFTER, STALE_AFTER, STALE_AFTER * 10] {
                let mut panes = vec![dot_pane(Some(age))];
                let painted = painted_text(theme, |ui| render_panes(ui, &mut panes, theme));
                assert!(
                    painted.iter().all(|text| !text.contains("updated")),
                    "{theme:?} at {age:?} still prints an age row: {painted:?}"
                );
                assert!(
                    painted.iter().all(|text| !text.contains("stale")),
                    "{theme:?} at {age:?} still prints a stale word: {painted:?}"
                );
            }
        }
    }

    #[test]
    fn the_header_carries_the_dot_at_every_age_in_either_theme() {
        for theme in [Theme::CipherPine, Theme::Plain] {
            for (age, expected) in [
                (Duration::ZERO, OPER_GREEN),
                (AGING_AFTER - Duration::from_secs(1), OPER_GREEN),
                (AGING_AFTER, AMBER),
                (STALE_AFTER - Duration::from_secs(1), AMBER),
                (STALE_AFTER, CARDINAL),
                (STALE_AFTER + Duration::from_secs(1), CARDINAL),
            ] {
                let mut panes = vec![dot_pane(Some(age))];
                let dots: Vec<egui::Color32> =
                    painted_shapes_themed(theme, |ui| render_panes(ui, &mut panes, theme))
                        .into_iter()
                        .filter_map(|shape| match shape {
                            // The colour a `RichText` sets lives in the
                            // galley's own section format; `fallback_color` is
                            // only what an uncoloured run would have used.
                            egui::Shape::Text(text) if text.galley.text() == FRESHNESS_DOT => {
                                Some(text.galley.job.sections[0].format.color)
                            }
                            _ => None,
                        })
                        .collect();
                assert_eq!(
                    dots,
                    vec![expected],
                    "{theme:?} at {age:?} painted {dots:?} dots"
                );
            }
        }
    }

    #[test]
    fn a_pane_with_no_poll_yet_has_no_dot() {
        // Nothing has been measured, so there is nothing to report the age of.
        // The header then renders exactly as it did before M14.
        for theme in [Theme::CipherPine, Theme::Plain] {
            let mut panes = vec![dot_pane(None)];
            let painted = painted_text(theme, |ui| render_panes(ui, &mut panes, theme));
            assert!(
                !painted.iter().any(|text| text == FRESHNESS_DOT),
                "{theme:?} drew a freshness dot before the first poll: {painted:?}"
            );
        }
    }

    #[test]
    fn the_dot_costs_the_pane_no_row_at_all() {
        // The whole point: a dot on a row that already exists. A pane with a
        // dot must be exactly as tall as one without.
        for theme in [Theme::CipherPine, Theme::Plain] {
            let mut with = vec![dot_pane(Some(Duration::from_secs(5)))];
            let mut without = vec![dot_pane(None)];
            let a = lay_out_themed(theme, |ui| render_panes(ui, &mut with, theme));
            let b = lay_out_themed(theme, |ui| render_panes(ui, &mut without, theme));
            assert_eq!(
                a.height, b.height,
                "{theme:?}: the dot changed the pane's height"
            );
            assert!(
                a.width <= a.available_width,
                "{theme:?}: the header wanted {}px inside {}px",
                a.width,
                a.available_width
            );
        }
    }

    #[test]
    fn the_error_lines_are_untouched_by_the_dot() {
        // The dot reports data AGE, never an error. An expired token still
        // renders M10's exact copy, and it still renders in CARDINAL.
        for theme in [Theme::CipherPine, Theme::Plain] {
            let mut pane = dot_pane(Some(STALE_AFTER));
            pane.latest_failure = Some("OAuth token expired at 2026-08-06T09:14:22Z".to_string());
            let mut panes = vec![pane];
            let painted = painted_text(theme, |ui| render_panes(ui, &mut panes, theme));
            assert!(
                painted
                    .iter()
                    .any(|text| text == token_expired_line(ProviderId::ClaudeSubscription)),
                "{theme:?} lost the expired-token copy: {painted:?}"
            );
            assert!(
                painted.iter().any(|text| text == FRESHNESS_DOT),
                "{theme:?}: the dot still reports the age beside an error"
            );
        }
    }

    #[test]
    fn reset_credits_line_shows_the_owned_count() {
        // The evidence shape: owns 1, none applicable right now. The line
        // reports what the account *has*, not the transient applicable count.
        assert_eq!(
            reset_credits_line(&ResetCredits {
                available: 1,
                applicable_now: Some(0),
            }),
            "resets available: 1"
        );
        // A genuine zero still prints — it is a fact, unlike `None`.
        assert_eq!(
            reset_credits_line(&ResetCredits {
                available: 0,
                applicable_now: None,
            }),
            "resets available: 0"
        );
    }

    #[test]
    fn reset_credits_line_renders_for_codex_and_not_for_none() {
        // Present adds exactly one line; absent must lay out identically to a
        // pane that never had the field — that equality is what keeps the
        // Claude pane untouched.
        let with = lay_out_pane(
            &credits_snapshot(Some(ResetCredits {
                available: 1,
                applicable_now: Some(0),
            })),
            false,
        );
        let without = lay_out_pane(&credits_snapshot(None), false);

        assert!(
            with.height > without.height,
            "expected the credits line to add height; got {}px vs {}px",
            with.height,
            without.height
        );
        // Width must still fit the fixed, non-resizable window.
        assert!(
            with.width <= with.available_width,
            "credits line wanted {}px inside {}px",
            with.width,
            with.available_width
        );
    }

    #[test]
    fn reset_credits_line_fits_beside_the_per_model_disclosure() {
        // Both M7a2 features at once, expanded: the line and a visible
        // per-model row coexist without overflowing the window's width.
        let snapshot = ProviderSnapshot {
            reset_credits: Some(ResetCredits {
                available: 99,
                applicable_now: Some(99),
            }),
            ..per_model_snapshot(vec![model_window_at("GPT-5.3-Codex-Max", Some(0.42))])
        };
        let laid = lay_out_pane(&snapshot, true);
        assert!(
            laid.width <= laid.available_width,
            "combined pane wanted {}px inside {}px",
            laid.width,
            laid.available_width
        );
    }

    // --- per-model disclosure state (M5a) ---

    #[test]
    fn panes_start_collapsed() {
        // Collapsed by default, per pane. `None` skips spawning a poller.
        let pane = ProviderPane::new::<ClaudeSubscription>(ProviderId::ClaudeSubscription, None);
        assert!(!pane.expanded);
    }

    #[test]
    fn panes_expand_independently() {
        let mut claude =
            ProviderPane::new::<ClaudeSubscription>(ProviderId::ClaudeSubscription, None);
        let codex = ProviderPane::new::<CodexSubscription>(ProviderId::CodexSubscription, None);
        claude.expanded = true;
        // One pane's state is not the other's — no shared/global flag.
        assert!(claude.expanded);
        assert!(!codex.expanded);
    }

    // --- not_signed_in_line: quiet-line mapping ---

    #[test]
    fn not_signed_in_lines_name_the_right_cli() {
        assert!(not_signed_in_line(ProviderId::ClaudeSubscription).contains("claude"));
        let codex = not_signed_in_line(ProviderId::CodexSubscription);
        assert!(codex.contains("Codex"));
        assert!(codex.contains("codex login"));
    }

    // --- token_expired_line: the refresh instruction ---

    #[test]
    fn token_expired_lines_name_the_exact_command() {
        // Full equality, not `contains`: the whole point of this milestone is
        // the exact words, and a `contains` check would pass on a line that had
        // quietly lost the command or the "recovers on its own" half.
        assert_eq!(
            token_expired_line(ProviderId::ClaudeSubscription),
            "token expired — start any claude session (even `claude -p hi`) to refresh it. QuotaPane recovers on its own within ~3 min."
        );
        assert_eq!(
            token_expired_line(ProviderId::CodexSubscription),
            "token expired — run `codex login` to refresh it. QuotaPane recovers on its own within ~3 min."
        );
    }

    // --- is_absent_credentials: absent-credential detection ---

    #[test]
    fn absent_credentials_detected_from_not_found_message() {
        // The real message the loader → provider → poller chain produces for a
        // missing file (see credentials::CredentialError::NotFound Display).
        let msg = "credential error: credential file not found: C:\\Users\\x\\.codex\\auth.json";
        assert!(is_absent_credentials(msg));
    }

    #[test]
    fn genuine_failures_are_not_treated_as_absent_credentials() {
        // None of the other ProviderError Display strings contain "not found".
        for msg in [
            "egress error: egress denied: host \"chatgpt.com:8443\" is not on the allowlist",
            "OAuth token expired — refresh it in the provider's official CLI (start a `claude` session, or run `codex login`); QuotaPane retries automatically",
            "rate limited by provider; retry after 30s",
            "provider response could not be interpreted",
            "credential error: failed to read credential file /x: permission denied",
        ] {
            assert!(!is_absent_credentials(msg), "false positive on: {msg}");
        }
    }

    // --- classify_failure: per-provider state selection ---

    #[test]
    fn classify_failure_selects_the_right_treatment() {
        assert_eq!(classify_failure(None), FailureDisplay::NoFailure);
        assert_eq!(
            classify_failure(Some("credential error: credential file not found: /x")),
            FailureDisplay::NotSignedIn
        );
        assert_eq!(
            classify_failure(Some("provider response could not be interpreted")),
            FailureDisplay::Banner
        );
    }

    /// The load-bearing pin of this milestone: the core's real `TokenExpired`
    /// message, produced by `Display` rather than copied here, must land in the
    /// `TokenExpired` treatment. Rewording the core marker or the UI matcher
    /// without the other fails CI instead of silently degrading the pane to a
    /// raw error banner that explains nothing.
    #[test]
    fn the_core_expired_token_message_classifies_as_token_expired() {
        use usage_core::providers::ProviderError;

        assert_eq!(
            classify_failure(Some(&ProviderError::TokenExpired.to_string())),
            FailureDisplay::TokenExpired
        );
    }

    // --- format_percent ---

    #[test]
    fn format_percent_known_fraction() {
        assert_eq!(format_percent(Some(0.42)), "42%");
    }

    #[test]
    fn format_percent_unknown_is_double_dash() {
        assert_eq!(format_percent(None), "--");
    }

    #[test]
    fn format_percent_zero() {
        assert_eq!(format_percent(Some(0.0)), "0%");
    }

    #[test]
    fn format_percent_clamps_above_one_to_100() {
        assert_eq!(format_percent(Some(1.5)), "100%");
    }

    #[test]
    fn format_percent_clamps_below_zero_to_0() {
        assert_eq!(format_percent(Some(-0.2)), "0%");
    }

    #[test]
    fn format_percent_rounds_to_nearest_whole() {
        assert_eq!(format_percent(Some(0.426)), "43%");
        assert_eq!(format_percent(Some(0.424)), "42%");
    }

    // --- format_reset ---

    #[test]
    fn format_reset_unknown_is_double_dash() {
        assert_eq!(format_reset(None), "--");
    }

    #[test]
    fn format_reset_zero_is_sub_minute() {
        assert_eq!(format_reset(Some(0)), "<1m");
    }

    #[test]
    fn format_reset_under_a_minute_is_sub_minute() {
        assert_eq!(format_reset(Some(45)), "<1m");
        assert_eq!(format_reset(Some(59)), "<1m");
    }

    #[test]
    fn format_reset_minutes_only() {
        assert_eq!(format_reset(Some(60)), "1m");
        assert_eq!(format_reset(Some(720)), "12m");
        assert_eq!(format_reset(Some(3599)), "59m");
    }

    #[test]
    fn format_reset_hours_and_minutes() {
        assert_eq!(format_reset(Some(3600)), "1h 0m");
        assert_eq!(format_reset(Some(3723)), "1h 2m");
    }

    #[test]
    fn format_reset_exact_hour() {
        assert_eq!(format_reset(Some(7200)), "2h 0m");
    }

    #[test]
    fn format_reset_just_under_a_day() {
        assert_eq!(format_reset(Some(86_399)), "23h 59m");
    }

    #[test]
    fn format_reset_multi_day() {
        assert_eq!(format_reset(Some(86_400)), "1d 0h");
        // 5 days 4 hours = 5*86400 + 4*3600 = 446_400.
        assert_eq!(format_reset(Some(446_400)), "5d 4h");
    }

    // --- format_age ---

    #[test]
    fn format_age_seconds() {
        assert_eq!(format_age(5), "5s ago");
    }

    #[test]
    fn format_age_boundary_59_stays_seconds() {
        assert_eq!(format_age(59), "59s ago");
    }

    #[test]
    fn format_age_boundary_60_switches_to_minutes() {
        assert_eq!(format_age(60), "1m ago");
    }

    #[test]
    fn format_age_minutes() {
        assert_eq!(format_age(125), "2m ago");
    }

    // --- is_stale ---

    #[test]
    fn not_stale_just_under_ten_minutes() {
        assert!(!is_stale(Duration::from_secs(599)));
    }

    #[test]
    fn stale_at_ten_minutes() {
        // M7b lowered the threshold from 15 minutes to 10.
        assert!(is_stale(Duration::from_secs(600)));
    }

    // --- fraction_color ---

    #[test]
    fn unknown_fraction_is_gray() {
        assert_eq!(fraction_color(None), egui::Color32::GRAY);
    }

    #[test]
    fn low_usage_is_pine() {
        assert_eq!(fraction_color(Some(0.0)), PINE);
        assert_eq!(fraction_color(Some(0.49)), PINE);
    }

    #[test]
    fn half_spent_is_amber() {
        assert_eq!(fraction_color(Some(0.50)), AMBER);
        assert_eq!(fraction_color(Some(0.79)), AMBER);
    }

    #[test]
    fn eighty_percent_is_cardinal() {
        assert_eq!(fraction_color(Some(0.80)), CARDINAL);
        assert_eq!(fraction_color(Some(1.0)), CARDINAL);
    }

    // --- M7b palette wiring ---

    #[test]
    fn theme_installs_monospace_everywhere() {
        // Every text style must be mono: one proportional leftover would make
        // the harness's width arbitration meaningless for that style.
        let ctx = egui::Context::default();
        install_theme(&ctx, Theme::CipherPine);
        let style = ctx.style_of(egui::Theme::Dark);
        for (text_style, font_id) in &style.text_styles {
            assert_eq!(
                font_id.family,
                egui::FontFamily::Monospace,
                "{text_style:?} is not monospace"
            );
        }
        assert_eq!(style.visuals.panel_fill, GROUND);
        assert_eq!(style.visuals.extreme_bg_color, PANEL);
        assert_eq!(style.visuals.override_text_color, Some(TEXT));
    }

    #[test]
    fn theme_type_scale_matches_the_spec() {
        let ctx = egui::Context::default();
        install_theme(&ctx, Theme::CipherPine);
        let style = ctx.style_of(egui::Theme::Dark);
        let size = |s: egui::TextStyle| style.text_styles.get(&s).unwrap().size;
        assert_eq!(size(egui::TextStyle::Heading), 16.0);
        assert_eq!(size(egui::TextStyle::Body), 13.0);
        assert_eq!(size(egui::TextStyle::Small), 11.5);
    }

    // --- M7b: the status cursor ---

    #[test]
    fn idle_healthy_cursor_is_solid_and_never_repaints() {
        // The load-bearing assertion of the whole feature: an idle, fresh
        // window must not schedule a single repaint on the cursor's account,
        // at any point in the period.
        for ms in [0, 100, 549, 550, 551, 1099, 5_000, 3_600_000] {
            let (visible, needs_repaint) = cursor_phase(false, Duration::from_millis(ms));
            assert!(visible, "solid cursor vanished at {ms}ms");
            assert!(!needs_repaint, "idle cursor asked for a repaint at {ms}ms");
        }
    }

    #[test]
    fn blinking_cursor_steps_on_and_off_each_half_period() {
        // 1.1s period: on for [0, 550), off for [550, 1100).
        let on = |ms: u64| cursor_phase(true, Duration::from_millis(ms)).0;
        assert!(on(0));
        assert!(on(549));
        assert!(!on(550));
        assert!(!on(1_099));
        // And it wraps.
        assert!(on(1_100));
        assert!(!on(1_650));
    }

    #[test]
    fn blinking_cursor_always_requests_a_repaint() {
        for ms in [0, 300, 550, 900, 1_100] {
            assert!(cursor_phase(true, Duration::from_millis(ms)).1);
        }
    }

    #[test]
    fn next_toggle_lands_on_the_half_period_boundary() {
        // Never zero (that would busy-loop) and never past a half period.
        for ms in [0, 1, 274, 549, 550, 1_099] {
            let d = cursor_next_toggle(Duration::from_millis(ms));
            assert!(d > Duration::ZERO, "zero delay at {ms}ms would busy-loop");
            assert!(d <= Duration::from_millis(550), "overshot at {ms}ms");
        }
        assert_eq!(
            cursor_next_toggle(Duration::from_millis(0)),
            Duration::from_millis(550)
        );
        assert_eq!(
            cursor_next_toggle(Duration::from_millis(500)),
            Duration::from_millis(50)
        );
    }

    #[test]
    fn cursor_blinks_while_a_first_poll_is_in_flight() {
        // Live poller, nothing reported yet — the in-flight state.
        assert!(pane_wants_blink(true, false, false, None));
        // ...and it settles the moment a fresh snapshot lands.
        assert!(!pane_wants_blink(
            true,
            true,
            false,
            Some(Duration::from_secs(1))
        ));
        // A failure is an answer too: it ends the in-flight state, and the
        // failure banner carries the message rather than the cursor.
        assert!(!pane_wants_blink(true, false, true, None));
        // No poller at all (startup error) is not "in flight".
        assert!(!pane_wants_blink(false, false, false, None));
    }

    #[test]
    fn cursor_blinks_when_a_snapshot_has_gone_stale() {
        let fresh = Some(Duration::from_secs(1));
        let just_under = Some(STALE_AFTER - Duration::from_secs(1));
        let stale = Some(STALE_AFTER + Duration::from_secs(1));

        assert!(!pane_wants_blink(true, true, false, fresh));
        assert!(!pane_wants_blink(true, true, false, just_under));
        assert!(pane_wants_blink(true, true, false, stale));
        // Stale wins even when everything else looks settled.
        assert!(pane_wants_blink(false, true, true, stale));
    }

    #[test]
    fn cursor_blink_is_any_pane_not_all_panes() {
        // One unhealthy provider is enough — the cursor reports the window's
        // worst state, not an average.
        let healthy = ProviderPane::new::<ClaudeSubscription>(ProviderId::ClaudeSubscription, None);
        // Both panes have no handle and a startup error => nothing blinks.
        let panes = vec![healthy];
        assert!(!cursor_should_blink(&panes));

        // The predicate is what `any` is applied to; prove the OR directly.
        assert!([false, true].iter().any(|&stale| pane_wants_blink(
            true,
            true,
            false,
            stale.then(|| STALE_AFTER + Duration::from_secs(1))
        )));
    }

    #[test]
    fn cursor_does_not_change_the_titlebar_height() {
        // Space is allocated whether or not the cursor is painted, so the
        // prompt cannot jitter between blink states.
        let lay = |visible: bool| {
            lay_out(|ui| {
                ui.horizontal(|ui| {
                    render_prompt(ui, Theme::CipherPine, false);
                    render_cursor(ui, visible);
                });
            })
        };
        let on = lay(true);
        let off = lay(false);
        assert_eq!(
            on.width, off.width,
            "prompt width jitters as the cursor blinks"
        );
        assert_eq!(on.height, off.height);

        // Measured, not asserted from constants: the prompt row plus its
        // cursor still fits inside the fixed titlebar strip.
        assert!(
            on.height <= TITLEBAR_HEIGHT,
            "prompt row wanted {}px inside the {TITLEBAR_HEIGHT}px titlebar",
            on.height
        );
    }

    #[test]
    fn plain_is_never_wider_than_cipher_pine() {
        // Justifies measuring only Cipher Pine everywhere else: if the wider
        // mono fits, the proportional default fits.
        let row = |theme: Theme| {
            lay_out_themed(theme, move |ui| {
                render_window_row(
                    ui,
                    &QuotaWindow {
                        label: "5h".to_string(),
                        used_fraction: Some(0.33),
                        resets_in_secs: Some(446_400),
                        duration_secs: Some(18_000),
                    },
                    theme,
                )
            })
        };
        let themed = row(Theme::CipherPine);
        let plain = row(Theme::Plain);
        assert!(
            plain.width <= themed.width,
            "Plain wanted {}px vs Cipher Pine's {}px — the harness measures the wrong theme",
            plain.width,
            themed.width
        );
        assert!(themed.width <= themed.available_width);
        assert!(plain.width <= plain.available_width);
    }

    #[test]
    fn plain_theme_installs_egui_defaults() {
        // Plain is the pre-M7b look: egui's own dark visuals and proportional
        // type, not a second hand-tuned palette that could drift.
        let ctx = egui::Context::default();
        install_theme(&ctx, Theme::Plain);
        let style = ctx.style_of(egui::Theme::Dark);
        assert_eq!(style.visuals.override_text_color, None);
        assert_eq!(
            style
                .text_styles
                .get(&egui::TextStyle::Body)
                .unwrap()
                .family,
            egui::FontFamily::Proportional
        );
    }

    #[test]
    fn severity_mapping_is_shared_by_both_themes() {
        // Bar colour is data truth, not decoration: `fraction_color` takes no
        // theme, so there is exactly one mapping and neither theme can drift.
        let _: fn(Option<f64>) -> egui::Color32 = fraction_color;
        assert_eq!(fraction_color(Some(0.9)), CARDINAL);
    }

    #[test]
    fn grid_alpha_stays_texture_not_noise() {
        // A grid loud enough to compete with content is the failure mode this
        // guards; 12/255 is the accepted value.
        assert_eq!(GRID_ALPHA, 6);
        assert_eq!(GRID_PITCH, 64.0);
    }
    // --- M15: the agents view ---

    /// Planted in every session-log fixture below, in exactly the fields a
    /// conversation occupies. The same sentinel `usage_core::agents` uses.
    const AGENT_SENTINEL: &str = "SENTINEL-DO-NOT-SURFACE";

    /// A synthetic agent session, ages and states supplied by the caller.
    fn agent_row(
        provider: ProviderId,
        short_id: &str,
        project: &str,
        branch: Option<&str>,
        age: Duration,
        is_subagent: bool,
    ) -> AgentSession {
        AgentSession {
            provider,
            short_id: short_id.to_string(),
            project: project.to_string(),
            branch: branch.map(str::to_string),
            state: agents::state_for_age(age).expect("fixture must be inside the lookback"),
            last_write: SystemTime::now() - age,
            age,
            is_subagent,
        }
    }

    /// Every colour the row dots were painted in, in paint order.
    fn agent_dot_colors(theme: Theme, sessions: &[AgentSession]) -> Vec<egui::Color32> {
        painted_shapes_themed(theme, |ui| render_agents(ui, sessions, theme))
            .into_iter()
            .filter_map(|shape| match shape {
                egui::Shape::Text(text) if text.galley.text() == FRESHNESS_DOT => {
                    Some(text.galley.job.sections[0].format.color)
                }
                _ => None,
            })
            .collect()
    }

    /// What a pointer gesture aimed at a switcher label actually did.
    #[derive(Debug, Default, PartialEq, Eq)]
    struct Gesture {
        /// The view the switcher reported, if any.
        switched: Option<View>,
        /// Whether the whole-strip drag handle *behind* the switcher began a
        /// window drag — the titlebar's real arrangement, replicated here.
        moved_window: bool,
    }

    /// Drive a real pointer gesture at a switcher label, through the titlebar's
    /// own ordering: the strip's drag handle first, the switcher on top of it.
    ///
    /// The label's position is discovered from the shapes the switcher actually
    /// painted, so this clicks where a user would rather than at a coordinate
    /// written down here and left to rot.
    fn gesture_on_switcher(current: View, label: &str, drag: bool) -> Gesture {
        let ctx = egui::Context::default();
        install_theme(&ctx, Theme::CipherPine);
        let input = |events: Vec<egui::Event>| egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WINDOW_WIDTH, TITLEBAR_HEIGHT),
            )),
            events,
            ..Default::default()
        };
        let mut clicked = Gesture::default();
        let run = |raw: egui::RawInput, clicked: &mut Gesture| {
            let out = ctx.run_ui(raw, |ui| {
                // Exactly `render_titlebar`'s order and sense.
                let strip = ui.interact(
                    ui.max_rect(),
                    ui.id().with("titlebar_drag"),
                    egui::Sense::click_and_drag(),
                );
                if strip.drag_started() {
                    clicked.moved_window = true;
                }
                ui.horizontal(|ui| {
                    if let Some(view) = render_view_switcher(ui, Theme::CipherPine, current) {
                        clicked.switched = Some(view);
                    }
                });
            });
            let mut flat = Vec::new();
            for shape in out.shapes {
                flatten_shape(shape.shape, &mut flat);
            }
            flat
        };

        // Two quiet frames: the first warms the font atlas, the second settles
        // the widget rects egui resolves the next frame's pointer against.
        run(input(vec![]), &mut clicked);
        let shapes = run(input(vec![]), &mut clicked);

        let target = shapes
            .into_iter()
            .find_map(|shape| match shape {
                egui::Shape::Text(text) if text.galley.text() == label => {
                    Some(egui::Rect::from_min_size(text.pos, text.galley.size()).center())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("the switcher never painted {label:?}"));

        let button = |pos, pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        if drag {
            // Press on the label, travel, release far away: a window drag that
            // happened to start on a word, which must not be read as a click.
            let away = target + egui::vec2(60.0, 0.0);
            run(
                input(vec![
                    egui::Event::PointerMoved(target),
                    button(target, true),
                ]),
                &mut clicked,
            );
            run(input(vec![egui::Event::PointerMoved(away)]), &mut clicked);
            run(input(vec![button(away, false)]), &mut clicked);
        } else {
            run(input(vec![egui::Event::PointerMoved(target)]), &mut clicked);
            run(input(vec![button(target, true)]), &mut clicked);
            run(input(vec![button(target, false)]), &mut clicked);
        }
        run(input(vec![]), &mut clicked);
        clicked
    }

    #[test]
    fn agents_demo_flag_defaults_off_and_is_recognized_anywhere() {
        assert!(!parse_args(args(&[])).unwrap().agents_demo);
        assert!(parse_args(args(&["--agents-demo"])).unwrap().agents_demo);
        let parsed = parse_args(args(&["--plain", "--agents-demo", "--no-tray"])).unwrap();
        assert!(parsed.agents_demo);
        assert!(parsed.no_tray);
        // The two demos are independent flags, not one mode.
        assert!(!parse_args(args(&["--agents-demo"])).unwrap().pace_demo);
        assert!(!parse_args(args(&["--pace-demo"])).unwrap().agents_demo);
    }

    #[test]
    fn clicking_a_switcher_label_switches_the_view() {
        for (current, label, expected) in [
            (View::Usage, VIEW_LABEL_AGENTS, View::Agents),
            (View::Agents, VIEW_LABEL_USAGE, View::Usage),
        ] {
            let gesture = gesture_on_switcher(current, label, false);
            assert_eq!(
                gesture.switched,
                Some(expected),
                "clicking {label:?} from {current:?} must switch"
            );
            assert!(
                !gesture.moved_window,
                "a click on {label:?} also started a window drag"
            );
        }
    }

    #[test]
    fn dragging_from_a_label_moves_the_window_and_switches_nothing() {
        // The gesture that moves this decoration-less window starts wherever
        // the pointer happens to be — including on a switcher label. A drag is
        // not a click, and the label must not swallow it either: `selectable`
        // is off precisely so the drag reaches the strip behind it.
        for (current, label) in [
            (View::Usage, VIEW_LABEL_AGENTS),
            (View::Agents, VIEW_LABEL_USAGE),
        ] {
            let gesture = gesture_on_switcher(current, label, true);
            assert_eq!(
                gesture.switched, None,
                "a drag from {label:?} switched the view"
            );
            assert!(
                gesture.moved_window,
                "a drag from {label:?} was swallowed instead of moving the window"
            );
        }
    }

    #[test]
    fn the_switcher_is_drawn_after_the_strips_drag_handle() {
        // Ordering is the whole reason a label keeps its click: the strip's
        // whole-rect handle is installed first, so every widget drawn after it
        // sits on top. The buttons already depend on this; the switcher now
        // does too, and nothing else in the file states it.
        const SRC: &str = include_str!("main.rs");
        let titlebar = SRC
            .split("fn render_titlebar")
            .nth(1)
            .expect("render_titlebar not found");
        let handle = titlebar
            .find("titlebar_drag")
            .expect("the strip's drag handle is gone");
        let switcher = titlebar
            .find("render_view_switcher(ui")
            .expect("the switcher is not drawn in the titlebar");
        assert!(
            handle < switcher,
            "the switcher must be drawn after the drag handle"
        );
    }

    #[test]
    fn the_switcher_reads_current_in_text_and_the_other_faint() {
        for (current, lit, dim) in [
            (View::Usage, VIEW_LABEL_USAGE, VIEW_LABEL_AGENTS),
            (View::Agents, VIEW_LABEL_AGENTS, VIEW_LABEL_USAGE),
        ] {
            let colors: Vec<(String, egui::Color32)> = painted_shapes(|ui| {
                let _ = render_view_switcher(ui, Theme::CipherPine, current);
            })
            .into_iter()
            .filter_map(|shape| match shape {
                egui::Shape::Text(text) => Some((
                    text.galley.text().to_string(),
                    text.galley.job.sections[0].format.color,
                )),
                _ => None,
            })
            .collect();
            let of = |label: &str| {
                colors
                    .iter()
                    .find(|(text, _)| text == label)
                    .unwrap_or_else(|| panic!("{label} not painted: {colors:?}"))
                    .1
            };
            assert_eq!(of(lit), TEXT, "{current:?}: the current view reads bright");
            assert_eq!(of(dim), TEXT_FAINT, "{current:?}: the other reads faint");
        }
    }

    #[test]
    fn no_scan_is_reachable_while_the_usage_view_shows() {
        // The whole policy, at every input it can be asked about.
        for demo in [false, true] {
            for since in [
                None,
                Some(Duration::ZERO),
                Some(SCAN_EVERY),
                Some(SCAN_EVERY * 1000),
            ] {
                assert!(
                    !should_scan(View::Usage, demo, since),
                    "usage view scanned (demo={demo}, since={since:?})"
                );
            }
        }
        // Switching in scans at once; after that, on the cadence.
        assert!(should_scan(View::Agents, false, None));
        assert!(!should_scan(View::Agents, false, Some(Duration::ZERO)));
        assert!(!should_scan(
            View::Agents,
            false,
            Some(SCAN_EVERY - Duration::from_millis(1))
        ));
        assert!(should_scan(View::Agents, false, Some(SCAN_EVERY)));
        // The demo's rows are a fixture; it never scans at all.
        for since in [None, Some(SCAN_EVERY * 1000)] {
            assert!(!should_scan(View::Agents, true, since));
        }
    }

    #[test]
    fn the_scanner_has_exactly_one_call_site_and_it_is_the_guarded_one() {
        // `should_scan` is only a promise if nothing routes around it. The one
        // call site is `refresh_agents`, and it asks first.
        const SRC: &str = include_str!("main.rs");
        let code = &SRC[..SRC
            .find("\n#[cfg(test)]\nmod tests {")
            .expect("test module not found")];
        let calls: Vec<&str> = code
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .filter(|line| line.contains("agents::scan("))
            .collect();
        assert_eq!(
            calls.len(),
            1,
            "agents::scan must have exactly one call site: {calls:?}"
        );

        let body = code
            .split("fn refresh_agents")
            .nth(1)
            .expect("refresh_agents not found");
        let body = &body[..body.find("\n    }").expect("unterminated refresh_agents")];
        assert!(
            body.contains("if !should_scan("),
            "refresh_agents no longer asks should_scan first"
        );
        assert!(
            body.contains("agents::scan("),
            "the call site moved out of refresh_agents"
        );
    }

    #[test]
    fn the_agents_pane_never_renders_conversation_content() {
        // The real scanner over a real fixture tree, drawn through the real
        // render path — the only arrangement that can prove the pane, rather
        // than the parser, keeps invariant 8.
        let mut root = std::env::temp_dir();
        root.push(format!("quotapane-ui-agents-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);

        let claude_dir = root.join("claude/projects/-home-j-dev-QuotaPane");
        std::fs::create_dir_all(&claude_dir).unwrap();
        std::fs::write(
            claude_dir.join("a1b2c3d4-e5f6.jsonl"),
            format!(
                concat!(
                    r#"{{"isSidechain":false,"cwd":"/home/j/dev/QuotaPane","sessionId":"a1b2c3d4-e5f6",
"#,
                    r#""gitBranch":"main","type":"user","timestamp":"2026-08-06T12:00:00.000Z",
"#,
                    r#""message":{{"role":"user","content":"{sentinel}"}}}}
"#,
                ),
                sentinel = AGENT_SENTINEL
            )
            .replace('\n', ""),
        )
        .unwrap();

        let codex_dir = root.join("codex/sessions/2026/08/06");
        std::fs::create_dir_all(&codex_dir).unwrap();
        std::fs::write(
            codex_dir.join("rollout-2026-08-06T12-00-00-9f8e7d6c.jsonl"),
            format!(
                concat!(
                    r#"{{"timestamp":"2026-08-06T12:00:00.000Z","type":"session_meta","#,
                    r#""payload":{{"id":"9f8e7d6c-1234","cwd":"/home/j/dev/other","#,
                    r#""git_branch":"feat/x","instructions":"{sentinel}"}}}}"#,
                ),
                sentinel = AGENT_SENTINEL
            ),
        )
        .unwrap();

        let roots = SessionRoots {
            claude: Some(root.join("claude")),
            codex: Some(root.join("codex")),
        };
        let sessions = agents::scan(&roots, SystemTime::now());
        assert_eq!(sessions.len(), 2, "fixture tree did not scan: {sessions:?}");

        for theme in [Theme::CipherPine, Theme::Plain] {
            let painted = painted_text(theme, |ui| render_agents(ui, &sessions, theme));
            assert!(
                painted.iter().all(|text| !text.contains(AGENT_SENTINEL)),
                "{theme:?} rendered conversation content: {painted:?}"
            );
            // ...and the row it *did* draw is the metadata one.
            assert!(
                painted.iter().any(|text| text.contains("QuotaPane")),
                "{theme:?} drew no identity at all: {painted:?}"
            );
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_agents_demo_shows_every_state_both_providers_and_a_subagent() {
        let sessions = demo_agents(SystemTime::now());
        for state in [AgentState::Working, AgentState::Idle, AgentState::Recent] {
            assert!(
                sessions.iter().any(|s| s.state == state),
                "the demo is missing {state:?}: {sessions:?}"
            );
        }
        for provider in [
            ProviderId::ClaudeSubscription,
            ProviderId::CodexSubscription,
        ] {
            assert!(
                sessions.iter().any(|s| s.provider == provider),
                "the demo is missing {provider:?}"
            );
        }
        assert!(sessions.iter().any(|s| s.is_subagent), "no subagent row");
        assert!(sessions.iter().any(|s| s.branch.is_none()), "no bare row");

        // ...and all of it reaches the screen.
        let painted = painted_text(Theme::CipherPine, |ui| {
            render_agents(ui, &sessions, Theme::CipherPine)
        });
        assert!(
            painted.iter().any(|text| text == SUBAGENT_PREFIX),
            "the subagent mark was not painted: {painted:?}"
        );
        let dots = agent_dot_colors(Theme::CipherPine, &sessions);
        for expected in [OPER_GREEN, AMBER, TEXT_FAINT] {
            assert!(
                dots.contains(&expected),
                "no {expected:?} dot among {dots:?}"
            );
        }
    }

    #[test]
    fn the_row_dot_carries_the_state_and_nothing_else_does() {
        for (age, expected) in [
            (Duration::ZERO, OPER_GREEN),
            (agents::ACTIVE_WITHIN, OPER_GREEN),
            (agents::ACTIVE_WITHIN + Duration::from_secs(1), AMBER),
            (agents::IDLE_WITHIN, AMBER),
            (agents::IDLE_WITHIN + Duration::from_secs(1), TEXT_FAINT),
            (agents::LOOKBACK, TEXT_FAINT),
        ] {
            let sessions = vec![agent_row(
                ProviderId::ClaudeSubscription,
                "a1b2c3d4",
                "QuotaPane",
                Some("main"),
                age,
                false,
            )];
            // The same dot in both themes, exactly as M14's freshness dot is.
            for theme in [Theme::CipherPine, Theme::Plain] {
                assert_eq!(
                    agent_dot_colors(theme, &sessions),
                    vec![expected],
                    "{theme:?} at {age:?}"
                );
            }
        }
    }

    #[test]
    fn an_empty_scan_says_so_and_draws_no_provider_headers() {
        for theme in [Theme::CipherPine, Theme::Plain] {
            let painted = painted_text(theme, |ui| render_agents(ui, &[], theme));
            assert!(
                painted.iter().any(|text| text == NO_AGENTS_LINE),
                "{theme:?} printed no empty state: {painted:?}"
            );
            assert!(
                !painted.iter().any(|text| text.contains("CLAUDE")),
                "{theme:?} drew a header over an empty list: {painted:?}"
            );
        }
    }

    #[test]
    fn a_provider_with_no_sessions_gets_no_header() {
        let sessions = vec![agent_row(
            ProviderId::CodexSubscription,
            "9f8e7d6c",
            "api",
            None,
            Duration::from_secs(30),
            false,
        )];
        let painted = painted_text(Theme::CipherPine, |ui| {
            render_agents(ui, &sessions, Theme::CipherPine)
        });
        assert!(painted.iter().any(|text| text == "CODEX"), "{painted:?}");
        assert!(!painted.iter().any(|text| text == "CLAUDE"), "{painted:?}");
        assert!(
            !painted.iter().any(|text| text == NO_AGENTS_LINE),
            "a list with a row in it is not empty: {painted:?}"
        );
    }

    #[test]
    fn a_row_omits_the_branch_it_does_not_have() {
        let with = agent_row(
            ProviderId::ClaudeSubscription,
            "a1b2c3d4",
            "QuotaPane",
            Some("main"),
            Duration::from_secs(5),
            false,
        );
        assert_eq!(agent_identity(&with), "QuotaPane · main · a1b2c3d4");
        let without = AgentSession {
            branch: None,
            ..with.clone()
        };
        assert_eq!(
            agent_identity(&without),
            "QuotaPane · a1b2c3d4",
            "the separator must collapse, not leave a gap"
        );
        // A scan that could name nothing but the file still says something.
        let bare = AgentSession {
            project: String::new(),
            branch: None,
            ..with
        };
        assert_eq!(agent_identity(&bare), "a1b2c3d4");
    }

    #[test]
    fn an_agent_row_fits_the_window_for_any_branch_name() {
        // A branch name is user-supplied text of no bounded length. The row
        // elides rather than overflows — this is the assertion that keeps it
        // that way.
        for branch in [
            Some("main"),
            Some("feature/the-longest-branch-name-anyone-has-ever-typed-into-a-terminal"),
            None,
        ] {
            let sessions = vec![agent_row(
                ProviderId::ClaudeSubscription,
                "a1b2c3d4",
                "a-project-with-a-long-directory-name",
                branch,
                Duration::from_secs(3_000),
                true,
            )];
            for theme in [Theme::CipherPine, Theme::Plain] {
                let laid = lay_out_themed(theme, |ui| render_agents(ui, &sessions, theme));
                assert!(
                    laid.width <= laid.available_width,
                    "{theme:?} row with {branch:?} wanted {}px inside {}px",
                    laid.width,
                    laid.available_width
                );
            }
        }
    }

    #[test]
    fn the_agents_demo_fits_the_window_it_opens() {
        // `--agents-demo` opens the ordinary window, so the fixture has to fit
        // the ordinary height — no demo-sized viewport, no scroll bar.
        let sessions = demo_agents(SystemTime::now());
        for theme in [Theme::CipherPine, Theme::Plain] {
            let laid = lay_out_themed(theme, |ui| render_agents(ui, &sessions, theme));
            let needed = snapped_height(laid.height, laid.panel_chrome_height);
            assert!(
                needed <= WINDOW_HEIGHT,
                "{theme:?} demo needs {needed}px inside the {WINDOW_HEIGHT}px window"
            );
            assert!(
                laid.width <= laid.available_width,
                "{theme:?} demo wanted {}px inside {}px",
                laid.width,
                laid.available_width
            );
        }
    }

    #[test]
    fn the_agents_demo_marks_itself_as_a_demo() {
        // M13's rule, applied to invented sessions: a decoration-less window
        // shows its OS title only in the taskbar, so the pane has to say so
        // where the fiction is.
        for theme in [Theme::CipherPine, Theme::Plain] {
            let painted = painted_text(theme, |ui| render_prompt(ui, theme, true));
            assert!(
                painted.iter().any(|text| text.trim() == "demo"),
                "{theme:?} drew no demo marker: {painted:?}"
            );
        }
        // ...and `--agents-demo` is one of the two flags that reaches it.
        const SRC: &str = include_str!("main.rs");
        let code = &SRC[..SRC
            .find("\n#[cfg(test)]\nmod tests {")
            .expect("test module not found")];
        assert!(
            code.contains("let demo = self.pace_demo || self.agents_demo;"),
            "the titlebar marker no longer covers --agents-demo"
        );
        assert!(
            code.contains("window_title(pace_demo || agents_demo)"),
            "the OS title no longer covers --agents-demo"
        );
    }

    #[test]
    fn the_switcher_fits_the_titlebar_beside_the_prompt_and_the_buttons() {
        // The budget, derived rather than guessed: the strip's width less the
        // two 24px buttons and the frame's own insets.
        let budget = WINDOW_WIDTH
            - 2.0 * TITLEBAR_HEIGHT
            - f32::from(TITLEBAR_MARGIN_LEFT)
            - f32::from(TITLEBAR_MARGIN_RIGHT);
        for theme in [Theme::CipherPine, Theme::Plain] {
            for view in [View::Usage, View::Agents] {
                let laid = lay_out_themed(theme, |ui| {
                    ui.horizontal(|ui| {
                        render_prompt(ui, theme, true);
                        if theme == Theme::CipherPine {
                            render_cursor(ui, true);
                        }
                        let _ = render_view_switcher(ui, theme, view);
                    });
                });
                assert!(
                    laid.width <= budget,
                    "{theme:?}/{view:?}: titlebar content wanted {}px of {budget}px",
                    laid.width
                );
            }
        }
    }
}

// Tray helpers are Windows/macOS-only, so their tests are too (they run on the
// windows-latest and macos-latest CI jobs, and locally on Windows).
#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod tray_tests {
    use super::*;
    use usage_core::model::{ProviderId, ProviderSnapshot, QuotaWindow, SnapshotSource};

    fn snap(windows: Vec<QuotaWindow>) -> ProviderSnapshot {
        ProviderSnapshot {
            provider: ProviderId::ClaudeSubscription,
            taken_at_unix_secs: 0,
            windows,
            per_model: vec![],
            reset_credits: None,
            source: SnapshotSource::UsageEndpoint,
        }
    }

    fn window(label: &str, fraction: Option<f64>) -> QuotaWindow {
        QuotaWindow {
            label: label.to_string(),
            used_fraction: fraction,
            resets_in_secs: None,
            duration_secs: None,
        }
    }

    fn pixel(px: &[u8], size: usize, x: usize, y: usize) -> [u8; 4] {
        let i = (y * size + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    // --- tooltip formatting ---

    #[test]
    fn tooltip_both_providers_matches_example_shape() {
        let claude = snap(vec![window("5h", Some(0.42)), window("week", Some(0.10))]);
        let codex = snap(vec![window("7d", Some(0.03))]);
        let tip = tray_tooltip(&[("Claude", Some(&claude)), ("Codex", Some(&codex))]);
        assert_eq!(tip, "Claude 5h 42% | Codex 7d 3%");
    }

    #[test]
    fn tooltip_unknown_providers_show_double_dash() {
        let tip = tray_tooltip(&[("Claude", None), ("Codex", None)]);
        assert_eq!(tip, "Claude -- | Codex --");
    }

    #[test]
    fn summary_window_without_fraction_is_double_dash() {
        let s = snap(vec![window("5h", None)]);
        assert_eq!(provider_tray_summary("Claude", Some(&s)), "Claude --");
    }

    #[test]
    fn summary_no_windows_is_double_dash() {
        let s = snap(vec![]);
        assert_eq!(provider_tray_summary("Claude", Some(&s)), "Claude --");
    }

    #[test]
    fn summary_no_snapshot_is_double_dash() {
        assert_eq!(provider_tray_summary("Codex", None), "Codex --");
    }

    #[test]
    fn representative_window_picks_highest_usage() {
        let s = snap(vec![window("5h", Some(0.42)), window("week", Some(0.90))]);
        assert_eq!(provider_tray_summary("Claude", Some(&s)), "Claude week 90%");
    }

    #[test]
    fn summary_rounds_percentage() {
        // 0.005 → 0.5% → rounds to 1%.
        let s = snap(vec![window("5h", Some(0.005))]);
        assert_eq!(provider_tray_summary("Claude", Some(&s)), "Claude 5h 1%");
    }

    #[test]
    fn per_model_windows_do_not_feed_the_tooltip() {
        // The tray summarizes the headline windows only. A per-model row that
        // is closer to its limit must not hijack the tooltip.
        let mut s = snap(vec![window("5h", Some(0.42))]);
        s.per_model = vec![window("7d-opus", Some(0.99))];
        assert_eq!(provider_tray_summary("Claude", Some(&s)), "Claude 5h 42%");
    }

    #[test]
    fn per_model_only_snapshot_has_no_representative_window() {
        // No headline windows at all → "--", not a per-model stand-in.
        let mut s = snap(vec![]);
        s.per_model = vec![window("7d-sonnet", Some(0.50))];
        assert_eq!(provider_tray_summary("Claude", Some(&s)), "Claude --");
    }

    #[test]
    fn single_provider_tooltip_has_no_separator() {
        let s = snap(vec![window("5h", Some(0.42))]);
        let tip = tray_tooltip(&[("Claude", Some(&s))]);
        assert_eq!(tip, "Claude 5h 42%");
        assert!(!tip.contains('|'));
    }

    // --- tray icon at tray scale (M7b) ---
    //
    // The mark's geometry and colour mapping are covered in `icon`'s own
    // tests at native 64px scale; these pin what the *tray* specifically
    // depends on at ICON_SIZE.

    #[test]
    fn tray_icon_has_expected_dimensions() {
        let px = icon::render_icon(None, None, ICON_SIZE);
        assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }

    #[test]
    fn tray_icon_corner_is_transparent() {
        let px = icon::render_icon(None, None, ICON_SIZE);
        assert_eq!(pixel(&px, ICON_SIZE as usize, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn tray_icon_center_is_opaque() {
        // Not blank: the tile centre is fully opaque even with no usage.
        let px = icon::render_icon(None, None, ICON_SIZE);
        let c = ICON_SIZE as usize / 2;
        assert_eq!(pixel(&px, ICON_SIZE as usize, c, c)[3], 255);
    }

    #[test]
    fn tray_icon_is_live_at_tray_scale() {
        // The whole point of re-rendering per poll: different usage must
        // produce a visibly different tray bitmap even at 32px, or the
        // set_icon_if_changed guard would never fire and the tray would lie.
        let quiet = icon::render_icon(Some(0.1), Some(0.1), ICON_SIZE);
        let busy = icon::render_icon(Some(0.9), Some(0.9), ICON_SIZE);
        assert_ne!(quiet, busy);

        // ...and unknown differs from known-empty's neighbours too.
        let unknown = icon::render_icon(None, None, ICON_SIZE);
        assert_ne!(unknown, busy);
    }

    #[test]
    fn tray_icon_is_stable_for_unchanged_usage() {
        // The cache guard compares whole buffers, so identical input must give
        // byte-identical output or the OS call would fire every frame.
        assert_eq!(
            icon::render_icon(Some(0.42), Some(0.17), ICON_SIZE),
            icon::render_icon(Some(0.42), Some(0.17), ICON_SIZE)
        );
    }
}
