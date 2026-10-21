//! Гибридные логические часы (ADR-0001 §2). Формат метки: `{wall_ms:013}-{counter:05}-{device_id}`.
//!
//! В слайсе 1 синхронизации нет, поэтому реализованы только локальное событие и продолжение счётчика после перезапуска.
//! Приём чужой метки (алгоритм Kulkarni et al., 2014, рис. 5) появится вместе с синхронизацией.

use std::fmt;

/// Наибольший счётчик: пять десятичных цифр.
pub const MAX_COUNTER: u32 = 99_999;
/// Наибольшее значение `wall_ms`: тринадцать десятичных цифр.
pub const MAX_WALL_MS: u64 = 9_999_999_999_999;
/// Длина метки: 13 + 1 + 5 + 1 + 36.
pub const HLC_LEN: usize = 56;

/// Ошибки разбора и тика часов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HlcError {
    /// Строка не соответствует формату `{wall_ms:013}-{counter:05}-{device_id}`.
    Format,
    /// `wall_ms` не помещается в 13 цифр.
    WallOverflow,
}

impl fmt::Display for HlcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HlcError::Format => f.write_str("неверный формат HLC"),
            HlcError::WallOverflow => f.write_str("wall_ms не помещается в 13 цифр"),
        }
    }
}

impl std::error::Error for HlcError {}

/// Состояние часов устройства: пара `(l, c)` из `sync_state` (`hlc_l`, `hlc_c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clock {
    /// Логическая «стенка», мс.
    pub l: u64,
    /// Счётчик внутри одной «стенки».
    pub c: u32,
}

impl Clock {
    /// Часы после запуска (ADR-0001 §2, «Хранение и перезапуск»): `l = max(stored_l, pt)`, счётчик продолжается,
    /// если `l` не сдвинулось, иначе начинается с нуля. Без продолжения счётчика метка после перезапуска могла бы повториться.
    pub fn resume(stored_l: u64, stored_c: u32, pt: u64) -> Clock {
        let l = stored_l.max(pt);
        let c = if l == stored_l { stored_c } else { 0 };
        Clock { l, c }
    }

    /// Локальное событие: `l' = l; l = max(l', pt); c = (l == l') ? c + 1 : 0`.
    /// Переполнение счётчика (больше 99999) переносит `l` на миллисекунду вперёд и обнуляет счётчик.
    pub fn tick(self, pt: u64) -> Result<Clock, HlcError> {
        let l = self.l.max(pt);
        let c = if l == self.l { self.c + 1 } else { 0 };
        let next = if c > MAX_COUNTER {
            Clock { l: l + 1, c: 0 }
        } else {
            Clock { l, c }
        };
        if next.l > MAX_WALL_MS {
            return Err(HlcError::WallOverflow);
        }
        Ok(next)
    }

    /// Метка в сортируемом виде.
    pub fn format(self, device_id: &str) -> String {
        format!("{:013}-{:05}-{device_id}", self.l, self.c)
    }
}

/// Строгий разбор метки: 56 символов, цифры, `device_id` — UUID в нижнем регистре.
pub fn parse(hlc: &str) -> Result<(Clock, &str), HlcError> {
    let bytes = hlc.as_bytes();
    if bytes.len() != HLC_LEN || bytes[13] != b'-' || bytes[19] != b'-' {
        return Err(HlcError::Format);
    }
    let digits = |range: std::ops::Range<usize>| -> Result<u64, HlcError> {
        let part = &hlc[range];
        if !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(HlcError::Format);
        }
        part.parse().map_err(|_| HlcError::Format)
    };
    let l = digits(0..13)?;
    let c = u32::try_from(digits(14..19)?).map_err(|_| HlcError::Format)?;
    let device = &hlc[20..];
    if !is_uuid(device) {
        return Err(HlcError::Format);
    }
    Ok((Clock { l, c }, device))
}

