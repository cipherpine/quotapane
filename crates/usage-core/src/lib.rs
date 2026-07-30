//! # usage-core — the trust boundary
//!
//! This crate is the **entire** security-sensitive surface of QuotaPane
//! (see `SECURITY.md` and `THREAT_MODEL.md` at the repository root).
//! Exactly two operations here touch secrets:
//!
//! 1. **Ingress (TB1):** [`credentials`] reads local credential files,
//!    read-only, and wraps tokens in [`credentials::Secret`] immediately.
//! 2. **Egress (TB2):** [`egress`] is the single, deny-by-default network
//!    chokepoint. A secret leaves the process only through it, and only to
//!    a host on the compile-time allowlist.
//!
//! Everything else — [`providers`], [`poller`], [`model`], [`pace`] —
//! schedules, normalizes, or does arithmetic over **non-secret** data. Secrets
//! never cross the channel to the UI or CLI; the message type
//! ([`model::ProviderSnapshot`]) contains no secret fields by construction.
//! [`pace`] is the furthest thing from the boundary in the crate: pure
//! functions over numbers, with no clock, no I/O, and no credentials in reach.
//!
//! ## Enforced invariants (each backed by a test — THREAT_MODEL.md §9)
//!
//! 1. No credential persistence.
//! 2. No credential leakage (redaction + zeroize).
//! 3. Deny-by-default egress.
//! 4. No first-party telemetry (checked in CI).
//! 5. No silent auto-update (M5; update check is notify-only, off by default).
//! 6. Read-only credentials.
//! 7. Proxy is opt-in.
//!
//! A change that weakens any invariant is a breaking security change and
//! must be called out in review (SECURITY.md).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod credentials;
pub mod egress;
pub mod model;
pub mod pace;
pub mod poller;
pub mod providers;
