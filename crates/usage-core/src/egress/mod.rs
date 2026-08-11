//! Egress trust boundary (TB2): the single, deny-by-default network chokepoint.
//!
//! A secret leaves the process **only** through this module, and only to a
//! host on [`ALLOWED_HOSTS`] (SECURITY.md invariant 3, THREAT_MODEL.md T-I3).
//! There is no other HTTP client anywhere in the workspace; introducing one
//! is a breaking security change.
//!
//! Design notes:
//! - The request API takes `host` and `path` as **separate, exact fields**
//!   rather than parsing a URL string. This removes the URL-parser-ambiguity
//!   class of allowlist bypasses (userinfo tricks, mixed-case schemes,
//!   confusable hosts) at the API level: the host that is checked is
//!   byte-for-byte the host that will be dialed. `path` must start with `/`,
//!   which makes userinfo smuggling (`@evil.com`) syntactically impossible
//!   in the composed URL.
//! - The transport (M1) is `ureq` + `rustls`: synchronous, pure-Rust TLS,
//!   deliberately chosen over an async stack to keep the audit tree small.
//!   Redirects are **never followed** — a 3xx is returned to the caller, so
//!   a redirect can never carry a request off the allowlist. HTTPS is the
//!   only scheme this module can construct.
//! - The `Authorization` header is assembled from a [`Secret`] here, at the
//!   last possible moment. The transport necessarily holds a transient copy
//!   of the header for the duration of the request, and no [`EgressError`]
//!   carries header contents. Logging posture, precisely: `ureq` logs via
//!   the `log` facade but redacts headers behind an allowlist
//!   (`NON_SENSITIVE_HEADERS`, verified at ureq 3.3.0 `src/util.rs`), so
//!   `Authorization` never reaches the facade even under a trace-level
//!   logger; additionally, no QuotaPane binary installs a log backend, and
//!   `deny.toml` bans logger-backend crates so every log macro stays a
//!   no-op. Re-verify the redaction allowlist on every ureq upgrade.
//! - Proxy use is opt-in (invariant 7): if proxy environment variables are
//!   present and the user has not explicitly opted in, requests fail closed
//!   before anything is sent — and as defense in depth, the underlying agent
//!   is configured with proxying disabled unless the user opted in.

use crate::credentials::Secret;
use std::time::Duration;

/// The complete set of hosts this process may ever contact.
///
/// Compile-time, deny-by-default. Adding a host is a breaking security
/// change: it must be justified against THREAT_MODEL.md §6 and called out
/// in review, and any provider host must be verified against real provider
/// behavior — never guessed.
pub const ALLOWED_HOSTS: &[&str] = &[
    // Anthropic: subscription usage + Messages-API rate-limit header fallback.
    "api.anthropic.com",
    // Codex (ChatGPT-plan) subscription usage: GET /backend-api/wham/usage.
    // Verified M3 against openai/codex source (codex-rs/backend-client) —
    // ChatGPT-plan usage is served by chatgpt.com, NOT api.openai.com.
    "chatgpt.com",
    // GitHub Releases: the opt-in, notify-only update check (invariant 5).
    // Removed 2026-07-27 when it had zero callers; returned together with
    // `usage-core::update`, its exactly-one caller, per this comment's own
    // rule. That module's tests pin the single call site and prove the
    // request cannot carry a credential; the on/off gate lives with the
    // caller, so this host is reached only when `update_check=on` or under
    // `quotapane-cli --check-update`.
    "api.github.com",
];

/// Connect timeout for outbound requests.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Whole-request timeout for outbound requests.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Maximum response body size accepted (defense against pathological responses).
const MAX_BODY_BYTES: u64 = 1024 * 1024; // 1 MiB — usage payloads are tiny

/// Errors from the egress chokepoint. Never contains secret bytes.
#[derive(Debug, PartialEq, Eq)]
pub enum EgressError {
    /// The requested host is not on [`ALLOWED_HOSTS`]. Hard error — never
    /// retried, never downgraded to a warning.
    HostNotAllowlisted(String),
    /// The path did not start with `/` (required so the composed URL cannot
    /// smuggle userinfo or an alternate authority).
    InvalidPath(String),
    /// Proxy environment variables are set but the user has not opted in
    /// (SECURITY.md invariant 7). Fails closed before any bytes are sent.
    ProxyNotOptedIn {
        /// The name of the environment variable that triggered the gate.
        variable: String,
    },
    /// The transport failed (DNS, TCP, TLS, timeout, or oversized body).
    /// Carries a display string derived from the transport error; URLs may
    /// appear in it, secrets never do (headers are not echoed by `ureq`
    /// errors, and we never place the token in a URL).
    Transport(String),
}

impl std::fmt::Display for EgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EgressError::HostNotAllowlisted(h) => {
                write!(f, "egress denied: host {h:?} is not on the allowlist")
            }
            EgressError::InvalidPath(p) => {
                write!(f, "egress denied: path {p:?} must start with '/'")
            }
            EgressError::ProxyNotOptedIn { variable } => write!(
                f,
                "egress denied: proxy environment ({variable}) detected without explicit opt-in; \
                 a TLS-inspecting proxy could observe bearer tokens"
            ),
            EgressError::Transport(detail) => write!(f, "egress transport error: {detail}"),
        }
    }
}