/// UUID в нижнем регистре, 36 символов (`8-4-4-4-12`).
pub fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";

    #[test]
    fn local_event_advances_wall_clock_and_resets_counter() {
        let clock = Clock { l: 1_000, c: 7 };
        assert_eq!(clock.tick(2_000).unwrap(), Clock { l: 2_000, c: 0 });
    }

    #[test]
    fn local_event_within_same_millisecond_increments_counter() {
        let clock = Clock { l: 2_000, c: 0 };
        assert_eq!(clock.tick(2_000).unwrap(), Clock { l: 2_000, c: 1 });
        assert_eq!(
            clock.tick(1_500).unwrap(),
            Clock { l: 2_000, c: 1 },
            "часы пошли назад: l не уменьшается"
        );
    }

    #[test]
    fn counter_overflow_moves_wall_clock_forward() {
        let clock = Clock {
            l: 5_000,
            c: MAX_COUNTER,
        };
        let next = clock.tick(5_000).unwrap();
        assert_eq!(next, Clock { l: 5_001, c: 0 });
        assert!(
            next.format(DEVICE) > clock.format(DEVICE),
            "метка остаётся строго возрастающей"
        );
    }

    #[test]
    fn ticks_are_strictly_increasing_as_strings() {
        let mut clock = Clock::default();
        let mut previous = clock.format(DEVICE);
        for pt in [10, 10, 10, 9, 11, 11, 5] {
            clock = clock.tick(pt).unwrap();
            let current = clock.format(DEVICE);
            assert!(
                current > previous,
                "{current} должна быть больше {previous}"
            );
            previous = current;
        }
    }

    #[test]
    fn restart_continues_counter_when_clock_has_not_moved() {
        // Приняли «будущую» метку (l = 9000), перезапустились, когда pt = 3000: счётчик продолжается, а не повторяется.
        let resumed = Clock::resume(9_000, 4, 3_000);
        assert_eq!(resumed, Clock { l: 9_000, c: 4 });
        assert_eq!(resumed.tick(3_000).unwrap(), Clock { l: 9_000, c: 5 });
    }

    #[test]
    fn restart_after_time_passed_resets_counter() {
        assert_eq!(Clock::resume(9_000, 4, 12_000), Clock { l: 12_000, c: 0 });
        assert_eq!(Clock::resume(0, 0, 12_000), Clock { l: 12_000, c: 0 });
    }

    #[test]
    fn restart_never_repeats_a_label() {
        let before = Clock { l: 9_000, c: 4 };
        let after = Clock::resume(before.l, before.c, 3_000)
            .tick(3_000)
            .unwrap();
        assert!(after.format(DEVICE) > before.format(DEVICE));
    }

    #[test]
    fn wall_overflow_is_an_error() {
        let clock = Clock {
            l: MAX_WALL_MS,
            c: MAX_COUNTER,
        };
        assert_eq!(clock.tick(0), Err(HlcError::WallOverflow));
        assert_eq!(
            Clock::default().tick(MAX_WALL_MS + 1),
            Err(HlcError::WallOverflow)
        );
    }

    #[test]
    fn format_is_fixed_width_and_parses_back() {
        let clock = Clock {
            l: 1_760_000_000_000,
            c: 42,
        };
        let text = clock.format(DEVICE);
        assert_eq!(
            text,
            "1760000000000-00042-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f"
        );
        assert_eq!(text.len(), HLC_LEN);
        assert_eq!(parse(&text).unwrap(), (clock, DEVICE));
        // Малые значения дополняются нулями: сравнение строк совпадает со сравнением кортежей.
        assert_eq!(
            Clock { l: 7, c: 3 }.format(DEVICE),
            "0000000000007-00003-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f"
        );
    }

    #[test]
    fn string_order_matches_tuple_order() {
        let samples = [
            Clock { l: 9, c: 99_999 },
            Clock { l: 10, c: 0 },
            Clock { l: 10, c: 1 },
            Clock {
                l: 1_000_000_000_000,
                c: 0,
            },
        ];
        for a in samples {
            for b in samples {
                assert_eq!(
                    a.format(DEVICE).cmp(&b.format(DEVICE)),
                    (a.l, a.c).cmp(&(b.l, b.c))
                );
            }
        }
    }

    #[test]
    fn parser_is_strict() {
        let ok = Clock { l: 1, c: 1 }.format(DEVICE);
        assert!(parse(&ok).is_ok());
        for bad in [
            "",
            &ok[..55],
            &format!("{ok}0"),
            &ok.replace("0000000000001-", "00000000000001-"),
            &ok.replacen('-', "_", 1),
            &ok.to_uppercase(),
            &ok.replace("0000000000001", "000000000000a"),
            &format!("0000000000001-00001-{}", "x".repeat(36)),
        ] {
            assert_eq!(parse(bad), Err(HlcError::Format), "{bad}");
        }
    }
}
