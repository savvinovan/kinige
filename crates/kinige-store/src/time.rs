//! Time and time zone: the moment of an entry, the zone offset at that moment, the local time of an entry.

use chrono::{DateTime, Local, Offset, TimeZone, Utc};
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

/// The entry time `HH:MM` (24 hours) in the zone where it was created. A corrupted or unknown zone means UTC.
pub fn local_hhmm(created_at_ms: i64, tz: &str) -> String {
    let offset = offset_secs(tz, created_at_ms).unwrap_or(0);
    let local = created_at_ms.div_euclid(1000) + i64::from(offset);
    let minutes = local.rem_euclid(86_400) / 60;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
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
}
