//! Время и часовой пояс: момент записи, смещение пояса на этот момент, местное время записи.

use chrono::{DateTime, Local, Offset, TimeZone, Utc};
use chrono_tz::Tz;

/// «Сейчас» для одной операции: UTC в мс, IANA-пояс устройства и его смещение на этот момент (с учётом летнего времени).
/// Операции получают его снаружи, поэтому тесты задают время и пояс сами.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Now {
    pub ms: i64,
    pub tz: String,
    pub utc_offset_secs: i32,
}

impl Now {
    /// Системные часы и пояс устройства.
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

    /// Заданный момент в заданном поясе (для тестов). Неизвестный пояс — UTC.
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

/// Смещение пояса `tz` на момент `ms` в секундах; `None`, если пояс неизвестен.
pub fn offset_secs(tz: &str, ms: i64) -> Option<i32> {
    let zone: Tz = tz.parse().ok()?;
    let utc: DateTime<Utc> = DateTime::from_timestamp_millis(ms)?;
    Some(
        zone.offset_from_utc_datetime(&utc.naive_utc())
            .fix()
            .local_minus_utc(),
    )
}

/// Смещение пояса операционной системы на момент `ms`.
fn local_offset_secs(ms: i64) -> i32 {
    match DateTime::from_timestamp_millis(ms) {
        Some(utc) => Local
            .offset_from_utc_datetime(&utc.naive_utc())
            .fix()
            .local_minus_utc(),
        None => 0,
    }
}

/// Время записи `HH:MM` (24 часа) в поясе, где она создана. Повреждённый или неизвестный пояс — UTC.
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
        // Берлин: 2026-03-29 01:00 UTC — переход на летнее время.
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
        assert_eq!(local_hhmm(-60_000, "UTC"), "23:59", "до эпохи");
    }
}
