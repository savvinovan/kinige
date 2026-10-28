//! Время импортированной записи (слайс 4, решение 2): `created_at` из экспорта, поправка для сообщений одной секунды,
//! `journal_date` с границей 04:00 по смещению пояса на этот момент.

use crate::{Now, offset_secs};
use chrono::{NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::telegram::TgMessage;
use std::collections::HashMap;

/// Наибольшая поправка: в одну секунду помещается до 999 сообщений.
const MAX_SUB_SECOND_MS: u32 = 999;

/// Насколько вперёд от момента импорта время сообщения ещё считается настоящим (запас на разницу часов).
const MAX_FUTURE_MS: i64 = 24 * 3_600_000;

/// Время сообщений одного чата Telegram по порядку файла. Если в одну секунду пришло несколько сообщений, k-е по порядку
/// получает +k мс (не больше 999): иначе случайный `id` записи перемешал бы их в ленте.
#[derive(Default)]
pub(crate) struct Timeline {
    seen_in_second: HashMap<i64, u32>,
    /// Сколько сообщений уже получили «момент импорта» (без даты или с датой вне окна).
    undated: u32,
}

impl Timeline {
    /// `created_at`, мс. Сообщения учитываются в порядке файла, все (в том числе пропускаемые): результат не зависит от
    /// того, что уже импортировано. Нет ни `date_unixtime`, ни разбираемой `date`, либо время вне окна
    /// `1970-01-01 … сейчас + сутки` — момент импорта: невозможная дата не должна ни ломать каноничные байты (числа JCS
    /// не выходят за ±2^53), ни давать запись в году 287396.
    pub(crate) fn created_at(&mut self, message: &TgMessage, now: &Now) -> i64 {
        let latest = now.ms.saturating_add(MAX_FUTURE_MS) / 1000;
        let Some(second) = message
            .unix
            .or_else(|| local_to_unix(message, &now.tz))
            .filter(|second| (0..=latest).contains(second))
        else {
            // Все такие сообщения получают «момент импорта»: порядок файла сохраняется тем же +k мс, иначе случайный `id`
            // записи расставил бы их в ленте как попало.
            let k = self.undated.min(MAX_SUB_SECOND_MS);
            self.undated += 1;
            return now.ms.saturating_add(i64::from(k));
        };
        let seen = self.seen_in_second.entry(second).or_insert(0);
        let k = (*seen).min(MAX_SUB_SECOND_MS);
        *seen += 1;
        second.saturating_mul(1000).saturating_add(i64::from(k))
    }
}

/// `date` (`ГГГГ-ММ-ДДTЧЧ:ММ:СС`) как местное время пояса устройства. При переходе на летнее время берётся более раннее
/// из двух возможных моментов; несуществующий местный час — `None`.
fn local_to_unix(message: &TgMessage, tz: &str) -> Option<i64> {
    let local =
        NaiveDateTime::parse_from_str(message.local.as_deref()?, "%Y-%m-%dT%H:%M:%S").ok()?;
    match tz.parse::<Tz>() {
        Ok(zone) => zone
            .from_local_datetime(&local)
            .earliest()
            .map(|t| t.timestamp()),
        Err(_) => Some(local.and_utc().timestamp()),
    }
}

/// `journal_date` записи, созданной в момент `created_at_ms`, по смещению пояса устройства на этот момент.
pub(crate) fn journal_date_of(created_at_ms: i64, now: &Now) -> String {
    let offset = offset_secs(&now.tz, created_at_ms).unwrap_or(now.utc_offset_secs);
    journal_date(created_at_ms, offset, DAY_BOUNDARY_MINUTES)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinige_domain::telegram::parse_export;

    fn messages(json: &str) -> Vec<TgMessage> {
        let text = format!(r#"{{"type":"saved_messages","id":1,"messages":[{json}]}}"#);
        parse_export(&text).unwrap().chats.remove(0).messages
    }

    #[test]
    fn same_second_messages_get_increasing_milliseconds_capped_at_999() {
        let many: String = (0..1005)
            .map(|i| format!(r#"{{"id":{i},"date_unixtime":"1700000000"}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let all = messages(&many);
        let now = Now::at(1_800_000_000_000, "Europe/Moscow");
        let mut timeline = Timeline::default();
        let times: Vec<i64> = all.iter().map(|m| timeline.created_at(m, &now)).collect();
        assert_eq!(times[0], 1_700_000_000_000);
        assert_eq!(times[1], 1_700_000_000_001);
        assert_eq!(times[998], 1_700_000_000_998);
        assert!(times[999..].iter().all(|t| *t == 1_700_000_000_999));
    }

    #[test]
    fn date_without_unixtime_is_local_time_of_the_device_zone() {
        let all = messages(r#"{"id":1,"date":"2024-03-05T09:41:07"}"#);
        let mut timeline = Timeline::default();
        let moscow = Now::at(1_800_000_000_000, "Europe/Moscow");
        // 09:41:07 по Москве (UTC+3) — 06:41:07 UTC.
        assert_eq!(timeline.created_at(&all[0], &moscow), 1_709_620_867_000);
        let utc = Now::at(1_800_000_000_000, "UTC");
        assert_eq!(
            Timeline::default().created_at(&all[0], &utc),
            1_709_631_667_000
        );
    }

    #[test]
    fn no_date_at_all_falls_back_to_the_import_moment() {
        let all = messages(r#"{"id":1},{"id":2,"date":"не дата"}"#);
        let now = Now::at(1_800_000_000_123, "UTC");
        let mut timeline = Timeline::default();
        // Момент импорта, но порядок файла сохранён: +k мс.
        assert_eq!(timeline.created_at(&all[0], &now), 1_800_000_000_123);
        assert_eq!(timeline.created_at(&all[1], &now), 1_800_000_000_124);
    }

    #[test]
    fn dates_outside_the_window_keep_the_file_order_among_themselves() {
        let all = messages(
            r#"{"id":1,"date_unixtime":"9999999999999999"},{"id":2},{"id":3,"date_unixtime":"-5"}"#,
        );
        let now = Now::at(1_800_000_000_000, "UTC");
        let mut timeline = Timeline::default();
        let times: Vec<i64> = all.iter().map(|m| timeline.created_at(m, &now)).collect();
        assert_eq!(times, [now.ms, now.ms + 1, now.ms + 2]);
    }

    #[test]
    fn times_outside_the_window_fall_back_to_the_import_moment() {
        let now = Now::at(1_800_000_000_000, "UTC");
        // За ±2^53, далёкое будущее (287396-й год), больше суток вперёд, отрицательное, до 1970.
        let wild = [
            "9999999999999999",
            "9007199254740",
            "-9999999999999999",
            "1800172801",
            "-1",
        ];
        for unix in wild {
            let all = messages(&format!(r#"{{"id":1,"date_unixtime":"{unix}"}}"#));
            let at = Timeline::default().created_at(&all[0], &now);
            assert_eq!(at, now.ms, "{unix}");
            assert!(kinige_domain::journal::is_valid_journal_date(
                &journal_date_of(at, &now)
            ));
        }
        // Границы окна: 1970-01-01 и ровно сутки вперёд принимаются.
        for (unix, expected) in [("0", 0), ("1800086400", 1_800_086_400_000)] {
            let all = messages(&format!(r#"{{"id":1,"date_unixtime":"{unix}"}}"#));
            assert_eq!(Timeline::default().created_at(&all[0], &now), expected);
        }
        // Окно действует и на `date` без `date_unixtime`.
        let all = messages(r#"{"id":1,"date":"9999-12-31T23:59:59"}"#);
        assert_eq!(Timeline::default().created_at(&all[0], &now), now.ms);
    }

    #[test]
    fn journal_date_uses_the_offset_at_that_moment_and_the_04_00_boundary() {
        // 2024-03-05 01:30 по Москве — ещё 4 марта.
        let now = Now::at(1_800_000_000_000, "Europe/Moscow");
        let at = 1_709_591_400_000; // 2024-03-04T22:30:00Z = 01:30 MSK
        assert_eq!(journal_date_of(at, &now), "2024-03-04");
        assert_eq!(journal_date_of(at + 3 * 3_600_000, &now), "2024-03-05");
        // Смещение берётся на момент записи, а не на сейчас: Берлин летом, 23:50 UTC 4 июля = 01:50 CEST 5 июля.
        let berlin = Now::at(1_800_000_000_000, "Europe/Berlin");
        assert_eq!(journal_date_of(1_720_137_000_000, &berlin), "2024-07-04");
    }
}
