//! Rate limits on login, recovery and the word checks (ADR-0001 §8.4): in memory, fixed windows, per normalized login and per
//! client address. Every attempt counts, whether or not the login exists, so a known and an unknown login are limited alike. The
//! numbers are the defaults of `Config`; real ones are slice 11's.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

/// Keys kept before expired windows are swept out.
const SWEEP_AT: usize = 10_000;

/// `limit` attempts in a window of `window_ms`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule {
    pub limit: u32,
    pub window_ms: u64,
}

impl Rule {
    pub const fn new(limit: u32, window_ms: u64) -> Rule {
        Rule { limit, window_ms }
    }
}

/// What a refused attempt is told: how long until its window ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limited {
    pub retry_after_secs: u64,
}

#[derive(Debug, Default)]
pub struct Limiter {
    windows: Mutex<HashMap<String, Window>>,
}

#[derive(Debug, Clone, Copy)]
struct Window {
    started_ms: u64,
    count: u32,
}

impl Limiter {
    /// Counts one attempt under `key` and says whether it is allowed. The window starts at the first attempt and is not extended by
    /// refused ones.
    pub fn hit(&self, key: &str, rule: Rule, now_ms: u64) -> Result<(), Limited> {
        let mut windows = self.windows.lock().unwrap_or_else(PoisonError::into_inner);
        if windows.len() >= SWEEP_AT {
            windows.retain(|_, w| now_ms.saturating_sub(w.started_ms) < rule.window_ms);
        }
        let window = windows.entry(key.to_string()).or_insert(Window {
            started_ms: now_ms,
            count: 0,
        });
        if now_ms.saturating_sub(window.started_ms) >= rule.window_ms {
            *window = Window {
                started_ms: now_ms,
                count: 0,
            };
        }
        if window.count >= rule.limit {
            let left_ms = rule
                .window_ms
                .saturating_sub(now_ms.saturating_sub(window.started_ms));
            return Err(Limited {
                retry_after_secs: left_ms.div_ceil(1000).max(1),
            });
        }
        window.count = window.count.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE: Rule = Rule::new(3, 60_000);

    #[test]
    fn the_attempt_after_the_limit_is_refused_until_the_window_ends() {
        let limiter = Limiter::default();
        for _ in 0..3 {
            assert!(limiter.hit("a", RULE, 1_000).is_ok());
        }
        assert_eq!(
            limiter.hit("a", RULE, 10_000),
            Err(Limited {
                retry_after_secs: 51
            })
        );
        assert!(limiter.hit("a", RULE, 61_000).is_ok(), "a new window");
    }

    #[test]
    fn keys_are_independent_and_refused_attempts_do_not_extend_the_window() {
        let limiter = Limiter::default();
        for _ in 0..3 {
            assert!(limiter.hit("a", RULE, 0).is_ok());
        }
        assert!(limiter.hit("a", RULE, 59_999).is_err());
        assert!(limiter.hit("b", RULE, 59_999).is_ok());
        assert!(limiter.hit("a", RULE, 60_000).is_ok());
    }

    #[test]
    fn expired_windows_are_swept_when_the_map_is_large() {
        let limiter = Limiter::default();
        for n in 0..SWEEP_AT {
            let _ = limiter.hit(&format!("k{n}"), RULE, 0);
        }
        assert!(limiter.hit("late", RULE, 120_000).is_ok());
        let size = limiter
            .windows
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();
        assert_eq!(size, 1, "only the live window is left");
    }
}
