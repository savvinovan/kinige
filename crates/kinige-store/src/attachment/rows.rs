//! Attachment rows: reading them into views, turning draft attachments into attachments in the send transaction, and the
//! tombstones of a purge (ADR-0001 appendix §C.6, ADR-0014).

use super::Dirs;
use crate::StoreError;
use crate::error::{read_err, write_err};
use crate::hlc_state::next_hlc;
use kinige_domain::attachment::{
    AttachmentKind, AttachmentRow, AttachmentState, AttachmentSummary, AttachmentView,
    MAX_PER_ENTRY, NewAttachment, hex_lower, new_attachment, summary_of,
};
use kinige_domain::history::{deleted_group_supported, purged_of_best_effort};
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::{Connection, OptionalExtension, Transaction};
use serde_json::Value;
use uuid::Uuid;

/// The live attachments of the entry `m` as one JSON array, a column of `view::VIEW_COLUMNS`. `LEFT JOIN`: a row without a local
/// row counts as `missing`. The order is restored in Rust (`o`), because the aggregate does not promise one.
macro_rules! attachments_column {
    () => {
        "(SELECT json_group_array(json_object('i', a.id, 'k', a.kind, 'n', a.filename, \
         'm', a.mime, 'b', a.byte_size, 'w', a.width, 'h', a.height, 'd', a.duration_ms, \
         's', COALESCE(l.state, 'missing'), 'o', a.sort_index)) \
         FROM attachment a LEFT JOIN attachment_local l ON l.attachment_id = a.id \
         WHERE a.message_id = m.id AND a.deleted_at IS NULL)"
    };
}
pub(crate) use attachments_column;

/// The array of [`ATTACHMENTS_COLUMN`] as views in order. A damaged element is skipped: the entry still opens.
pub(crate) fn parse_attachments(json: Option<String>) -> Vec<AttachmentView> {
    let Some(Value::Array(items)) = json.and_then(|text| serde_json::from_str(&text).ok()) else {
        return Vec::new();
    };
    let mut views: Vec<(i64, AttachmentView)> = items.iter().filter_map(parse_one).collect();
    views.sort_by_key(|(order, _)| *order);
    views.into_iter().map(|(_, view)| view).collect()
}

fn parse_one(item: &Value) -> Option<(i64, AttachmentView)> {
    let text = |key: &str| item.get(key)?.as_str().map(str::to_string);
    let number = |key: &str| item.get(key).and_then(Value::as_i64);
    let side = |key: &str| number(key).and_then(|n| u32::try_from(n).ok());
    let state = match item.get("s")?.as_str()? {
        "local" => AttachmentState::Local,
        _ => AttachmentState::Missing,
    };
    Some((
        number("o")?,
        AttachmentView {
            id: text("i")?,
            kind: AttachmentKind::parse(&text("k")?)?,
            filename: text("n")?,
            mime: text("m")?,
            byte_size: number("b")?,
            width: side("w"),
            height: side("h"),
            duration_ms: number("d"),
            state,
        },
    ))
}

/// A file ready to become an attachment: it already lies in `attachments/` under `file_name`.
pub(crate) struct NewFile {
    pub id: String,
    pub file_name: String,
    pub sha256: String,
    pub byte_size: i64,
    pub filename: String,
    pub kind: AttachmentKind,
    pub mime: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<i64>,
}

/// Writes the `attachment` and `attachment_local` rows of one file under an HLC of its own, with a fresh `remote_blob_id` (UUID
/// v7) and `blob_key` (32 bytes from the OS CSPRNG), both made here, in the transaction of the send or the import (ADR-0001 §7).
pub(crate) fn insert_attachment(
    tx: &Transaction<'_>,
    message_id: &str,
    sort_index: u32,
    file: &NewFile,
    now_ms: i64,
) -> Result<AttachmentView, StoreError> {
    let hlc = next_hlc(tx, now_ms)?;
    insert_attachment_at(tx, &hlc, message_id, sort_index, file)
}

