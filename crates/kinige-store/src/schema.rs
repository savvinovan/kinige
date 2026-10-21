//! Схема SQLite и миграции (`PRAGMA user_version`). Миграция 2 добавляет локальную таблицу черновиков (слайс 2).
//! Миграция 1 создаёт только таблицы, в которые пишет слайс 1
//! (слайс 1, «Затрагивает чувствительные зоны»): остальные таблицы ADR-0001 §C добавят свои слайсы миграциями
//! `CREATE TABLE` без изменения существующих строк. Колонки `hlc_json`, `server_known`, `groups_raw` и `schema_version`
//! есть с первой записи: задним числом их не исправить.

use rusqlite::Connection;

const MIGRATION_1: &str = "
CREATE TABLE chat (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  icon TEXT,
  color TEXT,
  sort_key TEXT NOT NULL,
  is_inbox INTEGER NOT NULL DEFAULT 0,
  ai_excluded INTEGER NOT NULL DEFAULT 0,
  pinned_at INTEGER,
  archived_at INTEGER,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted_at INTEGER,
  hlc_json TEXT NOT NULL,
  server_known INTEGER NOT NULL DEFAULT 0,
  schema_version INTEGER NOT NULL,
  groups_raw TEXT NOT NULL
);

CREATE TABLE message (
  id TEXT PRIMARY KEY,
  chat_id TEXT NOT NULL,
  body_md TEXT NOT NULL DEFAULT '',
  journal_date TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  created_tz TEXT NOT NULL,
  edited_at INTEGER,
  is_task INTEGER NOT NULL DEFAULT 0,
  task_done_at INTEGER,
  favorite_at INTEGER,
  pinned_at INTEGER,
  reply_to_id TEXT,
  origin TEXT NOT NULL DEFAULT 'user',
  ext_json TEXT,
  deleted_at INTEGER,
  body_base TEXT,
  body_supersedes TEXT NOT NULL DEFAULT '[]',
  placement_base TEXT,
  placement_supersedes TEXT NOT NULL DEFAULT '[]',
  hlc_json TEXT NOT NULL,
  server_known INTEGER NOT NULL DEFAULT 0,
  schema_version INTEGER NOT NULL,
  groups_raw TEXT NOT NULL
);
CREATE INDEX message_feed ON message(chat_id, journal_date, created_at, id);
CREATE INDEX message_day ON message(journal_date);

CREATE TABLE sync_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE app_setting (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE TABLE mirror_dirty (
  chat_id TEXT NOT NULL,
  journal_date TEXT NOT NULL,
  seq INTEGER NOT NULL,
  PRIMARY KEY (chat_id, journal_date)
);
";

/// Миграция 2 (слайс 2): черновики. Локальная несинхронизируемая таблица, строки сообщений и чатов не меняются.
/// Ключ — `capture` (окно захвата) или `chat:<id>` (поле ввода главного окна).
const MIGRATION_2: &str = "
CREATE TABLE draft (
  key TEXT PRIMARY KEY,
  body TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
";

/// Миграции по порядку; номер версии — индекс + 1.
const MIGRATIONS: &[&str] = &[MIGRATION_1, MIGRATION_2];

/// Версия схемы, которую понимает это приложение.
pub const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

/// База создана более новым приложением: откат после миграции не поддерживается (ТЗ §10.4).
#[derive(Debug)]
pub enum MigrateError {
    TooNew { found: i64 },
    Sqlite(rusqlite::Error),
}

impl From<rusqlite::Error> for MigrateError {
    fn from(error: rusqlite::Error) -> Self {
        MigrateError::Sqlite(error)
    }
}

/// Применяет недостающие миграции, каждую в своей транзакции вместе с `user_version`.
pub fn migrate(conn: &mut Connection) -> Result<(), MigrateError> {
    let current: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > LATEST_VERSION {
        return Err(MigrateError::TooNew { found: current });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
