//! `journal_date` is the entry's day with a 04:00 boundary (PRD §4.1). Pure arithmetic: the time zone offset at the moment
//! of the entry comes from outside, so the daylight saving switch is handled by whoever knows that offset. The calendar is
//! `chrono`'s (code style rule 6).

use chrono::{DateTime, Datelike, NaiveDate, TimeDelta};

/// The day ends at 04:00: an entry at 00:30 belongs to yesterday. Setting the boundary is not in slice 1.
pub const DAY_BOUNDARY_MINUTES: u32 = 4 * 60;

/// The entry date `YYYY-MM-DD`: local time (`UTC + offset`) minus the day boundary. The date is stored and synced, so the
/// bytes must not change: `journal_oracle_tests` compares this function with the hand-written algorithm it replaced.
pub fn journal_date(created_at_ms: i64, utc_offset_secs: i32, boundary_minutes: u32) -> String {
    let day = shifted_day(created_at_ms, utc_offset_secs, boundary_minutes);
    format!("{:04}-{:02}-{:02}", day.year(), day.month(), day.day())
}

/// The calendar day of the moment `created_at_ms` shifted by the zone offset and back by the day boundary.
///
/// Beyond the years `chrono` can represent (about +-262,000), the day is clamped to its first or last day by the sign of
/// the moment, and nothing panics. No such moment reaches this function in the app: the clock and the zone come from
/// `chrono` itself, and an imported time is cut to `1970 ... now + 1 day` before it is used.
fn shifted_day(created_at_ms: i64, utc_offset_secs: i32, boundary_minutes: u32) -> NaiveDate {
    let shifted = DateTime::from_timestamp_millis(created_at_ms).and_then(|moment| {
        let offset = TimeDelta::try_seconds(i64::from(utc_offset_secs))?;
        let boundary = TimeDelta::try_minutes(i64::from(boundary_minutes))?;
        moment
            .naive_utc()
            .checked_add_signed(offset.checked_sub(&boundary)?)
    });
    match shifted {
        Some(moment) => moment.date(),
        None if created_at_ms < 0 => NaiveDate::MIN,
        None => NaiveDate::MAX,
    }
}

/// `YYYY-MM-DD` with real digits and a month 01-12, a day 01-31 (a format check; whether the date exists is not checked).
pub fn is_journal_date(text: &str) -> bool {
    date_parts(text)
        .is_some_and(|(_, month, day)| (1..=12).contains(&month) && (1..=31).contains(&day))
}

/// The year, month and day from `YYYY-MM-DD`: exactly 10 ASCII bytes, separators in place, the rest digits. Ranges are
/// not checked. It works on bytes, so on any string (including non-ASCII) it answers `None` instead of panicking.
pub fn date_parts(text: &str) -> Option<(i32, u32, u32)> {
    let [y1, y2, y3, y4, b'-', m1, m2, b'-', d1, d2] = *text.as_bytes() else {
        return None;
    };
    let number = |digits: &[u8]| -> Option<u32> {
        digits.iter().try_fold(0u32, |acc, &digit| {
            digit
                .is_ascii_digit()
                .then(|| acc * 10 + u32::from(digit - b'0'))
        })
    };
    let year = i32::try_from(number(&[y1, y2, y3, y4])?).ok()?;
    Some((year, number(&[m1, m2])?, number(&[d1, d2])?))
}