/// The same under an HLC the caller made: a bulk import keeps its clock in memory and saves it once.
pub(crate) fn insert_attachment_at(
    tx: &Transaction<'_>,
    hlc: &str,
    message_id: &str,
    sort_index: u32,
    file: &NewFile,
) -> Result<AttachmentView, StoreError> {
    let mut key = [0u8; 32];
    getrandom::fill(&mut key)
        .map_err(|_| StoreError::Internal("the system random source failed".into()))?;
    let blob_key = hex_lower(&key);
    let remote_blob_id = Uuid::now_v7().to_string();
    let row = new_attachment(
        &NewAttachment {
            id: &file.id,
            message_id,
            kind: file.kind,
            sort_index,
            sha256: &file.sha256,
            filename: &file.filename,
            mime: &file.mime,
            byte_size: file.byte_size,
            width: file.width,
            height: file.height,
            duration_ms: file.duration_ms,
            remote_blob_id: &remote_blob_id,
            blob_key: &blob_key,
        },
        hlc,
    )?;
    write_row(tx, &row)?;
    tx.execute(
        "INSERT INTO attachment_local (attachment_id, state, file_name) VALUES (?1, 'local', ?2)",
        (&file.id, &file.file_name),
    )
    .map_err(write_err)?;
    Ok(AttachmentView {
        id: row.id,
        kind: row.kind,
        filename: row.filename,
        mime: row.mime,
        byte_size: row.byte_size,
        width: row.width,
        height: row.height,
        duration_ms: row.duration_ms,
        state: AttachmentState::Local,
    })
}

fn write_row(tx: &Transaction<'_>, row: &AttachmentRow) -> Result<(), StoreError> {
    tx.execute(
        "INSERT INTO attachment (id, message_id, kind, sort_index, sha256, filename, mime, byte_size, width, height, \
         duration_ms, remote_blob_id, blob_key, hlc_json, server_known, schema_version, groups_raw) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 0, ?15, ?16)",
        rusqlite::params![
            row.id,
            row.message_id,
            row.kind.as_str(),
            row.sort_index,
            row.sha256,
            row.filename,
            row.mime,
            row.byte_size,
            row.width,
            row.height,
            row.duration_ms,
            row.remote_blob_id,
            row.blob_key,
            row.hlc_json,
            row.schema_version,
            row.groups_raw
        ],
    )
    .map_err(write_err)?;
    Ok(())
}

/// The draft attachments of a draft key as views, in order.
pub(crate) fn draft_views(conn: &Connection, key: &str) -> Result<Vec<AttachmentView>, StoreError> {
    let mut statement = conn
        .prepare_cached(
            "SELECT attachment_id, kind, filename, mime, byte_size, width, height FROM draft_attachment \
             WHERE draft_key = ?1 ORDER BY sort_index, created_at, attachment_id",
        )
        .map_err(read_err)?;
    let rows = statement
        .query_map([key], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, Option<u32>>(5)?,
                row.get::<_, Option<u32>>(6)?,
            ))
        })
        .map_err(read_err)?;
    let mut views = Vec::new();
    for found in rows {
        let (id, kind, filename, mime, byte_size, width, height) = found.map_err(read_err)?;
        let Some(kind) = AttachmentKind::parse(&kind) else {
            continue;
        };
        views.push(AttachmentView {
            id,
            kind,
            filename,
            mime,
            byte_size,
            width,
            height,
            duration_ms: None,
            state: AttachmentState::Local,
        });
    }
    Ok(views)
}

/// Turns the draft attachments `ids` of the draft `key` into attachments of the entry `message_id`, in the order given. Only the
/// draft's own attachments, each once, at most ten, files present: otherwise nothing is written (the caller rolls back).
pub(crate) fn promote_draft(
    tx: &Transaction<'_>,
    dirs: &Dirs,
    key: &str,
    ids: &[String],
    message_id: &str,
    now_ms: i64,
) -> Result<Vec<AttachmentView>, StoreError> {
    if ids.len() > MAX_PER_ENTRY {
        return Err(StoreError::AttachmentLimit);
    }
    let mut seen: Vec<&str> = Vec::with_capacity(ids.len());
    let mut files = Vec::with_capacity(ids.len());
    for id in ids {
        if !is_uuid_v4(id) || seen.contains(&id.as_str()) {
            return Err(StoreError::AttachmentNotFound);
        }
        seen.push(id);
        files.push(load_draft_file(tx, key, id)?);
    }
    let mut views = Vec::with_capacity(files.len());
    for (index, file) in files.iter().enumerate() {
        let on_disk = dirs
            .stored(&file.file_name)
            .is_some_and(|path| path.is_file());
        if !on_disk {
            return Err(StoreError::AttachmentUnreadable);
        }
        let sort_index = u32::try_from(index).unwrap_or(u32::MAX);
        views.push(insert_attachment(tx, message_id, sort_index, file, now_ms)?);
        tx.execute(
            "DELETE FROM draft_attachment WHERE attachment_id = ?1",
            [&file.id],
        )
        .map_err(write_err)?;
    }
    Ok(views)
}

