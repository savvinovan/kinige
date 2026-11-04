//! SQLite schema and migrations (`PRAGMA user_version`). Migration 8 (slice 7a) adds the local waveform column of a voice note; migration 7 (slice 7) adds attachments; migration 6 (slice 5) adds a reply to the draft; migration 5 (slice 4a) adds the agent access log; migration 4 (slice 4) adds the index of external import ids and the search
//! index; migration 3 (slice 3) adds revisions, tombstones and derived local tables; migration 2 adds the local table
//! of drafts (slice 2).
//! Migration 1 creates only the tables that slice 1 writes to
//! (slice 1, "Touches sensitive areas"): the other tables of ADR-0001 §C will be added by their slices as migrations
//! with `CREATE TABLE` and no change to existing rows. The columns `hlc_json`, `server_known`, `groups_raw` and
//! `schema_version` exist from the first write: they cannot be fixed retroactively.

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

/// Migration 2 (slice 2): drafts. A local table that is not synced; message and chat rows do not change.
/// The key is `capture` (the capture window) or `chat:<id>` (the input field of the main window).
const MIGRATION_2: &str = "
CREATE TABLE draft (
  key TEXT PRIMARY KEY,
  body TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);
";

/// Migration 3 (slice 3): `CREATE TABLE` with no change to rows of synced tables (ADR-0001, appendix §C).
/// - `message_revision` -- synced, append-only; `hlc_json` is only `core` (a revision is an entry with one group);
/// - `tombstone(entity, id, purged_of, purged_at, cascade_of, held)` -- traces of physical deletion (trash, 30 days): they
///   live until the first binding to an account and for at least 180 days; `purged_of` is a snapshot of the HLC of all
///   groups; for children `cascade_of` = the parent id, `held` = NULL;
/// - `message_tag` -- a local derivative of `body_md` (the `#tag` filter), recomputed in this same migration;
/// - `mirror_chat` -- in which Markdown copy folder the chat's files lie now (renaming a chat moves the folder);
/// - a partial index of the trash.
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

/// Migration 4 (slice 4): `CREATE TABLE` with no change to rows of synced tables (ADR-0001, appendix §C.3).
/// - `message_ext(src, ext_chat, ext_msg, message_id)` -- a local, non-synced index of external import ids, derived from
///   `message.ext_json`; deduplication of a repeated import;
/// - `search_fts` -- FTS5 without storing the text (`contentless_delete`) over word stems, `search_map` -- `rowid →
///   message_id` (ADR-0008); a local derivative of `message.body_md`.
///
/// Both tables are filled by `after_migration(4)` with Rust code and SQL from existing entries. The column
/// `message.body_search` from the PRD §4.2 draft does not exist: a derivative does not change rows of a synced table.
pub(crate) const MIGRATION_4: &str = "
CREATE TABLE message_ext (
  src TEXT NOT NULL,
  ext_chat TEXT NOT NULL,
  ext_msg TEXT NOT NULL,
  message_id TEXT NOT NULL,
  PRIMARY KEY (src, ext_chat, ext_msg)
) WITHOUT ROWID;
CREATE INDEX message_ext_message ON message_ext(message_id);

CREATE TABLE search_map (
  rowid INTEGER PRIMARY KEY,
  message_id TEXT NOT NULL UNIQUE
);
CREATE VIRTUAL TABLE search_fts USING fts5(
  t, content='', contentless_delete=1, tokenize='unicode61 remove_diacritics 0'
);
";

