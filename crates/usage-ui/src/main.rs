//! QuotaPane desktop window — M2 milestone.
//!
//! Pure render: this crate receives non-secret `ProviderSnapshot` values
//! over a channel and draws them. It never touches credentials or the
//! network. The egui/eframe window arrives in M2, after the trust boundary
//! (M0) and the headless pipeline (M1) are proven.

fn main() {
    eprintln!("usage-ui: the desktop window arrives in milestone M2 (see ARCHITECTURE.md §9).");
    eprintln!("Until then, use `usage-cli` for headless output.");
    std::process::exit(2);
}
