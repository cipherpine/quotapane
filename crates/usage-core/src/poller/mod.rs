//! Thread-based poll scheduler (M2).
//!
//! One lightweight thread per provider. Each cycle calls
//! [`UsageProvider::poll`] through the shared [`Egress`] chokepoint and
//! forwards the outcome to the UI over a channel whose message type,
//! [`Update`], carries **no secrets by construction** — a
//! [`ProviderSnapshot`] or a non-secret error string. This is the
//! process → UI non-boundary from THREAT_MODEL.md §4: the type system, not
//! convention, is what keeps tokens on this side of the channel.
//!
//! Scheduling rules (all enforced in [`next_delay`], a pure, tested function):
//! - **Hard floor:** no two polls of a provider are ever closer than
//!   [`MIN_INTERVAL`] (180 s) — the Claude usage endpoint rate-limits
//!   aggressive callers (THREAT_MODEL.md T-D1), and being a polite client is
//!   part of the project's posture. The floor is applied *after* every other
//!   rule, so no configuration or backoff math can undercut it.
//! - **Cadence:** success schedules the next poll per the provider's
//!   [`Cadence`] hint (fast ≈5 min, normal ≈7 min, slow ≈20 min).
//! - **Backoff:** failures back off exponentially from the cadence interval,
//!   capped at [`MAX_BACKOFF`]. A 429's `retry-after` is honored when it is
//!   longer than the computed backoff, **but never beyond [`MAX_BACKOFF`]**:
//!   a read-only usage poll never owes a provider more than 30 minutes of
//!   silence, so a hostile or malformed hint (`retry-after: 99999999`) buys a
//!   half hour, not an unbounded stall of the pane. Impact of failure is stale
//!   display, never a crash.
//! - **Auth errors do not escalate:** [`ProviderError::TokenExpired`] — and
//!   only that variant — retries at [`MIN_INTERVAL`] no matter how many times
//!   it has repeated. An expired token clears when the user refreshes it via
//!   the official `claude`/`codex` CLI (invariant 6), never by our waiting
//!   longer; escalating would postpone *noticing* that refresh by up to 30
//!   minutes while the pane's own error text says "then retry". A provider's
//!   `retry-after` hint still outranks this floor (politeness wins), capped as
//!   above.
//! - Tokens are **not** held between cycles: the provider loads the
//!   credential lazily inside `poll` and drops (zeroizes) it before the
//!   thread sleeps.

use crate::egress::Egress;
use crate::model::{ProviderId, ProviderSnapshot};
use crate::providers::{Cadence, ProviderError, UsageProvider};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::time::Duration;

/// Hard minimum spacing between polls of one provider. Never undercut.
pub const MIN_INTERVAL: Duration = Duration::from_secs(180);
/// Upper bound for failure backoff.
pub const MAX_BACKOFF: Duration = Duration::from_secs(1800);
/// How often the sleep loop re-checks the stop flag (shutdown latency bound).
const STOP_POLL_SLICE: Duration = Duration::from_millis(100);

/// What the poller sends to the UI. Contains no secret-typed fields;
/// adding one is a breaking security change.
#[derive(Debug, Clone)]
pub enum Update {
    /// A fresh snapshot from a successful poll.
    Snapshot(ProviderSnapshot),
    /// A failed poll. `message` is the error's `Display` output — provider
    /// error types are documented (and tested) never to carry secret bytes.
    Failure {
        /// Which provider failed.
        provider: ProviderId,
        /// Non-secret, human-readable description of the failure.
        message: String,
    },
}

/// Convert a provider's cadence hint to its target interval.
fn cadence_interval(cadence: Cadence) -> Duration {
    match cadence {
        Cadence::Fast => Duration::from_secs(300),   // ~5 min
        Cadence::Normal => Duration::from_secs(420), // ~7 min
        Cadence::Slow => Duration::from_secs(1200),  // ~20 min
    }
}

