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
//! carries in `per_model` (Claude's `7d-opus`/`7d-sonnet`, Codex's
//! `additional_rate_limits`). It is collapsed by default and its state is per
//! pane, so the providers expand independently. The toggle is suppressed when
//! a provider reports no per-model data. This is presentational only: the rows
//! use the same renderer as the headline rows and no label is ever parsed.

use eframe::egui;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use usage_core::egress::Egress;
use usage_core::model::{ProviderId, ProviderSnapshot, QuotaWindow};
use usage_core::poller::{self, PollerHandle, Update};
use usage_core::providers::{
    ClaudeSubscription, CodexSubscription, UsageProvider, CODEX_DEFAULT_USER_AGENT,
};

/// Sent when `--client-version` is omitted. Mirrors `usage-cli`'s default —
/// real Claude Code versions avoid the provider's aggressively rate-limited
/// fallback bucket (see `claude_subscription` module docs in usage-core).
const DEFAULT_CLIENT_VERSION: &str = "0.0.0";

/// A snapshot is considered stale once it's this old without a fresh poll.
const STALE_AFTER: Duration = Duration::from_secs(15 * 60);

const NORMAL_COLOR: egui::Color32 = egui::Color32::from_rgb(46, 160, 67);
const WARNING_COLOR: egui::Color32 = egui::Color32::from_rgb(230, 162, 60);
const CRITICAL_COLOR: egui::Color32 = egui::Color32::from_rgb(217, 62, 62);

/// Slim custom titlebar — the window is borderless (`with_decorations(false)`),
/// so it draws its own. ~24px tall.
const TITLEBAR_HEIGHT: f32 = 24.0;
/// Dark strip echoing the tray icon's tile (the tray's `TILE` slate), so the
/// titlebar reads as the same product as the tray.
const TITLEBAR_BG: egui::Color32 = egui::Color32::from_rgb(24, 27, 33);
/// Near-white app-name text on the dark strip.
const TITLEBAR_TEXT: egui::Color32 = egui::Color32::from_rgb(220, 223, 228);

/// Caption beside the per-model disclosure triangle.
const PER_MODEL_CAPTION: &str = "per-model";

/// The window's fixed inner size. It is **not resizable**, so every layout has
/// to fit inside this — there is no user escape hatch when content overflows.
const WINDOW_WIDTH: f32 = 320.0;
const WINDOW_HEIGHT: f32 = 240.0;

/// How far a per-model row's bar line is inset under its model label.
const PER_MODEL_ROW_INDENT: f32 = 8.0;

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
}

