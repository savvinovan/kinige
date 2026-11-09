//! Kinige store (PRD §3.2): SQLite via `rusqlite`, one writer thread, reads on a separate connection (WAL),
//! migrations, the Markdown copy, the database file backup. Business rules live in `kinige-domain`; here only
//! transactions and files.

mod agent;
mod agent_entry;
mod agent_log;
mod agent_read;
mod agent_settings;
mod agent_write;
mod attachment;
mod backup;
mod capture;
mod chat_color;
mod chats;
mod data_dir;
mod draft;
mod error;
mod ext;
mod feed;
mod feed_page;
mod forest;
mod hlc_state;
mod import;
mod import_batch;
mod import_lookup;
mod import_media;
mod import_time;
mod maintenance;
mod message_body;
mod messages;
mod mirror;
mod mirror_day;
mod mirror_files;
mod on_this_day;
mod recent_chats;
mod recovery;
mod schema;
mod search;
mod search_query;
mod search_sql;
mod send;
mod settings;
mod settings_file;
mod tags;
mod test_hooks;
mod time;
mod trash;
mod view;
mod writer;

pub use agent::{AgentError, AgentReply, AgentRequest, AgentSettings};
pub use agent_log::{LOG_DAYS, LOG_MAX_ROWS, LOG_PAGE};
pub use attachment::{
    AttachFault, AudioFile, MAX_DECODE_BYTES, Media, MediaVariant, OpenTarget, SweepReport,
};
pub use backup::{Integrity, backups_dir, list_backups, verify_database};
pub use capture::{CONTEXT_LEN, CaptureView};
pub use draft::MAX_DRAFT_BYTES;
pub use error::StoreError;
pub use feed::{DEFAULT_PAGE, MAX_PAGE};
pub use feed_page::PageAt;
pub use import::IMPORT_BATCH;
pub use maintenance::MaintenanceReport;
pub use recent_chats::RecentChat;
pub use recovery::{
    RestoreReport, StartupCheck, backups, clear_needs_recovery, database_missing_but_backups_exist,
    mark_needs_recovery, needs_recovery, restore_backup, restore_backup_with, startup_check,
};
pub use schema::LATEST_VERSION;
pub use settings_file::{
    FILE_NAME as SETTINGS_FILE_NAME, FileError, LOG_CLIENT as SETTINGS_LOG_CLIENT,
    LOG_TOOL as SETTINGS_LOG_TOOL, MAX_BYTES as SETTINGS_FILE_MAX_BYTES, SettingsFile,
    SettingsFileOptions, ShortcutApplier, ShortcutRefusal, file_path as settings_file_path,
};
pub use time::{Now, local_hhmm, offset_secs, utc_stamp};

use backup::BackupShared;
use error::{read_err, write_err};
use kinige_domain::groups;
use maintenance::MaintenanceShared;
use mirror::MirrorShared;
use rusqlite::{Connection, OpenFlags};
use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use writer::Writer;

/// A background thread of the store (for panic tests).
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub enum BackgroundThread {
    Mirror,
    Maintenance,
}

/// Signal about database corruption found by the background integrity check: called once per launch, from the
/// maintenance thread. The Tauri layer uses it to set the "restore needed" mark and tell the window.
#[derive(Clone)]
pub struct CorruptionHook(pub Arc<dyn Fn() + Send + Sync>);

impl fmt::Debug for CorruptionHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CorruptionHook")
    }
}

/// Options for opening the store.
#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// The SQLite database file.
    pub db_path: PathBuf,
    /// Default Markdown copy folder (`~/Documents/Kinige`); `None` means there is no copy by default until a folder is chosen.
    pub default_mirror_dir: Option<PathBuf>,
    /// Whether to start the background copy writer (tests of a process killed right after a commit turn it off).
    pub start_mirror: bool,
    /// How long to wait before retrying the copy write after an error.
    pub mirror_retry: Duration,
    /// Whether to start background maintenance: trash auto-delete, integrity check, the day's backup (tests turn it off
    /// and call [`Store::run_maintenance`] themselves).
    pub start_maintenance: bool,
    /// The first maintenance pass waits for [`Store::begin_maintenance`] (the app calls it once the main window has loaded), or 20 s.
    pub defer_maintenance: bool,
    /// What to do when the integrity check finds corruption.
    pub on_corruption: Option<CorruptionHook>,
}

impl StoreOptions {
    pub fn new(db_path: impl Into<PathBuf>, default_mirror_dir: Option<PathBuf>) -> Self {
        StoreOptions {
            db_path: db_path.into(),
            default_mirror_dir,
            start_mirror: true,
            mirror_retry: Duration::from_secs(10),
            start_maintenance: true,
            defer_maintenance: false,
            on_corruption: None,
        }
    }
}

