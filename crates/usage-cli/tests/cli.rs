//! End-to-end checks on the real `quotapane-cli` executable.
//!
//! The unit tests in `main.rs` cover parsing; these cover the thing a stranger
//! actually experiences — the process exit code and what lands on stdout. The
//! gap report found `--help` exiting 2 as an unrecognized argument, which unit
//! tests alone could not have caught.
//!
//! `CARGO_BIN_EXE_quotapane-cli` only resolves if the binary is named
//! `quotapane-cli`, so this file also pins the D3 rename: revert the `[[bin]]`
//! block and this test stops compiling.
//!
//! Most invocations here short-circuit before any network or credential
//! access. The proxy-gate tests (M9b) go one step further on purpose: they
//! point `CODEX_HOME` at a temp directory holding a **synthetic** `auth.json`
//! so the run gets past credential loading and reaches the egress chokepoint,
//! where the proxy gate refuses it *before any bytes are sent*. No real token
//! is read and no request leaves the machine.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_quotapane-cli");

/// Proxy variables the egress gate watches, in both spellings. Cleared from a
/// child's environment before a test sets the one it is actually exercising,
/// so an inherited variable on the developer's machine or a CI runner cannot
/// make a proxy test pass for the wrong reason.
const PROXY_VARS: &[&str] = &[
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "all_proxy",
];

/// Synthetic Codex credentials — recognizably fake bytes, never a real token,
/// and deliberately not shaped like one so secret scanners have nothing to
/// flag (CLAUDE.md: fixtures use synthetic tokens only).
const SYNTHETIC_AUTH_JSON: &str = r#"{"tokens":{"access_token":"synthetic-not-a-real-token-DO-NOT-USE","account_id":"synthetic-account-id"}}"#;

/// Write a synthetic `auth.json` into a fresh temp dir and return the dir, for
/// use as `CODEX_HOME`. Named per test so parallel runs cannot collide.
fn synthetic_codex_home(tag: &str) -> std::path::PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(format!("quotapane-cli-{}-{}", tag, std::process::id()));
    std::fs::create_dir_all(&dir).expect("failed to create temp CODEX_HOME");
    std::fs::write(dir.join("auth.json"), SYNTHETIC_AUTH_JSON).expect("failed to write auth.json");
    dir
}

/// Run the CLI against a synthetic Codex credential with exactly one proxy
/// variable set, and return (exit code, stderr).
fn run_with_proxy_var(tag: &str, var: &str, extra_args: &[&str]) -> (Option<i32>, String) {
    let home = synthetic_codex_home(tag);
    let mut cmd = Command::new(BIN);
    cmd.args(["--once", "--provider", "codex"])
        .args(extra_args)
        .env("CODEX_HOME", &home);
    for name in PROXY_VARS {
        cmd.env_remove(name);
    }
    cmd.env(var, "http://127.0.0.1:9");

    let out = cmd.output().expect("failed to run quotapane-cli");
    let _ = std::fs::remove_dir_all(&home);
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn help_exits_zero_and_prints_real_usage() {
    for flag in ["--help", "-h"] {
        let out = Command::new(BIN)
            .arg(flag)
            .output()
            .expect("failed to run quotapane-cli");

        assert!(
            out.status.success(),
            "`{flag}` exited with {:?}, expected 0",
            out.status.code()
        );

        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            stdout.contains("usage: quotapane-cli"),
            "`{flag}` printed no usage line: {stdout}"
        );
        // A help text that documents nothing would satisfy "exits 0" alone.
        for expected in ["--once", "--json", "--provider", "--debug-raw"] {
            assert!(
                stdout.contains(expected),
                "`{flag}` output omits {expected}: {stdout}"
            );
        }
    }
}

/// M9b: both debug-raw flags reach the user's screen, and the help text says
/// which one is the safe default rather than leaving them to guess from the
/// names. A new test rather than an edit to the one above, which pins the
/// pre-M9b surface.
#[test]
fn help_lists_both_debug_raw_flags_and_states_the_default() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run quotapane-cli");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(stdout.contains("--debug-raw-unsafe"), "{stdout}");
    // The redaction default is described, with the keys it covers named.
    assert!(stdout.contains("«redacted»"), "{stdout}");
    for key in ["email", "user_id", "account_id"] {
        assert!(
            stdout.contains(key),
            "help omits the redacted key {key}: {stdout}"
        );
    }
    assert!(stdout.contains("withheld"), "{stdout}");
}

