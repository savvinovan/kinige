//! Migration 11 (slice 5c, appendix ADR-0001 §C.10): the sticker library and the two derived columns of the `sticker` group. Local tables
//! and `ALTER TABLE .. ADD COLUMN` only (a metadata change in SQLite, no rewrite); no row of a synced table changes, so every row of
//! version 10 keeps its bytes and there is no backfill: an absent group reads as a picture.
//! - `sticker_pack` -- a pack of the library: where it came from, the link name (unique, for a pack added by link), the title and its
//!   place in the strip;
//! - `sticker` -- a sticker of a pack: its place, emoji (only a valid emoji sequence, or none), the hash of the file, the extension, size
//!   and the pixels read from the header. The file is `stickers/<id>.<ext>`; a row without its file is hidden from the panel;
//! - `attachment.sticker`, `attachment.sticker_emoji` -- the output of `kinige_domain::attachment::read_sticker` for the row's groups,
//!   rebuilt on open when `app_setting.sticker_rules_version` differs. The raw bytes stay verbatim in `groups_raw`.
//!
//! The library, the recent stickers (`app_setting.recent_stickers`) and the bot token are local to this device and never synced.

pub(crate) const MIGRATION_11: &str = "
CREATE TABLE sticker_pack (
  id TEXT PRIMARY KEY,
  source TEXT NOT NULL CHECK (source IN ('telegram_export', 'telegram_link')),
  link_name TEXT,
  title TEXT NOT NULL,
  position INTEGER NOT NULL
);
CREATE UNIQUE INDEX sticker_pack_link ON sticker_pack(link_name) WHERE link_name IS NOT NULL;

CREATE TABLE sticker (
  id TEXT PRIMARY KEY,
  pack_id TEXT NOT NULL,
  position INTEGER NOT NULL,
  emoji TEXT,
  sha256 TEXT NOT NULL,
  ext TEXT NOT NULL,
  byte_size INTEGER NOT NULL,
  width INTEGER NOT NULL,
  height INTEGER NOT NULL
);
CREATE INDEX sticker_in_pack ON sticker(pack_id, position);
CREATE UNIQUE INDEX sticker_once_in_pack ON sticker(pack_id, sha256);

ALTER TABLE attachment ADD COLUMN sticker INTEGER NOT NULL DEFAULT 0;
ALTER TABLE attachment ADD COLUMN sticker_emoji TEXT;
";