pub(crate) struct Inner {
    writer: Writer,
    reader: Mutex<Connection>,
    /// A second `query_only` connection for the MCP server's tool calls (`read_agent`, `with_reader`): the window's reads
    /// and an agent's reads do not wait for each other. One connection, not a pool (KINIG-47.2).
    agent_reader: Mutex<Connection>,
    mirror: Arc<MirrorShared>,
    default_mirror_dir: Option<PathBuf>,
    db_path: PathBuf,
    backup: BackupShared,
    maintenance: Arc<MaintenanceShared>,
    on_corruption: Option<CorruptionHook>,
    /// How many times the search index was healed during this launch (`search::heal_search_index`).
    search_heals: Arc<std::sync::atomic::AtomicU64>,
    /// Made at startup: names this process's `.tmp/` and `open/` directories, so the attachment sweep never touches a live copy.
    process_id: String,
    /// Failures injected by tests (attachments).
    faults: Arc<attachment::Faults>,
    /// The threads behind the `kinige-media` protocol.
    media_pool: attachment::MediaPool,
    /// Images whose thumbnail did not decode in this launch: not tried again on every render.
    thumb_failed: Mutex<HashSet<String>>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.mirror.stop();
        self.maintenance.stop();
    }
}

/// The store. Cheap to clone (a shared `Arc`): Tauri commands hold their own instance.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

/// Before migrating an existing database: an integrity check (synchronous) and a backup. A fresh database (no file or an
/// empty one, schema version 0) and a database of the current version need no backup.
fn prepare_migration(db_path: &Path, now: &Now) -> Result<(), StoreError> {
    let exists = std::fs::metadata(db_path).is_ok_and(|m| m.len() > 0);
    if !exists {
        return Ok(());
    }
    let conn = open_connection(db_path, true)?;
    let current: i64 = conn
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(read_err)?;
    if current == 0 || current >= schema::LATEST_VERSION {
        return Ok(());
    }
    match backup::check_integrity(&conn) {
        // The search index is derived: its corruption does not block the migration (it is rebuilt on open or on the first query).
        backup::Integrity::Ok | backup::Integrity::SearchIndex => {}
        backup::Integrity::Corrupt => {
            return Err(StoreError::Corrupt(
                "integrity check before the migration".into(),
            ));
        }
        // Not corruption: the database is busy or an I/O call failed. Migrating without a verdict is not allowed, and
        // offering backups for a healthy database is even less so.
        backup::Integrity::Unknown => {
            return Err(StoreError::Read(
                "could not check integrity before the migration".into(),
            ));
        }
    }
    // No backup, no migration: rolling the app back after a migration is not supported (PRD §10.4).
    backup::before_migration(&conn, db_path, current, &time::utc_stamp(now.ms))
        .map_err(|error| StoreError::Write(format!("backup before the migration: {error:?}")))
}

impl Store {
    /// Opens the database, applies migrations (a backup is taken before migrating an existing database), on the first
    /// launch creates `device_id`, the HLC clock and Inbox (idempotently), starts the writer, the background copy
    /// writer and maintenance.
    pub fn open(options: StoreOptions) -> Result<Store, StoreError> {
        Store::open_at(options, &Now::system())
    }

    /// Same as [`Store::open`], but with a given "now" (tests).
    pub fn open_at(options: StoreOptions, now: &Now) -> Result<Store, StoreError> {
        if let Some(parent) = options.db_path.parent() {
            data_dir::create(parent).map_err(|e| StoreError::Internal(e.to_string()))?;
        }
        prepare_migration(&options.db_path, now)?;
        let mut conn = open_connection(&options.db_path, false)?;
        schema::migrate(&mut conn).map_err(|e| match e {
            schema::MigrateError::TooNew { found } => StoreError::TooNew(found),
            schema::MigrateError::Sqlite(e) => write_err(e),
        })?;
        send::ensure_initialized(&mut conn, now)?;
        search::rebuild_if_stale(&mut conn)?;

        let reader = open_connection(&options.db_path, true)?;
        let agent_reader = open_connection(&options.db_path, true)?;
        let writer = Writer::spawn(conn)?;
        let mirror = MirrorShared::new(options.mirror_retry);
        let maintenance = MaintenanceShared::new(options.defer_maintenance);
        let media_pool = attachment::MediaPool::spawn()?;
        let store = Store {
            inner: Arc::new(Inner {
                writer,
                reader: Mutex::new(reader),
                agent_reader: Mutex::new(agent_reader),
                mirror: mirror.clone(),
                default_mirror_dir: options.default_mirror_dir.clone(),
                db_path: options.db_path.clone(),
                backup: BackupShared::default(),
                maintenance: maintenance.clone(),
                on_corruption: options.on_corruption.clone(),
                search_heals: Arc::default(),
                process_id: uuid::Uuid::new_v4().to_string(),
                faults: Arc::default(),
                media_pool,
                thumb_failed: Mutex::default(),
            }),
        };
        if options.start_mirror {
            let reader = open_connection(&options.db_path, true)?;
            mirror::spawn(
                mirror,
                reader,
                store.inner.writer.clone(),
                options.default_mirror_dir,
                store.dirs().attachments(),
            )?;
        }
        if options.start_maintenance {
            maintenance::spawn(maintenance, Arc::downgrade(&store.inner))?;
        }
        Ok(store)
    }

