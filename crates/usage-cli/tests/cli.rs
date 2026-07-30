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
