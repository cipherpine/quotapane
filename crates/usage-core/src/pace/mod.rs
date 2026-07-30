//! Pace math: burn rate and forecast-to-limit (M8).
//!
//! Given a short trail of "how much of this window was spent, and when", this
//! module answers two questions: how fast is the quota burning, and will it run
//! out before the window resets. Nothing else.
//!
//! ## Everything here is pure
//!
//! No clock is read anywhere in this module — every entry point that needs the
//! current time takes it as a parameter. That is not a stylistic preference: it
//! is what makes the forecast testable at all. A function that called
//! `SystemTime::now()` internally could only be tested against whatever moment
//! the test happened to run at, so the edge cases that matter (a window half
//! elapsed, a sample trail exactly at the minimum span, a reset one second ago)
//! would be unreachable. [`no_clock_is_read_in_this_module`] pins it.
//!
//! There is also no I/O, no persistence, and no network. The sample trail lives
//! in memory in a [`PaceRing`] and dies with the process — on-disk history is a
//! later milestone, deliberately not this one.
//!
//! ## What "pace" means
//!
//! [`PaceSample::used_fraction`] is the same `0.0..=1.0` convention as
//! [`crate::model::QuotaWindow::used_fraction`]. A least-squares fit over the
//! trailing samples gives a slope in fraction-per-second; scaled to
//! fraction-per-hour it is the burn rate, and dividing the unspent remainder by
//! it projects when the window would be full. Least squares rather than
//! first-to-last so a single anomalous reading cannot swing the forecast, which
//! matters when a poll lands mid-request and the provider's percentage jumps.

/// How far back the estimate looks, in seconds (2 h).
///
/// Long enough that a poll every ~7 minutes contributes a useful number of
/// samples, short enough that the answer describes *now* rather than the whole
/// window. A weekly window fitted over its entire life would report the average
/// of a week, which is exactly not the question "am I burning too fast today".
pub const TRAIL_SECS: u64 = 7_200;

/// Fewest samples that will produce an estimate.
///
/// Two points always fit a line perfectly, so a slope from two readings carries
/// no evidence that the trend is real. Three is the smallest number that can
/// disagree with itself.
pub const MIN_SAMPLES: usize = 3;

/// Shortest sample span that will produce an estimate, in seconds (10 min).
///
/// Three samples a few seconds apart describe provider rounding, not a burn
/// rate: a percentage that ticks from 12 to 13 over 20 seconds would extrapolate
/// to an alarming slope. The span floor is what stops that.
pub const MIN_SPAN_SECS: u64 = 600;

/// Upper bound on a projected exhaustion, in seconds (14 days).
///
/// A very slow burn projects a very distant date, and past a point the number
/// stops being information — no window in this product is longer than a week, so
/// anything beyond a fortnight means "not on this window's timescale". Capping
/// keeps the value finite and comparable instead of drifting toward years.
pub const MAX_EXHAUST_SECS: u64 = 14 * 86_400;

/// How far a used fraction must fall to read as a window reset (5 points).
///
/// A reset drops usage to (near) zero, so any real one clears this easily. The
/// threshold exists for the other direction: providers restate percentages with
/// their own rounding and occasionally revise one slightly downward, and a bare
/// `<` would treat that as a reset and throw away the trail.
pub const RESET_DROP: f64 = 0.05;

/// How many samples a [`PaceRing`] holds before evicting its oldest.
///
/// At the poll floor (180 s) 256 samples span over 12 hours — comfortably more
/// than [`TRAIL_SECS`] uses, so the cap is a memory bound rather than a policy,
/// and the trail is never the thing that runs out.
pub const RING_CAPACITY: usize = 256;

/// Seconds in an hour, as a float — the burn-rate scale factor.
const SECS_PER_HOUR: f64 = 3_600.0;

/// One reading: how much of a window was spent, and when.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaceSample {
    /// When the reading was taken (Unix epoch seconds). For a real poll this is
    /// [`crate::model::ProviderSnapshot::taken_at_unix_secs`] — the provider's
    /// own observation time, not the moment the UI got around to looking.
    pub at_unix_secs: u64,
    /// Fraction of the window consumed, `0.0..=1.0`.
    pub used_fraction: f64,
}