/// Is this failure one that only the *user* can clear?
///
/// True for [`ProviderError::TokenExpired`] and nothing else: that is the one
/// failure whose remedy is a human running `claude` / `codex login`, so backing
/// off merely delays noticing the fix. Every other variant (transport blips,
/// rate limits, schema drift, credential-file problems) may resolve on its own
/// and keeps the normal exponential backoff.
fn is_auth_error(err: &ProviderError) -> bool {
    matches!(err, ProviderError::TokenExpired)
}

/// Compute the delay before the next poll. Pure — the scheduling policy in
/// one auditable place.
///
/// `consecutive_failures` counts failures since the last success (0 after a
/// success). `retry_after` is the provider's 429 hint, if any. `auth_error` is
/// [`is_auth_error`] applied to the failure that produced this call.
fn next_delay(
    cadence: Cadence,
    consecutive_failures: u32,
    retry_after: Option<Duration>,
    auth_error: bool,
) -> Duration {
    let base = cadence_interval(cadence);
    let mut delay = if auth_error {
        // Carve-out: an expired token clears only when the user refreshes via
        // the official CLI. Escalating would just delay noticing that, so this
        // sits at the floor regardless of how many attempts have failed.
        MIN_INTERVAL
    } else if consecutive_failures == 0 {
        base
    } else {
        // Exponential backoff: base * 2^failures, capped. The shift is
        // clamped so large counters cannot overflow.
        let shift = consecutive_failures.min(6); // 2^6 * 5min already > cap
        base.saturating_mul(1u32 << shift).min(MAX_BACKOFF)
    };
    if let Some(ra) = retry_after {
        // Honor the provider's explicit hint when it is more conservative —
        // but cap it at MAX_BACKOFF first. An uncapped hint is an unbounded
        // stall handed to us by whoever controls the response headers.
        delay = delay.max(ra.min(MAX_BACKOFF));
    }
    // The floor is applied last: nothing above can undercut it.
    delay.max(MIN_INTERVAL)
}