#[test]
fn version_exits_zero_and_prints_the_workspace_version() {
    let out = Command::new(BIN)
        .arg("--version")
        .output()
        .expect("failed to run quotapane-cli");

    assert!(
        out.status.success(),
        "`--version` exited with {:?}, expected 0",
        out.status.code()
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains(env!("CARGO_PKG_VERSION")),
        "`--version` did not print the version: {stdout}"
    );
}

// --- M9b: the proxy gate, end to end ---

// INV:7 — registered in invariants.manifest (checked in CI)
#[test]
fn proxy_env_without_the_flag_fails_closed_and_prints_the_hint() {
    // The gate itself is egress's (SECURITY.md invariant 7) and is untouched
    // here: the run must still FAIL. What M9b adds is the one line telling the
    // user how to proceed.
    let (code, stderr) = run_with_proxy_var("uppercase", "HTTPS_PROXY", &[]);

    assert_eq!(code, Some(1), "a refused run must exit non-zero: {stderr}");
    assert!(
        stderr.contains("egress denied: proxy environment"),
        "expected the egress refusal: {stderr}"
    );
    assert!(
        stderr.contains("hint: re-run with --allow-proxy to opt in, or unset the proxy variable."),
        "expected the hint line after the error: {stderr}"
    );
    // The error names the variable; the hint does not re-enumerate names.
    assert!(stderr.contains("HTTPS_PROXY"), "{stderr}");
    // Nothing here claims the variable was ignored — it was not.
    assert!(
        !stderr.to_lowercase().contains("ignored"),
        "the output must not claim the proxy environment was ignored: {stderr}"
    );
}

// INV:7 — registered in invariants.manifest (checked in CI)
#[test]
fn lowercase_proxy_env_fails_closed_too() {
    // Pins the invariant's "upper- or lowercase" claim. The helper clears the
    // uppercase spellings first, so this can only pass because of `https_proxy`.
    let (code, stderr) = run_with_proxy_var("lowercase", "https_proxy", &[]);

    assert_eq!(code, Some(1), "a refused run must exit non-zero: {stderr}");
    assert!(
        stderr.contains("egress denied: proxy environment"),
        "lowercase proxy variable did not trip the gate: {stderr}"
    );
    assert!(
        stderr.contains("hint: re-run with --allow-proxy"),
        "expected the hint line: {stderr}"
    );
}

// INV:7 — registered in invariants.manifest (checked in CI)
#[test]
fn allow_proxy_prints_the_token_visibility_warning_and_passes_the_gate() {
    // With the flag the gate no longer refuses, so the run proceeds past it
    // and fails later — at the transport, dialing the closed discard port this
    // test points the proxy at. That "different failure" IS the observation:
    // `Egress::new(true)` was reached.
    let (_code, stderr) = run_with_proxy_var("optin", "HTTPS_PROXY", &["--allow-proxy"]);

    assert!(
        stderr.contains("warning: --allow-proxy"),
        "expected the opt-in warning: {stderr}"
    );
    assert!(
        stderr.contains("bearer token"),
        "the warning must say what is at risk: {stderr}"
    );
    assert!(
        !stderr.contains("without explicit opt-in"),
        "the gate must not refuse a run that opted in: {stderr}"
    );
    assert!(
        !stderr.contains("hint: re-run with --allow-proxy"),
        "no hint when the flag is already on: {stderr}"
    );
    // Reaching the transport is the proof that `Egress::new(true)` was
    // constructed: the request was attempted *through the proxy* (the closed
    // discard port this test names), so nothing was sent to the real host.
    assert!(
        stderr.contains("egress transport error"),
        "expected the run to get past the gate and fail at the proxy connection: {stderr}"
    );
}

#[test]
fn unknown_flag_still_errors() {
    let out = Command::new(BIN)
        .arg("--definitely-not-a-flag")
        .output()
        .expect("failed to run quotapane-cli");

    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown flag must still exit 2"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unrecognized argument"),
        "expected the unrecognized-argument diagnostic, got: {stderr}"
    );
}

// --- M12 P1: --fail-at ---