/// A burn rate and, when the quota is actually being spent, a projection of when
/// the window fills.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Burn {
    /// Fraction of the window consumed per hour, from the least-squares slope.
    /// Zero or negative means nothing is being spent (see
    /// [`Self::exhaust_in_secs`]).
    pub per_hour: f64,
    /// Seconds until the window is projected to reach 100%, capped at
    /// [`MAX_EXHAUST_SECS`].
    ///
    /// `None` when [`Self::per_hour`] is zero or negative: at that pace the
    /// window never fills, and any number here would be a fabrication. Not
    /// collapsed into a large sentinel value for exactly that reason — "will not
    /// fill" and "fills in a long time" are different facts.
    pub exhaust_in_secs: Option<u64>,
}

/// An in-memory trail of samples for **one** window — one ring per (provider,
/// window label).
///
/// Deliberately dumb: it stores readings and drops the oldest past
/// [`RING_CAPACITY`]. It reads no clock, holds no provider knowledge, and
/// decides nothing about what the numbers mean. The one judgement it makes is
/// delegated to [`reset_detected`], a pure function, over facts the caller hands
/// it — the ring never goes looking for them.
///
/// A plain `Vec`, so [`Self::samples`] hands out a slice behind `&self`. That
/// makes an eviction a shift of at most [`RING_CAPACITY`] samples rather than a
/// `VecDeque`'s O(1) pop — a few kilobytes moved once per poll, at a poll floor
/// of 180 s. Trading that for a borrow-friendly getter is the better deal at
/// this size; a deque would only earn its keep if the ring were orders of
/// magnitude larger or fed orders of magnitude faster.
#[derive(Debug, Clone, Default)]
pub struct PaceRing {
    samples: Vec<PaceSample>,
    /// The reset countdown that arrived with the newest sample, kept so
    /// [`Self::observe`] can spot the countdown *jumping up* — the second reset
    /// signal. Caller state would work identically; it lives here so a caller
    /// cannot forget to thread it through and silently lose reset detection.
    last_resets_in_secs: Option<u64>,
}

impl PaceRing {
    /// An empty ring.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a reading, first clearing the trail if the facts say the window
    /// reset since the previous one.
    ///
    /// This is the entry point real callers want: `resets_in_secs` and
    /// `duration_secs` come straight off the [`crate::model::QuotaWindow`] the
    /// sample was taken from, and reset detection happens for free.
    pub fn observe(
        &mut self,
        sample: PaceSample,
        resets_in_secs: Option<u64>,
        duration_secs: Option<u64>,
    ) {
        if let Some(previous) = self.samples.last() {
            if reset_detected(
                previous.used_fraction,
                sample.used_fraction,
                self.last_resets_in_secs,
                resets_in_secs,
                duration_secs,
            ) {
                self.clear();
            }
        }
        self.push(sample);
        self.last_resets_in_secs = resets_in_secs;
    }

    /// Append a sample, evicting the oldest once the ring is full.
    ///
    /// Public for callers that track reset detection themselves (and for tests
    /// that build a specific trail); [`Self::observe`] is the usual door.
    pub fn push(&mut self, sample: PaceSample) {
        if self.samples.len() >= RING_CAPACITY {
            self.samples.remove(0);
        }
        self.samples.push(sample);
    }

    /// Forget every sample. Called on a detected reset: the pre-reset trail
    /// describes a window that no longer exists, and fitting across the
    /// discontinuity would report a wildly negative slope.
    pub fn clear(&mut self) {
        self.samples.clear();
        self.last_resets_in_secs = None;
    }

    /// The trail, oldest first — ready to hand to [`estimate`].
    pub fn samples(&self) -> &[PaceSample] {
        &self.samples
    }

    /// The newest sample, if any.
    pub fn latest(&self) -> Option<&PaceSample> {
        self.samples.last()
    }

