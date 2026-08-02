//! Ingress trust boundary (TB1): read-only credential loading.
//!
//! Secrets enter the process here and are wrapped in [`Secret`] before
//! anything else can observe them. This module **never writes** to a
//! credential file (SECURITY.md invariant 6); token refresh is delegated
//! to the official `claude` / `codex` CLIs (implemented in M1+).

mod secret;

pub use secret::Secret;

use std::fs::OpenOptions;
use std::io::Read;
use std::path::{Path, PathBuf};

/// Errors surfaced by credential loading. Never contains secret bytes.
#[derive(Debug)]
pub enum CredentialError {
    /// The credential file was not found at the resolved path.
    NotFound(PathBuf),
    /// The credential file exists but could not be read.
    Io {
        /// Path of the file that failed to read.
        path: PathBuf,
        /// The underlying I/O error kind (kind only — no OS message that
        /// could echo file contents).
        kind: std::io::ErrorKind,
    },
}

impl std::fmt::Display for CredentialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CredentialError::NotFound(p) => write!(f, "credential file not found: {}", p.display()),
            CredentialError::Io { path, kind } => {
                write!(
                    f,
                    "failed to read credential file {}: {kind}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for CredentialError {}

/// Resolve the default Claude Code credential path (`~/.claude/.credentials.json`).
///
/// Returns `None` when no home directory can be determined.
pub fn claude_credentials_path() -> Option<PathBuf> {
    home_dir().map(|h| h.join(".claude").join(".credentials.json"))
}

/// Resolve the Codex credential path: `$CODEX_HOME/auth.json` if `CODEX_HOME`
/// is set (and non-empty), else `~/.codex/auth.json`.
pub fn codex_credentials_path() -> Option<PathBuf> {
    match std::env::var_os("CODEX_HOME") {
        Some(dir) if !dir.is_empty() => Some(PathBuf::from(dir).join("auth.json")),
        _ => home_dir().map(|h| h.join(".codex").join("auth.json")),
    }
}

/// Load the raw contents of a credential file, **read-only**, wrapped in
/// [`Secret`] before return.
///
/// Invariants upheld here (THREAT_MODEL.md §9):
/// - invariant 6: the file is opened with a read-only handle
///   (`OpenOptions::new().read(true)` — no write, append, create, or truncate).
/// - invariant 2: the bytes are wrapped in [`Secret`] before this function
///   returns; no copy is logged or persisted.
pub fn load_credential_file(path: &Path) -> Result<Secret<String>, CredentialError> {
    if !path.exists() {
        return Err(CredentialError::NotFound(path.to_path_buf()));
    }
    // Read-only open. Never add write(true)/create(true)/truncate(true) here:
    // that would violate SECURITY.md invariant 6 and is a breaking security change.
    let mut file = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|e| CredentialError::Io {
            path: path.to_path_buf(),
            kind: e.kind(),
        })?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .map_err(|e| CredentialError::Io {
            path: path.to_path_buf(),
            kind: e.kind(),
        })?;
    Ok(Secret::new(buf))
}

/// Best-effort home directory resolution without extra dependencies.
fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// A synthetic token for tests. NEVER place a real credential in this
    /// repository, including in tests (SECURITY.md, supply-chain policy).
    const SYNTHETIC_TOKEN: &str = "sk-synthetic-0000-TEST-FIXTURE-not-a-real-token";

    fn fixture_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("quotapane-test-{}-{name}", std::process::id()));
        p
    }

    // INV:1,6 — registered in invariants.manifest (checked in CI)
    #[test]
    fn loads_credential_readonly_and_redacted() {
        let path = fixture_path("cred.json");
        std::fs::File::create(&path)
            .unwrap()
            .write_all(SYNTHETIC_TOKEN.as_bytes())
            .unwrap();
        let before = std::fs::read(&path).unwrap();

        let secret = load_credential_file(&path).unwrap();

        // Invariant 6: the file is byte-identical after loading — no write path.
        let after = std::fs::read(&path).unwrap();
        assert_eq!(
            before, after,
            "credential file must not be modified by loading"
        );

        // Invariant 2: Debug output never contains the token bytes.
        let debug = format!("{secret:?}");
        assert!(!debug.contains(SYNTHETIC_TOKEN), "Debug leaked the token");
        assert!(debug.contains("redacted"));

        // The secret is still usable inside the boundary.
        assert_eq!(secret.expose(), SYNTHETIC_TOKEN);

        std::fs::remove_file(&path).unwrap();
    }

    // INV:2 — registered in invariants.manifest (checked in CI)
    #[test]
    fn missing_file_is_a_clean_error_without_secret_bytes() {
        let path = fixture_path("does-not-exist.json");
        let err = load_credential_file(&path).unwrap_err();
        match err {
            CredentialError::NotFound(p) => assert_eq!(p, path),
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    // INV:2 — registered in invariants.manifest (checked in CI)
    #[test]
    fn error_display_never_panics_and_has_no_token() {
        let err = CredentialError::Io {
            path: PathBuf::from("/tmp/x"),
            kind: std::io::ErrorKind::PermissionDenied,
        };
        let msg = format!("{err} / {err:?}");
        assert!(!msg.contains(SYNTHETIC_TOKEN));
    }
}
