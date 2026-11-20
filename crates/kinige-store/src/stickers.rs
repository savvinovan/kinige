//! The sticker library (slice 5c, PRD §4.3, appendix ADR-0001 §C.10): local tables and files, never synced. Packs and their stickers live in
//! `sticker_pack` and `sticker` (migration 11), the files in `<data>/stickers/<id>.<ext>`, the recent stickers in `app_setting.recent_stickers`.
//! This file reads the library, builds the derived columns of the `sticker` group (`attachment.sticker`, `sticker_emoji`) with the one
//! domain function and rebuilds them when `app_setting.sticker_rules_version` differs. Writing a pack is [`pack`], sending a sticker is
//! [`send`], the GIFs of the journal are [`gif`], the sweep is [`sweep`].

mod gif;
mod pack;
mod probe;
mod send;
mod sweep;

pub(crate) use pack::add_to_export_pack;
pub use pack::{NewPack, PackItem, PackStage};
pub(crate) use probe::{StickerProbe, probe_sticker};
pub(crate) use send::is_sticker_entry;
pub use sweep::StickerSweep;

use crate::attachment::{Dirs, Faults, Media};
use crate::error::{read_err, write_err};
use crate::settings::get_setting;
use crate::{Store, StoreError};
use kinige_domain::attachment::AttachmentKind;
use kinige_domain::attachment::sticker::{RULES_VERSION, StickerCore, clean_emoji, read_sticker};
use kinige_domain::hlc::is_uuid_v4;
use kinige_domain::stickers::{
    MAX_RECENT, MAX_STICKER_BYTES, PackSource, StickerLibrary, StickerPackView, StickerView,
};
use rusqlite::types::ValueRef;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// `app_setting`: the rules version the derived columns were built with.
const VERSION_KEY: &str = "sticker_rules_version";
/// `app_setting`: the stickers sent from this device, the most recent first (a JSON array of library ids, at most [`MAX_RECENT`]).
const RECENT_KEY: &str = "recent_stickers";
/// `app_setting`: "1" when a bot token is saved in the OS secret store. A flag only; the token is never in the database.
const TOKEN_FLAG_KEY: &str = "telegram_token_saved";

/// Every derived column is rebuilt from the groups, and the rules version is stored.
pub(crate) fn rebuild_all(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE attachment SET sticker = 0, sticker_emoji = NULL WHERE sticker != 0 OR sticker_emoji IS NOT NULL",
        [],
    )?;
    let found: Vec<Candidate> = {
        let mut statement = conn.prepare(
            "SELECT id, kind, mime, width, height, byte_size, groups_raw, hlc_json FROM attachment \
             WHERE instr(groups_raw, '\"sticker\"') > 0",
        )?;
        let rows = statement.query_map([], |row| Ok(Candidate::read(row)))?;
        // A row whose cells do not fit the types is a picture, never an error: it must not stop the database from opening.
        rows.filter_map(|row| row.transpose()).collect::<Result<_, _>>()?
    };
    for row in found {
        let Some(kind) = AttachmentKind::parse(&row.kind) else {
            continue;
        };
        let core = StickerCore {
            kind,
            mime: &row.mime,
            width: row.width,
            height: row.height,
            byte_size: row.byte_size,
        };
        let read = read_sticker(&core, &row.groups_raw, &row.hlc_json);
        if read.sticker {
            conn.execute(
                "UPDATE attachment SET sticker = 1, sticker_emoji = ?2 WHERE id = ?1",
                (&row.id, &read.emoji),
            )?;
        }
    }
    conn.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (VERSION_KEY, RULES_VERSION.to_string()),
    )?;
    Ok(())
}

/// An attachment row as the rebuild reads it: the columns of the core and the two maps.
struct Candidate {
    id: String,
    kind: String,
    mime: String,
    width: Option<u32>,
    height: Option<u32>,
    byte_size: i64,
    groups_raw: String,
    hlc_json: String,
}

impl Candidate {
    /// Reads the cells without trusting their types (SQLite lets any cell hold anything): `None` when one does not fit, which is a picture.
    /// A side that is negative or does not fit in 32 bits is `None` and fails the core's check, like a missing one.
    fn read(row: &rusqlite::Row<'_>) -> Option<Self> {
        let text = |index: usize| match row.get_ref(index) {
            Ok(ValueRef::Text(bytes)) => std::str::from_utf8(bytes).ok().map(str::to_owned),
            _ => None,
        };
        let number = |index: usize| match row.get_ref(index) {
            Ok(ValueRef::Integer(value)) => Some(value),
            _ => None,
        };
        let side = |index: usize| number(index).and_then(|value| u32::try_from(value).ok());
        Some(Self {
            id: text(0)?,
            kind: text(1)?,
            mime: text(2)?,
            width: side(3),
            height: side(4),
            byte_size: number(5)?,
            groups_raw: text(6)?,
            hlc_json: text(7)?,
        })
    }
}

