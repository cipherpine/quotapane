//! The [`UsageProvider`] trait. One implementation per data source.
//!
//! The trust boundary (egress chokepoint, `Secret<T>`, their tests, CI) shipped
//! and passed before any provider code was written (security-first build order).
//! [`ClaudeSubscription`] (M1) is the first implementation.

mod claude_subscription;

pub use claude_subscription::ClaudeSubscription;

use crate::egress::{Egress, EgressError};
use crate::model::{ProviderId, ProviderSnapshot};

/// Polling cadence hint from a provider (the poller adapts around it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cadence {
    /// Active use: poll fast (~5 min).
    Fast,
    /// Normal (~7 min).
    Normal,
    /// Idle (~20 min).
    Slow,
}

/// Errors a provider can surface. Never contains secret bytes.
#[derive(Debug)]
pub enum ProviderError {
    /// The egress chokepoint refused or failed the request.
    Egress(EgressError),
    /// A credential could not be loaded or parsed. The string is a
    /// non-secret description (I/O error kind or "malformed …") — token
    /// bytes never appear here.
    Credential(String),
    /// The OAuth token is expired (detected locally via `expiresAt`, or a
    /// 401/403 from the provider). The user should refresh it by running the
    /// official `claude` CLI; QuotaPane never writes the credential file.
    TokenExpired,
    /// The provider rate-limited us (HTTP 429). Carries a `retry-after` hint
    /// when the response provided one.
    RateLimited {
        /// Seconds to wait before retrying, if the response said so.
        retry_after_secs: Option<u64>,
    },
    /// The provider responded but the payload could not be interpreted
    /// (undocumented endpoints may change shape at any time — fail closed,
    /// show stale/error, never leak; THREAT_MODEL.md R4).
    UnexpectedPayload,
}

impl From<EgressError> for ProviderError {
    fn from(e: EgressError) -> Self {
        ProviderError::Egress(e)
    }
}

impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderError::Egress(e) => write!(f, "egress error: {e}"),
            ProviderError::Credential(msg) => write!(f, "credential error: {msg}"),
            ProviderError::TokenExpired => write!(
                f,
                "OAuth token expired — run `claude` to refresh it, then retry"
            ),
            ProviderError::RateLimited {
                retry_after_secs: Some(s),
            } => {
                write!(f, "rate limited by provider; retry after {s}s")
            }
            ProviderError::RateLimited {
                retry_after_secs: None,
            } => {
                write!(f, "rate limited by provider")
            }
            ProviderError::UnexpectedPayload => {
                write!(f, "provider response could not be interpreted")
            }
        }
    }
}

impl std::error::Error for ProviderError {}

/// A single usage data source (subscription quota or official billing).
///
/// Implementations receive the shared [`Egress`] chokepoint — they cannot
/// construct their own network path. Signature matches ARCHITECTURE.md §4
/// (synchronous since the M1 `ureq`/thread-based stack decision).
pub trait UsageProvider {
    /// Stable identifier for this provider.
    fn id(&self) -> ProviderId;
    /// Fetch a fresh snapshot through the egress chokepoint.
    fn poll(&self, http: &Egress) -> Result<ProviderSnapshot, ProviderError>;
    /// Current cadence hint for the poller.
    fn cadence(&self) -> Cadence;
}
