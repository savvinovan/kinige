//! `journal_date` is the entry's day with a 04:00 boundary (PRD §4.1). Pure arithmetic: the time zone offset at the moment
//! of the entry comes from outside, so the daylight saving switch is handled by whoever knows that offset.

/// The day ends at 04:00: an entry at 00:30 belongs to yesterday. Setting the boundary is not in slice 1.
pub const DAY_BOUNDARY_MINUTES: u32 = 4 * 60;

/// The entry date `YYYY-MM-DD`: local time (`UTC + offset`) minus the day boundary.
pub fn journal_date(created_at_ms: i64, utc_offset_secs: i32, boundary_minutes: u32) -> String {
    let local_secs = created_at_ms.div_euclid(1000) + i64::from(utc_offset_secs);
    let shifted = local_secs - i64::from(boundary_minutes) * 60;
    let (year, month, day) = civil_from_days(shifted.div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Days since 1970-01-01 -> (year, month, day) in the Gregorian calendar (Howard Hinnant's algorithm, `civil_from_days`).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let mp = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
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
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year >= 1000 && (1..=days).contains(&day)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A UTC moment in ms from components (the test does not depend on the machine's time zone).
    fn utc(y: i64, m: u32, d: u32, h: i64, min: i64) -> i64 {
        let days = days_from_civil(y, m, d);
        ((days * 24 + h) * 60 + min) * 60_000
    }

    fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = y.div_euclid(400);
        let yoe = y.rem_euclid(400);
        let mp = i64::from(if m > 2 { m - 3 } else { m + 9 });
        let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
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