/// On opening the database: the rules version stored differs from the app's, so the columns are rebuilt in one transaction.
pub(crate) fn rebuild_if_stale(conn: &mut Connection) -> Result<(), StoreError> {
    let stored = get_setting(conn, VERSION_KEY).map_err(write_err)?;
    if stored.as_deref() == Some(RULES_VERSION.to_string().as_str()) {
        return Ok(());
    }
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    rebuild_all(&tx).map_err(write_err)?;
    tx.commit().map_err(write_err)
}

impl Store {
    /// What the Stickers tab shows. A sticker whose file is gone is left out (a row without its file is hidden), and so is a pack that
    /// has none left. `has_token` is the flag the caller reads from `app_setting`; this function never touches a secret.
    pub fn sticker_library(&self, has_token: bool) -> Result<StickerLibrary, StoreError> {
        let dir = self.dirs().stickers();
        self.read(|conn| library(conn, &dir, has_token))
    }

    /// The file of a library sticker and its MIME type, for the `sticker/<id>` path of the protocol: `None` for anything that is not a
    /// UUID of a sticker whose file is there.
    pub fn sticker_file(&self, id: &str) -> Result<Option<(PathBuf, &'static str)>, StoreError> {
        if !is_uuid_v4(id) {
            return Ok(None);
        }
        let dirs = self.dirs();
        self.read(|conn| {
            let ext: Option<String> = conn
                .query_row("SELECT ext FROM sticker WHERE id = ?1", [id], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(read_err)?;
            Ok(ext.and_then(|ext| {
                let mime = match ext.as_str() {
                    "webp" => "image/webp",
                    "png" => "image/png",
                    _ => return None,
                };
                let path = dirs.library_file(&format!("{id}.{ext}"))?;
                path.is_file().then_some((path, mime))
            }))
        })
    }

    /// [`Store::sticker_file`] with the bytes, on the media pool, for the `sticker/<id>` path of the protocol. `None` is 404: a bad id, no row,
    /// no file, a file over 2 MiB. The probe is not repeated here (the picture is only displayed, and a file is probed at send time).
    pub fn sticker_media_async(
        &self,
        id: String,
        done: impl FnOnce(Option<Media>) + Send + 'static,
    ) {
        let store = self.clone();
        let job: Box<dyn FnOnce() + Send> = Box::new(move || {
            let media = store
                .sticker_file(&id)
                .ok()
                .flatten()
                .and_then(|(path, content_type)| {
                    let meta = std::fs::metadata(&path).ok()?;
                    if meta.len() > MAX_STICKER_BYTES {
                        return None;
                    }
                    let bytes = std::fs::read(path).ok()?;
                    Some(Media {
                        content_type,
                        bytes,
                    })
                });
            done(media);
        });
        if !self.inner.media_pool.submit(job) {
            log::error!("media: the pool has stopped");
        }
    }

    /// Whether the bot token is said to be saved: a plain flag in `app_setting` that can disagree with the OS secret store (a restored
    /// database, a removed item); the pack job corrects it when it reads the store. Never the token itself.
    pub fn telegram_token_saved(&self) -> Result<bool, StoreError> {
        self.read(|conn| {
            Ok(get_setting(conn, TOKEN_FLAG_KEY)
                .map_err(read_err)?
                .as_deref()
                == Some("1"))
        })
    }

    /// Sets the flag of [`Store::telegram_token_saved`].
    pub fn set_telegram_token_saved(&self, saved: bool) -> Result<(), StoreError> {
        self.write(move |conn| {
            conn.execute(
                "INSERT INTO app_setting (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (TOKEN_FLAG_KEY, if saved { "1" } else { "0" }),
            )
            .map_err(write_err)?;
            Ok(())
        })
    }

    /// "Remove pack": the pack, its stickers and their files go. The stickers already sent stay in their entries (each sent sticker is an
    /// attachment with a file of its own). Recent stickers of the pack drop out of the strip when it is read.
    pub fn remove_sticker_pack(&self, pack_id: &str) -> Result<(), StoreError> {
        if !is_uuid_v4(pack_id) {
            return Err(StoreError::StickerNotFound);
        }
        let (dirs, faults, pack_id) = (self.dirs(), self.inner.faults.clone(), pack_id.to_string());
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let files = pack_files(&tx, &pack_id)?;
            let removed = tx
                .execute("DELETE FROM sticker_pack WHERE id = ?1", [&pack_id])
                .map_err(write_err)?;
            if removed == 0 {
                return Err(StoreError::StickerNotFound);
            }
            tx.execute("DELETE FROM sticker WHERE pack_id = ?1", [&pack_id])
                .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            delete_library_files(&dirs, &faults, &files);
            Ok(())
        })
    }
}

