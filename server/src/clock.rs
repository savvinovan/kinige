//! The server's clock, injected so that tests and the S4a adapter run on virtual time (spec: compaction and `clock_skew`).

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub trait Clock: Send + Sync {
    /// Milliseconds since the Unix epoch.
    fn now_ms(&self) -> u64;

    /// Whether the time that matters for a stamp or a backup's cut time is the **database's** clock, read inside the locked transaction
    /// (`store::lock::stamp`): the production clock. One process's wall clock is not enough, because the backup may run on another
    /// host than the server, and a restore into a clean installation on a host whose clock is behind. A test clock returns false and
    /// is read as it is; both go through the same floor (never below the newest cut time of the lineage).
    fn is_wall_clock(&self) -> bool {
        false
    }
}

/// Production: the system clock. A clock before 1970 reads as 0.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|elapsed| u64::try_from(elapsed.as_millis()).ok())
            .unwrap_or(0)
    }

    fn is_wall_clock(&self) -> bool {
        true
    }
}

/// Tests: a clock that only moves when told to.
#[derive(Debug, Default)]
pub struct ManualClock(AtomicU64);

impl ManualClock {
    pub fn new(now_ms: u64) -> ManualClock {
        ManualClock(AtomicU64::new(now_ms))
    }

    pub fn set(&self, now_ms: u64) {
        self.0.store(now_ms, Ordering::SeqCst);
    }

    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}
