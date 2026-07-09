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
//!   byte-for-byte the host that will be dialed.
//! - M0 ships this module deny-everything: host checking, proxy gating, and
//!   their tests exist, but no actual network dial. The real HTTP client
//!   lands in M1 *behind* this same API, so the allowlist test is already
//!   guarding the only door.
//! - Proxy use is opt-in (invariant 7): if proxy environment variables are
//!   present and the user has not explicitly opted in, requests fail closed
//!   before anything is sent.

/// The complete set of hosts this process may ever contact.
///
/// Compile-time, deny-by-default. Adding a host is a breaking security
/// change: it must be justified against THREAT_MODEL.md §6 and called out
/// in review (and the Codex usage host, deferred to M3, must be verified
/// against provider behavior — never guessed).
pub const ALLOWED_HOSTS: &[&str] = &[
    // Anthropic: subscription usage + Messages-API header fallback; official Admin API (M4).
    "api.anthropic.com",
    // OpenAI: official usage/costs API (M4). The Codex usage host is added in M3 after verification.
    "api.openai.com",
    // GitHub: release update *check* only, and only when the user enables it (invariant 5).
    "api.github.com",
];

/// Errors from the egress chokepoint. Never contains secret bytes.
#[derive(Debug, PartialEq, Eq)]
pub enum EgressError {
    /// The requested host is not on [`ALLOWED_HOSTS`]. Hard error — never
    /// retried, never downgraded to a warning.
    HostNotAllowlisted(String),
    /// Proxy environment variables are set but the user has not opted in
    /// (SECURITY.md invariant 7). Fails closed before any bytes are sent.
    ProxyNotOptedIn {
        /// The name of the environment variable that triggered the gate.
        variable: String,
    },
    /// Network transport is not yet implemented (M0). The chokepoint exists
    /// and denies; the actual client arrives in M1 behind this same API.
    NotImplemented,
}

impl std::fmt::Display for EgressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EgressError::HostNotAllowlisted(h) => {
                write!(f, "egress denied: host {h:?} is not on the allowlist")
            }
            EgressError::ProxyNotOptedIn { variable } => write!(
                f,
                "egress denied: proxy environment ({variable}) detected without explicit opt-in; \
                 a TLS-inspecting proxy could observe bearer tokens"
            ),
            EgressError::NotImplemented => write!(f, "egress transport not implemented (M0)"),
        }
    }
}

impl std::error::Error for EgressError {}

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

    /// Issue an HTTPS GET to `https://{host}{path}`.
    ///
    /// Order of gates (all fail closed, before any bytes leave the process):
    /// 1. host allowlist (invariant 3)
    /// 2. proxy opt-in (invariant 7)
    /// 3. transport — TLS-only; lands in M1.
    pub fn get(&self, host: &str, path: &str) -> Result<Vec<u8>, EgressError> {
        Self::check_host(host)?;
        let env: Vec<(String, String)> = std::env::vars().collect();
        Self::check_proxy_gate(self.proxy_opt_in, &env)?;
        let _ = path;
        // M1: perform the TLS request here, through this one code path only.
        Err(EgressError::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SECURITY.md invariant 3 / THREAT_MODEL.md §9 row 3:
    /// a non-allowlisted host is a hard error.
    #[test]
    fn non_allowlisted_host_is_rejected() {
        for host in [
            "example.com",
            "evil.api.anthropic.com",     // subdomain of an allowed host
            "api.anthropic.com.evil.com", // allowed host as a prefix label
            "api.anthropic.com:8443",     // port smuggling
            "anthropic.com",
            "localhost",
            "127.0.0.1",
            "",
        ] {
            let err = Egress::check_host(host).unwrap_err();
            assert_eq!(err, EgressError::HostNotAllowlisted(host.to_string()));
        }
    }

    #[test]
    fn allowlisted_hosts_pass_host_check() {
        for host in ALLOWED_HOSTS {
            Egress::check_host(host).unwrap();
        }
        // Case-insensitive on the host, per RFC 3986.
        Egress::check_host("API.ANTHROPIC.COM").unwrap();
    }

    /// End-to-end through the public API: the denial holds at the chokepoint,
    /// not just in the helper.
    #[test]
    fn get_refuses_non_allowlisted_host() {
        let egress = Egress::new(false);
        let err = egress.get("attacker.example", "/exfil").unwrap_err();
        assert_eq!(
            err,
            EgressError::HostNotAllowlisted("attacker.example".to_string())
        );
    }

    /// SECURITY.md invariant 7: proxy env present without opt-in fails closed.
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

    #[test]
    fn proxy_env_with_explicit_opt_in_passes_the_gate() {
        let env = vec![("HTTPS_PROXY".to_string(), "http://gateway:3128".to_string())];
        Egress::check_proxy_gate(true, &env).unwrap();
    }

    #[test]
    fn empty_proxy_vars_do_not_trigger_the_gate() {
        let env = vec![("HTTPS_PROXY".to_string(), String::new())];
        Egress::check_proxy_gate(false, &env).unwrap();
    }

    /// M0 definition: even a fully-allowed request does not reach a network —
    /// the transport does not exist yet.
    #[test]
    fn m0_transport_is_not_implemented() {
        let egress = Egress::new(true); // opt-in irrelevant; nothing is sent either way
        let err = egress.get("api.anthropic.com", "/v1/anything").unwrap_err();
        assert_eq!(err, EgressError::NotImplemented);
    }
}
