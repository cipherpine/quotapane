//! The [`UsageProvider`] trait. One implementation per data source.
//!
//! No implementations exist in M0 — per the security-first build order,
//! no provider or network-calling code is written until the egress
//! chokepoint, `Secret<T>`, their tests, and CI all exist and pass.
//! `ClaudeSubscription` arrives in M1.

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

/// A single usage data source (subscription quota or official billing).
///
/// Implementations receive the shared [`Egress`] chokepoint — they cannot
/// construct their own network path. Signature matches ARCHITECTURE.md §4.
#[allow(async_fn_in_trait)] // single-process consumer; Send bounds revisited in M1 with the runtime
pub trait UsageProvider {
    /// Stable identifier for this provider.
    fn id(&self) -> ProviderId;
    /// Fetch a fresh snapshot through the egress chokepoint.
    async fn poll(&self, http: &Egress) -> Result<ProviderSnapshot, ProviderError>;
    /// Current cadence hint for the poller.
    fn cadence(&self) -> Cadence;
}
