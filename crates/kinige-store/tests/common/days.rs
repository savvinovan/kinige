//! Моменты прошлых лет для тестов «В этот день» и ответов: дата и время по Москве (UTC+3 круглый год с 2014).

#![allow(dead_code)]

use super::moscow;
use chrono::NaiveDate;
use kinige_store::Now;

/// Миллисекунды UTC для местного времени Москвы.
pub fn ms(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap()
        .and_hms_opt(hour, minute, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
        - 3 * 3_600_000
}

/// «Сейчас» по Москве.
pub fn at(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> Now {
    moscow(ms(year, month, day, hour, minute))
}
