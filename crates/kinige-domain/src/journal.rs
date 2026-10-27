//! `journal_date` — день записи с границей 04:00 (ТЗ §4.1). Чистая арифметика: смещение часового пояса на момент записи
//! приходит снаружи, поэтому переход на летнее время учитывается тем, кто это смещение знает.

/// День заканчивается в 04:00: запись в 00:30 относится ко вчера. Настройка границы — не в слайсе 1.
pub const DAY_BOUNDARY_MINUTES: u32 = 4 * 60;

/// Дата записи `YYYY-MM-DD`: локальное время (`UTC + offset`) минус граница дня.
pub fn journal_date(created_at_ms: i64, utc_offset_secs: i32, boundary_minutes: u32) -> String {
    let local_secs = created_at_ms.div_euclid(1000) + i64::from(utc_offset_secs);
    let shifted = local_secs - i64::from(boundary_minutes) * 60;
    let (year, month, day) = civil_from_days(shifted.div_euclid(86_400));
    format!("{year:04}-{month:02}-{day:02}")
}

/// Дни от 1970-01-01 → (год, месяц, день) по григорианскому календарю (алгоритм Говарда Хиннанта, `civil_from_days`).
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

/// `YYYY-MM-DD` с настоящими цифрами и месяцем 01–12, днём 01–31 (проверка формата; существование даты не проверяется).
pub fn is_journal_date(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let digits = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    if !(digits(0..4) && digits(5..7) && digits(8..10)) {
        return false;
    }
    let month: u32 = text[5..7].parse().unwrap_or(0);
    let day: u32 = text[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Настоящая дата `YYYY-MM-DD`: формат, месяц 01–12, день в пределах месяца (с високосными годами). Будущие даты
/// допустимы (ADR-0001 §2): `journal_date` — содержимое записи, а не часы. Год — не раньше 1000 и не позже 9999, чтобы
/// запись не уезжала в «год 0000», где календарь не определён.
pub fn is_valid_journal_date(text: &str) -> bool {
    if !is_journal_date(text) {
        return false;
    }
    let year: i32 = text[..4].parse().unwrap_or(0);
    let month: u32 = text[5..7].parse().unwrap_or(0);
    let day: u32 = text[8..10].parse().unwrap_or(0);
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

    /// Момент UTC в мс по компонентам (тест не зависит от часового пояса машины).
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
        // 2026-10-21 по Москве (UTC+3).
        assert_eq!(
            journal_date(
                utc(2026, 10, 21, 0, 30) - i64::from(MOSCOW) * 1000,
                MOSCOW,
                DAY_BOUNDARY_MINUTES
            ),
            "2026-10-20",
            "00:30 — ещё вчера"
        );
        assert_eq!(
            journal_date(
                utc(2026, 10, 21, 3, 59) - i64::from(MOSCOW) * 1000,
                MOSCOW,
                DAY_BOUNDARY_MINUTES
            ),
            "2026-10-20",
            "03:59 — ещё вчера"
        );
        assert_eq!(
            journal_date(
                utc(2026, 10, 21, 4, 0) - i64::from(MOSCOW) * 1000,
                MOSCOW,
                DAY_BOUNDARY_MINUTES
            ),
            "2026-10-21",
            "04:00 — уже сегодня"
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
        // Берлин, 2026-03-29: в 01:00 UTC часы переходят с +01:00 на +02:00 (02:00 → 03:00 по местному).
        let before = 3600; // CET
        let after = 7200; // CEST
        // 00:59 UTC = 01:59 CET: до границы 04:00 — предыдущий день.
        assert_eq!(
            journal_date(utc(2026, 3, 29, 0, 59), before, DAY_BOUNDARY_MINUTES),
            "2026-03-28"
        );
        // 01:59 UTC = 03:59 CEST: переход уже случился, но до 04:00 — всё ещё предыдущий день.
        assert_eq!(
            journal_date(utc(2026, 3, 29, 1, 59), after, DAY_BOUNDARY_MINUTES),
            "2026-03-28"
        );
        // 02:00 UTC = 04:00 CEST: граница дня; без учёта нового смещения (+01:00) это была бы 03:00 и вчерашняя дата.
        assert_eq!(
            journal_date(utc(2026, 3, 29, 2, 0), after, DAY_BOUNDARY_MINUTES),
            "2026-03-29"
        );
        assert_eq!(
            journal_date(utc(2026, 3, 29, 2, 0), before, DAY_BOUNDARY_MINUTES),
            "2026-03-28",
            "старое смещение дало бы другой день"
        );
    }

    #[test]
    fn boundary_across_fall_back() {
        // Берлин, 2026-10-25: в 01:00 UTC часы переходят с +02:00 на +01:00 (03:00 → 02:00).
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
        // Нью-Йорк (UTC−5), 2024-02-29 03:00 по местному — ещё 28-е (високосный год).
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
            "до 04:00 первого марта — 29 февраля"
        );
        assert_eq!(
            journal_date(utc(2026, 1, 1, 3, 59), 0, DAY_BOUNDARY_MINUTES),
            "2025-12-31",
            "граница года"
        );
        assert_eq!(
            journal_date(utc(1969, 12, 31, 12, 0), 0, 0),
            "1969-12-31",
            "до эпохи"
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
