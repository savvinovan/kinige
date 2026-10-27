//! Схема SQLite и миграции (`PRAGMA user_version`). Миграция 3 (слайс 3) добавляет ревизии, tombstones и производные
//! локальные таблицы; миграция 2 — локальную таблицу черновиков (слайс 2).
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

/// Миграция 3 (слайс 3): `CREATE TABLE` без изменения строк синхронизируемых таблиц (ADR-0001, приложение §C).
/// - `message_revision` — синхронизируемая, append-only; `hlc_json` только `core` (ревизия — запись с одной группой);
/// - `tombstone(entity, id, purged_of, purged_at, cascade_of, held)` — следы физического удаления (корзина, 30 дней): живут
///   до первой привязки к аккаунту и не меньше 180 дней; `purged_of` — снимок HLC всех групп; у детей `cascade_of` = id
///   родителя, `held` = NULL;
/// - `message_tag` — локальная производная от `body_md` (фильтр по `#тегу`), пересчитывается в этой же миграции;
/// - `mirror_chat` — в какой папке Markdown-копии лежат файлы чата сейчас (переименование чата переносит папку);
/// - частичный индекс корзины.
const MIGRATION_3: &str = "
CREATE TABLE message_revision (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL,
  snapshot_json TEXT NOT NULL,
  saved_at INTEGER NOT NULL,
  reason TEXT NOT NULL,
  hlc_json TEXT NOT NULL,
  server_known INTEGER NOT NULL DEFAULT 0,
  schema_version INTEGER NOT NULL,
  groups_raw TEXT NOT NULL
);
CREATE INDEX message_revision_message ON message_revision(message_id);

CREATE TABLE tombstone (
  entity TEXT NOT NULL,
  id TEXT NOT NULL,
  purged_of TEXT NOT NULL,
  purged_at INTEGER NOT NULL,
  cascade_of TEXT,
  held TEXT,
  PRIMARY KEY (entity, id)
);

CREATE TABLE message_tag (
  message_id TEXT NOT NULL,
  tag TEXT NOT NULL,
  PRIMARY KEY (message_id, tag)
) WITHOUT ROWID;
CREATE INDEX message_tag_tag ON message_tag(tag, message_id);

CREATE TABLE mirror_chat (
  chat_id TEXT PRIMARY KEY,
  folder TEXT NOT NULL
);

CREATE INDEX message_deleted ON message(deleted_at) WHERE deleted_at IS NOT NULL;
";

/// Миграции по порядку; номер версии — индекс + 1.
const MIGRATIONS: &[&str] = &[MIGRATION_1, MIGRATION_2, MIGRATION_3];

/// Версия схемы, которую понимает это приложение.
pub const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

/// Данные, которые миграция считает кодом Rust, а не SQL: теги из `body_md` (правила разбора — `kinige_domain::tags`).
fn after_migration(tx: &rusqlite::Transaction<'_>, version: i64) -> rusqlite::Result<()> {
    if version == 3 {
        crate::tags::rebuild_all(tx)?;
    }
    Ok(())
}

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
        after_migration(&tx, index as i64 + 1)?;
        tx.pragma_update(None, "user_version", index as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
