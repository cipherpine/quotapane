//! Theme preference: one word on disk, and nothing else (M7b-r1).
//!
//! ## Why this file is boring, on purpose
//! QuotaPane's whole thesis is a tiny credential-touching surface. A settings
//! file is a place where scope accretes — a window position here, a token cache
//! there — so this one is deliberately incapable of holding anything but the
//! word `plain` or `cipherpine`. Parsing is a `trim`, a lowercase, and a
//! `match`; there is no key-value format to extend and no serializer to reach
//! for. If a future preference needs storing, that is a design conversation,
//! not a field to append here (SECURITY.md invariant 1: config stores
//! preferences only, never credential material).
//!
//! ## No dependencies
//! `std::env` and `std::fs` cover all of it: no config crate, no TOML parser,
//! no `dirs`. The platform config directory is resolved from environment
//! variables the OS already guarantees.
//!
//! Every failure path degrades to [`Theme::CipherPine`] and every write failure
//! is swallowed: a theme preference is never worth an error dialog, a log line,
//! or a panic.

// The only way to *change* the theme at runtime is the tray menu, so on a
// platform without a tray (Linux) `save` and the label/toggle helpers are
// unreachable — `load` and the flags still work, which is the whole surface
// there. This is not dead code; it is code for a platform this build does not
// have, and deleting it to satisfy the lint would break Windows and macOS.
#![cfg_attr(not(any(target_os = "windows", target_os = "macos")), allow(dead_code))]

use std::path::PathBuf;

/// Which look the window wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// The Cipher Pine terminal theme — mono type, blueprint grid, `//`
    /// headers, cardinal prompt and status cursor. The default.
    #[default]
    CipherPine,
    /// The pre-M7b look: egui's default dark visuals and proportional type,
    /// no grid, no cursor, plain titlebar and provider names.
    Plain,
}

impl Theme {
    /// The single word written to disk.
    fn as_word(self) -> &'static str {
        match self {
            Theme::CipherPine => "cipherpine",
            Theme::Plain => "plain",
        }
    }

    /// Parse the stored word. Anything unrecognized is the default — a
    /// corrupted or hand-edited file must never leave the window unstyled or
    /// crash it.
    fn from_word(word: &str) -> Theme {
        match word.trim().to_ascii_lowercase().as_str() {
            "plain" => Theme::Plain,
            _ => Theme::CipherPine,
        }
    }

    /// The other theme — what the tray toggle switches to.
    pub fn toggled(self) -> Theme {
        match self {
            Theme::CipherPine => Theme::Plain,
            Theme::Plain => Theme::CipherPine,
        }
    }

    /// Menu label for the current theme.
    pub fn menu_label(self) -> &'static str {
        match self {
            Theme::CipherPine => "Theme: Cipher Pine",
            Theme::Plain => "Theme: Plain",
        }
    }
}

/// Environment variable that overrides the config directory.
///
/// Exists so the tests can round-trip through a temp directory instead of the
/// developer's real config dir — a test that writes to `%APPDATA%` is a test
/// that changes the machine it runs on.
const CONFIG_DIR_OVERRIDE: &str = "QUOTAPANE_CONFIG_DIR";

/// The platform's config directory for this app, or `None` if the environment
/// does not name one.
///
/// Windows uses `%APPDATA%`, macOS `~/Library/Application Support`, and
/// everything else the XDG convention — `$XDG_CONFIG_HOME`, falling back to
/// `$HOME/.config`.
fn config_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_OVERRIDE) {
        return Some(PathBuf::from(dir));
    }

    let base: PathBuf = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(PathBuf::from)?
    } else if cfg!(target_os = "macos") {
        let mut home = PathBuf::from(std::env::var_os("HOME")?);
        home.push("Library");
        home.push("Application Support");
        home
    } else if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(xdg)
    } else {
        let mut home = PathBuf::from(std::env::var_os("HOME")?);
        home.push(".config");
        home
    };

    Some(base.join("quotapane"))
}

/// Full path to the theme file.
fn theme_path() -> Option<PathBuf> {
    Some(config_dir()?.join("theme.cfg"))
}

/// Read the saved theme. Absent, unreadable, or unrecognized → the default.
pub fn load() -> Theme {
    theme_path()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .map(|word| Theme::from_word(&word))
        .unwrap_or_default()
}