/// The conflicts, through the real binary. Deliberately **not** a test of a
/// successful `--check-update` run: that dials `api.github.com`, and no test in
/// this repository is allowed to make a network request. The three outcomes are
/// covered as pure values in the unit tests instead.
#[test]
fn check_update_combined_with_another_flag_exits_two_without_asking_anyone() {
    for extra in [
        vec!["--once"],
        vec!["--watch", "300"],
        vec!["--json"],
        vec!["--statusline"],
        vec!["--client-version", "2.1.90"],
        vec!["--allow-proxy"],
    ] {
        let mut command = Command::new(BIN);
        command.arg("--check-update");
        for a in &extra {
            command.arg(a);
        }
        let out = command.output().expect("failed to run quotapane-cli");
        assert_eq!(
            out.status.code(),
            Some(2),
            "--check-update {extra:?} should be a usage error"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("cannot be combined with"),
            "{extra:?} produced: {stderr}"
        );
    }
}

#[test]
fn help_lists_the_update_check_on_its_own_usage_line() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run quotapane-cli");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("       quotapane-cli --check-update\n"),
        "the update check needs its own usage line: {stdout}"
    );
    assert!(
        stdout.contains("--check-update          Ask GitHub"),
        "{stdout}"
    );
}

#[test]
fn help_prints_the_exit_codes_block() {
    // The gate is only usable if a script author can look up what 3 means.
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run quotapane-cli");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        stdout.contains(
            "\
exit codes:
  0  success; with --fail-at: all windows under the threshold
  1  a provider or credential error; with --check-update: the check failed
  2  usage error
  3  --fail-at tripped: a window reached the threshold
"
        ),
        "the exit-code section is missing or reworded: {stdout}"
    );
    assert!(stdout.contains("--fail-at <N>"), "{stdout}");
}

#[test]
fn fail_at_outside_one_to_hundred_exits_two_before_polling() {
    // Rejected at parse time, so nothing is read and nothing is sent — a
    // mistyped threshold costs a usage error, not a request.
    for bad in ["0", "101", "ninety"] {
        let out = Command::new(BIN)
            .args(["--once", "--fail-at", bad])
            .output()
            .expect("failed to run quotapane-cli");

        assert_eq!(
            out.status.code(),
            Some(2),
            "`--fail-at {bad}` must exit 2, got {:?}",
            out.status.code()
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("--fail-at must be a whole number from 1 to 100"),
            "expected the range diagnostic for {bad}: {stderr}"
        );
    }

    // A missing value is a usage error too, not a threshold of "nothing".
    let out = Command::new(BIN)
        .args(["--once", "--fail-at"])
        .output()
        .expect("failed to run quotapane-cli");
    assert_eq!(out.status.code(), Some(2));
}

// --- M12 P2: --watch, the second mode ---

#[test]
fn help_lists_the_watch_mode_and_the_two_mode_usage_line() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run quotapane-cli");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(stdout.contains("--watch <SECS>"), "{stdout}");
    assert!(
        stdout.contains("usage: quotapane-cli (--once | --watch <SECS>)"),
        "the usage line must show two modes, not one: {stdout}"
    );
    // The floor and the NDJSON behavior are both things a script author needs
    // before writing the loop.
    assert!(stdout.contains("180"), "{stdout}");
    assert!(stdout.contains("NDJSON"), "{stdout}");
}

#[test]
fn watch_below_the_polling_floor_exits_two_with_the_floor_message() {
    // Rejected at parse time — the floor cannot be argued past by starting the
    // run and rate-limiting later.
    for bad in ["1", "60", "179"] {
        let out = Command::new(BIN)
            .args(["--watch", bad])
            .output()
            .expect("failed to run quotapane-cli");

        assert_eq!(
            out.status.code(),
            Some(2),
            "`--watch {bad}` must exit 2, got {:?}",
            out.status.code()
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("--watch interval must be at least 180 seconds (the polling floor)"),
            "expected the floor message verbatim for {bad}: {stderr}"
        );
    }
}

#[test]
fn the_two_modes_cannot_be_combined() {
    let out = Command::new(BIN)
        .args(["--once", "--watch", "300"])
        .output()
        .expect("failed to run quotapane-cli");

    assert_eq!(
        out.status.code(),
        Some(2),
        "--once with --watch must be a usage error"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("--once and --watch cannot be combined"),
        "expected the mode-conflict diagnostic: {stderr}"
    );
}

