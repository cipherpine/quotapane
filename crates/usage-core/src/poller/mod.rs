//! Poll scheduler (M1+).
//!
//! Will own: per-provider staggered scheduling, adaptive intervals
//! (fast ≈5 min / normal ≈7 min / slow ≈20 min), snap-to-reset, exponential
//! backoff on 429 (THREAT_MODEL.md T-D1), and lazy credential loading —
//! tokens held in memory only, wrapped in `Secret`, zeroized after use.
//!
//! Deliberately empty in M0: the trust boundary ships and is tested before
//! any scheduling or provider code exists (security-first build order,
//! ARCHITECTURE.md §9). The async runtime dependency is deferred to M1 and
//! must be justified in CONTRIBUTING.md when it lands.
