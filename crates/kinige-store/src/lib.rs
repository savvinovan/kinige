//! Хранилище Kinige (ТЗ §3.2): SQLite через `rusqlite`, один поток-писатель, чтение отдельным соединением (WAL),
//! миграции, Markdown-копия. Бизнес-правила — в `kinige-domain`; здесь только транзакции и файлы.

mod error;
mod feed;
mod hlc_state;
mod mirror;
mod schema;
mod send;
mod settings;
mod time;
mod writer;

pub use error::StoreError;
pub use feed::{DEFAULT_PAGE, MAX_PAGE};
pub use schema::LATEST_VERSION;
pub use time::{Now, local_hhmm, offset_secs};

use kinige_domain::groups;
use mirror::MirrorShared;
use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use writer::Writer;

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
}

impl StoreOptions {
    pub fn new(db_path: impl Into<PathBuf>, default_mirror_dir: Option<PathBuf>) -> Self {
        StoreOptions {
            db_path: db_path.into(),
            default_mirror_dir,
            start_mirror: true,
            mirror_retry: Duration::from_secs(10),
        }
    }
}

struct Inner {
    writer: Writer,
    reader: Mutex<Connection>,
    mirror: Arc<MirrorShared>,
    default_mirror_dir: Option<PathBuf>,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.mirror.stop();
    }
}

/// Хранилище. Копируется дёшево (общий `Arc`): команды Tauri держат свой экземпляр.
#[derive(Clone)]
pub struct Store {
    inner: Arc<Inner>,
}

impl Store {
    /// Открывает базу, применяет миграции, при первом запуске создаёт `device_id`, часы HLC и Inbox (идемпотентно),
    /// запускает писателя и фоновую запись копии.
    pub fn open(options: StoreOptions) -> Result<Store, StoreError> {
        Store::open_at(options, &Now::system())
    }

    /// То же, что [`Store::open`], но с заданным «сейчас» (тесты).
    pub fn open_at(options: StoreOptions, now: &Now) -> Result<Store, StoreError> {
        if let Some(parent) = options.db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| StoreError::Internal(e.to_string()))?;
        }
        let mut conn = open_connection(&options.db_path, false)?;
        schema::migrate(&mut conn).map_err(|e| match e {
            schema::MigrateError::TooNew { found } => {
                StoreError::Internal(format!("база создана более новой версией (схема {found})"))
            }
            schema::MigrateError::Sqlite(e) => StoreError::Write(e.to_string()),
        })?;
        send::ensure_initialized(&mut conn, now)?;

        let reader = open_connection(&options.db_path, true)?;
        let writer = Writer::spawn(conn);
        let mirror = MirrorShared::new(options.mirror_retry);
        let store = Store {
            inner: Arc::new(Inner {
                writer,
                reader: Mutex::new(reader),
                mirror: mirror.clone(),
                default_mirror_dir: options.default_mirror_dir.clone(),
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

    /// Путь файла базы нужен тестам и диагностике.
    pub fn default_mirror_dir(&self) -> Option<&Path> {
        self.inner.default_mirror_dir.as_deref()
    }
}

/// Соединение с общими настройками. Для чтения — `query_only`.
fn open_connection(path: &Path, read_only: bool) -> Result<Connection, StoreError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn =
        Connection::open_with_flags(path, flags).map_err(|e| StoreError::Write(e.to_string()))?;
    conn.busy_timeout(Duration::from_secs(5))
        .map_err(|e| StoreError::Write(e.to_string()))?;
    if read_only {
        conn.pragma_update(None, "query_only", true)
            .map_err(|e| StoreError::Read(e.to_string()))?;
    } else {
        // WAL: читатели не блокируют писателя. FULL: запись фиксируется на диске до ответа (переживает и убийство процесса,
        // и потерю питания, ТЗ §12.3).
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| StoreError::Write(e.to_string()))?;
        conn.pragma_update(None, "synchronous", "FULL")
            .map_err(|e| StoreError::Write(e.to_string()))?;
    }
    Ok(conn)
}