/// A real date `YYYY-MM-DD`: the format, a month 01-12, a day within the month (with leap years). Future dates are
/// allowed (ADR-0001 §2): `journal_date` is the content of the entry, not a clock. The year is not earlier than 1000 and
/// not later than 9999, so that an entry does not drift into "year 0000", where the calendar is undefined.
pub fn is_valid_journal_date(text: &str) -> bool {
    let Some((year, month, day)) = date_parts(text) else {
        return false;
    };
    year >= 1000 && NaiveDate::from_ymd_opt(year, month, day).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    /// A UTC moment in ms from components (the test does not depend on the machine's time zone).
    fn utc(y: i64, m: u32, d: u32, h: i64, min: i64) -> i64 {
        let midnight = NaiveDate::from_ymd_opt(y as i32, m, d)
            .unwrap()
            .and_time(NaiveTime::MIN)
            .and_utc();
        midnight.timestamp_millis() + (h * 60 + min) * 60_000
    }

    const MOSCOW: i32 = 3 * 3600;

    #[test]
    fn epoch() {
        assert_eq!(journal_date(0, 0, 0), "1970-01-01");
        assert_eq!(journal_date(0, 0, DAY_BOUNDARY_MINUTES), "1969-12-31");
    }

    #[test]
    fn boundary_at_four_in_the_morning() {
        // 2026-10-21 in Moscow (UTC+3).
        assert_eq!(
            journal_date(
                utc(2026, 10, 21, 0, 30) - i64::from(MOSCOW) * 1000,
                MOSCOW,
                DAY_BOUNDARY_MINUTES
            ),
            "2026-10-20",
            "00:30 is still yesterday"
        );
        assert_eq!(
            journal_date(
                utc(2026, 10, 21, 3, 59) - i64::from(MOSCOW) * 1000,
                MOSCOW,
                DAY_BOUNDARY_MINUTES
            ),
            "2026-10-20",
            "03:59 is still yesterday"
        );
        assert_eq!(
            journal_date(
                utc(2026, 10, 21, 4, 0) - i64::from(MOSCOW) * 1000,
                MOSCOW,
                DAY_BOUNDARY_MINUTES
            ),
            "2026-10-21",
            "04:00 is already today"
        );
        assert_eq!(
            journal_date(
                utc(2026, 10, 21, 23, 59) - i64::from(MOSCOW) * 1000,
                MOSCOW,
                DAY_BOUNDARY_MINUTES
            ),
            "2026-10-21"
        );
    }

    #[test]
    fn boundary_across_spring_forward() {
        // Berlin, 2026-03-29: at 01:00 UTC the clocks go from +01:00 to +02:00 (02:00 -> 03:00 local).
        let before = 3600; // CET
        let after = 7200; // CEST
        // 00:59 UTC = 01:59 CET: before the 04:00 boundary, so the previous day.
        assert_eq!(
            journal_date(utc(2026, 3, 29, 0, 59), before, DAY_BOUNDARY_MINUTES),
            "2026-03-28"
        );
        // 01:59 UTC = 03:59 CEST: the switch has already happened, but it is before 04:00, so still the previous day.
        assert_eq!(
            journal_date(utc(2026, 3, 29, 1, 59), after, DAY_BOUNDARY_MINUTES),
            "2026-03-28"
        );
        // 02:00 UTC = 04:00 CEST: the day boundary; without the new offset (+01:00) it would be 03:00 and yesterday's date.
        assert_eq!(
            journal_date(utc(2026, 3, 29, 2, 0), after, DAY_BOUNDARY_MINUTES),
            "2026-03-29"
        );
        assert_eq!(
            journal_date(utc(2026, 3, 29, 2, 0), before, DAY_BOUNDARY_MINUTES),
            "2026-03-28",
            "the old offset would give another day"
        );
    }

    #[test]
    fn boundary_across_fall_back() {
        // Berlin, 2026-10-25: at 01:00 UTC the clocks go from +02:00 to +01:00 (03:00 -> 02:00).
        assert_eq!(
            journal_date(utc(2026, 10, 25, 1, 59), 3600, DAY_BOUNDARY_MINUTES),
            "2026-10-24",
            "02:59 CET"
        );
        assert_eq!(
            journal_date(utc(2026, 10, 25, 2, 59), 3600, DAY_BOUNDARY_MINUTES),
            "2026-10-24",
            "03:59 CET"
        );
        assert_eq!(
            journal_date(utc(2026, 10, 25, 3, 0), 3600, DAY_BOUNDARY_MINUTES),
            "2026-10-25",
            "04:00 CET"
        );
    }

    #[test]
    fn negative_offsets_and_leap_days() {
        // New York (UTC-5), 2024-02-29 03:00 local is still the 28th (a leap year).
        assert_eq!(
            journal_date(utc(2024, 2, 29, 8, 0), -5 * 3600, DAY_BOUNDARY_MINUTES),
            "2024-02-28"
        );
        assert_eq!(
            journal_date(utc(2024, 2, 29, 9, 0), -5 * 3600, DAY_BOUNDARY_MINUTES),
            "2024-02-29"
        );
        assert_eq!(
            journal_date(utc(2024, 3, 1, 3, 0), 0, DAY_BOUNDARY_MINUTES),
            "2024-02-29",
            "before 04:00 on March 1 it is February 29"
        );
        assert_eq!(
            journal_date(utc(2026, 1, 1, 3, 59), 0, DAY_BOUNDARY_MINUTES),
            "2025-12-31",
            "year boundary"
        );
        assert_eq!(
            journal_date(utc(1969, 12, 31, 12, 0), 0, 0),
            "1969-12-31",
            "before the epoch"
        );
    }

    #[test]
    fn journal_date_format_check() {
        assert!(is_journal_date("2026-10-21"));
        for bad in [
            "",
            "2026-1-21",
            "2026/10/21",
            "2026-13-01",
            "2026-00-10",
            "2026-10-00",
            "../../etc/x",
            "2026-10-211",
            "202a-10-21",
        ] {
            assert!(!is_journal_date(bad), "{bad}");
        }
    }
}
