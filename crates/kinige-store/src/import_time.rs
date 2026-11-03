//! The time of an imported entry (slice 4, decision 2): `created_at` from the export, the correction for messages of one
//! second, `journal_date` with the 04:00 boundary by the zone offset at that moment.

use crate::{Now, offset_secs};
use chrono::{NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::telegram::TgMessage;
use std::collections::HashMap;

/// The largest correction: up to 999 messages fit into one second.
const MAX_SUB_SECOND_MS: u32 = 999;

/// How far ahead of the import moment a message time is still considered genuine (a margin for clock differences).
const MAX_FUTURE_MS: i64 = 24 * 3_600_000;

/// The times of the messages of one Telegram chat in file order. If several messages came in one second, the k-th in
/// order gets +k ms (at most 999): otherwise the random entry `id` would shuffle them in the feed.
#[derive(Default)]
pub(crate) struct Timeline {
    seen_in_second: HashMap<i64, u32>,
    /// How many messages have already received the "import moment" (no date, or a date outside the window).
    undated: u32,
}

impl Timeline {
    /// `created_at`, ms. Messages are counted in file order, all of them (including the ones that are skipped): the
    /// result does not depend on what is already imported. If there is neither `date_unixtime` nor a parsable `date`, or
    /// the time is outside the window `1970-01-01 ... now + one day`, it is the import moment: an impossible date must
    /// neither break the canonical bytes (JCS numbers do not go beyond +-2^53) nor give an entry in the year 287396.
    pub(crate) fn created_at(&mut self, message: &TgMessage, now: &Now) -> i64 {
        let latest = now.ms.saturating_add(MAX_FUTURE_MS) / 1000;
        let Some(second) = message
            .unix
            .or_else(|| local_to_unix(message, &now.tz))
            .filter(|second| (0..=latest).contains(second))
        else {
            // All such messages get the "import moment": the file order is kept by the same +k ms, otherwise the random
            // entry `id` would arrange them in the feed in any order.
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

/// `date` (`YYYY-MM-DDTHH:MM:SS`) as local time of the device zone. On the switch to summer time the earlier of the two
/// possible moments is taken; a nonexistent local hour gives `None`.
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

/// `journal_date` of an entry created at `created_at_ms`, by the device zone offset at that moment.
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
        // 09:41:07 in Moscow (UTC+3) is 06:41:07 UTC.
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
        // The import moment, but the file order is kept: +k ms.
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
        // Beyond +-2^53, the far future (year 287396), more than a day ahead, negative, before 1970.
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
        // The window edges: 1970-01-01 and exactly one day ahead are accepted.
        for (unix, expected) in [("0", 0), ("1800086400", 1_800_086_400_000)] {
            let all = messages(&format!(r#"{{"id":1,"date_unixtime":"{unix}"}}"#));
            assert_eq!(Timeline::default().created_at(&all[0], &now), expected);
        }
        // The window also applies to `date` without `date_unixtime`.
        let all = messages(r#"{"id":1,"date":"9999-12-31T23:59:59"}"#);
        assert_eq!(Timeline::default().created_at(&all[0], &now), now.ms);
    }

    #[test]
    fn journal_date_uses_the_offset_at_that_moment_and_the_04_00_boundary() {
        // 2024-03-05 01:30 in Moscow is still 4 March.
        let now = Now::at(1_800_000_000_000, "Europe/Moscow");
        let at = 1_709_591_400_000; // 2024-03-04T22:30:00Z = 01:30 MSK
        assert_eq!(journal_date_of(at, &now), "2024-03-04");
        assert_eq!(journal_date_of(at + 3 * 3_600_000, &now), "2024-03-05");
        // The offset is taken at the moment of the entry, not now: Berlin in summer, 23:50 UTC on 4 July = 01:50 CEST on 5 July.
        let berlin = Now::at(1_800_000_000_000, "Europe/Berlin");
        assert_eq!(journal_date_of(1_720_137_000_000, &berlin), "2024-07-04");
    }
}