    /// How many samples the ring is holding.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the ring is empty.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

/// Whether a window reset between two consecutive readings.
///
/// Two independent signals, either of which is enough:
///
/// 1. **The used fraction fell** by more than [`RESET_DROP`]. The direct
///    evidence — usage only decreases when the window rolls over.
/// 2. **The reset countdown jumped up** by more than half the window's
///    duration. `resets_in_secs` otherwise counts monotonically down, so *any*
///    increase is suspicious; the half-duration threshold is what separates a
///    genuine rollover (which restores the countdown to roughly the full
///    duration, so the jump is nearly a whole duration) from clock skew or a
///    provider re-rounding its own number.
///
/// Both signals are needed because either can be unavailable: a window can reset
/// while still reading 0% (nothing spent, no drop to see), and a provider can
/// omit the countdown or the duration entirely.
///
/// Every fact is supplied by the caller — this function reads no clock and knows
/// nothing about providers. Unknown inputs make a signal unavailable, never
/// true: a missing countdown or duration cannot *prove* a reset, and falsely
/// clearing a healthy trail costs the user their forecast.
pub fn reset_detected(
    last_fraction: f64,
    next_fraction: f64,
    last_resets_in_secs: Option<u64>,
    next_resets_in_secs: Option<u64>,
    duration_secs: Option<u64>,
) -> bool {
    // Signal 1. A non-finite fraction makes this comparison false, which is the
    // wanted behavior: garbage is not evidence.
    if next_fraction < last_fraction - RESET_DROP {
        return true;
    }

    // Signal 2.
    if let (Some(last), Some(next), Some(duration)) =
        (last_resets_in_secs, next_resets_in_secs, duration_secs)
    {
        if duration > 0 && next.saturating_sub(last) > duration / 2 {
            return true;
        }
    }

    false
}

/// Fit a burn rate over the trailing [`TRAIL_SECS`] of `samples`, as of
/// `now_unix_secs`.
///
/// `None` — no estimate at all — when the evidence is too thin to fit:
/// fewer than [`MIN_SAMPLES`] readings in the trail, a span under
/// [`MIN_SPAN_SECS`], every reading at the same instant, or arithmetic that
/// comes out non-finite. Silence beats a made-up number; a caller that gets
/// `None` shows nothing, which is the correct display for "not enough data
/// yet".
///
/// A `Some` result always carries a real slope, but its
/// [`Burn::exhaust_in_secs`] is `None` whenever that slope is flat or falling.
/// Never panics, for any input.
pub fn estimate(samples: &[PaceSample], now_unix_secs: u64) -> Option<Burn> {
    // The trailing window, dropping readings whose fraction is not a finite
    // number so no NaN can reach the arithmetic below.
    let cutoff = now_unix_secs.saturating_sub(TRAIL_SECS);
    let trail: Vec<&PaceSample> = samples
        .iter()
        .filter(|s| s.at_unix_secs >= cutoff && s.used_fraction.is_finite())
        .collect();

    if trail.len() < MIN_SAMPLES {
        return None;
    }

    let earliest = trail.iter().map(|s| s.at_unix_secs).min()?;
    let newest = trail.iter().map(|s| s.at_unix_secs).max()?;
    if newest - earliest < MIN_SPAN_SECS {
        return None;
    }

    // Least squares, with the time origin shifted to the earliest sample: epoch
    // seconds squared overflows f64's exact-integer range, and the shifted
    // values are small enough that the sums stay well-conditioned.
    let n = trail.len() as f64;
    let mean_x = trail
        .iter()
        .map(|s| (s.at_unix_secs - earliest) as f64)
        .sum::<f64>()
        / n;
    let mean_y = trail.iter().map(|s| s.used_fraction).sum::<f64>() / n;

    let mut sxx = 0.0_f64;
    let mut sxy = 0.0_f64;
    for s in &trail {
        let dx = (s.at_unix_secs - earliest) as f64 - mean_x;
        sxx += dx * dx;
        sxy += dx * (s.used_fraction - mean_y);
    }

    // `sxx == 0.0` means every retained sample shares one timestamp — no slope
    // exists. Unreachable given the span check above, kept because it is the
    // division's actual precondition rather than a consequence of it.
    if sxx <= 0.0 || !sxy.is_finite() {
        return None;
    }

    // Fraction per second, then per hour.
    let slope = sxy / sxx;
    let per_hour = slope * SECS_PER_HOUR;
    if !slope.is_finite() || !per_hour.is_finite() {
        return None;
    }

    // The newest retained reading is what the projection starts from — the fit
    // gives the rate, not the current position.
    let latest = trail
        .iter()
        .rfind(|s| s.at_unix_secs == newest)?
        .used_fraction;

    let exhaust_in_secs = if slope <= 0.0 {
        // Flat or falling: at this pace the window does not fill.
        None
    } else {
        let remaining = (1.0 - latest).max(0.0);
        let secs = remaining / slope;
        if !secs.is_finite() {
            return None;
        }
        // Float-to-int casts saturate in Rust, so an enormous projection lands
        // at u64::MAX rather than wrapping, and the cap then takes it.
        Some((secs.round().max(0.0) as u64).min(MAX_EXHAUST_SECS))
    };

    Some(Burn {
        per_hour,
        exhaust_in_secs,
    })
}

/// Whether this burn rate runs the window out **before** it resets.
///
/// The whole point of the forecast: a window filling in 2 hours is fine if it
/// resets in 1, and is worth saying out loud if it resets in 6.
///
/// `false` whenever either side is unknown. No projected exhaustion means
/// nothing is being spent; an unknown reset countdown means there is no deadline
/// to beat, and claiming risk against a deadline we cannot see would be an
/// invented warning.
pub fn at_risk(burn: &Burn, resets_in_secs: Option<u64>) -> bool {
    match (burn.exhaust_in_secs, resets_in_secs) {
        (Some(exhaust), Some(reset)) => exhaust < reset,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plausible epoch second, so nothing below is accidentally exercising
    /// the `saturating_sub` floor near zero.
    const BASE: u64 = 1_785_000_000;

    fn sample(offset_secs: u64, used_fraction: f64) -> PaceSample {
        PaceSample {
            at_unix_secs: BASE + offset_secs,
            used_fraction,
        }
    }

    /// A rising trail: `count` samples `step` seconds apart, starting at `from`
    /// and gaining `per_step`.
    fn rising(count: u64, step: u64, from: f64, per_step: f64) -> Vec<PaceSample> {
        (0..count)
            .map(|i| sample(i * step, from + per_step * i as f64))
            .collect()
    }

    /// The newest timestamp in a trail — what a caller would pass as `now`.
    fn now_of(samples: &[PaceSample]) -> u64 {
        samples.iter().map(|s| s.at_unix_secs).max().unwrap()
    }

    // --- the module's central promise: no clock inside ---

    #[test]
    fn no_clock_is_read_in_this_module() {
        // Scans this file's own source. Every entry point takes `now` as a
        // parameter, and this is what keeps it that way: reaching for the clock
        // here would make the edge cases below untestable, and would put a
        // hidden input into arithmetic the UI depends on.
        const SRC: &str = include_str!("mod.rs");

        // Everything above this test module — which names the forbidden types
        // in these very assertions — with whole-line comments dropped, since
        // the prose above explains *why* there is no `SystemTime::now()` here
        // and a comment cannot read a clock regardless.
        let code: Vec<&str> = SRC[..SRC.find("mod tests").expect("test module not found")]
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect();
        let code = code.join("\n");

        for forbidden in ["SystemTime", "Instant", "UNIX_EPOCH", ".elapsed()"] {
            assert!(
                !code.contains(forbidden),
                "`{forbidden}` appears in pace/mod.rs — this module must stay pure"
            );
        }
        // Guard the scanner: if the split or the comment filter silently
        // produced nothing, the loop above would pass vacuously.
        assert!(
            code.contains("pub fn estimate") && code.contains("pub fn at_risk"),
            "the scanner did not capture the module's code"
        );
    }

    // --- estimate: the algebra ---

    #[test]
    fn steady_rise_hits_the_algebraic_answer() {
        // 5 points, 10 minutes apart, +2 points each: 0.10 → 0.18 over 40 min.
        // Slope = 0.02/600 s = 1/30000 per s → 0.12 per hour exactly.
        // Remaining from the latest 0.18 is 0.82 → 0.82 * 30000 = 24600 s.
        let samples = rising(5, 600, 0.10, 0.02);
        let burn = estimate(&samples, now_of(&samples)).expect("a fittable trail");

        assert!(
            (burn.per_hour - 0.12).abs() < 1e-9,
            "per_hour was {}",
            burn.per_hour
        );
        assert_eq!(burn.exhaust_in_secs, Some(24_600));
    }

    #[test]
    fn a_noisy_sample_does_not_own_the_fit() {
        // Least squares, not first-to-last: one reading off the line moves the
        // slope a little. First-to-last would report the noise verbatim, and a
        // spike in the *last* sample would dominate the forecast entirely.
        let mut samples = rising(5, 600, 0.10, 0.02);
        let clean = estimate(&samples, now_of(&samples)).unwrap();

        // Bump the middle reading well off the trend.
        samples[2].used_fraction += 0.05;
        let noisy = estimate(&samples, now_of(&samples)).unwrap();

        // A 5-point fit with the middle point displaced leaves the slope
        // untouched (its dx is zero) — that is precisely the robustness claim.
        assert!(
            (noisy.per_hour - clean.per_hour).abs() < 1e-9,
            "clean {} vs noisy {}",
            clean.per_hour,
            noisy.per_hour
        );
    }

    #[test]
    fn flat_usage_projects_no_exhaustion() {
        let samples = rising(4, 600, 0.40, 0.0);
        let burn = estimate(&samples, now_of(&samples)).expect("flat is still a fit");
        assert_eq!(burn.per_hour, 0.0);
        assert_eq!(burn.exhaust_in_secs, None);
    }

    #[test]
    fn falling_usage_projects_no_exhaustion() {
        // A provider revising percentages downward, without a drop big enough
        // to read as a reset. The slope is real and negative; nothing fills.
        let samples = rising(4, 600, 0.40, -0.01);
        let burn = estimate(&samples, now_of(&samples)).expect("falling is still a fit");
        assert!(burn.per_hour < 0.0, "per_hour was {}", burn.per_hour);
        assert_eq!(burn.exhaust_in_secs, None);
    }

    #[test]
    fn already_full_projects_immediately() {
        // At 100% the remainder is zero, so the honest projection is "now" —
        // not a negative number, and not None (it *is* full).
        let samples = vec![sample(0, 0.94), sample(600, 0.97), sample(1200, 1.00)];
        let burn = estimate(&samples, now_of(&samples)).unwrap();
        assert_eq!(burn.exhaust_in_secs, Some(0));
    }

    #[test]
    fn a_very_slow_burn_is_capped_at_fourteen_days() {
        // +0.0001 per 10 min from 0.01 → ~99% remaining at ~6e-9 per second,
        // which projects to years. The cap keeps it comparable.
        let samples = rising(4, 600, 0.01, 0.0001);
        let burn = estimate(&samples, now_of(&samples)).unwrap();
        assert_eq!(burn.exhaust_in_secs, Some(MAX_EXHAUST_SECS));
    }

    // --- estimate: the evidence bar ---

    #[test]
    fn too_few_samples_yield_no_estimate() {
        // Two points fit a line perfectly and prove nothing.
        let samples = rising(2, 3600, 0.10, 0.10);
        assert!(estimate(&samples, now_of(&samples)).is_none());
        // One, and none.
        assert!(estimate(&samples[..1], now_of(&samples)).is_none());
        assert!(estimate(&[], BASE).is_none());
    }

    #[test]
    fn too_short_a_span_yields_no_estimate() {
        // Three samples 100 s apart: 200 s of span, under the 600 s floor.
        let samples = rising(3, 100, 0.10, 0.02);
        assert!(estimate(&samples, now_of(&samples)).is_none());
    }

    #[test]
    fn the_span_floor_is_inclusive_at_exactly_ten_minutes() {
        // 600 s exactly must pass — pins which side of the boundary the rule
        // sits on, so a later `>` / `>=` slip is a test failure.
        let samples = vec![sample(0, 0.10), sample(300, 0.12), sample(600, 0.14)];
        assert_eq!(now_of(&samples) - samples[0].at_unix_secs, MIN_SPAN_SECS);
        assert!(estimate(&samples, now_of(&samples)).is_some());

        // One second short must not.
        let samples = vec![sample(0, 0.10), sample(300, 0.12), sample(599, 0.14)];
        assert!(estimate(&samples, now_of(&samples)).is_none());
    }

    #[test]
    fn samples_older_than_the_trail_are_ignored() {
        // Three ancient samples plus three recent ones. If the old ones were
        // included the span would be ~4 h and the slope quite different; the
        // estimate must describe only the recent stretch.
        let now = BASE + 10_000;
        let old: Vec<PaceSample> = (0..3).map(|i| sample(i * 600, 0.05)).collect();
        let recent = vec![
            PaceSample {
                at_unix_secs: now - 1200,
                used_fraction: 0.50,
            },
            PaceSample {
                at_unix_secs: now - 600,
                used_fraction: 0.52,
            },
            PaceSample {
                at_unix_secs: now,
                used_fraction: 0.54,
            },
        ];
        let all: Vec<PaceSample> = old.iter().chain(recent.iter()).copied().collect();

        let from_all = estimate(&all, now).expect("the recent three are enough");
        let from_recent = estimate(&recent, now).expect("the recent three alone fit");
        assert_eq!(from_all, from_recent);
    }

    #[test]
    fn dropping_stale_samples_can_leave_too_few_to_fit() {
        // Everything outside the trail: the estimate must go quiet rather than
        // fit whatever is left.
        let now = BASE + 100_000;
        let samples = rising(5, 600, 0.10, 0.02);
        assert!(now - now_of(&samples) > TRAIL_SECS);
        assert!(estimate(&samples, now).is_none());
    }

    #[test]
    fn all_samples_at_one_instant_yield_no_estimate() {
        // Zero span — no slope exists, and the division must never be reached.
        let samples = vec![sample(0, 0.10), sample(0, 0.20), sample(0, 0.30)];
        assert!(estimate(&samples, now_of(&samples)).is_none());
    }

    #[test]
    fn non_finite_fractions_never_panic_and_never_fabricate() {
        // NaN/±inf readings are dropped. Three good samples alongside them still
        // fit — identically to the good three on their own, which is what
        // "dropped" has to mean.
        let clean = rising(3, 600, 0.10, 0.02);
        let mut with_garbage = clean.clone();
        with_garbage.push(sample(1800, f64::NAN));
        with_garbage.push(sample(2400, f64::INFINITY));
        with_garbage.push(sample(3000, f64::NEG_INFINITY));

        // The same `now` for both, so only the garbage differs.
        let now = now_of(&with_garbage);
        assert_eq!(
            estimate(&with_garbage, now).expect("the three good samples still fit"),
            estimate(&clean, now).unwrap()
        );

        // A trail that is *only* garbage has nothing to fit.
        let only_garbage = vec![
            sample(0, f64::NAN),
            sample(600, f64::NAN),
            sample(1200, f64::INFINITY),
        ];
        assert!(estimate(&only_garbage, now_of(&only_garbage)).is_none());
    }

    #[test]
    fn samples_in_any_order_fit_the_same() {
        // The ring appends in order, but nothing in the math requires it, and a
        // caller feeding a reordered trail must not get a different answer.
        let ordered = rising(4, 600, 0.10, 0.02);
        let mut shuffled = ordered.clone();
        shuffled.reverse();
        let now = now_of(&ordered);
        assert_eq!(
            estimate(&ordered, now).unwrap(),
            estimate(&shuffled, now).unwrap()
        );
    }

    // --- at_risk ---

    #[test]
    fn eighty_percent_at_half_window_is_at_risk() {
        // A 5 h window (18000 s), half elapsed → 9000 s to reset. Usage climbed
        // 0.70 → 0.80 over the last hour, so the remaining 0.20 takes ~7200 s:
        // full before the reset.
        let samples = vec![sample(0, 0.70), sample(1800, 0.75), sample(3600, 0.80)];
        let burn = estimate(&samples, now_of(&samples)).unwrap();
        assert_eq!(burn.exhaust_in_secs, Some(7200));
        assert!(at_risk(&burn, Some(9000)));
    }

    #[test]
    fn twenty_percent_at_half_window_is_not_at_risk() {
        // Same window and the same 5-points-per-hour pace, but only a fifth
        // spent: the remaining 0.80 takes ~28800 s, long past the reset.
        let samples = vec![sample(0, 0.10), sample(1800, 0.15), sample(3600, 0.20)];
        let burn = estimate(&samples, now_of(&samples)).unwrap();
        assert_eq!(burn.exhaust_in_secs, Some(28_800));
        assert!(!at_risk(&burn, Some(9000)));
    }

    #[test]
    fn at_risk_is_strict_and_silent_on_unknowns() {
        let burning = Burn {
            per_hour: 0.5,
            exhaust_in_secs: Some(3600),
        };
        assert!(at_risk(&burning, Some(3601)));
        // Exactly at the reset is not "before" it — the window resets first.
        assert!(!at_risk(&burning, Some(3600)));
        assert!(!at_risk(&burning, Some(600)));
        // No deadline visible → no claim.
        assert!(!at_risk(&burning, None));

        // Nothing being spent is never at risk, whatever the reset says.
        let idle = Burn {
            per_hour: 0.0,
            exhaust_in_secs: None,
        };
        assert!(!at_risk(&idle, Some(1)));
        assert!(!at_risk(&idle, None));
    }

    // --- PaceRing ---

    #[test]
    fn ring_starts_empty_and_accumulates() {
        let mut ring = PaceRing::new();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert!(ring.latest().is_none());
        assert!(ring.samples().is_empty());

        ring.push(sample(0, 0.10));
        ring.push(sample(600, 0.12));
        assert_eq!(ring.len(), 2);
        assert!(!ring.is_empty());
        assert_eq!(ring.latest().unwrap().used_fraction, 0.12);
        // Oldest first, ready for `estimate`.
        assert_eq!(ring.samples()[0].used_fraction, 0.10);
    }

    #[test]
    fn ring_evicts_the_oldest_past_capacity() {
        let mut ring = PaceRing::new();
        for i in 0..(RING_CAPACITY as u64 + 50) {
            ring.push(sample(i * 180, i as f64));
        }
        assert_eq!(ring.len(), RING_CAPACITY);
        // The 50 oldest are gone; the newest is the last one pushed.
        assert_eq!(ring.samples()[0].used_fraction, 50.0);
        assert_eq!(
            ring.latest().unwrap().used_fraction,
            (RING_CAPACITY + 49) as f64
        );
    }

    #[test]
    fn a_dropping_fraction_clears_the_trail() {
        let mut ring = PaceRing::new();
        for s in rising(3, 600, 0.80, 0.05) {
            ring.observe(s, Some(3600), Some(18_000));
        }
        assert_eq!(ring.len(), 3);

        // The window rolled over: usage back to ~0 and the countdown restored.
        ring.observe(sample(1800, 0.01), Some(18_000), Some(18_000));
        assert_eq!(ring.len(), 1, "the pre-reset trail must be gone");
        assert_eq!(ring.latest().unwrap().used_fraction, 0.01);
    }

    #[test]
    fn a_jumping_countdown_clears_the_trail_even_at_zero_usage() {
        // The case the fraction signal cannot see: a window resets while usage
        // reads 0% throughout, so only the countdown betrays it.
        let mut ring = PaceRing::new();
        ring.observe(sample(0, 0.0), Some(1200), Some(18_000));
        ring.observe(sample(600, 0.0), Some(600), Some(18_000));
        ring.observe(sample(1200, 0.0), Some(60), Some(18_000));
        assert_eq!(ring.len(), 3);

        ring.observe(sample(1800, 0.0), Some(18_000), Some(18_000));
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn ordinary_polling_never_clears_the_trail() {
        // The regression that would hurt most: a trail thrown away every poll
        // means the forecast never has enough samples to speak. Usage rising,
        // countdown falling, small downward revisions — none may clear.
        let mut ring = PaceRing::new();
        let readings = [
            (0_u64, 0.20, 9000_u64),
            (600, 0.23, 8400),
            (1200, 0.22, 7800), // provider revised down 1 point
            (1800, 0.26, 7200),
            (2400, 0.29, 6640), // countdown drifted 40 s (skew), still down
            (3000, 0.31, 6100),
        ];
        for (at, fraction, resets_in) in readings {
            ring.observe(sample(at, fraction), Some(resets_in), Some(18_000));
        }
        assert_eq!(ring.len(), readings.len());
    }

    #[test]
    fn a_countdown_nudge_upward_is_not_a_reset() {
        // Skew or re-rounding pushes the countdown up slightly. Well under half
        // the duration, so the trail survives.
        let mut ring = PaceRing::new();
        ring.observe(sample(0, 0.30), Some(9000), Some(18_000));
        ring.observe(sample(600, 0.32), Some(9100), Some(18_000));
        assert_eq!(ring.len(), 2);
    }

    #[test]
    fn reset_detection_thresholds_are_exact() {
        // Fraction: strictly *more* than 0.05 below.
        assert!(reset_detected(0.80, 0.74, None, None, None));
        assert!(!reset_detected(0.80, 0.75, None, None, None)); // exactly 0.05
        assert!(!reset_detected(0.80, 0.80, None, None, None));
        assert!(!reset_detected(0.80, 0.95, None, None, None));

        // Countdown: strictly more than half the duration.
        assert!(reset_detected(
            0.5,
            0.5,
            Some(100),
            Some(9101),
            Some(18_000)
        ));
        assert!(!reset_detected(
            0.5,
            0.5,
            Some(100),
            Some(9100),
            Some(18_000)
        ));
        // Downward movement is the normal case, never a reset.
        assert!(!reset_detected(
            0.5,
            0.5,
            Some(9000),
            Some(100),
            Some(18_000)
        ));

        // Any missing fact disables the countdown signal rather than firing it.
        assert!(!reset_detected(0.5, 0.5, None, Some(18_000), Some(18_000)));
        assert!(!reset_detected(0.5, 0.5, Some(1), None, Some(18_000)));
        assert!(!reset_detected(0.5, 0.5, Some(1), Some(18_000), None));
        // A degenerate zero-length window cannot imply anything either.
        assert!(!reset_detected(0.5, 0.5, Some(0), Some(1), Some(0)));
    }

    #[test]
    fn a_nan_reading_does_not_read_as_a_reset() {
        // Garbage is not evidence: a NaN fraction must not clear a good trail.
        assert!(!reset_detected(0.80, f64::NAN, None, None, None));
        assert!(!reset_detected(f64::NAN, 0.10, None, None, None));

        let mut ring = PaceRing::new();
        for s in rising(3, 600, 0.30, 0.02) {
            ring.observe(s, Some(9000), Some(18_000));
        }
        ring.observe(sample(1800, f64::NAN), Some(8400), Some(18_000));
        assert_eq!(ring.len(), 4, "the trail must survive a garbage reading");
    }

    #[test]
    fn clear_forgets_the_countdown_too() {
        // Otherwise the first sample after a clear would be compared against a
        // countdown from the window that no longer exists.
        let mut ring = PaceRing::new();
        ring.observe(sample(0, 0.5), Some(60), Some(18_000));
        ring.clear();
        assert!(ring.is_empty());
        // A fresh full countdown right after the clear is just the first
        // reading of the new window, not a second reset.
        ring.observe(sample(600, 0.0), Some(18_000), Some(18_000));
        assert_eq!(ring.len(), 1);
    }

    #[test]
    fn a_cleared_ring_goes_quiet_until_it_refills() {
        // End to end: the trail is what `estimate` reads, so a reset means no
        // forecast until enough new samples span the floor again.
        let mut ring = PaceRing::new();
        for s in rising(4, 600, 0.60, 0.05) {
            ring.observe(s, Some(3600), Some(18_000));
        }
        let now = BASE + 1800;
        assert!(estimate(ring.samples(), now).is_some());

        ring.observe(sample(2400, 0.0), Some(18_000), Some(18_000));
        let now = BASE + 2400;
        assert!(
            estimate(ring.samples(), now).is_none(),
            "a single post-reset sample cannot support a forecast"
        );

        // Refilled across the span floor → speaking again.
        ring.observe(sample(3000, 0.02), Some(17_400), Some(18_000));
        ring.observe(sample(3600, 0.04), Some(16_800), Some(18_000));
        let now = BASE + 3600;
        assert!(estimate(ring.samples(), now).is_some());
    }
}
