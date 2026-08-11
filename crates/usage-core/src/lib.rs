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
//! Everything else — [`providers`], [`poller`], [`model`], [`pace`],
//! [`history`], [`agents`] — schedules, normalizes, or does arithmetic over
//! **non-secret** data. Secrets never cross the channel to the UI or CLI; the
//! message type
//! ([`model::ProviderSnapshot`]) contains no secret fields by construction.
//! [`pace`] is the furthest thing from the boundary in the crate: pure
//! functions over numbers, with no clock, no I/O, and no credentials in reach.
//! [`history`] is the one module here that writes to disk, and it writes only
//! what the window already displays — timestamps, window labels, and
//! percentages, in a record type with no field a credential could occupy
//! (invariant 1). It is off unless the user turns it on.
//! [`agents`] is the one module that reads provider files which are **not**
//! credential files: the local Claude Code / Codex CLI session logs, read-only
//! and through a fixed allowlist of metadata keys, so the window can say which
//! sessions are running. It never reaches conversation content, never writes,
//! and never sends (invariant 8).
//! [`update`] is the one module whose request carries no credential at all: an
//! anonymous GET for the latest release tag, through the same chokepoint, and
//! only when the user has switched it on. It reads one field, keeps no state,
//! and cannot report a failure (invariant 5).
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
//! 8. Agent visibility is metadata-only (M15; [`agents`]).
//!
//! A change that weakens any invariant is a breaking security change and
//! must be called out in review (SECURITY.md).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod agents;
pub mod credentials;
pub mod egress;
pub mod history;
pub mod model;
pub mod pace;
pub mod poller;
pub mod providers;
pub mod update;
