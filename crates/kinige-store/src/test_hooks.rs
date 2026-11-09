//! Hooks for the integration tests (`#[doc(hidden)]`): injected panics and faults, and a few probes of the store's internals. Not
//! part of the API the app uses: this module exists only with the `test-hooks` feature, which only the crate's own
//! dev-dependency turns on (`scripts/check-release-features.sh` proves a release graph does not).

use crate::agent_read::ReadFail;
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

    /// Tests only: runs `held` while an agent read occupies the agent connection (its lock is taken and kept). The window's reads
    /// made inside `held` must return, because they use their own connection.
    #[doc(hidden)]
    pub fn with_agent_read_held_for_test<T>(&self, held: impl FnOnce() -> T) -> T {
        let _agent = self
            .inner
            .agent_reader
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        held()
    }

    /// Tests only: tries to write through the agent's read connection. It is `query_only`, so the error text is returned
    /// (`attempt to write a readonly database`) and nothing is written.
    #[doc(hidden)]
    pub fn write_through_agent_read_for_test(&self) -> String {
        let attempt = self.read_agent(|conn| {
            Ok(conn
                .execute("UPDATE chat SET title = title || ' (agent)'", [])
                .map(|_| String::new())
                .unwrap_or_else(|error| error.to_string()))
        });
        attempt.unwrap_or_default()
    }

    /// Tests only: a panic inside `query_map` on the agent read connection. The following agent reads must keep working.
    #[doc(hidden)]
    #[allow(clippy::panic)] // a hook for the integration tests of the panic safety net: the panic is its whole purpose
    pub fn panic_in_agent_read_for_test(&self) -> Result<(), StoreError> {
        let result = self.with_reader(|conn| -> Result<(), ReadFail> {
            let mut statement = conn.prepare("SELECT id FROM chat")?;
            let rows = statement.query_map([], |_| -> rusqlite::Result<()> {
                panic!("test: panic in an agent query")
            })?;
            for row in rows {
                row?;
            }
            Ok(())
        });
        match result {
            Ok(()) => Ok(()),
            Err(ReadFail::Store(error)) => Err(error),
            Err(ReadFail::NotFound) => Err(StoreError::Internal("not found".into())),
        }
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
