//! Dependency-free timestamp parsing shared by providers.
//!
//! Kept deliberately small and inside the trust boundary crate: pulling a
//! date/time dependency for two functions would grow the audit tree for no
//! gain (CONTRIBUTING.md dependency policy).
//!
//! Two validation rules are stricter than a naive range check, because the
//! output of this module is a *countdown the user reads and trusts*:
//!
//! - **Days are calendar-true.** The day is checked against
//!   [`days_in_month`] for the actual year and month — leap years included
//!   (divisible by 4, except centuries unless also divisible by 400). A
//!   plain `1..=31` check accepted `2026-02-31` and `2026-04-31` and let
//!   [`days_from_civil`] silently roll them forward into March and May, so a
//!   malformed reset timestamp became a *wrong* reset time rather than an
//!   unknown one.
//! - **A missing UTC offset is rejected.** RFC 3339 requires an offset, and
//!   inferring UTC from its absence is exactly the kind of silent assumption
//!   this codebase does not make: a bare local time read as UTC is off by
//!   the reader's own timezone, quietly. Every rejection fails closed —
//!   `None` renders as "reset unknown", never as a confident wrong answer.

/// Parse an RFC 3339 / ISO 8601 timestamp to a Unix second count.
///
/// Handles the forms provider endpoints return:
/// `YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`. Fractional seconds are
/// ignored; the offset is applied and is **required**. Days are validated
/// against the real length of the month. Returns `None` on any malformed
/// input — callers treat that as "reset unknown".
pub(crate) fn parse_rfc3339_to_unix(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    // Split date and time on 'T' (accept lowercase 't' defensively).
    let t_pos = bytes.iter().position(|&b| b == b'T' || b == b't')?;
    let (date, rest) = (&s[..t_pos], &s[t_pos + 1..]);

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) {
        return None;
    }
    // Calendar-true, not `1..=31`: `days_from_civil` below happily rolls an
    // impossible day into the next month, turning a malformed timestamp into a
    // plausible-looking wrong instant.
    if !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }

    // Locate the offset marker: 'Z'/'z', or a '+'/'-' after the time.
    let (time_str, offset_secs) = split_offset(rest)?;

    // Strip any fractional part.
    let time_core = time_str.split('.').next()?;
    let mut time_parts = time_core.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    if time_parts.next().is_some()
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let secs = days * 86_400 + hour * 3_600 + minute * 60 + second;
    // A positive offset means local time is ahead of UTC → subtract to get UTC.
    Some(secs - offset_secs)
}

/// Split the time-plus-offset tail into (time_without_offset, offset_seconds).
fn split_offset(rest: &str) -> Option<(&str, i64)> {
    if let Some(stripped) = rest.strip_suffix(['Z', 'z']) {
        return Some((stripped, 0));
    }
    // Find the sign that introduces the numeric offset. It follows the time,
    // so search from the part after position 0 (hour digits can't be signed).
    let rb = rest.as_bytes();
    for i in 1..rb.len() {
        if rb[i] == b'+' || rb[i] == b'-' {
            let (time, off) = (&rest[..i], &rest[i..]);
            let sign = if rb[i] == b'+' { 1 } else { -1 };
            let mut op = off[1..].split(':');
            let oh: i64 = op.next()?.parse().ok()?;
            let om: i64 = op.next().unwrap_or("0").parse().ok()?;
            if op.next().is_some() || !(0..=23).contains(&oh) || !(0..=59).contains(&om) {
                return None;
            }
            return Some((time, sign * (oh * 3_600 + om * 60)));
        }
    }
    // No offset marker at all. RFC 3339 requires one; assuming UTC would be a
    // guess about the writer's timezone, and a wrong guess yields a countdown
    // that is confidently off by hours. Fail closed — the caller renders
    // "unknown" instead.
    None
}

