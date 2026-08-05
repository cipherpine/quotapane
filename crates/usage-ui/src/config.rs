//! Preferences: a handful of `key=value` lines on disk, and nothing else (M13).
//!
//! ## Why this file is still boring, on purpose
//! QuotaPane's whole thesis is a tiny credential-touching surface. A settings
//! file is a place where scope accretes — a window position here, a token cache
//! there — so this one stays deliberately incapable of holding anything but the
//! five preferences named in [`Config`]. Parsing is a `split_once('=')`, a
//! `trim`, a lowercase, and a `match` on a closed key list; every value is a
//! word or a small integer, and an unrecognized key is *ignored* rather than
//! stored. There is no place to put a token, no nesting, no escapes, and no
//! serializer to reach for (SECURITY.md invariant 1: config stores preferences
//! only, never credential material).
//!
//! ## The one-word era, and the migration out of it
//! From v1.2.0 to v1.5.1 this module held exactly one word — `plain` or
//! `cipherpine` — in `theme.cfg`, and its own header said that a second
//! preference would be "a design conversation, not a field to append here".
//! M13 is that conversation: on-disk history, quota alerts and the alert
//! threshold each need a stored choice, and four more one-word files would have
//! been a worse answer than one boring grammar.
//!
//! So `config.cfg` supersedes `theme.cfg` in the same directory. When
//! `config.cfg` is absent the legacy file is still read for the theme, so an
//! existing install keeps its look; `theme.cfg` is never written again and never
//! deleted — deleting a file a previous version wrote is not this module's
//! business, and leaving it costs one word of disk.
//!
//! ## No dependencies
//! `std::env` and `std::fs` cover all of it: no config crate, no TOML parser,
//! no `dirs`. The platform config directory is resolved from environment
//! variables the OS already guarantees.
//!
//! Every failure path degrades to [`Config::default`] and every write failure is
//! swallowed: a preference is never worth an error dialog, a log line, or a
//! panic.

// The only way to *change* a preference at runtime is the tray menu, so on a
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

/// When a quota alert is allowed to fire once usage has crossed the threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertMode {
    /// Time-aware, and the default: crossing the threshold only speaks when the
    /// window is *also* being spent faster than it is elapsing. A weekly quota
    /// that is 85% spent six days in is on budget, and saying so would be noise.
    #[default]
    Pace,
    /// Every crossing of the threshold fires, regardless of the clock.
    Threshold,
}

impl AlertMode {
    /// The word written to disk.
    fn as_word(self) -> &'static str {
        match self {
            AlertMode::Pace => "pace",
            AlertMode::Threshold => "threshold",
        }
    }

    /// Parse the stored word; anything unrecognized is the default.
    fn from_word(word: &str) -> AlertMode {
        match word {
            "threshold" => AlertMode::Threshold,
            _ => AlertMode::Pace,
        }
    }
}

/// The threshold an alert fires at, in whole percent, when the file says
/// nothing usable.
pub const DEFAULT_ALERT_AT: u8 = 80;

/// Every preference QuotaPane persists. Five values, all of them a user's
/// display choice — there is deliberately no field here that could hold
/// account state, a cached response, or credential material of any kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    /// Which look the window wears.
    pub theme: Theme,
    /// Whether usage percentages are appended to `history.jsonl` (opt-in, off).
    pub history: bool,
    /// Whether quota alerts are raised at all (opt-in, off).
    pub alerts: bool,
    /// Percent of a window at which an alert becomes a candidate, `1..=100`.
    pub alert_at: u8,
    /// Whether a candidate must also be burning faster than the clock.
    pub alert_mode: AlertMode,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            theme: Theme::default(),
            // Both features are off until asked for: history is the app's first
            // rolling write path, and an alert is an interruption. Neither is
            // something to hand a user who never opted in.
            history: false,
            alerts: false,
            alert_at: DEFAULT_ALERT_AT,
            alert_mode: AlertMode::default(),
        }
    }
}

/// Parse an `on`/`off` switch; anything else keeps the default.
fn switch(value: &str, default: bool) -> bool {
    match value {
        "on" => true,
        "off" => false,
        _ => default,
    }
}

