//! Attaching to the input field (PRD §5): `attach_path` (the dialog and a drop), `attach_bytes` (a paste), removing a tile.
//! Stage 1 (copy, hash, header) runs on the caller's thread; stage 2 is one writer job: rename into `attachments/` and insert the
//! `draft_attachment` row. The sweep is a writer job too, so it cannot see the file without its row.

use super::copy::{Hints, Source, Staged, stage};
use super::{Dirs, delete_stored_files};
use crate::draft::chat_key;
use crate::error::{read_err, write_err};
use crate::{Now, Store, StoreError};
use kinige_domain::attachment::{AttachmentState, AttachmentView, MAX_PASTE_BYTES, MAX_PER_ENTRY};
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::fs;
use std::path::Path;

impl Store {
    /// Attaches a file by path to the chat's draft (the paperclip dialog, a drop). Nothing is written on an error.
    pub fn attach_path(
        &self,
        chat_id: &str,
        path: &Path,
        now: &Now,
    ) -> Result<AttachmentView, StoreError> {
        self.attach_source(
            chat_id,
            Source::Path(path),
            &Hints {
                name: None,
                mime: None,
            },
            now,
        )
    }

    /// Attaches pasted bytes (an image from the clipboard). `name_hint` is only a hint and is cleaned; with no name the file is
    /// called `pasted-HHMMSS.<ext>`. More than 64 MiB is `AttachmentTooLarge`.
    pub fn attach_bytes(
        &self,
        chat_id: &str,
        name_hint: Option<&str>,
        bytes: &[u8],
        now: &Now,
    ) -> Result<AttachmentView, StoreError> {
        if bytes.len() > MAX_PASTE_BYTES {
            return Err(StoreError::AttachmentTooLarge);
        }
        let hinted = name_hint.map(str::trim).filter(|name| !name.is_empty());
        let name = match hinted {
            Some(name) => name.to_string(),
            None => format!(
                "pasted-{}",
                crate::time::hhmmss(now.ms, now.utc_offset_secs)
            ),
        };
        self.attach_source(
            chat_id,
            Source::Bytes(bytes),
            &Hints {
                name: Some(&name),
                mime: None,
            },
            now,
        )
    }

    /// Removes a tile: the draft row and the stored file. A tile of another chat or an unknown id is `AttachmentNotFound`.
    pub fn remove_draft_attachment(
        &self,
        chat_id: &str,
        attachment_id: &str,
    ) -> Result<(), StoreError> {
        if !is_uuid_v4(attachment_id) {
            return Err(StoreError::AttachmentNotFound);
        }
        let (key, id) = (chat_key(chat_id), attachment_id.to_string());
        let dirs = self.dirs();
        let file_name = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let file_name: Option<String> = tx
                .query_row(
                    "SELECT file_name FROM draft_attachment WHERE attachment_id = ?1 AND draft_key = ?2",
                    (&id, &key),
                    |row| row.get(0),
                )
                .optional()
                .map_err(write_err)?;
            let file_name = file_name.ok_or(StoreError::AttachmentNotFound)?;
            tx.execute("DELETE FROM draft_attachment WHERE attachment_id = ?1", [&id])
                .map_err(write_err)?;
            crate::draft::delete_empty_draft(&tx, &key).map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(file_name)
        })?;
        delete_stored_files(
            &dirs,
            self.faults(),
            &[(attachment_id.to_string(), Some(file_name))],
        );
        Ok(())
    }

    fn attach_source(
        &self,
        chat_id: &str,
        source: Source<'_>,
        hints: &Hints<'_>,
        now: &Now,
    ) -> Result<AttachmentView, StoreError> {
        let key = chat_key(chat_id);
        // Refuse before copying gigabytes: no such chat, or the tenth tile is already there.
        self.read(|conn| check_room(conn, chat_id, &key))?;
        let dirs = self.dirs();
        let staged = stage(&dirs, self.faults(), source, hints)?;
        let part = staged.part.clone();
        let (view, ctx) = (
            view_of(&staged),
            (chat_id.to_string(), key, now.ms, dirs.clone()),
        );
        let faults = self.inner.faults.clone();
        let result = self.write(move |conn| commit_staged(conn, &faults, &ctx, &staged));
        if result.is_err() {
            let _ = fs::remove_file(&part);
        }
        result.map(|()| view)
    }
}

/// The chat exists and the draft has room for one more tile.
fn check_room(conn: &Connection, chat_id: &str, key: &str) -> Result<(), StoreError> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
            [chat_id],
            |row| row.get(0),
        )
        .map_err(read_err)?;
    if !exists {
        return Err(StoreError::ChatNotFound);
    }
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM draft_attachment WHERE draft_key = ?1",
            [key],
            |row| row.get(0),
        )
        .map_err(read_err)?;
    if usize::try_from(count).unwrap_or(usize::MAX) >= MAX_PER_ENTRY {
        return Err(StoreError::AttachmentLimit);
    }
    Ok(())
}

/// Stage 2, in the writer: the chat and room are checked again (they may have changed during the copy), the file is renamed into
/// place, the row is inserted and committed. A crash after the rename leaves a file without a row: the sweep moves it away.
fn commit_staged(
    conn: &mut Connection,
    faults: &super::Faults,
    (chat_id, key, now_ms, dirs): &(String, String, i64, Dirs),
    staged: &Staged,
) -> Result<(), StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    check_room(&tx, chat_id, key)?;
    let target = dirs
        .stored(&staged.file_name)
        .ok_or_else(|| StoreError::Internal("bad stored file name".into()))?;
    super::make_dir(&dirs.attachments()).map_err(super::copy::io_error)?;
    fs::rename(&staged.part, &target).map_err(super::copy::io_error)?;
    let inserted = (|| {
        faults.run_after_rename();
        tx.execute(
            "INSERT INTO draft_attachment (attachment_id, draft_key, sort_index, kind, filename, mime, byte_size, width, \
             height, sha256, file_name, created_at) \
             VALUES (?1, ?2, (SELECT COALESCE(MAX(sort_index) + 1, 0) FROM draft_attachment WHERE draft_key = ?3), \
             ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            rusqlite::params![
                staged.id,
                key,
                key,
                staged.kind.as_str(),
                staged.filename,
                staged.mime,
                i64::try_from(staged.byte_size).unwrap_or(i64::MAX),
                staged.width,
                staged.height,
                staged.sha256,
                staged.file_name,
                now_ms
            ],
        )
        .map_err(write_err)?;
        tx.commit().map_err(write_err)
    })();
    if inserted.is_err() {
        // The row did not make it: the renamed file belongs to nobody.
        let _ = fs::remove_file(&target);
    } else {
        super::sync_dir(&dirs.attachments());
    }
    inserted
}

fn view_of(staged: &Staged) -> AttachmentView {
    AttachmentView {
        id: staged.id.clone(),
        kind: staged.kind,
        filename: staged.filename.clone(),
        mime: staged.mime.clone(),
        byte_size: i64::try_from(staged.byte_size).unwrap_or(i64::MAX),
        width: staged.width,
        height: staged.height,
        duration_ms: None,
        state: AttachmentState::Local,
        waveform: None,
    }
}
