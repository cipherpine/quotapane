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
//! Every invocation here short-circuits before any network or credential
//! access, so these tests touch no real tokens and make no requests.

use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_quotapane-cli");

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
