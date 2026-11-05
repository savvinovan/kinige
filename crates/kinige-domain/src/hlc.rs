//! Hybrid logical clock (ADR-0001 §2). Stamp format: `{wall_ms:013}-{counter:05}-{device_id}`.
//!
//! Slice 1 has no sync, so only the local event and continuing the counter after a restart are implemented.
//! Receiving a foreign stamp (the algorithm of Kulkarni et al., 2014, fig. 5) will appear together with sync.

use std::fmt;

/// The largest counter: five decimal digits.
pub const MAX_COUNTER: u32 = 99_999;
/// The largest `wall_ms` value: thirteen decimal digits.
pub const MAX_WALL_MS: u64 = 9_999_999_999_999;
/// The stamp length: 13 + 1 + 5 + 1 + 36.
pub const HLC_LEN: usize = 56;

/// Parse and clock tick errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HlcError {
    /// The string does not match the format `{wall_ms:013}-{counter:05}-{device_id}`.
    Format,
    /// `wall_ms` does not fit in 13 digits.
    WallOverflow,
}

impl fmt::Display for HlcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HlcError::Format => f.write_str("invalid HLC format"),
            HlcError::WallOverflow => f.write_str("wall_ms does not fit in 13 digits"),
        }
    }
}

impl std::error::Error for HlcError {}

/// The state of the device clock: the pair `(l, c)` from `sync_state` (`hlc_l`, `hlc_c`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clock {
    /// The logical "wall", ms.
    pub l: u64,
    /// The counter within one "wall".
    pub c: u32,
}

impl Clock {
    /// The clock after start (ADR-0001 §2, "Storage and restart"): `l = max(stored_l, pt)`, the counter continues
    /// if `l` did not move, otherwise it starts from zero. Without continuing the counter a stamp could repeat after a restart.
    pub fn resume(stored_l: u64, stored_c: u32, pt: u64) -> Clock {
        let l = stored_l.max(pt);
        let c = if l == stored_l { stored_c } else { 0 };
        Clock { l, c }
    }

    /// A local event: `l' = l; l = max(l', pt); c = (l == l') ? c + 1 : 0`.
    /// A counter overflow (above 99999) moves `l` one millisecond forward and resets the counter.
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

    /// Receiving a foreign stamp `remote` at physical time `pt` (ADR-0001 §2, Kulkarni et al. 2014, fig. 5):
    /// `l = max(l', l_m, pt)`, then the counter by which of the three equal `l`. After this the next local stamp is greater
    /// than `remote`, which is what makes "a restore that saw a delete beats it" true even when the device clock is behind.
    /// A counter overflow moves `l` one millisecond forward, as in [`Clock::tick`].
    pub fn receive(self, remote: Clock, pt: u64) -> Result<Clock, HlcError> {
        let l = self.l.max(remote.l).max(pt);
        // The counters of a received stamp are at most 99999 after `parse`, but this takes any `Clock`: no addition may overflow.
        let c = if l == self.l && l == remote.l {
            self.c.max(remote.c).saturating_add(1)
        } else if l == self.l {
            self.c.saturating_add(1)
        } else if l == remote.l {
            remote.c.saturating_add(1)
        } else {
            0
        };
        let next = if c > MAX_COUNTER {
            Clock {
                l: l.checked_add(1).ok_or(HlcError::WallOverflow)?,
                c: 0,
            }
        } else {
            Clock { l, c }
        };
        if next.l > MAX_WALL_MS {
            return Err(HlcError::WallOverflow);
        }
        Ok(next)
    }

    /// [`Clock::receive`] with the drift rules of ADR-0001 §2: a stamp more than [`SKEW_WARN_MS`] ahead of `pt` is accepted with a
    /// warning (work is not blocked, otherwise a restore would lose to a "future" deletion); a stamp more than [`SKEW_MAX_MS`]
    /// ahead is applied to the data by the caller but **not accepted into the clock**.
    pub fn receive_checked(self, remote: Clock, pt: u64) -> Result<(Clock, Skew), HlcError> {
        let skew = Skew::of(remote.l, pt);
        if skew == Skew::TooFar {
            return Ok((self, skew));
        }
        Ok((self.receive(remote, pt)?, skew))
    }

    /// The stamp in a sortable form.
    pub fn format(self, device_id: &str) -> String {
        format!("{:013}-{:05}-{device_id}", self.l, self.c)
    }
}

/// A foreign stamp ahead of the local physical time by more than this is a warning (PRD §10.2).
pub const SKEW_WARN_MS: u64 = 10 * 60 * 1000;
/// `SKEW_MAX`: the protocol constant, the same on the server (ADR-0001 §2, decision 5).
pub const SKEW_MAX_MS: u64 = 24 * 60 * 60 * 1000;

/// How far a received stamp is ahead of the local physical time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skew {
    /// Within ten minutes (or behind).
    Fine,
    /// More than ten minutes ahead: the user is warned, nothing is blocked.
    Warning,
    /// More than `SKEW_MAX` ahead: applied to the data, not accepted into the clock.
    TooFar,
}

impl Skew {
    /// Classification of a stamp's `wall_ms` against the local physical time `pt`.
    pub fn of(remote_l: u64, pt: u64) -> Skew {
        let ahead = remote_l.saturating_sub(pt);
        if ahead > SKEW_MAX_MS {
            Skew::TooFar
        } else if ahead > SKEW_WARN_MS {
            Skew::Warning
        } else {
            Skew::Fine
        }
    }
}