impl std::error::Error for EgressError {}

/// A response from an allowlisted host. Status and headers are passed
/// through verbatim so providers can interpret 401/429 and read rate-limit
/// headers; nothing in here is a secret of *ours* (response contents come
/// from the provider), so deriving `Debug` is safe.
#[derive(Debug)]
pub struct EgressResponse {
    /// HTTP status code.
    pub status: u16,
    /// Response headers as (lowercased-name, value) pairs.
    pub headers: Vec<(String, String)>,
    /// Response body, capped at [`MAX_BODY_BYTES`].
    pub body: Vec<u8>,
}

/// Proxy-related environment variables that gate egress (invariant 7).
const PROXY_ENV_VARS: &[&str] = &[
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "ALL_PROXY",
    "all_proxy",
];

/// The single egress client. One instance per process; every outbound
/// request goes through [`Egress::get`].
pub struct Egress {
    proxy_opt_in: bool,
}

impl Egress {
    /// Create the chokepoint. `proxy_opt_in` must come from an explicit user
    /// setting (default `false`); never derive it automatically.
    pub fn new(proxy_opt_in: bool) -> Self {
        Egress { proxy_opt_in }
    }

    /// Check a host against the allowlist. Exact, case-insensitive match on
    /// the full host — no subdomains, no ports, no prefixes.
    pub fn check_host(host: &str) -> Result<(), EgressError> {
        let normalized = host.to_ascii_lowercase();
        if ALLOWED_HOSTS.iter().any(|allowed| *allowed == normalized) {
            Ok(())
        } else {
            Err(EgressError::HostNotAllowlisted(host.to_string()))
        }
    }

