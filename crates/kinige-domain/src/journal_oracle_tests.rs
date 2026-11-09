//! `journal_date` and `is_valid_journal_date` against the hand-written code that `chrono` replaced (KINIG-103). The date is
//! stored in `message.journal_date` and goes into the sync payload, so the bytes must not move: the old algorithms live
//! here as the oracle, and the new functions are compared with them on random moments, on the edges of the calendar and
//! on every shape of a date. The seed is fixed, so a failure repeats.

use crate::journal::{DAY_BOUNDARY_MINUTES, date_parts, is_valid_journal_date, journal_date};
use chrono::{Datelike, NaiveDate, NaiveTime};

const SEED: u64 = 0x4B49_4E49_4745;

/// About 253,000 years in milliseconds: well inside the years `chrono` represents (+-262,000), with room for any offset.
const IN_RANGE_MS: i64 = 8_000_000_000_000_000;

const OFFSETS: [i32; 12] = [
    -43_200, -28_800, -18_000, -3_600, 0, 3_600, 7_200, 10_800, 19_800, 20_700, 43_200, 50_400,
];

// The oracle: the code as it was before KINIG-103, copied unchanged.

fn old_journal_date(created_at_ms: i64, utc_offset_secs: i32, boundary_minutes: u32) -> String {
    let local_secs = created_at_ms.div_euclid(1000) + i64::from(utc_offset_secs);
    let shifted = local_secs - i64::from(boundary_minutes) * 60;
    let (year, month, day) = old_civil_from_days(shifted.div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

fn old_civil_from_days(days: i64) -> (i64, u32, u32) {
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

fn old_is_valid_journal_date(text: &str) -> bool {
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

#[test]
fn random_moments_match_the_old_algorithm() {
    let mut rng = Rng(SEED);
    for _ in 0..400_000 {
        let ms = match rng.below(5) {
            0 => rng.signed(IN_RANGE_MS),
            1 => rng.signed(300_000_000_000_000), // about +-9,500 years
            2 => 1_767_225_600_000 + rng.signed(2 * 365 * DAY_MS), // around 2026
            3 => rng.signed(5_000_000_000_000) - 1, // 1970 +- 160 years
            _ => {
                // A second or two around a midnight or the 04:00 boundary of a random day.
                let day = rng.signed(3_000_000);
                day * DAY_MS + [0, 4 * 3_600_000][rng.below(2) as usize] + rng.signed(3000)
            }
        };
        let offset = match rng.below(3) {
            0 => OFFSETS[rng.below(OFFSETS.len() as u64) as usize],
            1 => rng.signed(50_400) as i32,
            _ => 0,
        };
        let boundary = match rng.below(4) {
            0 => 0,
            1 => DAY_BOUNDARY_MINUTES,
            2 => rng.below(2000) as u32,
            _ => rng.below(1_000_000_000) as u32,
        };
        assert_eq!(
            journal_date(ms, offset, boundary),
            old_journal_date(ms, offset, boundary),
            "ms={ms} offset={offset} boundary={boundary}"
        );
    }
}

#[test]
fn calendar_edges_match_the_old_algorithm() {
    // Years: the lowest and highest the app accepts, the end of the Julian-looking ones (1900, 2100 are not leap, 2000 and
    // 2400 are), before the epoch, and the years where the formatting changes (0, -1).
    let years = [
        -1, 0, 1, 1000, 1600, 1899, 1900, 1969, 1970, 1999, 2000, 2023, 2024, 2025, 2026, 2100,
        2400, 9998, 9999,
    ];
    let around = [
        -86_401, -86_400, -14_401, -14_400, -2, -1, 0, 1, 2, 14_399, 14_400, 14_401, 86_399, 86_400,
    ];
    let mut compared = 0;
    for year in years {
        for month in 1..=12 {
            for day in [1, 2, 27, 28, 29, 30, 31] {
                let Some(date) = NaiveDate::from_ymd_opt(year, month, day) else {
                    continue;
                };
                let midnight = date.and_time(NaiveTime::MIN).and_utc().timestamp_millis();
                for secs in around {
                    for offset in OFFSETS {
                        for boundary in [0, DAY_BOUNDARY_MINUTES] {
                            let ms = midnight + secs * 1000 + 999;
                            assert_eq!(
                                journal_date(ms, offset, boundary),
                                old_journal_date(ms, offset, boundary),
                                "{date} {secs}s offset={offset} boundary={boundary}"
                            );
                            compared += 1;
                        }
                    }
                }
            }
        }
    }
    assert!(compared > 100_000, "the edge loop ran: {compared}");
}

#[test]
fn days_of_a_zone_switch_match_the_old_algorithm() {
    // (the UTC moment of the switch, the offset before, the offset after): Berlin, New York and Sydney, both directions.
    let switches = [
        (1_774_746_000_000, 3_600, 7_200),     // Berlin 2026-03-29T01:00Z
        (1_792_890_000_000, 7_200, 3_600),     // Berlin 2026-10-25T01:00Z
        (1_772_953_200_000, -18_000, -14_400), // New York 2026-03-08T07:00Z
        (1_793_512_800_000, -14_400, -18_000), // New York 2026-11-01T06:00Z
        (1_775_318_400_000, 39_600, 36_000),   // Sydney 2026-04-04T16:00Z
        (1_791_043_200_000, 36_000, 39_600),   // Sydney 2026-10-03T16:00Z
    ];
    for (switch, before, after) in switches {
        // Every five minutes for three days around the switch, with both offsets (the caller picks the right one).
        for step in -432..=432 {
            let ms = switch + step * 300_000;
            for offset in [before, after] {
                for boundary in [0, DAY_BOUNDARY_MINUTES] {
                    assert_eq!(
                        journal_date(ms, offset, boundary),
                        old_journal_date(ms, offset, boundary),
                        "switch {switch} step {step} offset={offset} boundary={boundary}"
                    );
                }
            }
        }
    }
}

#[test]
fn known_dates_are_spelled_as_before() {
    // Literals, so that the oracle and the new code cannot both be wrong the same way.
    assert_eq!(journal_date(0, 0, 0), "1970-01-01");
    assert_eq!(journal_date(951_782_400_000, 0, 0), "2000-02-29");
    assert_eq!(journal_date(4_107_542_400_000, 0, 0), "2100-03-01");
    assert_eq!(journal_date(253_402_300_799_000, 0, 0), "9999-12-31");
    assert_eq!(journal_date(-30_610_224_000_000, 0, 0), "1000-01-01");
    assert_eq!(journal_date(-62_135_596_800_000, 0, 0), "0001-01-01");
    assert_eq!(journal_date(-62_167_219_200_000, 0, 0), "0000-01-01");
    assert_eq!(journal_date(-62_167_219_200_001, 0, 0), "-001-12-31");
    assert_eq!(journal_date(253_402_300_800_000, 0, 0), "10000-01-01");
}

#[test]
fn every_date_shape_is_judged_as_before() {
    for year in 0..=9999 {
        for month in 0..=13 {
            for day in [0, 1, 27, 28, 29, 30, 31, 32] {
                let text = format!("{year:04}-{month:02}-{day:02}");
                assert_eq!(
                    is_valid_journal_date(&text),
                    old_is_valid_journal_date(&text),
                    "{text}"
                );
            }
        }
    }
    // The leap rule itself, by name.
    for (text, valid) in [
        ("2024-02-29", true),
        ("2023-02-29", false),
        ("1900-02-29", false),
        ("2000-02-29", true),
        ("2100-02-29", false),
        ("2400-02-29", true),
        ("0999-12-31", false),
        ("1000-01-01", true),
        ("9999-12-31", true),
    ] {
        assert_eq!(is_valid_journal_date(text), valid, "{text}");
    }
}

#[test]
fn a_moment_beyond_the_calendar_is_clamped_and_never_panics() {
    // The old code printed a date for any i64; `chrono` stops at about +-262,000 years. Such a moment cannot reach the
    // function in the app (the clock and the zone come from `chrono`, an imported time is cut to 1970 ... now + 1 day);
    // if one ever does, it gets the first or the last day `chrono` has, by the sign of the moment.
    let far_past = "-262143-01-01";
    let far_future = "262142-12-31";
    let beyond = 270_000 * 366 * DAY_MS; // about 270,000 years from 1970
    for offset in [i32::MIN, -50_400, 0, 50_400, i32::MAX] {
        for boundary in [0, DAY_BOUNDARY_MINUTES, u32::MAX] {
            assert_eq!(journal_date(i64::MIN, offset, boundary), far_past);
            assert_eq!(journal_date(-beyond, offset, boundary), far_past);
            assert_eq!(journal_date(beyond, offset, boundary), far_future);
            assert_eq!(journal_date(i64::MAX, offset, boundary), far_future);
        }
    }
    // A boundary of u32::MAX minutes (8,000 years) is a legal shift for a moment in range.
    assert_eq!(
        journal_date(0, 0, u32::MAX),
        old_journal_date(0, 0, u32::MAX)
    );
    // The last days `chrono` can hold are still exact.
    assert_eq!(
        NaiveDate::MAX.and_time(NaiveTime::MIN).and_utc().year(),
        262_142
    );
    assert_eq!(
        journal_date(
            NaiveDate::MAX
                .and_time(NaiveTime::MIN)
                .and_utc()
                .timestamp_millis(),
            0,
            0
        ),
        far_future
    );
}

const DAY_MS: i64 = 86_400_000;

/// xorshift64: enough for test inputs, and the same on every machine.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }

    /// A number in `-limit ..= limit`.
    fn signed(&mut self, limit: i64) -> i64 {
        self.below(2 * limit as u64 + 1) as i64 - limit
    }
}
