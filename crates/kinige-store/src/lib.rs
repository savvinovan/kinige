//! Хранилище Kinige (ТЗ §3.2): SQLite через `rusqlite`, один поток-писатель, чтение отдельным соединением (WAL),
//! миграции, Markdown-копия, копия файла базы. Бизнес-правила — в `kinige-domain`; здесь только транзакции и файлы.

mod backup;
mod capture;
mod chats;
mod draft;
mod error;
mod feed;
mod hlc_state;
mod maintenance;
mod messages;
mod mirror;
mod recovery;
mod schema;
mod send;
mod settings;
mod tags;
mod time;
mod trash;
mod view;
mod writer;

pub use backup::{Integrity, backups_dir, list_backups, verify_database};
pub use capture::{CONTEXT_LEN, CaptureView};
pub use chats::RecentChat;
pub use draft::MAX_DRAFT_BYTES;
pub use error::StoreError;
pub use feed::{DEFAULT_PAGE, MAX_PAGE};
pub use maintenance::MaintenanceReport;
pub use recovery::{
    RestoreReport, StartupCheck, backups, clear_needs_recovery, database_missing_but_backups_exist,
    mark_needs_recovery, needs_recovery, restart_for_recovery_allowed, restore_backup,
    restore_backup_with, startup_check,
};
pub use schema::LATEST_VERSION;
pub use time::{Now, local_hhmm, offset_secs, utc_stamp};

use backup::BackupShared;
use error::{read_err, write_err};
use kinige_domain::groups;
use maintenance::MaintenanceShared;
use mirror::MirrorShared;
use rusqlite::{Connection, OpenFlags};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use writer::Writer;

/// Сигнал о повреждении базы, найденном проверкой целостности в фоне: вызывается один раз за запуск, из потока
/// обслуживания. Слой Tauri по нему ставит отметку «нужно восстановление» и сообщает окну.
#[derive(Clone)]
pub struct CorruptionHook(pub Arc<dyn Fn() + Send + Sync>);

impl fmt::Debug for CorruptionHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CorruptionHook")
    }
}

/// Параметры открытия хранилища.
#[derive(Debug, Clone)]
pub struct StoreOptions {
    /// Файл базы SQLite.
    pub db_path: PathBuf,
    /// Папка Markdown-копии по умолчанию (`~/Documents/Kinige`); `None` — по умолчанию копии нет, пока папку не выберут.
    pub default_mirror_dir: Option<PathBuf>,
    /// Запускать ли фоновую запись копии (тесты процесса, убитого сразу после фиксации, выключают её).
    pub start_mirror: bool,
    /// Через сколько повторять запись копии после ошибки.
    pub mirror_retry: Duration,
    /// Запускать ли фоновое обслуживание: автоудаление корзины, проверка целостности, копия дня (тесты выключают и зовут
    /// [`Store::run_maintenance`] сами).
    pub start_maintenance: bool,
    /// Что сделать, когда проверка целостности нашла повреждение.
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
            on_corruption: None,
        }
    }
}

pub(crate) struct Inner {
    writer: Writer,
    reader: Mutex<Connection>,
    mirror: Arc<MirrorShared>,
    default_mirror_dir: Option<PathBuf>,
    db_path: PathBuf,
    backup: BackupShared,
    maintenance: Arc<MaintenanceShared>,
    on_corruption: Option<CorruptionHook>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.mirror.stop();
        self.maintenance.stop();
    }
}

/// Хранилище. Копируется дёшево (общий `Arc`): команды Tauri держат свой экземпляр.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

/// Перед миграцией существующей базы: проверка целостности (синхронно) и копия. Свежая база (файла нет или пустой, версия
/// схемы 0) и база текущей версии копии не требуют.
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
        backup::Integrity::Ok => {}
        backup::Integrity::Corrupt => {
            return Err(StoreError::Corrupt(
                "проверка целостности перед миграцией".into(),
            ));
        }
        // Не повреждение: база занята или ввод-вывод не удался. Мигрировать без вердикта нельзя, а предлагать копии
        // здоровой базе — тем более.
        backup::Integrity::Unknown => {
            return Err(StoreError::Read(
                "не удалось проверить целостность перед миграцией".into(),
            ));
        }
    }
    // Нет копии — нет миграции: откат приложения после миграции не поддерживается (ТЗ §10.4).
    backup::before_migration(&conn, db_path, current, &time::utc_stamp(now.ms))
        .map_err(|error| StoreError::Write(format!("копия перед миграцией: {error:?}")))
}

