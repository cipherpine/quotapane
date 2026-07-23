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
        ProviderId::AnthropicAdmin => "Anthropic Admin",
        ProviderId::OpenAiUsage => "OpenAI",
    }
}

/// The quiet one-liner shown when a provider's credential file is absent —
/// points the user at the official CLI that signs them in (invariant 6:
/// QuotaPane never writes credentials itself).
fn not_signed_in_line(id: ProviderId) -> &'static str {
    match id {
        ProviderId::ClaudeSubscription => "Claude: not signed in — run `claude` to sign in",
        ProviderId::CodexSubscription => "Codex: not signed in — run `codex login`",
        ProviderId::AnthropicAdmin => "Anthropic Admin: not configured",
        ProviderId::OpenAiUsage => "OpenAI: not configured",
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

/// Format a reset countdown, e.g. "45s", "2m", "1h 2m".
fn format_countdown(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
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

            for (i, pane) in self.panes.iter().enumerate() {
                if i > 0 {
                    ui.separator();
                }
                render_pane(ui, pane);
            }
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
fn render_pane(ui: &mut egui::Ui, pane: &ProviderPane) {
    ui.heading(provider_label(pane.id));

    if let Some(err) = &pane.startup_error {
        ui.colored_label(CRITICAL_COLOR, err);
        return;
    }

    if let Some(snapshot) = &pane.latest_snapshot {
        let age = pane.snapshot_received_at.map(|t| t.elapsed());
        render_windows(ui, snapshot, age);
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

/// Render a snapshot's quota bars plus its freshness/staleness line.
fn render_windows(ui: &mut egui::Ui, snapshot: &ProviderSnapshot, age: Option<Duration>) {
    for window in &snapshot.windows {
        render_window_row(ui, window);
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
}

fn render_window_row(ui: &mut egui::Ui, window: &QuotaWindow) {
    ui.horizontal(|ui| {
        ui.label(&window.label);
        match window.used_fraction {
            Some(fraction) => {
                ui.add(
                    egui::ProgressBar::new(fraction as f32)
                        .desired_width(120.0)
                        .fill(fraction_color(Some(fraction))),
                );
            }
            None => {
                ui.add(
                    egui::ProgressBar::new(0.0)
                        .desired_width(120.0)
                        .fill(fraction_color(None))
                        .text("?"),
                );
            }
        }
        let reset_text = match window.resets_in_secs {
            Some(secs) => format!("resets in {}", format_countdown(secs)),
            None => "resets in ?".to_string(),
        };
        ui.label(reset_text);
    });
}

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            eprintln!(
                "usage: usage-ui [--client-version <VER>] [--codex-user-agent <UA>] [--no-tray]"
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
            .with_inner_size([320.0, 240.0])
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
        assert_eq!(
            provider_label(ProviderId::AnthropicAdmin),
            "Anthropic Admin"
        );
        assert_eq!(provider_label(ProviderId::OpenAiUsage), "OpenAI");
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

    // --- format_countdown ---

    #[test]
    fn format_countdown_seconds_only() {
        assert_eq!(format_countdown(45), "45s");
    }

    #[test]
    fn format_countdown_minutes_only() {
        assert_eq!(format_countdown(125), "2m");
    }

    #[test]
    fn format_countdown_hours_and_minutes() {
        assert_eq!(format_countdown(3723), "1h 2m");
    }

    #[test]
    fn format_countdown_exact_hour() {
        assert_eq!(format_countdown(7200), "2h 0m");
    }

    #[test]
    fn format_countdown_zero() {
        assert_eq!(format_countdown(0), "0s");
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
