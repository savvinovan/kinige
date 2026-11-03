//! Moments of past years for the "On this day" and replies tests: date and time in Moscow (UTC+3 all year round since 2014).

#![allow(dead_code)]

use super::moscow;
use chrono::NaiveDate;
use kinige_store::Now;

/// UTC milliseconds for Moscow local time.
pub fn ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_opt(hour, minute, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
        - 3 * 3_600_000
}

/// "Now" in Moscow.
pub fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Now {
    moscow(ms(year, month, day, hour, minute))
}