fn load_draft_file(tx: &Transaction<'_>, key: &str, id: &str) -> Result<NewFile, StoreError> {
    tx.query_row(
        "SELECT kind, filename, mime, byte_size, width, height, sha256, file_name FROM draft_attachment \
         WHERE attachment_id = ?1 AND draft_key = ?2",
        (id, key),
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, Option<u32>>(4)?,
                row.get::<_, Option<u32>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        },
    )
    .optional()
    .map_err(read_err)?
    .and_then(
        |(kind, filename, mime, byte_size, width, height, sha256, file_name)| {
            Some(NewFile {
                id: id.to_string(),
                file_name,
                sha256,
                byte_size,
                filename,
                kind: AttachmentKind::parse(&kind)?,
                mime,
                width,
                height,
                duration_ms: None,
            })
        },
    )
    .ok_or(StoreError::AttachmentNotFound)
}

/// A file left behind by a purge: the stored file and thumbnails are deleted after the commit.
#[derive(Debug, Clone)]
pub(crate) struct PurgedFile {
    pub id: String,
    pub file_name: Option<String>,
}

/// In the purge transaction of an entry: a tombstone per attachment (`cascade_of` = the entry, a snapshot of the HLCs of
/// all its groups), then the attachment, local and draft rows go. Returns the files to delete after the commit.
pub(crate) fn delete_for_purge(
    tx: &Connection,
    message_id: &str,
    now_ms: i64,
) -> Result<Vec<PurgedFile>, StoreError> {
    let found: Vec<(String, String, Option<String>)> = {
        let mut statement = tx
            .prepare_cached(
                "SELECT a.id, a.hlc_json, l.file_name FROM attachment a \
                 LEFT JOIN attachment_local l ON l.attachment_id = a.id WHERE a.message_id = ?1",
            )
            .map_err(write_err)?;
        let rows = statement
            .query_map([message_id], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(write_err)?;
        rows.collect::<Result<_, _>>().map_err(write_err)?
    };
    // As for the entry itself (ADR-0001 §4 item 4): a purge is created only when every `deleted` group is in a supported `sv`;
    // otherwise a newer device could still restore the row, so the whole entry stays as it is (`ReadOnly`, nothing written).
    if found
        .iter()
        .any(|(_, hlc_json, _)| !deleted_group_supported(hlc_json))
    {
        return Err(StoreError::ReadOnly);
    }
    let mut files = Vec::with_capacity(found.len());
    for (id, hlc_json, file_name) in found {
        tx.execute(
            "INSERT OR REPLACE INTO tombstone (entity, id, purged_of, purged_at, cascade_of, held) \
             VALUES ('attachment', ?1, ?2, ?3, ?4, NULL)",
            (&id, purged_of_best_effort(&hlc_json)?, now_ms, message_id),
        )
        .map_err(write_err)?;
        tx.execute(
            "DELETE FROM attachment_local WHERE attachment_id = ?1",
            [&id],
        )
        .map_err(write_err)?;
        tx.execute(
            "DELETE FROM draft_attachment WHERE attachment_id = ?1",
            [&id],
        )
        .map_err(write_err)?;
        files.push(PurgedFile { id, file_name });
    }
    tx.execute("DELETE FROM attachment WHERE message_id = ?1", [message_id])
        .map_err(write_err)?;
    Ok(files)
}

/// How many live attachments an entry has (the empty-body rule of appendix §C.6).
pub(crate) fn live_count(conn: &Connection, message_id: &str) -> Result<i64, StoreError> {
    conn.query_row(
        "SELECT COUNT(*) FROM attachment WHERE message_id = ?1 AND deleted_at IS NULL",
        [message_id],
        |row| row.get(0),
    )
    .map_err(read_err)
}

/// What an entry's live attachments amount to, for lines that have no room for them (reply quote, "On this day", the forest
/// card, the capture context). Empty summary for an entry without attachments or one that does not exist.
pub(crate) fn summary_for(
    conn: &Connection,
    message_id: &str,
) -> Result<AttachmentSummary, StoreError> {
    let json: Option<Option<String>> = conn
        .query_row(
            concat!(
                "SELECT ",
                attachments_column!(),
                " FROM message m WHERE m.id = ?1"
            ),
            [message_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(read_err)?;
    Ok(summary_of(&parse_attachments(json.flatten())))
}