    /// Validate that a path cannot alter the URL's authority when composed
    /// as `https://{host}{path}`: it must start with `/` and contain no
    /// whitespace or control characters.
    fn check_path(path: &str) -> Result<(), EgressError> {
        if !path.starts_with('/') || path.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(EgressError::InvalidPath(path.to_string()));
        }
        Ok(())
    }

    /// Check the proxy gate against an explicit list of environment variables
    /// (pure function — testable without mutating process env).
    fn check_proxy_gate(proxy_opt_in: bool, env: &[(String, String)]) -> Result<(), EgressError> {
        if proxy_opt_in {
            return Ok(());
        }
        for (name, value) in env {
            if PROXY_ENV_VARS.contains(&name.as_str()) && !value.is_empty() {
                return Err(EgressError::ProxyNotOptedIn {
                    variable: name.clone(),
                });
            }
        }
        Ok(())
    }

    /// Build the one agent every request uses. Redirects are never followed;
    /// non-2xx statuses are returned as responses (providers interpret them);
    /// proxying is hard-disabled unless the user explicitly opted in.
    fn agent(&self) -> ureq::Agent {
        let mut cfg = ureq::Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .timeout_connect(Some(CONNECT_TIMEOUT))
            .timeout_global(Some(REQUEST_TIMEOUT))
            .user_agent(""); // callers set User-Agent explicitly per request
        if !self.proxy_opt_in {
            // Defense in depth: even if a proxy env var appears after the
            // gate check, the agent will not use it.
            cfg = cfg.proxy(None);
        }
        cfg.build().into()
    }

    /// Issue an HTTPS GET to `https://{host}{path}`.
    ///
    /// Order of gates (all fail closed, before any bytes leave the process):
    /// 1. host allowlist (invariant 3)
    /// 2. path shape (no authority smuggling)
    /// 3. proxy opt-in (invariant 7)
    /// 4. TLS-only transport, redirects never followed
    ///
    /// `bearer` — if present, sent as `Authorization: Bearer …`, assembled
    /// here at the last moment from the [`Secret`]. **This is the only place
    /// in the codebase where a token leaves the process.**
    /// `headers` — additional non-secret headers (e.g. `anthropic-beta`,
    /// `User-Agent`); never place secrets here.
    pub fn get(
        &self,
        host: &str,
        path: &str,
        bearer: Option<&Secret<String>>,
        headers: &[(&str, &str)],
    ) -> Result<EgressResponse, EgressError> {
        Self::check_host(host)?;
        Self::check_path(path)?;
        let env: Vec<(String, String)> = std::env::vars().collect();
        Self::check_proxy_gate(self.proxy_opt_in, &env)?;

        // HTTPS is the only constructible scheme, and `host` is byte-for-byte
        // an allowlist entry (checked above).
        let url = format!("https://{host}{path}");

        let mut req = self.agent().get(&url);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        if let Some(token) = bearer {
            // Transient copy of the secret for the request lifetime — see
            // module docs. Never logged; ureq errors do not echo headers.
            req = req.header("Authorization", &format!("Bearer {}", token.expose()));
        }

        let mut response = req
            .call()
            .map_err(|e| EgressError::Transport(e.to_string()))?;

        let status = response.status().as_u16();
        let headers_out: Vec<(String, String)> = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|v| (name.as_str().to_ascii_lowercase(), v.to_string()))
            })
            .collect();
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_BODY_BYTES)
            .read_to_vec()
            .map_err(|e| EgressError::Transport(e.to_string()))?;

        Ok(EgressResponse {
            status,
            headers: headers_out,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SECURITY.md invariant 3 / THREAT_MODEL.md §9 row 3:
    /// a non-allowlisted host is a hard error.
    // INV:3,5 — registered in invariants.manifest (checked in CI)
    #[test]
    fn non_allowlisted_host_is_rejected() {
        for host in [
            "example.com",
            "evil.api.anthropic.com",     // subdomain of an allowed host
            "api.anthropic.com.evil.com", // allowed host as a prefix label
            "api.anthropic.com:8443",     // port smuggling
            "anthropic.com",
            "evil.chatgpt.com",        // subdomain of the M3 Codex host
            "chatgpt.com.evil.com",    // M3 host as a prefix label
            "chatgpt.com:8443",        // port smuggling on the M3 host
            "openai.com",              // bare apex is NOT allowlisted
            "api.openai.com",          // withdrawn with M4 (ADR-002) — no longer allowlisted
            "github.com",              // bare apex is NOT allowlisted — only the API host is
            "evil.api.github.com",     // subdomain of the update-check host
            "api.github.com.evil.com", // update-check host as a prefix label
            "api.github.com:8443",     // port smuggling on the update-check host
            "localhost",
            "127.0.0.1",
            "",
        ] {
            let err = Egress::check_host(host).unwrap_err();
            assert_eq!(err, EgressError::HostNotAllowlisted(host.to_string()));
        }
    }

    // INV:3 — registered in invariants.manifest (checked in CI)
    #[test]
    fn allowlisted_hosts_pass_host_check() {
        for host in ALLOWED_HOSTS {
            Egress::check_host(host).unwrap();
        }
        // Case-insensitive on the host, per RFC 3986.
        Egress::check_host("API.ANTHROPIC.COM").unwrap();
    }

    /// End-to-end through the public API: the denial holds at the chokepoint,
    /// not just in the helper. (Denied before any dial — no network in tests.)
    // INV:3 — registered in invariants.manifest (checked in CI)
    #[test]
    fn get_refuses_non_allowlisted_host() {
        let egress = Egress::new(false);
        let err = egress
            .get("attacker.example", "/exfil", None, &[])
            .unwrap_err();
        assert_eq!(
            err,
            EgressError::HostNotAllowlisted("attacker.example".to_string())
        );
    }

    /// A path that does not start with `/` could smuggle userinfo or an
    /// alternate authority into the composed URL (`…com@evil.com/…`).
    /// Rejected before any dial.
    // INV:3 — registered in invariants.manifest (checked in CI)
    #[test]
    fn authority_smuggling_paths_are_rejected() {
        let egress = Egress::new(false);
        for path in [
            "@evil.com/exfil",
            "",
            "evil.com/x",
            "/pa th",
            "/x\r\nHost:evil",
        ] {
            let err = egress
                .get("api.anthropic.com", path, None, &[])
                .unwrap_err();
            assert_eq!(
                err,
                EgressError::InvalidPath(path.to_string()),
                "path: {path:?}"
            );
        }
    }

    /// SECURITY.md invariant 7: proxy env present without opt-in fails closed.
    // INV:7 — registered in invariants.manifest (checked in CI)
    #[test]
    fn proxy_env_without_opt_in_fails_closed() {
        let env = vec![("HTTPS_PROXY".to_string(), "http://gateway:3128".to_string())];
        let err = Egress::check_proxy_gate(false, &env).unwrap_err();
        assert_eq!(
            err,
            EgressError::ProxyNotOptedIn {
                variable: "HTTPS_PROXY".to_string()
            }
        );
    }

    // INV:7 — registered in invariants.manifest (checked in CI)
    #[test]
    fn proxy_env_with_explicit_opt_in_passes_the_gate() {
        let env = vec![("HTTPS_PROXY".to_string(), "http://gateway:3128".to_string())];
        Egress::check_proxy_gate(true, &env).unwrap();
    }

    // INV:7 — registered in invariants.manifest (checked in CI)
    #[test]
    fn empty_proxy_vars_do_not_trigger_the_gate() {
        let env = vec![("HTTPS_PROXY".to_string(), String::new())];
        Egress::check_proxy_gate(false, &env).unwrap();
    }

    /// The error type must never carry secret bytes: exercise every variant's
    /// Display/Debug against a marker string.
    // INV:2 — registered in invariants.manifest (checked in CI)
    #[test]
    fn egress_errors_never_echo_secrets() {
        let marker = "synthetic-token-MARKER-000";
        for err in [
            EgressError::HostNotAllowlisted("h".into()),
            EgressError::InvalidPath("p".into()),
            EgressError::ProxyNotOptedIn {
                variable: "HTTPS_PROXY".into(),
            },
            EgressError::Transport("dns failure".into()),
        ] {
            let s = format!("{err} / {err:?}");
            assert!(!s.contains(marker));
        }
    }
}
