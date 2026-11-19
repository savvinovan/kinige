//! The draft attachments (the tiles above the input field) as views, and their turning into attachments in the send transaction
//! (ADR-0001 appendix §C.6, §C.8).

use super::Dirs;
use super::rows::{DraftDetails, NewFile, insert_attachment, name_of};
use crate::StoreError;
use crate::error::read_err;
use kinige_domain::attachment::{
    AttachmentKind, AttachmentState, AttachmentView, MAX_PER_ENTRY, normalize_description,
    valid_display_name,
};
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::{Connection, OptionalExtension, Transaction};

/// The draft attachments of a draft key as views, in order.
pub(crate) fn draft_views(conn: &Connection, key: &str) -> Result<Vec<AttachmentView>, StoreError> {
    let mut statement = conn
        .prepare_cached(
            "SELECT attachment_id, kind, filename, mime, byte_size, width, height, display_name, description, spoiler \
             FROM draft_attachment WHERE draft_key = ?1 ORDER BY sort_index, created_at, attachment_id",
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
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
                row.get::<_, i64>(9)?,
            ))
        })
        .map_err(read_err)?;
    let mut views = Vec::new();
    for found in rows {
        let (
            id,
            kind,
            filename,
            mime,
            byte_size,
            width,
            height,
            display_name,
            description,
            spoiler,
        ) = found.map_err(read_err)?;
        let Some(kind) = AttachmentKind::parse(&kind) else {
            continue;
        };
        views.push(AttachmentView {
            id,
            kind,
            name: name_of(&filename, display_name.as_deref()),
            filename,
            description,
            spoiler: spoiler != 0 && kind == AttachmentKind::Image,
            mime,
            byte_size,
            width,
            height,
            duration_ms: None,
            state: AttachmentState::Local,
            waveform: None,
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
        .map_err(crate::error::write_err)?;
    }
    Ok(views)
}

/// One draft row as a file to promote. The promote rule (appendix §C.8): a draft name or description that fails the rules becomes
/// `null` (the row is only ours, written by [`super::details`], so this is a guard against a damaged row, not a normal path); the
/// spoiler is kept, for an image.
fn load_draft_file(tx: &Transaction<'_>, key: &str, id: &str) -> Result<NewFile, StoreError> {
    tx.query_row(
        "SELECT kind, filename, mime, byte_size, width, height, sha256, file_name, display_name, description, spoiler \
         FROM draft_attachment WHERE attachment_id = ?1 AND draft_key = ?2",
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
                row.get::<_, Option<String>>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, i64>(10)?,
            ))
        },
    )
    .optional()
    .map_err(read_err)?
    .and_then(
        |(kind, filename, mime, byte_size, width, height, sha256, file_name, name, text, spoiler)| {
            let kind = AttachmentKind::parse(&kind)?;
            let details = DraftDetails {
                display_name: name.filter(|name| valid_display_name(name, &filename) && *name != filename),
                description: text.filter(|text| {
                    normalize_description(text).is_ok_and(|normal| normal.as_deref() == Some(text))
                }),
                spoiler: spoiler != 0 && kind == AttachmentKind::Image,
            };
            Some(NewFile {
                id: id.to_string(),
                file_name,
                sha256,
                byte_size,
                filename,
                kind,
                mime,
                width,
                height,
                duration_ms: None,
                details,
            })
        },
    )
    .ok_or(StoreError::AttachmentNotFound)
}