/// Parse the alert threshold. Out of range (including `0`) or unparsable is
/// [`DEFAULT_ALERT_AT`] — a hand-edited `alert_at=0` must not turn every window
/// into a permanent alert, and `alert_at=900` must not silence them all.
fn alert_at(value: &str) -> u8 {
    match value.parse::<u32>() {
        Ok(n) if (1..=100).contains(&n) => n as u8,
        _ => DEFAULT_ALERT_AT,
    }
}

/// Parse the whole file.
///
/// The grammar in full: one `key=value` per line; blank lines and `#`-prefixed
/// lines ignored; keys and values trimmed and ASCII-lowercased; unknown keys
/// ignored so a newer build's file still loads in an older one; no quoting, no
/// sections, no escapes. Nothing here can fail — every malformed line is simply
/// not a preference, and the default stands.
fn parse(text: &str) -> Config {
    let mut config = Config::default();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        match key.as_str() {
            "theme" => config.theme = Theme::from_word(&value),
            "history" => config.history = switch(&value, Config::default().history),
            "alerts" => config.alerts = switch(&value, Config::default().alerts),
            "alert_at" => config.alert_at = alert_at(&value),
            "alert_mode" => config.alert_mode = AlertMode::from_word(&value),
            // Forward compatibility: a key this build does not know is a key a
            // newer build wrote, not an error to report at someone.
            _ => {}
        }
    }
    config
}

/// Render the whole config, every key present, with the two-line header that
/// tells a curious reader what the file is for.
fn render(config: &Config) -> String {
    format!(
        "# QuotaPane preferences — display choices only, written by the app.\n\
         # Never credentials: this file cannot hold a token (SECURITY.md invariant 1).\n\
         theme={}\n\
         history={}\n\
         alerts={}\n\
         alert_at={}\n\
         alert_mode={}\n",
        config.theme.as_word(),
        if config.history { "on" } else { "off" },
        if config.alerts { "on" } else { "off" },
        config.alert_at,
        config.alert_mode.as_word(),
    )
}

/// Environment variable that overrides the config directory.
///
/// Exists so the tests can round-trip through a temp directory instead of the
/// developer's real config dir — a test that writes to `%APPDATA%` is a test
/// that changes the machine it runs on.
const CONFIG_DIR_OVERRIDE: &str = "QUOTAPANE_CONFIG_DIR";

/// The preferences file.
const CONFIG_FILE: &str = "config.cfg";

/// The pre-M13 one-word theme file. Read as a fallback, never written.
const LEGACY_THEME_FILE: &str = "theme.cfg";