/// Migration 5 (slice 4a): the log of AI agent accesses via MCP (ADR-0009, appendix ADR-0001 §C.4). Local, non-synced
/// tables; rows of synced tables do not change.
/// - `mcp_log` -- one row per access (a tool call, connect, disconnect): the time, the client name as the client states it,
///   the tool, `args_json` (only an allowlist of fields, **without entry text**) and the outcome;
/// - `entry_count` -- how many entries the access touched; written in the same transaction as the row and **does not
///   change** after a `purge` of an entry: the access history is not rewritten by deletion (the window shows the number
///   from here);
/// - `mcp_log_message` -- ids of the entries the access touched; a `purge` of an entry and log rotation delete these rows
///   in the same transaction, no orphans are left.
pub(crate) const MIGRATION_5: &str = "
CREATE TABLE mcp_log (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  at INTEGER NOT NULL,
  client TEXT NOT NULL,
  tool TEXT NOT NULL,
  args_json TEXT NOT NULL,
  outcome TEXT NOT NULL,
  entry_count INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE mcp_log_message (
  log_id INTEGER NOT NULL,
  message_id TEXT NOT NULL,
  PRIMARY KEY (log_id, message_id)
) WITHOUT ROWID;
CREATE INDEX mcp_log_message_message ON mcp_log_message(message_id);
";

/// Migration 6 (slice 5, appendix ADR-0001 §C.5): the entry being replied to is added to the local table of drafts (the
/// "Reply to entry" row above the input field). The table is not synced, rows of synced tables do not change; existing
/// drafts keep the value `NULL`.
pub(crate) const MIGRATION_6: &str = "
ALTER TABLE draft ADD COLUMN reply_to_id TEXT;
";

/// Migration 7 (slice 7, appendix ADR-0001 §C.6): attachments. `CREATE TABLE` only; rows of synced tables and existing drafts do
/// not change.
/// - `attachment` -- the first synced attachment table (groups `core`, `order`, `deleted`), no foreign key;
/// - `attachment_local` -- the state of the file on this device (`local` or `missing` until slice 11) and the stored file name
///   `<uuid>.<ext>` inside `attachments/`; not synced;
/// - `draft_attachment` -- the tiles above the input field, with the facts the send needs; the id is the future attachment id
///   and the file already lies in `attachments/`; not synced;
/// - `mirror_file` -- what the Markdown copy wrote into a day folder, so that only our own stale files are ever removed.
pub(crate) const MIGRATION_7: &str = "
CREATE TABLE attachment (
  id TEXT PRIMARY KEY,
  message_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  sort_index INTEGER NOT NULL,
  sha256 TEXT NOT NULL,
  filename TEXT NOT NULL,
  mime TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  width INTEGER,
  height INTEGER,
  duration_ms INTEGER,
  url TEXT,
  link_title TEXT,
  link_site TEXT,
  remote_blob_id TEXT NOT NULL,
  blob_key TEXT NOT NULL,
  deleted_at INTEGER,
  hlc_json TEXT NOT NULL,
  server_known INTEGER NOT NULL DEFAULT 0,
  schema_version INTEGER NOT NULL,
  groups_raw TEXT NOT NULL
);
CREATE INDEX attachment_message ON attachment(message_id, sort_index);

CREATE TABLE attachment_local (
  attachment_id TEXT PRIMARY KEY,
  state TEXT NOT NULL CHECK (state IN ('local', 'missing')),
  file_name TEXT NOT NULL
);

CREATE TABLE draft_attachment (
  attachment_id TEXT PRIMARY KEY,
  draft_key TEXT NOT NULL,
  sort_index INTEGER NOT NULL,
  kind TEXT NOT NULL,
  filename TEXT NOT NULL,
  mime TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  width INTEGER,
  height INTEGER,
  sha256 TEXT NOT NULL,
  file_name TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX draft_attachment_key ON draft_attachment(draft_key, sort_index);

CREATE TABLE mirror_file (
  chat_id TEXT NOT NULL,
  journal_date TEXT NOT NULL,
  attachment_id TEXT NOT NULL,
  name TEXT NOT NULL,
  PRIMARY KEY (chat_id, journal_date, attachment_id)
) WITHOUT ROWID;
";

/// Migration 8 (slice 7a, ADR-0015): the waveform of a voice note. One nullable column on the device-local `attachment_local`; no synced
/// table changes, so `core` and every synced row stay byte-identical (ADR-0001 appendix §C.6).
/// - `NULL` -- not known yet: the player decodes the file once and saves the peaks;
/// - an empty blob -- the decoder could not read the file, do not try again;
/// - otherwise exactly `WAVEFORM_PEAKS` bytes (0 to 255, the loudest part of the note is 255).
///
/// Existing rows keep `NULL`. `ALTER TABLE .. ADD COLUMN` with no default is a metadata change in SQLite: no rewrite of the table.
pub(crate) const MIGRATION_8: &str = "
ALTER TABLE attachment_local ADD COLUMN waveform BLOB;
";

/// Migrations in order; the version number is the index + 1.
const MIGRATIONS: &[&str] = &[
    MIGRATION_1,
    MIGRATION_2,
    MIGRATION_3,
    MIGRATION_4,
    MIGRATION_5,
    MIGRATION_6,
    MIGRATION_7,
    MIGRATION_8,
];

/// The schema version this app understands.
pub const LATEST_VERSION: i64 = MIGRATIONS.len() as i64;

/// Data a migration computes with Rust code, not SQL: tags from `body_md` (the parsing rules are in `kinige_domain::tags`).
fn after_migration(tx: &rusqlite::Transaction<'_>, version: i64) -> rusqlite::Result<()> {
    if version == 3 {
        crate::tags::rebuild_all(tx)?;
    }
    if version == 4 {
        crate::ext::rebuild_all(tx)?;
        crate::search::rebuild_all(tx)?;
    }
    Ok(())
}

/// The database was created by a newer app: rolling back after a migration is not supported (PRD §10.4).
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

/// Applies the missing migrations, each in its own transaction together with `user_version`.
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