fn parse_args<I: IntoIterator<Item = String>>(argv: I) -> Result<Args, String> {
    let mut client_version: Option<String> = None;
    let mut codex_user_agent: Option<String> = None;
    let mut no_tray = false;

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
            other => return Err(format!("unrecognized argument: {other}")),
        }
    }

    let client_version_defaulted = client_version.is_none();
    Ok(Args {
        client_version: client_version.unwrap_or_else(|| DEFAULT_CLIENT_VERSION.to_string()),
        client_version_defaulted,
        codex_user_agent: codex_user_agent.unwrap_or_else(|| CODEX_DEFAULT_USER_AGENT.to_string()),
        no_tray,
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

/// Whether a provider failure message indicates an absent credential file (as
/// opposed to a genuine error worth a red banner). The credential loader
/// reports a missing file as `credential file not found: <path>` (see
/// `usage_core::credentials::CredentialError::NotFound`); no other
/// `ProviderError` variant's message contains this phrase.
fn is_absent_credentials(message: &str) -> bool {
    message.contains("not found")
}

/// How a provider's latest failure should be presented.
#[derive(Debug, PartialEq, Eq)]
enum FailureDisplay {
    /// No failure recorded.
    NoFailure,
    /// Credentials absent — show the quiet "not signed in" line, not an error.
    NotSignedIn,
    /// A genuine failure — show a red banner with the message.
    Banner,
}

/// Per-provider state selection: decide how to present the latest failure.
fn classify_failure(failure: Option<&str>) -> FailureDisplay {
    match failure {
        None => FailureDisplay::NoFailure,
        Some(msg) if is_absent_credentials(msg) => FailureDisplay::NotSignedIn,
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

/// Bar color for a quota window's used fraction: green/amber/red by
/// severity threshold, or gray when the fraction is unknown.
fn fraction_color(fraction: Option<f64>) -> egui::Color32 {
    match fraction {
        None => egui::Color32::GRAY,
        Some(f) if f >= 0.95 => CRITICAL_COLOR,
        Some(f) if f >= 0.80 => WARNING_COLOR,
        Some(_) => NORMAL_COLOR,
    }
}

// --------------------------------------------------------------------------
// System tray (Windows + macOS only) — see the CONTRIBUTING.md / deny.toml
// rationale. Everything below the pure helpers is gated to the tray targets;
// Linux compiles to exactly the pre-M3.5 window-only behavior.
// --------------------------------------------------------------------------

/// A tray/menu interaction, forwarded from the OS event handlers into the app.
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TrayMessage {
    /// Left-click: show and focus the window.
    ShowAndFocus,
    /// "Show/Hide" menu item: flip the window's visibility.
    ToggleShowHide,
    /// "Quit" menu item: exit the app for real.
    Quit,
}

/// The window that best represents a provider at a glance: the one closest to
/// its limit (highest used fraction). Ties and unknown fractions resolve to the
/// earliest such window. `None` when the snapshot has no windows.
#[cfg(any(target_os = "windows", target_os = "macos"))]
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

/// Side length (px) of the square tray icon generated at runtime.
#[cfg(any(target_os = "windows", target_os = "macos"))]
const ICON_SIZE: u32 = 32;

/// Write one RGBA pixel into a row-major `size`×`size` buffer.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn put_px(px: &mut [u8], size: usize, x: usize, y: usize, rgba: [u8; 4]) {
    let i = (y * size + x) * 4;
    px[i..i + 4].copy_from_slice(&rgba);
}

/// Draw a horizontal gauge bar: the `track` color across `[left, right)` on
/// rows `[top, bottom)`, with the leading `fraction` filled in `color`.
#[cfg(any(target_os = "windows", target_os = "macos"))]
#[allow(clippy::too_many_arguments)]
fn fill_bar(
    px: &mut [u8],
    size: usize,
    top: usize,
    bottom: usize,
    left: usize,
    right: usize,
    color: [u8; 4],
    track: [u8; 4],
    fraction: f64,
) {
    let span = right - left;
    let fill_end = left + ((span as f64) * fraction.clamp(0.0, 1.0)).round() as usize;
    for y in top..bottom {
        for x in left..right {
            put_px(px, size, x, y, if x < fill_end { color } else { track });
        }
    }
}

/// Generate the tray icon as raw RGBA8 (`ICON_SIZE`×`ICON_SIZE`, row-major).
///
/// Drawn entirely in code — no asset file, no build script, no image decoder.
/// It's a tiny two-bar gauge on a dark tile, echoing the window's quota bars: a
/// fuller green bar over a shorter amber one.
#[cfg(any(target_os = "windows", target_os = "macos"))]
fn tray_icon_rgba() -> Vec<u8> {
    let size = ICON_SIZE as usize;
    let mut px = vec![0u8; size * size * 4]; // transparent background

    const TILE: [u8; 4] = [24, 27, 33, 255]; // dark slate
    const TRACK: [u8; 4] = [55, 60, 68, 255]; // unfilled bar background
    const GREEN: [u8; 4] = [46, 160, 67, 255]; // matches NORMAL_COLOR
    const AMBER: [u8; 4] = [230, 162, 60, 255]; // matches WARNING_COLOR

    // Dark tile, inset 2px, with the four hard corners clipped for a soft look.
    for y in 2..size - 2 {
        for x in 2..size - 2 {
            let corner = (x < 4 || x >= size - 4) && (y < 4 || y >= size - 4);
            if !corner {
                put_px(&mut px, size, x, y, TILE);
            }
        }
    }

    // Two horizontal gauge bars echoing the window's quota bars.
    let left = 7;
    let right = size - 7;
    fill_bar(&mut px, size, 10, 15, left, right, GREEN, TRACK, 0.70);
    fill_bar(&mut px, size, 18, 23, left, right, AMBER, TRACK, 0.40);

    px
}

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

    use super::TrayMessage;

    const INITIAL_TOOLTIP: &str = "QuotaPane";

    /// A live tray icon. Dropping it removes the icon, so it lives as long as
    /// the app; the owned menu (and its items) live with it.
    pub struct Tray {
        icon: TrayIcon,
        rx: Receiver<TrayMessage>,
        /// Whether the window is currently shown (the tray toggles this).
        pub visible: bool,
        last_tooltip: String,
    }

    impl Tray {
        /// Build the tray on the calling (main) thread — both Windows and macOS
        /// require it there. Returns `None` if the OS refuses to create it, in
        /// which case the app falls back to close-to-quit. Registers the
        /// process-wide tray/menu event handlers, which forward into an mpsc
        /// channel drained each frame.
        pub fn create(ctx: &egui::Context) -> Option<Tray> {
            let (tx, rx) = mpsc::channel::<TrayMessage>();
            // The event handlers must be `Send + Sync`; a bare `Sender` is not
            // `Sync`, so guard it behind a mutex.
            let tx = Arc::new(Mutex::new(tx));

            let menu = Menu::new();
            let show_hide = MenuItem::new("Show/Hide", true, None);
            let quit = MenuItem::new("Quit", true, None);
            menu.append_items(&[&show_hide, &PredefinedMenuItem::separator(), &quit])
                .ok()?;
            let show_hide_id = show_hide.id().clone();
            let quit_id = quit.id().clone();

            {
                let tx = Arc::clone(&tx);
                let ctx = ctx.clone();
                MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                    let msg = if event.id == show_hide_id {
                        TrayMessage::ToggleShowHide
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

            let icon = Icon::from_rgba(super::tray_icon_rgba(), super::ICON_SIZE, super::ICON_SIZE)
                .ok()?;
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
                last_tooltip: INITIAL_TOOLTIP.to_string(),
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
}

impl ProviderPane {
    /// Build a pane from an optional provider. `None` means the credential
    /// path could not be resolved at all (no home directory), which is a
    /// startup error rather than a per-poll failure.
    fn new<P: UsageProvider + Send + 'static>(id: ProviderId, provider: Option<P>) -> Self {
        let (handle, startup_error) = match provider {
            // Each provider gets its own egress instance; it moves into the
            // poller thread. Proxy opt-in stays false (SECURITY.md invariant 7).
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
        }
    }

    /// Drain this pane's channel, folding every pending update into its state.
    fn drain(&mut self) {
        let Some(handle) = &self.handle else {
            return;
        };
        for update in handle.updates().try_iter() {
            match update {
                Update::Snapshot(snapshot) => {
                    self.latest_snapshot = Some(snapshot);
                    self.snapshot_received_at = Some(Instant::now());
                    // A fresh success supersedes any prior failure.
                    self.latest_failure = None;
                }
                Update::Failure { message, .. } => {
                    self.latest_failure = Some(message);
                }
            }
        }
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
    #[cfg(any(target_os = "windows", target_os = "macos"))]
    tray: Option<tray::Tray>,
}

impl QuotaPaneApp {
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

        let frame = egui::Frame::new()
            .fill(TITLEBAR_BG)
            .inner_margin(egui::Margin {
                left: 8,
                right: 2,
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
                        ui.label(
                            egui::RichText::new("QuotaPane")
                                .color(TITLEBAR_TEXT)
                                .strong(),
                        );
                    });
                });
            });

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
        // Build the tooltip from the same snapshots the window renders.
        let tooltip = {
            let entries: Vec<(&str, Option<&ProviderSnapshot>)> = self
                .panes
                .iter()
                .map(|pane| (provider_label(pane.id), pane.latest_snapshot.as_ref()))
                .collect();
            tray_tooltip(&entries)
        };

        // Update the tooltip and collect events without holding the tray borrow
        // across the app mutations below.
        let messages = match self.tray.as_mut() {
            Some(tray) => {
                tray.set_tooltip_if_changed(&tooltip);
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
        for pane in &mut self.panes {
            pane.drain();
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

        // Keep polling for updates (and tray events) about once a second, even
        // when the window is hidden.
        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Slim custom titlebar first (a top panel): app name + minimize/close,
        // and a window-drag handle. Takes its ~24px; the CentralPanel fills the
        // rest below it.
        self.render_titlebar(ui, &ctx);

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

            // Vertical safety net. The window is a fixed 240px with no resize,
            // so content that outgrows it has nowhere to go: expanding several
            // per-model rows (two lines each), or Claude also reporting
            // per-model windows, would push the age footer out of the window
            // with no way to reach it. This converts that silent truncation
            // into a scroll.
            //
            // Deliberately invisible until needed: egui shows no scroll bar
            // while the content fits, which is every state accepted so far, so
            // this changes nothing the owner has already signed off on.
            //
            // `DragScroll::Never` matters — the default is `OnTouch`, and on a
            // touch-capable Windows machine that would turn a drag on the pane
            // background into a scroll, stealing the only gesture that moves
            // this decoration-less window. Wheel and scroll bar stay enabled.
            egui::ScrollArea::vertical()
                .scroll_source(egui::containers::scroll_area::ScrollSource {
                    drag: egui::containers::scroll_area::DragScroll::Never,
                    ..Default::default()
                })
                .show(ui, |ui| {
                    for (i, pane) in self.panes.iter_mut().enumerate() {
                        if i > 0 {
                            ui.separator();
                        }
                        render_pane(ui, pane);
                    }
                });
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // Stop every poller thread, not just the first.
        for pane in &mut self.panes {
            pane.stop();
        }
    }
}

/// Render one provider's titled section.
fn render_pane(ui: &mut egui::Ui, pane: &mut ProviderPane) {
    ui.heading(provider_label(pane.id));

    if let Some(err) = &pane.startup_error {
        ui.colored_label(CRITICAL_COLOR, err);
        return;
    }

    // The disclosure toggle only *records* intent while the snapshot is
    // borrowed; the flip happens after, mirroring the titlebar buttons.
    let mut toggled = false;
    if let Some(snapshot) = &pane.latest_snapshot {
        let age = pane.snapshot_received_at.map(|t| t.elapsed());
        toggled = render_windows(ui, snapshot, age, pane.expanded);
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
        FailureDisplay::Banner => {
            if let Some(message) = &pane.latest_failure {
                ui.colored_label(CRITICAL_COLOR, message);
            }
        }
    }
}

/// Render a snapshot's quota bars, the per-model disclosure, and its
/// freshness/staleness line.
///
/// `expanded` is the pane's current disclosure state. Returns `true` when the
/// user clicked the toggle this frame; the caller owns the flip.
fn render_windows(
    ui: &mut egui::Ui,
    snapshot: &ProviderSnapshot,
    age: Option<Duration>,
    expanded: bool,
) -> bool {
    for window in &snapshot.windows {
        render_window_row(ui, window);
    }

    // Per-model disclosure, between the headline rows and the age footer.
    // Suppressed entirely when there is nothing to disclose — no affordance
    // that opens onto an empty list.
    let mut toggled = false;
    if !snapshot.per_model.is_empty() {
        toggled = disclosure_toggle(ui, expanded, PER_MODEL_CAPTION).clicked();
        if expanded {
            // Salted with the provider so the two panes' indent regions get
            // distinct ids.
            ui.indent(("per_model", snapshot.provider), |ui| {
                for window in &snapshot.per_model {
                    render_per_model_row(ui, window);
                }
            });
        }
    }

    if let Some(age) = age {
        let age_secs = age.as_secs();
        let mut age_text = format!("updated {}", format_age(age_secs));
        if is_stale(age) {
            age_text.push_str("  •  stale");
        }
        let color = if is_stale(age) {
            WARNING_COLOR
        } else {
            ui.visuals().weak_text_color()
        };
        ui.colored_label(color, age_text);
    }

    toggled
}

fn render_window_row(ui: &mut egui::Ui, window: &QuotaWindow) {
    ui.horizontal(|ui| {
        ui.label(&window.label);
        // The numeric percent rides on the bar itself; `--` when unknown (an
        // unknown fraction also draws an empty gray bar).
        ui.add(
            egui::ProgressBar::new(window.used_fraction.unwrap_or(0.0) as f32)
                .desired_width(120.0)
                .fill(fraction_color(window.used_fraction))
                .text(format_percent(window.used_fraction)),
        );
        // Compact reset countdown, e.g. "resets in 3h 12m"; `--` when unknown.
        ui.label(format!("resets in {}", format_reset(window.resets_in_secs)));
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
/// The label is small + weak so the two lines read as one subordinate row
/// rather than as two unrelated ones. The bar keeps the headline row's
/// `desired_width(120.0)`, `fraction_color`, `format_percent`, and the
/// `"resets in {}"` phrasing, so a per-model gauge stays comparable at a
/// glance with a headline gauge.
fn render_per_model_row(ui: &mut egui::Ui, window: &QuotaWindow) {
    let weak = ui.visuals().weak_text_color();
    ui.label(
        egui::RichText::new(window.label.as_str())
            .small()
            .color(weak),
    );
    ui.horizontal(|ui| {
        ui.add_space(PER_MODEL_ROW_INDENT);
        // 120.0 deliberately matches `render_window_row`'s bar; that function
        // is not edited to share a constant, so keep the two in step by hand.
        ui.add(
            egui::ProgressBar::new(window.used_fraction.unwrap_or(0.0) as f32)
                .desired_width(120.0)
                .fill(fraction_color(window.used_fraction))
                .text(format_percent(window.used_fraction)),
        );
        ui.label(format!("resets in {}", format_reset(window.resets_in_secs)));
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
                "usage: quotapane [--client-version <VER>] [--codex-user-agent <UA>] [--no-tray]"
            );
            return ExitCode::from(2);
        }
    };

    if args.client_version_defaulted {
        eprintln!(
            "note: no --client-version given; using \"{DEFAULT_CLIENT_VERSION}\" — pass a real claude-code version to avoid provider throttling"
        );
    }

    // `--no-tray` disables the tray; on platforms without one (Linux) the flag
    // is accepted and ignored. Reading it via `cfg!` keeps the field live on
    // every platform and yields today's behavior wherever there is no tray.
    let tray_active = !args.no_tray && cfg!(any(target_os = "windows", target_os = "macos"));

    // One pane per provider. A missing home directory becomes a per-pane
    // startup error; a missing credential file surfaces later as a quiet
    // "not signed in" line — neither aborts the window or the other provider.
    let claude = ProviderPane::new(
        ProviderId::ClaudeSubscription,
        ClaudeSubscription::with_default_path(args.client_version),
    );
    let codex = ProviderPane::new(
        ProviderId::CodexSubscription,
        CodexSubscription::with_default_path(args.codex_user_agent),
    );

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top(),
        ..Default::default()
    };

    let result = eframe::run_native(
        "QuotaPane",
        native_options,
        Box::new(move |_cc| {
            // Create the tray on the main thread, now that eframe/winit is up.
            // If creation fails, fall back to close-to-quit.
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            let (tray, tray_active) = if tray_active {
                let tray = tray::Tray::create(&_cc.egui_ctx);
                let active = tray.is_some();
                (tray, active)
            } else {
                (None, false)
            };

            let app = QuotaPaneApp {
                panes: vec![claude, codex],
                tray_active,
                quitting: false,
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

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
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
    fn lay_out(mut add_contents: impl FnMut(&mut egui::Ui)) -> Laid {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(WINDOW_WIDTH, WINDOW_HEIGHT),
            )),
            ..Default::default()
        };
        let (mut width, mut height, mut available_width) = (0.0, 0.0, 0.0);
        // Two frames: the first warms the font atlas and the id-keyed state
        // the indent and scroll area allocate, so the second measures a
        // settled layout.
        for _ in 0..2 {
            let _ = ctx.run_ui(input.clone(), |ui| {
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
        }
    }

    /// A per-model window with a realistic provider model name.
    fn model_window(label: &str) -> QuotaWindow {
        QuotaWindow {
            label: label.to_string(),
            used_fraction: Some(0.42),
            // 5d 4h — the widest reset string the formatter produces.
            resets_in_secs: Some(446_400),
        }
    }

    #[test]
    fn per_model_row_fits_the_window() {
        // The exact label from the Codex fixture that clipped in the window.
        let laid = lay_out(|ui| {
            ui.indent("t", |ui| {
                render_per_model_row(ui, &model_window("GPT-5.3-Codex-Spark"))
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
            ui.indent("t", |ui| render_per_model_row(ui, &model_window(absurd)));
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
                render_window_row(ui, &model_window("GPT-5.3-Codex-Spark"))
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
                    },
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
                },
                QuotaWindow {
                    label: "7d".to_string(),
                    used_fraction: Some(0.18),
                    resets_in_secs: Some(86_400),
                },
            ],
            per_model: vec![
                model_window("GPT-5.3-Codex-Spark"),
                model_window("GPT-5.3-Codex-Max"),
            ],
            source: SnapshotSource::UsageEndpoint,
        };
        let laid = lay_out(|ui| {
            render_windows(ui, &snapshot, Some(Duration::from_secs(30)), true);
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
        // eats the age footer with no way to reach it.
        let usable_height = WINDOW_HEIGHT - TITLEBAR_HEIGHT;
        let snapshot = ProviderSnapshot {
            provider: ProviderId::CodexSubscription,
            taken_at_unix_secs: 0,
            windows: vec![QuotaWindow {
                label: "5h".to_string(),
                used_fraction: Some(0.25),
                resets_in_secs: Some(3600),
            }],
            per_model: (0..6)
                .map(|i| model_window(&format!("GPT-5.3-Codex-Variant-{i}")))
                .collect(),
            source: SnapshotSource::UsageEndpoint,
        };
        let laid = lay_out(|ui| {
            render_windows(ui, &snapshot, Some(Duration::from_secs(30)), true);
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
        let _: fn(&mut egui::Ui, &QuotaWindow) = render_per_model_row;
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
            "OAuth token expired — refresh via the provider's official CLI (`claude` or `codex login`), then retry",
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
    fn not_stale_just_under_fifteen_minutes() {
        assert!(!is_stale(Duration::from_secs(899)));
    }

    #[test]
    fn stale_at_fifteen_minutes() {
        assert!(is_stale(Duration::from_secs(900)));
    }

    // --- fraction_color ---

    #[test]
    fn unknown_fraction_is_gray() {
        assert_eq!(fraction_color(None), egui::Color32::GRAY);
    }

    #[test]
    fn low_usage_is_green() {
        assert_eq!(fraction_color(Some(0.0)), NORMAL_COLOR);
        assert_eq!(fraction_color(Some(0.79)), NORMAL_COLOR);
    }

    #[test]
    fn eighty_percent_is_amber() {
        assert_eq!(fraction_color(Some(0.80)), WARNING_COLOR);
        assert_eq!(fraction_color(Some(0.94)), WARNING_COLOR);
    }

    #[test]
    fn ninety_five_percent_is_red() {
        assert_eq!(fraction_color(Some(0.95)), CRITICAL_COLOR);
        assert_eq!(fraction_color(Some(1.0)), CRITICAL_COLOR);
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
            source: SnapshotSource::UsageEndpoint,
        }
    }

    fn window(label: &str, fraction: Option<f64>) -> QuotaWindow {
        QuotaWindow {
            label: label.to_string(),
            used_fraction: fraction,
            resets_in_secs: None,
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

    // --- icon pixel generation invariants ---

    #[test]
    fn icon_has_expected_dimensions() {
        let px = tray_icon_rgba();
        assert_eq!(px.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }

    #[test]
    fn icon_corner_is_transparent() {
        let px = tray_icon_rgba();
        assert_eq!(pixel(&px, ICON_SIZE as usize, 0, 0), [0, 0, 0, 0]);
    }

    #[test]
    fn icon_center_is_opaque() {
        // The icon is not blank: its tile center is fully opaque.
        let px = tray_icon_rgba();
        let c = ICON_SIZE as usize / 2;
        assert_eq!(pixel(&px, ICON_SIZE as usize, c, c)[3], 255);
    }

    #[test]
    fn icon_draws_green_and_amber_bars() {
        let px = tray_icon_rgba();
        let size = ICON_SIZE as usize;
        // Green bar (rows 10..15), inside the filled portion.
        assert_eq!(pixel(&px, size, 10, 12), [46, 160, 67, 255]);
        // Amber bar (rows 18..23), inside the filled portion.
        assert_eq!(pixel(&px, size, 10, 20), [230, 162, 60, 255]);
    }

    #[test]
    fn icon_bar_shows_unfilled_track() {
        let px = tray_icon_rgba();
        let size = ICON_SIZE as usize;
        // Past the 70% fill of the green bar: the track color.
        assert_eq!(pixel(&px, size, 23, 12), [55, 60, 68, 255]);
    }
}