/// The platform's config directory for this app, or `None` if the environment
/// does not name one.
///
/// Windows uses `%APPDATA%`, macOS `~/Library/Application Support`, and
/// everything else the XDG convention — `$XDG_CONFIG_HOME`, falling back to
/// `$HOME/.config`.
pub fn config_dir() -> Option<PathBuf> {
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

/// Full path to the preferences file.
fn config_path() -> Option<PathBuf> {
    Some(config_dir()?.join(CONFIG_FILE))
}

/// Full path to the legacy one-word theme file.
fn legacy_theme_path() -> Option<PathBuf> {
    Some(config_dir()?.join(LEGACY_THEME_FILE))
}

/// Read the saved preferences. Absent, unreadable, or garbage → the defaults,
/// key by key.
///
/// The migration lives here and nowhere else: with no `config.cfg` the legacy
/// one-word `theme.cfg` still speaks for the theme, and for nothing else — it
/// never held anything else to say.
pub fn load() -> Config {
    if let Some(text) = config_path().and_then(|path| std::fs::read_to_string(path).ok()) {
        return parse(&text);
    }

    let mut config = Config::default();
    if let Some(word) = legacy_theme_path().and_then(|path| std::fs::read_to_string(path).ok()) {
        config.theme = Theme::from_word(&word);
    }
    config
}

/// Persist every preference, ignoring every failure.
///
/// A read-only config directory, a full disk, a locked file — none of these are
/// worth interrupting someone who just wanted a different colour scheme. The
/// preference simply does not survive the session.
///
/// Always writes the *whole* file: a setter that rewrote one line would have to
/// parse and re-emit the rest, and re-emitting everything from the in-memory
/// [`Config`] is both shorter and impossible to get half-right.
pub fn save(config: &Config) {
    let Some(path) = config_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, render(config));
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

    fn write_raw(name: &str, contents: &str) {
        let path = config_dir().unwrap().join(name);
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
    fn defaults_are_off_pace_and_eighty() {
        let config = Config::default();
        assert_eq!(config.theme, Theme::CipherPine);
        assert!(!config.history, "history must be opt-in");
        assert!(!config.alerts, "alerts must be opt-in");
        assert_eq!(config.alert_at, 80);
        assert_eq!(config.alert_mode, AlertMode::Pace);
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

    #[test]
    fn an_empty_or_garbage_file_parses_to_the_defaults() {
        for text in [
            "",
            "\n\n\n",
            "   ",
            "# only a comment\n",
            "nonsense",
            "{\"theme\":\"plain\"}",
            "=\n==\n=value\n",
            "\u{0}\u{1}",
        ] {
            assert_eq!(
                parse(text),
                Config::default(),
                "{text:?} should parse to the defaults"
            );
        }
    }

    #[test]
    fn every_key_parses() {
        let config =
            parse("theme=plain\nhistory=on\nalerts=on\nalert_at=42\nalert_mode=threshold\n");
        assert_eq!(
            config,
            Config {
                theme: Theme::Plain,
                history: true,
                alerts: true,
                alert_at: 42,
                alert_mode: AlertMode::Threshold,
            }
        );
    }

    #[test]
    fn keys_and_values_are_trimmed_and_case_folded() {
        let config = parse("  THEME  =  Plain  \n\tHistory\t=\tON\t\nALERT_MODE=Threshold\n");
        assert_eq!(config.theme, Theme::Plain);
        assert!(config.history);
        assert_eq!(config.alert_mode, AlertMode::Threshold);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let config = parse("# theme=plain\n\n   \nhistory=on\n#alerts=on\n");
        assert_eq!(
            config.theme,
            Theme::CipherPine,
            "a commented key is not a key"
        );
        assert!(config.history);
        assert!(!config.alerts, "a commented key is not a key");
    }

    #[test]
    fn unknown_keys_are_ignored() {
        // Forward compatibility: a future build's key must not disturb the
        // keys this build does understand.
        let config = parse("theme=plain\nwindow_position=12,34\nalert_sound=chime\n");
        assert_eq!(config.theme, Theme::Plain);
        assert_eq!(config.history, Config::default().history);
        assert_eq!(config.alerts, Config::default().alerts);
    }

    #[test]
    fn a_garbage_value_keeps_that_key_at_its_default() {
        let config = parse("theme=neon\nhistory=maybe\nalerts=yes\nalert_mode=whenever\n");
        assert_eq!(config, Config::default());
    }

    #[test]
    fn alert_at_out_of_range_or_garbage_is_eighty() {
        for value in [
            "0",
            "101",
            "-1",
            "999999999999999999999",
            "",
            "80%",
            "eighty",
            "79.5",
        ] {
            assert_eq!(
                parse(&format!("alert_at={value}")).alert_at,
                80,
                "alert_at={value:?} should fall back to 80"
            );
        }
        // The ends of the range are in it.
        assert_eq!(parse("alert_at=1").alert_at, 1);
        assert_eq!(parse("alert_at=100").alert_at, 100);
    }

    #[test]
    fn the_last_line_for_a_key_wins() {
        // No ordering rule is documented, but a duplicated key must resolve
        // deterministically rather than by chance.
        assert_eq!(
            parse("theme=plain\ntheme=cipherpine\n").theme,
            Theme::CipherPine
        );
    }

    #[test]
    fn render_emits_every_key_under_a_two_line_header() {
        let text = render(&Config {
            theme: Theme::Plain,
            history: true,
            alerts: true,
            alert_at: 65,
            alert_mode: AlertMode::Threshold,
        });
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 7, "two header lines plus five keys: {text:?}");
        assert!(lines[0].starts_with('#'));
        assert!(lines[1].starts_with('#'));
        assert_eq!(
            &lines[2..],
            [
                "theme=plain",
                "history=on",
                "alerts=on",
                "alert_at=65",
                "alert_mode=threshold",
            ]
        );
        // The header says what the file is for, and what it is not for.
        assert!(text.contains("Never credentials"), "{text:?}");
    }

    #[test]
    fn render_then_parse_is_identity() {
        for config in [
            Config::default(),
            Config {
                theme: Theme::Plain,
                history: true,
                alerts: true,
                alert_at: 1,
                alert_mode: AlertMode::Threshold,
            },
            Config {
                theme: Theme::CipherPine,
                history: false,
                alerts: true,
                alert_at: 100,
                alert_mode: AlertMode::Pace,
            },
        ] {
            assert_eq!(
                parse(&render(&config)),
                config,
                "{config:?} did not survive"
            );
        }
    }

    /// All filesystem cases in one test: the config-dir override is a global
    /// env var, so running them concurrently would let one test's temp dir
    /// leak into another's `load()`.
    #[test]
    fn config_file_round_trips_migrates_and_degrades() {
        // --- save then load is identity, for every value ---
        {
            let _tmp = TempConfig::new("roundtrip");
            for config in [
                Config::default(),
                Config {
                    theme: Theme::Plain,
                    history: true,
                    alerts: true,
                    alert_at: 55,
                    alert_mode: AlertMode::Threshold,
                },
            ] {
                save(&config);
                assert_eq!(load(), config, "{config:?} did not survive a round trip");
            }
            // Saving writes config.cfg and leaves theme.cfg alone.
            assert!(config_path().unwrap().exists());
            assert!(!legacy_theme_path().unwrap().exists());
        }

        // --- absent file → defaults ---
        {
            let _tmp = TempConfig::new("absent");
            assert!(!config_path().unwrap().exists());
            assert_eq!(load(), Config::default());
        }

        // --- migration: no config.cfg, legacy theme.cfg speaks for the theme ---
        {
            let _tmp = TempConfig::new("migrate");
            write_raw(LEGACY_THEME_FILE, "plain");
            assert_eq!(
                load(),
                Config {
                    theme: Theme::Plain,
                    ..Config::default()
                },
                "the legacy file should still choose the theme, and only the theme"
            );

            // Once config.cfg exists it is the only file consulted — the legacy
            // word does not come back to override it.
            save(&Config {
                theme: Theme::CipherPine,
                ..Config::default()
            });
            assert_eq!(load().theme, Theme::CipherPine);
            // And the legacy file is still there, untouched.
            assert_eq!(
                std::fs::read_to_string(legacy_theme_path().unwrap()).unwrap(),
                "plain"
            );
        }

        // --- garbage contents → defaults, no panic ---
        {
            let _tmp = TempConfig::new("garbage");
            for junk in ["", "nonsense", "{\"theme\":\"plain\"}", "\u{0}\u{1}"] {
                write_raw(CONFIG_FILE, junk);
                assert_eq!(
                    load(),
                    Config::default(),
                    "junk {junk:?} broke the fallback"
                );
            }
        }

        // --- unreadable path (a directory where the file should be) → defaults ---
        {
            let _tmp = TempConfig::new("unreadable");
            std::fs::create_dir_all(config_path().unwrap()).unwrap();
            assert_eq!(load(), Config::default());
            // And saving over it fails silently rather than panicking.
            save(&Config::default());
        }

        // --- save creates the directory when missing ---
        {
            let _tmp = TempConfig::new("mkdir");
            let nested = _tmp.dir.join("deeper");
            unsafe { std::env::set_var(CONFIG_DIR_OVERRIDE, &nested) };
            let config = Config {
                theme: Theme::Plain,
                ..Config::default()
            };
            save(&config);
            assert_eq!(load(), config);
        }
    }
}