impl Store {
    /// Открывает базу, применяет миграции (перед миграцией существующей базы снимается копия), при первом запуске создаёт
    /// `device_id`, часы HLC и Inbox (идемпотентно), запускает писателя, фоновую запись копии и обслуживание.
    pub fn open(options: StoreOptions) -> Result<Store, StoreError> {
        Store::open_at(options, &Now::system())
    }

    /// То же, что [`Store::open`], но с заданным «сейчас» (тесты).
    pub fn open_at(options: StoreOptions, now: &Now) -> Result<Store, StoreError> {
        if let Some(parent) = options.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::Internal(e.to_string()))?;
        }
        prepare_migration(&options.db_path, now)?;
        let mut conn = open_connection(&options.db_path, false)?;
        schema::migrate(&mut conn).map_err(|e| match e {
            schema::MigrateError::TooNew { found } => StoreError::TooNew(found),
            schema::MigrateError::Sqlite(e) => write_err(e),
        })?;
        send::ensure_initialized(&mut conn, now)?;

        let reader = open_connection(&options.db_path, true)?;
        let writer = Writer::spawn(conn);
        let mirror = MirrorShared::new(options.mirror_retry);
        let maintenance = MaintenanceShared::new();
        let store = Store {
            inner: Arc::new(Inner {
                writer,
                reader: Mutex::new(reader),
                mirror: mirror.clone(),
                default_mirror_dir: options.default_mirror_dir.clone(),
                db_path: options.db_path.clone(),
                backup: BackupShared::default(),
                maintenance: maintenance.clone(),
                on_corruption: options.on_corruption.clone(),
            }),
        };
        if options.start_mirror {
            let reader = open_connection(&options.db_path, true)?;
            mirror::spawn(
                mirror,
                reader,
                store.inner.writer.clone(),
                options.default_mirror_dir,
            );
        }
        if options.start_maintenance {
            maintenance::spawn(maintenance, Arc::downgrade(&store.inner));
        }
        Ok(store)
    }

    /// Выполняет запись на потоке-писателе.
    fn write<T: Send + 'static>(
        &self,
        job: impl FnOnce(&mut Connection) -> Result<T, StoreError> + Send + 'static,
    ) -> Result<T, StoreError> {
        self.inner.writer.run(job)?
    }

    /// Выполняет чтение на отдельном соединении.
    fn read<T>(
        &self,
        job: impl FnOnce(&Connection) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let conn = self
            .inner
            .reader
            .lock()
            .map_err(|_| StoreError::Internal("соединение чтения отравлено".into()))?;
        job(&conn)
    }

    fn wake_mirror(&self) {
        self.inner.mirror.wake();
    }

    /// Id Inbox: константа (ADR-0001, §3.6).
    pub fn inbox_id(&self) -> &'static str {
        groups::INBOX_ID
    }

    /// Файл базы: нужен слою Tauri для восстановления и диагностики.
    pub fn db_path(&self) -> &Path {
        &self.inner.db_path
    }

    /// Прагмы соединения-писателя `(synchronous, fullfsync, journal_mode)`: для проверки, что запись долговечна.
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

    /// Путь папки Markdown-копии по умолчанию нужен тестам и диагностике.
    pub fn default_mirror_dir(&self) -> Option<&Path> {
        self.inner.default_mirror_dir.as_deref()
    }
}

/// Соединение с общими настройками. Для чтения — `query_only`.
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
        // WAL: читатели не блокируют писателя. FULL: запись фиксируется на диске до ответа (переживает и убийство процесса,
        // и потерю питания, ТЗ §12.3).
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(write_err)?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(write_err)?;
        // macOS: обычный fsync не сбрасывает кэш накопителя, F_FULLFSYNC сбрасывает (иначе потеря питания может
        // унести уже подтверждённую запись). На других ОС прагма ничего не меняет.
        conn.pragma_update(None, "fullfsync", true)
            .map_err(write_err)?;
    }
    Ok(conn)
}