/// Handle to a running poller thread. Dropping the handle does **not** stop
/// the thread; call [`PollerHandle::stop`] for a clean shutdown.
pub struct PollerHandle {
    receiver: Receiver<Update>,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl PollerHandle {
    /// The channel of non-secret updates. Use `try_iter()` from a UI frame
    /// loop, or `recv()`/`recv_timeout()` from a dedicated consumer.
    pub fn updates(&self) -> &Receiver<Update> {
        &self.receiver
    }

    /// Request shutdown and join the thread. Returns once the thread has
    /// exited (bounded by the stop-poll slice, not by the poll interval).
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Spawn a poll thread for one provider.
///
/// The provider polls once immediately, then per the scheduling rules. The
/// `Egress` chokepoint moves into the thread; the returned handle carries
/// only the non-secret update channel and shutdown control.
pub fn spawn<P>(provider: P, egress: Egress) -> PollerHandle
where
    P: UsageProvider + Send + 'static,
{
    let (sender, receiver) = std::sync::mpsc::channel::<Update>();
    let stop = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::clone(&stop);

    let join = std::thread::Builder::new()
        .name("quotapane-poller".to_string())
        .spawn(move || run_loop(provider, egress, sender, stop_flag))
        .expect("failed to spawn poller thread");

    PollerHandle {
        receiver,
        stop,
        join: Some(join),
    }
}

/// The poll loop. Exits when `stop` is set or the receiver is dropped.
fn run_loop<P: UsageProvider>(
    provider: P,
    egress: Egress,
    sender: Sender<Update>,
    stop: Arc<AtomicBool>,
) {
    let mut consecutive_failures: u32 = 0;

    loop {
        if stop.load(Ordering::Relaxed) {
            return;
        }

        // Poll. The provider loads and drops (zeroizes) its credential
        // entirely within this call; no secret survives into the sleep.
        let (update, retry_after, auth_error) = match provider.poll(&egress) {
            Ok(snapshot) => {
                consecutive_failures = 0;
                (Update::Snapshot(snapshot), None, false)
            }
            Err(err) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                let retry_after = match &err {
                    ProviderError::RateLimited {
                        retry_after_secs: Some(s),
                    } => Some(Duration::from_secs(*s)),
                    _ => None,
                };
                let auth_error = is_auth_error(&err);
                (
                    Update::Failure {
                        provider: provider.id(),
                        message: err.to_string(),
                    },
                    retry_after,
                    auth_error,
                )
            }
        };

        // A closed receiver means the UI is gone — exit quietly.
        if sender.send(update).is_err() {
            return;
        }

        // Sleep in short slices so stop() is honored promptly.
        let delay = next_delay(
            provider.cadence(),
            consecutive_failures,
            retry_after,
            auth_error,
        );
        let mut slept = Duration::ZERO;
        while slept < delay {
            if stop.load(Ordering::Relaxed) {
                return;
            }
            let slice = STOP_POLL_SLICE.min(delay - slept);
            std::thread::sleep(slice);
            slept += slice;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::Secret;
    use crate::model::SnapshotSource;
    use std::time::Instant;

    // --- next_delay: the scheduling policy, exhaustively ---

    /// The ordinary (non-auth-error) call, so the scheduling assertions below
    /// stay readable now that `next_delay` also takes the auth-error carve-out.
    fn delay(cadence: Cadence, failures: u32, retry_after: Option<Duration>) -> Duration {
        next_delay(cadence, failures, retry_after, false)
    }

    #[test]
    fn success_uses_cadence_interval() {
        assert_eq!(delay(Cadence::Fast, 0, None), Duration::from_secs(300));
        assert_eq!(delay(Cadence::Normal, 0, None), Duration::from_secs(420));
        assert_eq!(delay(Cadence::Slow, 0, None), Duration::from_secs(1200));
    }

    #[test]
    fn floor_is_never_undercut() {
        // Even inputs that would compute a smaller delay clamp to the floor.
        // (No cadence maps below the floor today; this guards future edits.)
        for cadence in [Cadence::Fast, Cadence::Normal, Cadence::Slow] {
            for failures in 0..10 {
                for ra in [None, Some(Duration::from_secs(1))] {
                    for auth in [false, true] {
                        assert!(
                            next_delay(cadence, failures, ra, auth) >= MIN_INTERVAL,
                            "floor violated: {cadence:?} failures={failures} ra={ra:?} auth={auth}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn failures_back_off_exponentially_and_cap() {
        let base = Duration::from_secs(300);
        assert_eq!(delay(Cadence::Fast, 1, None), base * 2);
        assert_eq!(delay(Cadence::Fast, 2, None), base * 4);
        // Caps at MAX_BACKOFF regardless of failure count (incl. huge counts).
        assert_eq!(delay(Cadence::Fast, 3, None), MAX_BACKOFF);
        assert_eq!(delay(Cadence::Fast, 30, None), MAX_BACKOFF);
        assert_eq!(delay(Cadence::Fast, u32::MAX, None), MAX_BACKOFF);
    }

    #[test]
    fn retry_after_is_honored_when_longer() {
        // Longer than backoff → wins.
        assert_eq!(
            delay(Cadence::Fast, 1, Some(Duration::from_secs(900))),
            Duration::from_secs(900)
        );
        // Shorter than backoff → backoff wins (we stay conservative).
        assert_eq!(
            delay(Cadence::Fast, 2, Some(Duration::from_secs(60))),
            Duration::from_secs(1200)
        );
    }

    // --- M9b: the Retry-After hint is capped ---

    #[test]
    fn hostile_retry_after_cannot_stall_beyond_the_cap() {
        // The hint comes from response headers — i.e. from whoever answers the
        // request. Before M9b it was applied AFTER the cap, so `retry-after:
        // 18446744073709551615` silenced the pane effectively forever. Now the
        // hint buys at most MAX_BACKOFF.
        let hostile = Duration::from_secs(u64::MAX);
        assert_eq!(delay(Cadence::Fast, 1, Some(hostile)), MAX_BACKOFF);
        // ...from any starting point: fresh failure, deep backoff, any cadence.
        for cadence in [Cadence::Fast, Cadence::Normal, Cadence::Slow] {
            for failures in [0, 1, 6, 30, u32::MAX] {
                assert_eq!(
                    next_delay(cadence, failures, Some(hostile), false),
                    MAX_BACKOFF,
                    "uncapped hint leaked through: {cadence:?} failures={failures}"
                );
            }
        }
        // A hint one second over the cap is still exactly the cap.
        assert_eq!(
            delay(Cadence::Fast, 1, Some(MAX_BACKOFF + Duration::from_secs(1))),
            MAX_BACKOFF
        );
    }

    #[test]
    fn retry_after_below_the_cap_is_honored_exactly() {
        // Capping must not round or clamp hints that are already reasonable —
        // the provider's number is used verbatim.
        for secs in [200, 600, 900, 1799] {
            let hint = Duration::from_secs(secs);
            assert_eq!(
                delay(Cadence::Fast, 1, Some(hint)),
                hint.max(Duration::from_secs(600)),
                "hint of {secs}s was not honored as-is"
            );
        }
        // Exactly at the cap: honored, unchanged.
        assert_eq!(delay(Cadence::Fast, 1, Some(MAX_BACKOFF)), MAX_BACKOFF);
    }

    // --- M9b: auth errors retry at the floor, without escalation ---

    #[test]
    fn auth_error_retries_at_the_floor_across_every_cadence_and_count() {
        // An expired token clears only when the user runs `claude`/`codex
        // login` (invariant 6). Backing off would postpone noticing that
        // refresh by up to 30 minutes while the error text says "then retry".
        for cadence in [Cadence::Fast, Cadence::Normal, Cadence::Slow] {
            for failures in [0, 1, 2, 3, 6, 7, 30, 1000, u32::MAX] {
                assert_eq!(
                    next_delay(cadence, failures, None, true),
                    MIN_INTERVAL,
                    "auth error escalated: {cadence:?} failures={failures}"
                );
            }
        }
    }

    #[test]
    fn auth_error_still_yields_to_a_retry_after_hint_capped() {
        // Politeness outranks the carve-out: if the provider asked us to wait,
        // we wait — but no longer than the cap.
        assert_eq!(
            next_delay(Cadence::Fast, 3, Some(Duration::from_secs(900)), true),
            Duration::from_secs(900)
        );
        assert_eq!(
            next_delay(Cadence::Fast, 3, Some(Duration::from_secs(u64::MAX)), true),
            MAX_BACKOFF
        );
        // A hint below the floor cannot undercut it.
        assert_eq!(
            next_delay(Cadence::Fast, 3, Some(Duration::from_secs(1)), true),
            MIN_INTERVAL
        );
    }

    #[test]
    fn only_token_expired_counts_as_an_auth_error() {
        // Enumerated deliberately rather than tested by negation: adding a
        // variant to `ProviderError` should force a decision here, and the
        // exhaustive `match` below is what makes the compiler ask.
        assert!(is_auth_error(&ProviderError::TokenExpired));

        let every_variant = [
            ProviderError::TokenExpired,
            ProviderError::Egress(crate::egress::EgressError::HostNotAllowlisted(
                "evil.example".to_string(),
            )),
            ProviderError::Egress(crate::egress::EgressError::InvalidPath("nope".to_string())),
            ProviderError::Egress(crate::egress::EgressError::ProxyNotOptedIn {
                variable: "HTTPS_PROXY".to_string(),
            }),
            ProviderError::Egress(crate::egress::EgressError::Transport("timeout".to_string())),
            ProviderError::Credential("malformed auth.json".to_string()),
            ProviderError::RateLimited {
                retry_after_secs: None,
            },
            ProviderError::RateLimited {
                retry_after_secs: Some(60),
            },
            ProviderError::UnexpectedPayload,
        ];
        // Completeness guard: this `match` is exhaustive by hand, so adding a
        // variant to `ProviderError` breaks compilation here and forces an
        // explicit decision rather than silently defaulting to "not auth".
        fn expected(e: &ProviderError) -> bool {
            match e {
                ProviderError::TokenExpired => true,
                ProviderError::Egress(_)
                | ProviderError::Credential(_)
                | ProviderError::RateLimited { .. }
                | ProviderError::UnexpectedPayload => false,
            }
        }

        for err in &every_variant {
            assert_eq!(
                is_auth_error(err),
                expected(err),
                "auth-error classification disagrees for {err:?}"
            );
            // ...and the classification is what actually drives scheduling.
            let scheduled = next_delay(Cadence::Slow, 5, None, is_auth_error(err));
            if expected(err) {
                assert_eq!(scheduled, MIN_INTERVAL, "{err:?} should hold at the floor");
            } else {
                assert_eq!(scheduled, MAX_BACKOFF, "{err:?} should back off normally");
            }
        }
    }

    // --- thread mechanics with a fake provider (no network, no real waits) ---

    struct FakeProvider;

    impl UsageProvider for FakeProvider {
        fn id(&self) -> ProviderId {
            ProviderId::ClaudeSubscription
        }
        fn poll(&self, _http: &Egress) -> Result<ProviderSnapshot, ProviderError> {
            Ok(ProviderSnapshot {
                provider: ProviderId::ClaudeSubscription,
                taken_at_unix_secs: 0,
                windows: vec![],
                per_model: vec![],
                reset_credits: None,
                source: SnapshotSource::UsageEndpoint,
            })
        }
        fn cadence(&self) -> Cadence {
            Cadence::Slow
        }
    }

    #[test]
    fn first_poll_is_immediate_and_stop_is_prompt() {
        let handle = spawn(FakeProvider, Egress::new(false));

        // The first update arrives without waiting out any interval.
        let first = handle
            .updates()
            .recv_timeout(Duration::from_secs(5))
            .expect("expected an immediate first update");
        assert!(matches!(first, Update::Snapshot(_)));

        // stop() returns promptly (bounded by the stop-poll slice), even
        // though the next scheduled poll is ~20 minutes away.
        let started = Instant::now();
        handle.stop();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "stop() took too long: {:?}",
            started.elapsed()
        );
    }

    /// Holds token material and — like careless future code might —
    /// interpolates its [`Secret`] into the failure it returns. Combined
    /// with the assertions below this proves the redaction invariant END TO
    /// END through the poller: even a provider that formats its secret into
    /// an error cannot leak token bytes onto the UI channel. (The previous
    /// version of this test asserted only that the message was non-empty,
    /// against a unit-variant error that could not carry a token at all —
    /// gap report, test-harness blind spots.)
    struct LeakyFailingProvider {
        token: Secret<String>,
    }

    /// Synthetic marker — recognizable bytes, deliberately NOT shaped like
    /// a real credential, so secret scanners have nothing to flag.
    const SENTINEL_TOKEN: &str = "synthetic-sentinel-token-DO-NOT-PRINT-1234567890";

    impl UsageProvider for LeakyFailingProvider {
        fn id(&self) -> ProviderId {
            ProviderId::ClaudeSubscription
        }
        fn poll(&self, _http: &Egress) -> Result<ProviderSnapshot, ProviderError> {
            let msg = format!("credential rejected for {:?}", self.token);
            Err(ProviderError::Credential(msg))
        }
        fn cadence(&self) -> Cadence {
            Cadence::Normal
        }
    }

    // INV:2 — registered in invariants.manifest (checked in CI)
    #[test]
    fn failures_are_forwarded_as_non_secret_messages() {
        let handle = spawn(
            LeakyFailingProvider {
                token: Secret::new(SENTINEL_TOKEN.to_string()),
            },
            Egress::new(false),
        );
        let first = handle
            .updates()
            .recv_timeout(Duration::from_secs(5))
            .expect("expected an immediate failure update");
        match first {
            Update::Failure { provider, message } => {
                assert_eq!(provider, ProviderId::ClaudeSubscription);
                assert!(
                    !message.contains(SENTINEL_TOKEN),
                    "token bytes leaked into a failure message: {message}"
                );
                assert!(
                    message.contains("«redacted»"),
                    "expected the redaction marker, proving the secret flowed \
                     through this path; got: {message}"
                );
                assert!(
                    message.starts_with("credential error:"),
                    "message must be the error's Display output; got: {message}"
                );
            }
            other => panic!("expected Failure, got {other:?}"),
        }
        handle.stop();
    }
}