/// Strict stamp parsing: 56 characters, digits, `device_id` is a lower-case UUID.
pub fn parse(hlc: &str) -> Result<(Clock, &str), HlcError> {
    let bytes = hlc.as_bytes();
    if bytes.len() != HLC_LEN || bytes.get(13) != Some(&b'-') || bytes.get(19) != Some(&b'-') {
        return Err(HlcError::Format);
    }
    let digits = |range: std::ops::Range<usize>| -> Result<u64, HlcError> {
        // The length of 56 bytes and the ASCII hyphens at 13 and 19 are already checked, so the boundaries 0, 13, 14, 19 and 20 lie on
        // character boundaries and `get` does not fail; it is here instead of a `[..]` slice so that a violation is an error, not a panic.
        let part = hlc.get(range).ok_or(HlcError::Format)?;
        if !part.bytes().all(|b| b.is_ascii_digit()) {
            return Err(HlcError::Format);
        }
        part.parse().map_err(|_| HlcError::Format)
    };
    let l = digits(0..13)?;
    let c = u32::try_from(digits(14..19)?).map_err(|_| HlcError::Format)?;
    let device = hlc.get(20..).ok_or(HlcError::Format)?;
    if !is_uuid(device) {
        return Err(HlcError::Format);
    }
    Ok((Clock { l, c }, device))
}

/// A lower-case UUID, 36 characters (`8-4-4-4-12`).
pub fn is_uuid(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.len() == 36
        && bytes.iter().enumerate().all(|(i, &b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_digit() || (b'a'..=b'f').contains(&b),
        })
}

/// A version 4 UUID in lower case: 36 characters, `4` at the start of the third group, variant `8`, `9`, `a` or `b` at the start of the fourth.
pub fn is_uuid_v4(text: &str) -> bool {
    let bytes = text.as_bytes();
    is_uuid(text)
        && bytes.get(14) == Some(&b'4')
        && matches!(bytes.get(19), Some(b'8' | b'9' | b'a' | b'b'))
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
            "the clock went back: l does not decrease"
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
            "the stamp stays strictly increasing"
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
                "{current} must be greater than {previous}"
            );
            previous = current;
        }
    }

    #[test]
    fn restart_continues_counter_when_clock_has_not_moved() {
        // Accepted a "future" stamp (l = 9000), restarted when pt = 3000: the counter continues instead of repeating.
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
    fn receive_follows_the_four_cases_of_the_paper() {
        let local = Clock { l: 100, c: 4 };
        // l == l' == l_m: the larger counter plus one.
        assert_eq!(
            local.receive(Clock { l: 100, c: 9 }, 50).unwrap(),
            Clock { l: 100, c: 10 }
        );
        // l == l' only: our counter plus one.
        assert_eq!(
            local.receive(Clock { l: 90, c: 9 }, 50).unwrap(),
            Clock { l: 100, c: 5 }
        );
        // l == l_m only: their counter plus one.
        assert_eq!(
            local.receive(Clock { l: 120, c: 9 }, 50).unwrap(),
            Clock { l: 120, c: 10 }
        );
        // Physical time is ahead of both: the counter restarts.
        assert_eq!(
            local.receive(Clock { l: 90, c: 9 }, 500).unwrap(),
            Clock { l: 500, c: 0 }
        );
    }

    #[test]
    fn a_stamp_made_after_receiving_is_greater_even_if_our_clock_is_behind() {
        // Device B's physical clock is 5 s behind A's. After B receives A's stamp its next local stamp must still be greater,
        // otherwise a restore made after seeing a delete would lose to it (PRD §10.2, criterion S4a).
        let a = Clock { l: 10_000, c: 0 };
        let b = Clock { l: 5_000, c: 0 }.receive(a, 5_000).unwrap();
        let restore = b.tick(5_001).unwrap();
        assert!(restore.format(DEVICE) > a.format(DEVICE));
    }

    #[test]
    fn receive_counter_overflow_is_monotonic() {
        let local = Clock {
            l: 7,
            c: MAX_COUNTER,
        };
        let next = local.receive(Clock { l: 7, c: 3 }, 7).unwrap();
        assert_eq!(next, Clock { l: 8, c: 0 });
        assert!(next.format(DEVICE) > local.format(DEVICE));
    }

    #[test]
    fn receive_overflow_of_wall_is_an_error() {
        let remote = Clock {
            l: MAX_WALL_MS,
            c: MAX_COUNTER,
        };
        assert_eq!(
            Clock::default().receive(remote, 0),
            Err(HlcError::WallOverflow)
        );
    }

    #[test]
    fn skew_is_classified_and_far_stamps_do_not_enter_the_clock() {
        let local = Clock { l: 1_000, c: 0 };
        let pt = 1_000_000;
        assert_eq!(Skew::of(pt + SKEW_WARN_MS, pt), Skew::Fine);
        assert_eq!(Skew::of(pt + SKEW_WARN_MS + 1, pt), Skew::Warning);
        assert_eq!(Skew::of(pt + SKEW_MAX_MS, pt), Skew::Warning);
        assert_eq!(Skew::of(pt + SKEW_MAX_MS + 1, pt), Skew::TooFar);
        assert_eq!(Skew::of(0, pt), Skew::Fine, "a stamp from the past is fine");

        let warned = Clock {
            l: pt + SKEW_WARN_MS + 5,
            c: 0,
        };
        let (clock, skew) = local.receive_checked(warned, pt).unwrap();
        assert_eq!(skew, Skew::Warning);
        assert_eq!(clock.l, warned.l, "a warning is accepted into the clock");

        let far = Clock {
            l: pt + SKEW_MAX_MS + 1,
            c: 0,
        };
        let (clock, skew) = local.receive_checked(far, pt).unwrap();
        assert_eq!(skew, Skew::TooFar);
        assert_eq!(clock, local, "beyond SKEW_MAX the clock is not touched");
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
        // Small values are padded with zeros: string comparison matches tuple comparison.
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
            ok.get(..55).unwrap(),
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
