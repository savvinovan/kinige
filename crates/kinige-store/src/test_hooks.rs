//! Hooks for the integration tests (`#[doc(hidden)]`): injected panics and faults, and a few probes of the store's internals. Not
//! part of the API the app uses.

use crate::error::read_err;
use crate::recent_chats::LAST_ENTRY_SQL;
use crate::{AttachFault, BackgroundThread, Store, StoreError};
use kinige_domain::groups::INBOX_ID;
use std::sync::Arc;

impl Store {
    /// Tests only: a panic inside a read. The following reads must work.
    #[doc(hidden)]
    #[allow(clippy::panic)] // a hook for integration tests of the panic safety net: the panic is its whole purpose
    pub fn panic_in_read_for_test(&self) -> Result<(), StoreError> {
        self.read(|_| -> Result<(), StoreError> { panic!("check: panic in a read") })
    }

    /// Tests only: a step in the Markdown copy pass after reading a batch of marks and before checking the folder; `None`
    /// removes it. In the step the test changes the copy folder in the middle of a batch (the pass must restart with the
    /// new folder).
    #[doc(hidden)]
    pub fn set_mirror_batch_hook_for_test(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        self.inner.mirror.set_batch_hook(hook);
    }

    /// Tests only: the next pass of the background copy (`mirror`) or maintenance (`maintenance`) panics once.
    #[doc(hidden)]
    pub fn inject_background_panic_for_test(&self, thread: BackgroundThread) {
        match thread {
            BackgroundThread::Mirror => self.inner.mirror.inject_panic(),
            BackgroundThread::Maintenance => self.inner.maintenance.inject_panic(),
        }
        self.wake_mirror();
        self.inner.maintenance.wake_now();
    }

    /// Tests only: wake the maintenance thread now.
    #[doc(hidden)]
    pub fn wake_maintenance_for_test(&self) {
        self.inner.maintenance.wake_now();
    }

    /// Tests only: an injected attachment failure (ENOSPC, a source that changes, a crash after the rename, a failed deletion).
    #[doc(hidden)]
    pub fn inject_attach_fault_for_test(&self, fault: AttachFault) {
        self.inner.faults.set(fault);
    }

    /// How many owners the store has (the `Store` itself and its clones; background threads via `Weak` are not counted):
    /// the MCP server test checks that a connected client does not hold a `Store` clone (a restore from a backup must not
    /// find a live writer).
    #[doc(hidden)]
    pub fn handle_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }

    /// The query plan (`EXPLAIN QUERY PLAN`, the `detail` column) of the per-chat step of "recent chats": the test checks that it
    /// goes over `message_feed` and does not scan `message`.
    #[doc(hidden)]
    pub fn recent_chats_plan_for_test(&self) -> Result<Vec<String>, StoreError> {
        self.read(|conn| {
            let mut statement = conn
                .prepare(&format!("EXPLAIN QUERY PLAN {LAST_ENTRY_SQL}"))
                .map_err(read_err)?;
            let rows = statement
                .query_map([INBOX_ID], |row| row.get::<_, String>(3))
                .map_err(read_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(read_err)
        })
    }

    /// Pragmas of the writer connection `(synchronous, fullfsync, journal_mode)`: to check that writes are durable.
    #[doc(hidden)]
    pub fn writer_pragmas(&self) -> Result<(i64, i64, String), StoreError> {
        self.write(|conn| {
            Ok((
                conn.pragma_query_value(None, "synchronous", |r| r.get(0))
                    .map_err(read_err)?,
                conn.pragma_query_value(None, "fullfsync", |r| r.get(0))
                    .map_err(read_err)?,
                conn.pragma_query_value(None, "journal_mode", |r| r.get(0))
                    .map_err(read_err)?,
            ))
        })
    }
}
