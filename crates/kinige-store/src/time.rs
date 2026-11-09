//! Time and time zone: the moment of an entry, the zone offset at that moment, the local time of an entry.

use chrono::{DateTime, Local, NaiveTime, Offset, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;

/// "Now" for one operation: UTC in ms, the device's IANA zone and its offset at that moment (including daylight saving
/// time). Operations receive it from outside, so tests set the time and the zone themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Now {
    pub ms: i64,
    pub tz: String,
    pub utc_offset_secs: i32,
}

impl Now {
    /// The system clock and the device's zone.
    pub fn system() -> Now {
        let ms = Utc::now().timestamp_millis();
        let tz = iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string());
        let utc_offset_secs = offset_secs(&tz, ms).unwrap_or_else(|| local_offset_secs(ms));
        Now {
            ms,
            tz,
            utc_offset_secs,
        }
    }

    /// A given moment in a given zone (for tests). An unknown zone means UTC.
    pub fn at(ms: i64, tz: &str) -> Now {
        match offset_secs(tz, ms) {
            Some(utc_offset_secs) => Now {
                ms,
                tz: tz.to_string(),
                utc_offset_secs,
            },
            None => Now {
                ms,
                tz: "UTC".to_string(),
                utc_offset_secs: 0,
            },
        }
    }
}

/// Offset of the zone `tz` at the moment `ms`, in seconds; `None` if the zone is unknown.
pub fn offset_secs(tz: &str, ms: i64) -> Option<i32> {
    let zone: Tz = tz.parse().ok()?;
    let utc: DateTime<Utc> = DateTime::from_timestamp_millis(ms)?;
    Some(
        zone.offset_from_utc_datetime(&utc.naive_utc())
            .fix()
            .local_minus_utc(),
    )
}

/// A UTC moment for a file name: `YYYYMMDDTHHMMSSZ` (sorts as a string).
pub fn utc_stamp(ms: i64) -> String {
    DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .format("%Y%m%dT%H%M%SZ")
        .to_string()
}

/// Offset of the operating system's zone at the moment `ms`.
fn local_offset_secs(ms: i64) -> i32 {
    match DateTime::from_timestamp_millis(ms) {
        Some(utc) => Local
            .offset_from_utc_datetime(&utc.naive_utc())
            .fix()
            .local_minus_utc(),
        None => 0,
    }
}

/// The time of day `HHMMSS` at the moment `ms` in a zone with the offset `utc_offset_secs`: the name of a pasted image.
pub(crate) fn hhmmss(ms: i64, utc_offset_secs: i32) -> String {
    wall_clock(ms, utc_offset_secs).format("%H%M%S").to_string()
}

/// The entry time `HH:MM` (24 hours) in the zone where it was created. A corrupted or unknown zone means UTC.
pub fn local_hhmm(created_at_ms: i64, tz: &str) -> String {
    let offset = offset_secs(tz, created_at_ms).unwrap_or(0);
    wall_clock(created_at_ms, offset)
        .format("%H:%M")
        .to_string()
}

