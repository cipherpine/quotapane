//! Dependency-free timestamp parsing shared by providers.
//!
//! Kept deliberately small and inside the trust boundary crate: pulling a
//! date/time dependency for two functions would grow the audit tree for no
//! gain (CONTRIBUTING.md dependency policy).

/// Parse an RFC 3339 / ISO 8601 timestamp to a Unix second count.
///
/// Handles the forms provider endpoints return:
/// `YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`. Fractional seconds are
/// ignored; the offset is applied. Returns `None` on any malformed input —
/// callers treat that as "reset unknown".
pub(crate) fn parse_rfc3339_to_unix(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    // Split date and time on 'T' (accept lowercase 't' defensively).
    let t_pos = bytes.iter().position(|&b| b == b'T' || b == b't')?;
    let (date, rest) = (&s[..t_pos], &s[t_pos + 1..]);

    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
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
    // No offset marker at all — treat as UTC (endpoints always send one, but
    // fail open to UTC rather than reject a bare local time).
    Some((rest, 0))
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
}