    /// Runs a write on the writer thread. If the write itself rebuilt the search index (corruption surfaced under
    /// `guarded`), the heal count grows: a query with an old `seen` will not rebuild a healthy index again. If index
    /// corruption surfaced where `guarded` is not set (for example, on `COMMIT`), the index is healed right away when
    /// possible so that a retry of the write passes; the error itself is returned as is.
    fn write<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let seen = self.search_generation();
        let heals = self.inner.search_heals.clone();
        let result = self.write_raw(move |conn| {
            let _ = search::take_rebuilt();
            let result = job(conn);
            if search::take_rebuilt() && result.is_ok() {
                heals.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            result
        });
        if matches!(result, Err(StoreError::SearchIndex(_))) {
            let _ = self.heal_search_index(seen);
        }
        result
    }

    /// A write without index healing and without counting: the index heals itself with it.
    fn write_raw<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        let result = self.inner.writer.run(job)?;
        self.note_if_corrupt(&result);
        result
    }

    /// Runs a read on a separate connection.
    fn read<T>(
        &self,
        job: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let result = self.read_unnoted(job);
        self.note_if_corrupt(&result);
        result
    }

    /// A read without the corruption signal: for a query that decides itself what to do with `Corrupt` (search heals its index).
    fn read_unnoted<T>(
        &self,
        job: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        read_on(&self.inner.reader, job)
    }

    /// The main window has loaded its page: a deferred first maintenance pass (`StoreOptions::defer_maintenance`) may start now.
    /// Only the first call does anything; without a deferred start it does nothing.
    pub fn begin_maintenance(&self) {
        self.inner.maintenance.begin();
    }

    /// Any command that runs into corruption (`SQLITE_CORRUPT`, `SQLITE_NOTADB`) takes the same path as the background
    /// check: one signal per launch (mark, flag, event to the window) -- otherwise "Restart" after such an error would
    /// be a dead button until maintenance reaches its own check.
    fn note_if_corrupt<T>(&self, result: &Result<T, StoreError>) {
        if matches!(result, Err(StoreError::Corrupt(_))) {
            self.report_corruption();
        }
    }

    fn wake_mirror(&self) {
        self.inner.mirror.wake();
    }

    /// Inbox id: a constant (ADR-0001, §3.6).
    pub fn inbox_id(&self) -> &'static str {
        groups::INBOX_ID
    }

    /// The database file: needed by the Tauri layer for restore and diagnostics.
    pub fn db_path(&self) -> &Path {
        &self.inner.db_path
    }

    /// The attachment directories (`<data>/attachments`, `thumbs`, `open`) of this process.
    pub(crate) fn dirs(&self) -> attachment::Dirs {
        let data = self
            .inner
            .db_path
            .parent()
            .unwrap_or_else(|| Path::new("."));
        attachment::Dirs::new(data, &self.inner.process_id)
    }

    pub(crate) fn faults(&self) -> &attachment::Faults {
        &self.inner.faults
    }

    /// The data directory: the folder of the database, where `attachments/` and `thumbs/` lie.
    pub fn data_dir(&self) -> &Path {
        self.inner
            .db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
    }

    /// The default Markdown copy folder path is needed by tests and diagnostics.
    pub fn default_mirror_dir(&self) -> Option<&Path> {
        self.inner.default_mirror_dir.as_deref()
    }
}

/// Runs `job` on the connection behind `reader`.
pub(crate) fn read_on<T>(
    reader: &Mutex<Connection>,
    job: impl FnOnce(&Connection) -> Result<T, StoreError>,
) -> Result<T, StoreError> {
    // A panic inside a read (the release profile now unwinds the stack) must not break all following reads: it is caught
    // here, the call gets `Internal`, and the lock, if it was poisoned anyway, is taken as is -- the read connection
    // (`query_only`; the query itself opens and closes a read transaction if needed) is consistent after unwinding.
    let conn = reader
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| job(&conn))).unwrap_or_else(|_| {
        log::error!("panic in a read, the call is cancelled");
        Err(StoreError::Internal("panic during a read".into()))
    })
}

/// A connection with the shared settings. For reading it is `query_only`.
pub(crate) fn open_connection(path: &Path, read_only: bool) -> Result<Connection, StoreError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(write_err)?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(write_err)?;
    if read_only {
        conn.pragma_update(None, "query_only", true)
            .map_err(read_err)?;
    } else {
        // WAL: readers do not block the writer. FULL: a write is committed to disk before the reply (survives both a
        // process kill and a power loss, PRD §12.3).
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(write_err)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(write_err)?;
        // macOS: a plain fsync does not flush the drive cache, F_FULLFSYNC does (otherwise a power loss can take away an
        // already confirmed write). On other OSes the pragma changes nothing.
        conn.pragma_update(None, "fullfsync", true)
            .map_err(write_err)?;
    }
    Ok(conn)
}