/// Is `year` a leap year in the proleptic Gregorian calendar?
///
/// Divisible by 4, except centuries, which must also be divisible by 400 —
/// so 2024 and 2000 are leap years and 2026 and 1900 are not.
fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Number of days in `month` (1–12) of `year`.
///
/// An out-of-range month yields 0, which rejects every day value — callers
/// validate the month first, so this arm is belt-and-braces, and it fails
/// closed rather than panicking inside a parser.
fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since the Unix epoch for a civil (proleptic Gregorian) date.
/// Howard Hinnant's `days_from_civil` algorithm.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_known_anchors() {
        assert_eq!(parse_rfc3339_to_unix("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            parse_rfc3339_to_unix("2000-01-01T00:00:00Z"),
            Some(946_684_800)
        );
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T00:00:00Z"),
            Some(1_767_225_600)
        );
        // Fractional seconds ignored; explicit +00:00 offset.
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T00:00:00.528743+00:00"),
            Some(1_767_225_600)
        );
    }

    #[test]
    fn rfc3339_applies_offset() {
        // 01:00:00+01:00 is the same instant as 00:00:00Z.
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T01:00:00+01:00"),
            Some(1_767_225_600)
        );
        // 23:00:00-01:00 on the prior day is also 2026-01-01T00:00:00Z.
        assert_eq!(
            parse_rfc3339_to_unix("2025-12-31T23:00:00-01:00"),
            Some(1_767_225_600)
        );
    }

    #[test]
    fn rfc3339_rejects_garbage() {
        assert_eq!(parse_rfc3339_to_unix("not-a-date"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-13-01T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T25:00:00Z"), None);
    }

    // --- M9b: calendar-true day validation ---

    #[test]
    fn leap_year_rule_covers_centuries() {
        assert!(is_leap_year(2024));
        assert!(is_leap_year(2000)); // century divisible by 400
        assert!(!is_leap_year(1900)); // century NOT divisible by 400
        assert!(!is_leap_year(2026));
        assert!(!is_leap_year(2100));
    }

    #[test]
    fn days_in_month_is_calendar_true() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(1900, 2), 28);
        assert_eq!(days_in_month(2000, 2), 29);
        // Out of range fails closed: no day can satisfy 1..=0.
        assert_eq!(days_in_month(2026, 0), 0);
        assert_eq!(days_in_month(2026, 13), 0);
    }

    #[test]
    fn rfc3339_rejects_days_the_month_does_not_have() {
        // Before M9b these passed a `1..=31` check and `days_from_civil`
        // rolled them forward — 2026-02-31 became March 3rd, i.e. a wrong
        // reset time presented as a real one.
        assert_eq!(parse_rfc3339_to_unix("2026-02-31T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-04-31T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-02-29T00:00:00Z"), None); // not a leap year
        assert_eq!(parse_rfc3339_to_unix("2026-01-32T00:00:00Z"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-01-00T00:00:00Z"), None);

        // 2024 IS a leap year, so the 29th is a real date.
        assert_eq!(
            parse_rfc3339_to_unix("2024-02-29T00:00:00Z"),
            Some(1_709_164_800)
        );
        // Century rules, end to end: 1900 has no Feb 29, 2000 does.
        assert_eq!(parse_rfc3339_to_unix("1900-02-29T00:00:00Z"), None);
        assert!(parse_rfc3339_to_unix("2000-02-29T00:00:00Z").is_some());

        // The last valid day of every 2026 month still parses.
        for (month, last) in [
            (1, 31),
            (2, 28),
            (3, 31),
            (4, 30),
            (5, 31),
            (6, 30),
            (7, 31),
            (8, 31),
            (9, 30),
            (10, 31),
            (11, 30),
            (12, 31),
        ] {
            let ok = format!("2026-{month:02}-{last:02}T00:00:00Z");
            assert!(
                parse_rfc3339_to_unix(&ok).is_some(),
                "{ok} should be a valid date"
            );
            let over = format!("2026-{month:02}-{:02}T00:00:00Z", last + 1);
            assert_eq!(
                parse_rfc3339_to_unix(&over),
                None,
                "{over} should be rejected"
            );
        }
    }

    // --- M9b: a missing UTC offset is rejected ---

    #[test]
    fn rfc3339_requires_an_explicit_offset() {
        // RFC 3339 requires one. Guessing UTC would silently produce a
        // countdown wrong by the writer's timezone; unknown is the honest
        // answer (the caller renders "unknown", never a wrong reset).
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:00:00"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:00:00.528743"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:00"), None);

        // The accepted spellings still work, both cases of the zulu marker.
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T00:00:00Z"),
            Some(1_767_225_600)
        );
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01t00:00:00z"),
            Some(1_767_225_600)
        );
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T00:00:00+00:00"),
            Some(1_767_225_600)
        );
    }

    #[test]
    fn rfc3339_round_trips_positive_and_negative_offsets() {
        // A half-hour offset exercises the minutes field, not just hours.
        // 05:30:00+05:30 is the same instant as 00:00:00Z.
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T05:30:00+05:30"),
            Some(1_767_225_600)
        );
        // ...and the negative direction, across a date boundary.
        assert_eq!(
            parse_rfc3339_to_unix("2025-12-31T18:30:00-05:30"),
            Some(1_767_225_600)
        );
        // The maximum sane offsets on either side.
        assert_eq!(
            parse_rfc3339_to_unix("2026-01-01T23:59:00+23:59"),
            parse_rfc3339_to_unix("2026-01-01T00:00:00Z")
        );
        assert_eq!(
            parse_rfc3339_to_unix("2025-12-31T00:01:00-23:59"),
            parse_rfc3339_to_unix("2026-01-01T00:00:00Z")
        );
        // An out-of-range offset is rejected, not wrapped.
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:00:00+24:00"), None);
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:00:00+00:60"), None);
    }

    #[test]
    fn rfc3339_enforces_time_field_bounds() {
        // Hour 0–23.
        assert!(parse_rfc3339_to_unix("2026-01-01T23:00:00Z").is_some());
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T24:00:00Z"), None);
        // Minute 0–59.
        assert!(parse_rfc3339_to_unix("2026-01-01T00:59:00Z").is_some());
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:60:00Z"), None);
        // Second 0–60 — 60 is deliberate, for a leap second.
        assert!(parse_rfc3339_to_unix("2026-01-01T00:00:59Z").is_some());
        assert!(parse_rfc3339_to_unix("2026-12-31T23:59:60Z").is_some());
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:00:61Z"), None);
        // A fourth time field is not a time.
        assert_eq!(parse_rfc3339_to_unix("2026-01-01T00:00:00:00Z"), None);
    }
}