// --- M18a: --statusline, end to end through a real pipe ---

/// Run `--statusline` with `payload` on stdin and return (exit code, stdout).
///
/// stdin is always a pipe, never inherited: the mode reads to EOF, and a test
/// that let it inherit the harness's stdin could block forever.
fn run_statusline(payload: &str, extra_args: &[&str]) -> (Option<i32>, String) {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new(BIN)
        .arg("--statusline")
        .args(extra_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn quotapane-cli");
    child
        .stdin
        .take()
        .expect("stdin was not piped")
        .write_all(payload.as_bytes())
        .expect("failed to write the payload");

    let out = child
        .wait_with_output()
        .expect("failed to run quotapane-cli");
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

#[test]
fn statusline_prints_one_line_from_a_real_payload_and_exits_zero() {
    // The documented shape, with a reset far enough out that the countdown is
    // stable whenever this test runs: `resets_at` is read against the real
    // clock here, so the assertion checks the segments, not the exact minutes.
    let resets_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before 1970")
        .as_secs()
        + 7_800;
    let payload = format!(
        r#"{{"cwd":"/tmp","model":{{"id":"claude-opus-5"}},"rate_limits":{{
             "five_hour":{{"used_percentage":12,"resets_at":{resets_at}}},
             "seven_day":{{"used_percentage":83.4,"resets_at":{resets_at}}}}}}}"#
    );

    let (code, stdout) = run_statusline(&payload, &[]);

    assert_eq!(code, Some(0), "a statusline must exit 0: {stdout}");
    assert_eq!(
        stdout.lines().count(),
        1,
        "the host displays one line; got: {stdout:?}"
    );
    let line = stdout.trim_end();
    assert!(line.starts_with("5h 12% · 7d 83%!"), "got: {line:?}");
    assert!(line.contains("· resets "), "got: {line:?}");
    // Nothing from the rest of the payload came along for the ride.
    assert!(!line.contains("/tmp") && !line.contains("opus"), "{line:?}");
}

#[test]
fn statusline_survives_garbage_and_quota_less_payloads_with_exit_zero() {
    // Every one of these is a real field condition: no rate_limits before the
    // first API response, the #40094 plan/auth gap, and a host that piped
    // something unexpected. None of them may break the status bar.
    for payload in [
        "",
        "not json",
        r#"{"cwd":"/tmp"}"#,
        r#"{"rate_limits":{}}"#,
        r#"{"rate_limits":null}"#,
    ] {
        let (code, stdout) = run_statusline(payload, &[]);
        assert_eq!(code, Some(0), "{payload:?} must still exit 0");
        assert!(
            stdout.trim().is_empty(),
            "{payload:?} must print nothing, got: {stdout:?}"
        );
    }
}

#[test]
fn statusline_combined_with_a_polling_flag_exits_two() {
    for (flag, value) in [
        ("--once", None),
        ("--json", None),
        ("--fail-at", Some("85")),
        ("--allow-proxy", None),
    ] {
        let mut cmd = Command::new(BIN);
        cmd.arg("--statusline").arg(flag);
        if let Some(v) = value {
            cmd.arg(v);
        }
        let out = cmd.output().expect("failed to run quotapane-cli");

        assert_eq!(
            out.status.code(),
            Some(2),
            "`--statusline {flag}` must be a usage error"
        );
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("--statusline cannot be combined with") && stderr.contains(flag),
            "expected the conflict diagnostic naming {flag}: {stderr}"
        );
    }
}

#[test]
fn help_lists_the_statusline_mode_on_its_own_usage_line() {
    let out = Command::new(BIN)
        .arg("--help")
        .output()
        .expect("failed to run quotapane-cli");
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        stdout.contains("       quotapane-cli --statusline"),
        "the statusline mode needs its own synopsis line: {stdout}"
    );
    // Both claims a reader must be able to check before wiring it into a
    // settings file: it sends nothing, and its output is not the contract.
    assert!(stdout.contains("sends nothing"), "{stdout}");
    assert!(stdout.contains("stability"), "{stdout}");
}

#[test]
fn missing_required_mode_still_errors() {
    let out = Command::new(BIN)
        .output()
        .expect("failed to run quotapane-cli");

    assert_eq!(
        out.status.code(),
        Some(2),
        "a bare invocation must still exit 2"
    );
}