/// The time of day at the moment `ms` in a zone with the offset `utc_offset_secs`. A moment beyond the years `chrono` can
/// hold (about +-262,000) reads as midnight, like in `utc_stamp`: none reaches here in the app (the clock comes from
/// `chrono`, stored times come from the clock or from an import cut to `1970 ... now + 1 day`).
fn wall_clock(ms: i64, utc_offset_secs: i32) -> NaiveTime {
    DateTime::from_timestamp_millis(ms)
        .and_then(|utc| {
            let offset = TimeDelta::try_seconds(i64::from(utc_offset_secs))?;
            utc.naive_utc().checked_add_signed(offset)
        })
        .map_or(NaiveTime::MIN, |local| local.time())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_follow_daylight_saving_time() {
        // Berlin: 2026-03-29 01:00 UTC is the switch to daylight saving time.
        let before = 1_774_746_000_000 - 1; // 2026-03-29T00:59:59.999Z
        let after = 1_774_746_000_000; // 2026-03-29T01:00:00Z
        assert_eq!(offset_secs("Europe/Berlin", before), Some(3600));
        assert_eq!(offset_secs("Europe/Berlin", after), Some(7200));
        assert_eq!(offset_secs("Europe/Moscow", after), Some(10_800));
        assert_eq!(offset_secs("Not/AZone", after), None);
    }

    #[test]
    fn now_at_unknown_zone_is_utc() {
        let now = Now::at(0, "Mars/Olympus");
        assert_eq!((now.tz.as_str(), now.utc_offset_secs), ("UTC", 0));
    }

    #[test]
    fn local_time_uses_the_zone_of_the_record() {
        let ms = 1_774_746_000_000; // 2026-03-29T01:00:00Z
        assert_eq!(local_hhmm(ms, "UTC"), "01:00");
        assert_eq!(local_hhmm(ms, "Europe/Berlin"), "03:00");
        assert_eq!(local_hhmm(ms, "Europe/Moscow"), "04:00");
        assert_eq!(local_hhmm(ms, "America/New_York"), "21:00");
        assert_eq!(local_hhmm(ms, "garbage"), "01:00");
        assert_eq!(local_hhmm(-60_000, "UTC"), "23:59", "before the epoch");
    }

    // The oracle: the time-of-day code as it was before KINIG-103, copied unchanged.

    fn old_hhmmss(ms: i64, utc_offset_secs: i32) -> String {
        let local = ms.div_euclid(1000) + i64::from(utc_offset_secs);
        let seconds = local.rem_euclid(86_400);
        format!(
            "{:02}{:02}{:02}",
            seconds / 3600,
            seconds % 3600 / 60,
            seconds % 60
        )
    }

    fn old_local_hhmm(created_at_ms: i64, tz: &str) -> String {
        let offset = offset_secs(tz, created_at_ms).unwrap_or(0);
        let local = created_at_ms.div_euclid(1000) + i64::from(offset);
        let minutes = local.rem_euclid(86_400) / 60;
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    }

    /// Milliseconds up to about 253,000 years from the epoch, in both directions: inside what `chrono` holds.
    const IN_RANGE_MS: i64 = 8_000_000_000_000_000;

    const ZONES: [&str; 7] = [
        "UTC",
        "Europe/Berlin",
        "Europe/Moscow",
        "America/New_York",
        "Asia/Kolkata",
        "Pacific/Auckland",
        "garbage",
    ];

    #[test]
    fn time_of_day_matches_the_old_arithmetic() {
        let mut state: u64 = 0x4B49_4E49_4745;
        let mut below = |n: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % n
        };
        for _ in 0..200_000 {
            let ms = match below(3) {
                0 => below(2 * IN_RANGE_MS as u64 + 1) as i64 - IN_RANGE_MS,
                1 => below(4_000_000_000_000) as i64 - 2_000_000_000_000, // 1970 +- 63 years
                _ => (below(2_000_000) as i64 - 1_000_000) * 1000 + below(3) as i64 - 1, // around whole seconds
            };
            let offset = below(100_801) as i32 - 50_400;
            assert_eq!(hhmmss(ms, offset), old_hhmmss(ms, offset), "{ms} {offset}");
            let tz = ZONES[below(ZONES.len() as u64) as usize];
            assert_eq!(local_hhmm(ms, tz), old_local_hhmm(ms, tz), "{ms} {tz}");
        }
    }

    #[test]
    fn time_of_day_edges_match_the_old_arithmetic() {
        // Midnight and the minute and second before and after it, a day before the epoch, the daylight saving switch.
        let moments = [
            0,
            -1,
            1,
            -999,
            999,
            -1000,
            1000,
            -86_400_000,
            86_399_999,
            86_400_000,
            -86_400_001,
            1_774_746_000_000 - 1, // Berlin, the switch to summer time
            1_774_746_000_000,
            1_792_890_000_000 - 1, // and back
            1_792_890_000_000,
            -30_610_224_000_000, // 1000-01-01
            253_402_300_799_999, // 9999-12-31T23:59:59.999Z
            IN_RANGE_MS,
            -IN_RANGE_MS,
        ];
        for ms in moments {
            for offset in [-50_400, -3_600, -1, 0, 1, 3_599, 3_600, 19_800, 50_400] {
                assert_eq!(hhmmss(ms, offset), old_hhmmss(ms, offset), "{ms} {offset}");
            }
            for tz in ZONES {
                assert_eq!(local_hhmm(ms, tz), old_local_hhmm(ms, tz), "{ms} {tz}");
            }
        }
    }

    #[test]
    fn a_moment_beyond_the_calendar_reads_as_midnight_and_never_panics() {
        for ms in [
            i64::MIN,
            i64::MAX,
            270_000 * 366 * 86_400_000,
            -270_000 * 366 * 86_400_000,
        ] {
            for offset in [i32::MIN, 0, i32::MAX] {
                assert_eq!(hhmmss(ms, offset), "000000");
            }
            assert_eq!(local_hhmm(ms, "Europe/Berlin"), "00:00");
        }
    }
}