/// `(id, ext)` of the stickers of a pack.
fn pack_files(conn: &Connection, pack_id: &str) -> Result<Vec<(String, String)>, StoreError> {
    let mut statement = conn
        .prepare("SELECT id, ext FROM sticker WHERE pack_id = ?1")
        .map_err(read_err)?;
    let rows = statement
        .query_map([pack_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(read_err)?;
    rows.collect::<Result<_, _>>().map_err(read_err)
}

/// Deletes library files after the commit; what could not be deleted is finished by the sweep (a file without a row).
fn delete_library_files(dirs: &Dirs, faults: &Faults, files: &[(String, String)]) {
    if faults.delete_fails() {
        return;
    }
    for (id, ext) in files {
        if let Some(path) = dirs.library_file(&format!("{id}.{ext}")) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn library(conn: &Connection, dir: &Path, has_token: bool) -> Result<StickerLibrary, StoreError> {
    let mut statement = conn
        .prepare(
            "SELECT s.id, s.pack_id, s.emoji, s.ext, s.width, s.height FROM sticker s \
             JOIN sticker_pack p ON p.id = s.pack_id ORDER BY s.pack_id, s.position, s.id",
        )
        .map_err(read_err)?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, u32>(4)?,
                row.get::<_, u32>(5)?,
            ))
        })
        .map_err(read_err)?;
    let mut by_pack: HashMap<String, Vec<StickerView>> = HashMap::new();
    let mut by_id: HashMap<String, StickerView> = HashMap::new();
    for row in rows {
        let (id, pack_id, emoji, ext, width, height) = row.map_err(read_err)?;
        if !dir.join(format!("{id}.{ext}")).is_file() {
            continue;
        }
        let view = StickerView {
            id: id.clone(),
            emoji: emoji.filter(|text| clean_emoji(text).is_some()),
            width,
            height,
        };
        by_id.insert(id, view.clone());
        by_pack.entry(pack_id).or_default().push(view);
    }
    let mut statement = conn
        .prepare(
            "SELECT id, source, title FROM sticker_pack \
             ORDER BY (source = 'telegram_export') DESC, position, id",
        )
        .map_err(read_err)?;
    let packs = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(read_err)?;
    let mut shown = Vec::new();
    for pack in packs {
        let (id, source, title) = pack.map_err(read_err)?;
        let (Some(source), Some(stickers)) = (PackSource::parse(&source), by_pack.remove(&id))
        else {
            continue;
        };
        shown.push(StickerPackView {
            id,
            source,
            title,
            stickers,
        });
    }
    let recent = recent_ids(get_setting(conn, RECENT_KEY).map_err(read_err)?.as_deref())
        .into_iter()
        .filter_map(|id| by_id.get(&id).cloned())
        .collect();
    Ok(StickerLibrary {
        recent,
        packs: shown,
        has_token,
    })
}

/// The ids of a stored recent list: a broken value is empty, and only UUIDs count.
fn recent_ids(stored: Option<&str>) -> Vec<String> {
    let parsed: Option<Vec<String>> = stored.and_then(|text| serde_json::from_str(text).ok());
    parsed
        .unwrap_or_default()
        .into_iter()
        .filter(|id| is_uuid_v4(id))
        .take(MAX_RECENT)
        .collect()
}

/// This sticker goes first in the recent list, inside the transaction of the send.
pub(crate) fn remember(tx: &rusqlite::Transaction<'_>, sticker_id: &str) -> Result<(), StoreError> {
    let stored = get_setting(tx, RECENT_KEY).map_err(read_err)?;
    let mut list = vec![sticker_id.to_string()];
    list.extend(
        recent_ids(stored.as_deref())
            .into_iter()
            .filter(|id| id != sticker_id),
    );
    list.truncate(MAX_RECENT);
    let text = serde_json::to_string(&list).map_err(|e| StoreError::Internal(e.to_string()))?;
    tx.execute(
        "INSERT INTO app_setting (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        (RECENT_KEY, text),
    )
    .map_err(write_err)?;
    Ok(())
}
