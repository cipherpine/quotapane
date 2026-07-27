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
//!   longer than the computed backoff. Impact of failure is stale display,
//!   never a crash.
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

/// Compute the delay before the next poll. Pure — the scheduling policy in
/// one auditable place.
///
/// `consecutive_failures` counts failures since the last success (0 after a
/// success). `retry_after` is the provider's 429 hint, if any.
fn next_delay(
    cadence: Cadence,
    consecutive_failures: u32,
    retry_after: Option<Duration>,
) -> Duration {
    let base = cadence_interval(cadence);
    let mut delay = if consecutive_failures == 0 {
        base
    } else {
        // Exponential backoff: base * 2^failures, capped. The shift is
        // clamped so large counters cannot overflow.
        let shift = consecutive_failures.min(6); // 2^6 * 5min already > cap
        base.saturating_mul(1u32 << shift).min(MAX_BACKOFF)
    };
    if let Some(ra) = retry_after {
        // Honor the provider's explicit hint when it is more conservative.
        delay = delay.max(ra);
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
        let (update, retry_after) = match provider.poll(&egress) {
            Ok(snapshot) => {
                consecutive_failures = 0;
                (Update::Snapshot(snapshot), None)
            }
            Err(err) => {
                consecutive_failures = consecutive_failures.saturating_add(1);
                let retry_after = match &err {
                    ProviderError::RateLimited {
                        retry_after_secs: Some(s),
                    } => Some(Duration::from_secs(*s)),
                    _ => None,
                };
                (
                    Update::Failure {
                        provider: provider.id(),
                        message: err.to_string(),
                    },
                    retry_after,
                )
            }
        };

        // A closed receiver means the UI is gone — exit quietly.
        if sender.send(update).is_err() {
            return;
        }

        // Sleep in short slices so stop() is honored promptly.
        let delay = next_delay(provider.cadence(), consecutive_failures, retry_after);
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

    #[test]
    fn success_uses_cadence_interval() {
        assert_eq!(next_delay(Cadence::Fast, 0, None), Duration::from_secs(300));
        assert_eq!(
            next_delay(Cadence::Normal, 0, None),
            Duration::from_secs(420)
        );
        assert_eq!(
            next_delay(Cadence::Slow, 0, None),
            Duration::from_secs(1200)
        );
    }

    #[test]
    fn floor_is_never_undercut() {
        // Even inputs that would compute a smaller delay clamp to the floor.
        // (No cadence maps below the floor today; this guards future edits.)
        for cadence in [Cadence::Fast, Cadence::Normal, Cadence::Slow] {
            for failures in 0..10 {
                for ra in [None, Some(Duration::from_secs(1))] {
                    assert!(
                        next_delay(cadence, failures, ra) >= MIN_INTERVAL,
                        "floor violated: {cadence:?} failures={failures} ra={ra:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn failures_back_off_exponentially_and_cap() {
        let base = Duration::from_secs(300);
        assert_eq!(next_delay(Cadence::Fast, 1, None), base * 2);
        assert_eq!(next_delay(Cadence::Fast, 2, None), base * 4);
        // Caps at MAX_BACKOFF regardless of failure count (incl. huge counts).
        assert_eq!(next_delay(Cadence::Fast, 3, None), MAX_BACKOFF);
        assert_eq!(next_delay(Cadence::Fast, 30, None), MAX_BACKOFF);
        assert_eq!(next_delay(Cadence::Fast, u32::MAX, None), MAX_BACKOFF);
    }

    #[test]
    fn retry_after_is_honored_when_longer() {
        // Longer than backoff → wins.
        assert_eq!(
            next_delay(Cadence::Fast, 1, Some(Duration::from_secs(900))),
            Duration::from_secs(900)
        );
        // Shorter than backoff → backoff wins (we stay conservative).
        assert_eq!(
            next_delay(Cadence::Fast, 2, Some(Duration::from_secs(60))),
            Duration::from_secs(1200)
        );
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
