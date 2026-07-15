//! QuotaPane desktop window — M2 milestone.
//!
//! Pure render: this crate receives non-secret `ProviderSnapshot` values
//! over a channel and draws them. It never touches credentials or the
//! network directly — `usage_core::poller` and the `Egress`/`UsageProvider`
//! types it wires together do that, entirely inside the trust boundary crate.

use eframe::egui;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use usage_core::egress::Egress;
use usage_core::model::{ProviderSnapshot, QuotaWindow};
use usage_core::poller::{self, PollerHandle, Update};
use usage_core::providers::ClaudeSubscription;

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
    client_version: String,
    client_version_defaulted: bool,
}

fn parse_args<I: IntoIterator<Item = String>>(argv: I) -> Result<Args, String> {
    let mut client_version: Option<String> = None;

    let mut iter = argv.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--client-version" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--client-version requires a value".to_string())?;
                client_version = Some(value);
            }
            other => return Err(format!("unrecognized argument: {other}")),
        }
    }

    let client_version_defaulted = client_version.is_none();
    Ok(Args {
        client_version: client_version.unwrap_or_else(|| DEFAULT_CLIENT_VERSION.to_string()),
        client_version_defaulted,
    })
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

/// Draggable desktop window showing live Claude quota. Holds the poller
/// handle and the latest non-secret update; never touches credentials.
struct QuotaPaneApp {
    handle: Option<PollerHandle>,
    latest_snapshot: Option<ProviderSnapshot>,
    snapshot_received_at: Option<Instant>,
    latest_failure: Option<String>,
    startup_error: Option<String>,
}

impl QuotaPaneApp {
    fn drain_updates(&mut self) {
        let Some(handle) = &self.handle else {
            return;
        };
        for update in handle.updates().try_iter() {
            match update {
                Update::Snapshot(snapshot) => {
                    self.latest_snapshot = Some(snapshot);
                    self.snapshot_received_at = Some(Instant::now());
                    // A fresh success supersedes any prior failure as "the
                    // latest update" — the failure banner clears.
                    self.latest_failure = None;
                }
                Update::Failure { message, .. } => {
                    self.latest_failure = Some(message);
                }
            }
        }
    }
}

impl eframe::App for QuotaPaneApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_updates();
        let ctx = ui.ctx().clone();

        // The root `Ui` eframe hands to `App::ui` has no margin or background
        // (see `eframe::App::ui` docs) — a `CentralPanel` supplies both.
        egui::CentralPanel::default().show(ui, |ui| {
            // Whole-panel drag handle: any click-drag not consumed by a
            // widget above it moves the (decoration-less) window.
            let bg_rect = ui.max_rect();
            let bg_response = ui.interact(
                bg_rect,
                ui.id().with("background_drag"),
                egui::Sense::drag(),
            );
            if bg_response.drag_started() {
                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            }

            if let Some(err) = &self.startup_error {
                ui.colored_label(CRITICAL_COLOR, err);
            } else {
                match &self.latest_snapshot {
                    Some(snapshot) => {
                        let age = self.snapshot_received_at.map(|t| t.elapsed());
                        render_snapshot(ui, snapshot, age);
                    }
                    None => {
                        ui.label("waiting for first poll…");
                    }
                }

                if let Some(message) = &self.latest_failure {
                    ui.colored_label(CRITICAL_COLOR, message);
                }
            }
        });

        ctx.request_repaint_after(Duration::from_secs(1));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(handle) = self.handle.take() {
            handle.stop();
        }
    }
}

fn render_snapshot(ui: &mut egui::Ui, snapshot: &ProviderSnapshot, age: Option<Duration>) {
    ui.heading("Claude");
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
            eprintln!("usage: usage-ui [--client-version <VER>]");
            return ExitCode::from(2);
        }
    };

    if args.client_version_defaulted {
        eprintln!(
            "note: no --client-version given; using \"{DEFAULT_CLIENT_VERSION}\" — pass a real claude-code version to avoid provider throttling"
        );
    }

    let (handle, startup_error) = match ClaudeSubscription::with_default_path(args.client_version) {
        Some(provider) => {
            let egress = Egress::new(false);
            (Some(poller::spawn(provider, egress)), None)
        }
        None => (
            None,
            Some("could not resolve a home directory for the credentials path".to_string()),
        ),
    };

    let app = QuotaPaneApp {
        handle,
        latest_snapshot: None,
        snapshot_received_at: None,
        latest_failure: None,
        startup_error,
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([320.0, 140.0])
            .with_decorations(false)
            .with_resizable(false)
            .with_always_on_top(),
        ..Default::default()
    };

    match eframe::run_native(
        "QuotaPane",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    ) {
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

    // --- parse_args ---

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