/// Persist the theme, ignoring every failure.
///
/// A read-only config directory, a full disk, a locked file — none of these are
/// worth interrupting someone who just wanted a different colour scheme. The
/// preference simply does not survive the session.
pub fn save(theme: Theme) {
    let Some(path) = theme_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, theme.as_word());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Point the config at a private temp directory for the duration of a
    /// test, then restore whatever was there.
    ///
    /// The env var is process-global, so these tests must not run in parallel
    /// with each other; they are all funnelled through the one serial test
    /// below rather than relying on luck.
    struct TempConfig {
        dir: PathBuf,
        previous: Option<std::ffi::OsString>,
    }

    impl TempConfig {
        fn new(tag: &str) -> TempConfig {
            let mut dir = std::env::temp_dir();
            dir.push(format!("quotapane-cfg-{}-{tag}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let previous = std::env::var_os(CONFIG_DIR_OVERRIDE);
            // Safety: single-threaded within this serial test.
            unsafe { std::env::set_var(CONFIG_DIR_OVERRIDE, &dir) };
            TempConfig { dir, previous }
        }
    }

    impl Drop for TempConfig {
        fn drop(&mut self) {
            unsafe {
                match &self.previous {
                    Some(v) => std::env::set_var(CONFIG_DIR_OVERRIDE, v),
                    None => std::env::remove_var(CONFIG_DIR_OVERRIDE),
                }
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn write_raw(contents: &str) {
        let path = theme_path().unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn word_parsing_is_total() {
        // Pure, no filesystem: every input maps somewhere, and only "plain"
        // maps away from the default.
        assert_eq!(Theme::from_word("plain"), Theme::Plain);
        assert_eq!(Theme::from_word("  PLAIN\n"), Theme::Plain);
        assert_eq!(Theme::from_word("cipherpine"), Theme::CipherPine);
        for garbage in ["", "   ", "dark", "plain2", "🙂", "plain plain", "0"] {
            assert_eq!(
                Theme::from_word(garbage),
                Theme::CipherPine,
                "{garbage:?} should fall back to the default"
            );
        }
    }

    #[test]
    fn default_is_cipherpine() {
        assert_eq!(Theme::default(), Theme::CipherPine);
    }

    #[test]
    fn toggling_round_trips() {
        assert_eq!(Theme::CipherPine.toggled(), Theme::Plain);
        assert_eq!(Theme::Plain.toggled(), Theme::CipherPine);
        assert_eq!(Theme::Plain.toggled().toggled(), Theme::Plain);
    }

    #[test]
    fn menu_labels_name_the_current_theme() {
        assert_eq!(Theme::CipherPine.menu_label(), "Theme: Cipher Pine");
        assert_eq!(Theme::Plain.menu_label(), "Theme: Plain");
    }

    /// All filesystem cases in one test: the config-dir override is a global
    /// env var, so running them concurrently would let one test's temp dir
    /// leak into another's `load()`.
    #[test]
    fn config_file_round_trips_and_degrades() {
        // --- save then load is identity, both ways ---
        {
            let _tmp = TempConfig::new("roundtrip");
            for theme in [Theme::Plain, Theme::CipherPine] {
                save(theme);
                assert_eq!(load(), theme, "{theme:?} did not survive a round trip");
            }
            // The file holds the bare word and nothing else.
            let contents = std::fs::read_to_string(theme_path().unwrap()).unwrap();
            assert_eq!(contents, "cipherpine");
        }

        // --- absent file → default ---
        {
            let _tmp = TempConfig::new("absent");
            assert!(!theme_path().unwrap().exists());
            assert_eq!(load(), Theme::CipherPine);
        }

        // --- garbage contents → default, no panic ---
        {
            let _tmp = TempConfig::new("garbage");
            for junk in ["", "nonsense", "{\"theme\":\"plain\"}", "\u{0}\u{1}"] {
                write_raw(junk);
                assert_eq!(
                    load(),
                    Theme::CipherPine,
                    "junk {junk:?} broke the fallback"
                );
            }
        }

        // --- unreadable path (a directory where the file should be) → default ---
        {
            let _tmp = TempConfig::new("unreadable");
            let path = theme_path().unwrap();
            std::fs::create_dir_all(&path).unwrap();
            assert_eq!(load(), Theme::CipherPine);
            // And saving over it fails silently rather than panicking.
            save(Theme::Plain);
        }

        // --- save creates the directory when missing ---
        {
            let _tmp = TempConfig::new("mkdir");
            let nested = _tmp.dir.join("deeper");
            unsafe { std::env::set_var(CONFIG_DIR_OVERRIDE, &nested) };
            save(Theme::Plain);
            assert_eq!(load(), Theme::Plain);
        }
    }
}
