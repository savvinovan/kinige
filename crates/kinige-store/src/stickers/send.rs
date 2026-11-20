//! Sending a sticker and sending a GIF again (slice 5c): one new entry with an empty body and one attachment that is a copy of a file of
//! this device. Stage 1 is the copy pipeline of every attachment (`.tmp/`, hash, `fsync`); stage 2 is one writer job that writes the entry,
//! renames the file, writes the rows, the search index, the copy mark, the recent list and the draft's reply line, and commits. The text
//! draft and the pending tiles are never touched. A crash between the rename and the commit leaves an orphan for the attachment sweep.

use super::probe::probe_sticker;
use super::remember;
use crate::attachment::{
    Dirs, DraftDetails, Faults, Hints, NewFile, Source, Staged, StickerNote, insert_attachment,
    io_error, make_dir, stage, sync_dir,
};
use crate::draft::{chat_key, delete_empty_draft};
use crate::error::{read_err, write_err};
use crate::send::{insert_new_message, insert_reply_message};
use crate::{Now, Store, StoreError};
use kinige_domain::MessageView;
use kinige_domain::attachment::sticker::{FILE_STEM, clean_emoji};
use kinige_domain::attachment::{AttachmentKind, ImageFormat};
use kinige_domain::hlc::is_uuid_v4;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::fs;
use std::path::PathBuf;

impl Store {
    /// A click on a sticker of the library: it is sent at once as its own entry (empty body, one image attachment that reads as a sticker).
    /// The library file is probed again and copied through the pipeline of every attachment; a file that fails the sticker probe refuses
    /// the command (`StickerUnreadable`) and is never sent as a photo. `reply_to_id` is checked in the same transaction and its line leaves
    /// the draft. Errors, nothing written: `StickerNotFound`, `ChatNotFound`, `ReplyTargetNotFound`, `StickerUnreadable`, `DiskFull`.
    pub fn send_sticker(
        &self,
        chat_id: &str,
        sticker_id: &str,
        reply_to_id: Option<&str>,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        if !is_uuid_v4(sticker_id) {
            return Err(StoreError::StickerNotFound);
        }
        self.read(|conn| check_chat(conn, chat_id))?;
        let dirs = self.dirs();
        let (name, emoji) = self.read(|conn| library_row(conn, sticker_id))?;
        let path = dirs
            .library_file(&name)
            .filter(|path| path.is_file())
            .ok_or(StoreError::StickerNotFound)?;
        // The probe of the library file, then of the copy: the file may change in between, and the entry must hold what was probed.
        probe_sticker(&path).ok_or(StoreError::StickerUnreadable)?;
        let ext = name.split_once('.').map_or("webp", |(_, ext)| ext);
        let filename = format!("{FILE_STEM}.{ext}");
        let staged = stage(
            &dirs,
            self.faults(),
            Source::Path(&path),
            &Hints {
                name: Some(&filename),
                mime: None,
            },
        )?;
        let fits = probe_sticker(&staged.part).is_some() && staged.kind == AttachmentKind::Image;
        if !fits {
            let _ = fs::remove_file(&staged.part);
            return Err(StoreError::StickerUnreadable);
        }
        let note = StickerNote {
            emoji: emoji.filter(|text| clean_emoji(text).is_some()),
        };
        self.send_copy(
            chat_id,
            reply_to_id,
            staged,
            Some(note),
            Some(sticker_id),
            now,
        )
    }

    /// "Send again" from the GIFs tab: the same as a new entry with a copy of the GIF of a live entry. The copy gets a fresh blob id and
    /// key, and the details of the source (a name, a description, a cover) are not copied. A covered, trashed or missing source is
    /// refused (`AttachmentNotFound`), and so is one that is not a GIF.
    pub fn send_gif_again(
        &self,
        chat_id: &str,
        attachment_id: &str,
        reply_to_id: Option<&str>,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        if !is_uuid_v4(attachment_id) {
            return Err(StoreError::AttachmentNotFound);
        }
        self.read(|conn| check_chat(conn, chat_id))?;
        let dirs = self.dirs();
        let (filename, file_name) = self.read(|conn| gif_source(conn, attachment_id))?;
        let path = dirs
            .stored(&file_name)
            .filter(|path| path.is_file())
            .ok_or(StoreError::AttachmentNotFound)?;
        let staged = stage(
            &dirs,
            self.faults(),
            Source::Path(&path),
            &Hints {
                name: Some(&filename),
                mime: Some(ImageFormat::Gif.mime()),
            },
        )?;
        if staged.kind != AttachmentKind::Image || staged.mime != ImageFormat::Gif.mime() {
            let _ = fs::remove_file(&staged.part);
            return Err(StoreError::AttachmentUnreadable);
        }
        self.send_copy(chat_id, reply_to_id, staged, None, None, now)
    }

    /// Stage 2 of both: the entry, the file, the rows, in one writer job.
    fn send_copy(
        &self,
        chat_id: &str,
        reply_to_id: Option<&str>,
        staged: Staged,
        sticker: Option<StickerNote>,
        remember_id: Option<&str>,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let part = staged.part.clone();
        let job = Job {
            chat_id: chat_id.to_string(),
            reply_to_id: reply_to_id.map(str::to_string),
            remember_id: remember_id.map(str::to_string),
            now: now.clone(),
            dirs: self.dirs(),
            file: NewFile {
                id: staged.id.clone(),
                file_name: staged.file_name.clone(),
                sha256: staged.sha256.clone(),
                byte_size: i64::try_from(staged.byte_size).unwrap_or(i64::MAX),
                filename: staged.filename.clone(),
                kind: staged.kind,
                mime: staged.mime.clone(),
                width: staged.width,
                height: staged.height,
                duration_ms: None,
                // A copy starts without details: nothing of the source's name, description or cover comes along (slice 7b).
                details: DraftDetails::default(),
                sticker,
            },
            part: part.clone(),
        };
        let faults = self.inner.faults.clone();
        let result = self.write(move |conn| commit(conn, &faults, &job));
        if result.is_err() {
            let _ = fs::remove_file(&part);
        }
        let view = result?;
        self.wake_mirror();
        Ok(view)
    }
}

/// An entry drawn as a sticker (the display rule of §C.10, in SQL): no text, one live attachment, which reads as a sticker and is not
/// covered. Such an entry is never an append target and the store refuses to edit it.
pub(crate) fn is_sticker_entry(conn: &Connection, message_id: &str) -> Result<bool, StoreError> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM message m WHERE m.id = ?1 AND m.body_md = '' \
           AND (SELECT COUNT(*) FROM attachment a WHERE a.message_id = m.id AND a.deleted_at IS NULL) = 1 \
           AND EXISTS(SELECT 1 FROM attachment a WHERE a.message_id = m.id AND a.deleted_at IS NULL \
                      AND a.sticker = 1 AND a.spoiler = 0))",
        [message_id],
        |row| row.get(0),
    )
    .map_err(read_err)
}

/// `<id>.<ext>` and the emoji of a library sticker.
fn library_row(conn: &Connection, id: &str) -> Result<(String, Option<String>), StoreError> {
    conn.query_row(
        "SELECT id || '.' || ext, emoji FROM sticker WHERE id = ?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(read_err)?
    .ok_or(StoreError::StickerNotFound)
}

/// The original name and the stored file name of a live, uncovered GIF of a live entry.
fn gif_source(conn: &Connection, attachment_id: &str) -> Result<(String, String), StoreError> {
    conn.query_row(
        "SELECT a.filename, l.file_name FROM attachment a \
         JOIN message m ON m.id = a.message_id \
         JOIN attachment_local l ON l.attachment_id = a.id AND l.state = 'local' \
         WHERE a.id = ?1 AND a.kind = 'image' AND a.mime = 'image/gif' AND a.spoiler = 0 \
           AND a.deleted_at IS NULL AND m.deleted_at IS NULL",
        [attachment_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(read_err)?
    .ok_or(StoreError::AttachmentNotFound)
}

fn check_chat(conn: &Connection, chat_id: &str) -> Result<(), StoreError> {
    let exists: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
            [chat_id],
            |row| row.get(0),
        )
        .map_err(read_err)?;
    if exists {
        Ok(())
    } else {
        Err(StoreError::ChatNotFound)
    }
}

/// What stage 2 needs, moved into the writer.
struct Job {
    chat_id: String,
    reply_to_id: Option<String>,
    remember_id: Option<String>,
    now: Now,
    dirs: Dirs,
    file: NewFile,
    part: PathBuf,
}

/// In the writer: `BEGIN IMMEDIATE`; the chat and the reply target are checked; the entry; the rename; the rows; the search index; the
/// recent list; the reply line of the draft; commit; `sync_dir`.
fn commit(conn: &mut Connection, faults: &Faults, job: &Job) -> Result<MessageView, StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    let mut view = match &job.reply_to_id {
        Some(target) => insert_reply_message(&tx, &job.chat_id, "", target, &job.now)?,
        None => insert_new_message(&tx, &job.chat_id, "", &job.now)?,
    };
    let target = job
        .dirs
        .stored(&job.file.file_name)
        .ok_or_else(|| StoreError::Internal("bad stored file name".into()))?;
    make_dir(&job.dirs.attachments()).map_err(io_error)?;
    fs::rename(&job.part, &target).map_err(io_error)?;
    let written = (|| {
        faults.run_after_rename();
        let attachment = insert_attachment(&tx, &view.id, 0, &job.file, job.now.ms)?;
        view.attachments = vec![attachment];
        // The file name is part of the entry's search text: index again, now that the row exists.
        crate::search::reindex(&tx, &view.id, &view.body_md).map_err(write_err)?;
        if let Some(id) = &job.remember_id {
            remember(&tx, id)?;
        }
        // Only a reply the entry took leaves the draft; the typed text and the pending tiles stay where they are.
        if let Some(target) = &job.reply_to_id {
            let key = chat_key(&job.chat_id);
            tx.execute(
                "UPDATE draft SET reply_to_id = NULL WHERE key = ?1 AND reply_to_id = ?2",
                (&key, target),
            )
            .map_err(write_err)?;
            delete_empty_draft(&tx, &key).map_err(write_err)?;
        }
        tx.commit().map_err(write_err)
    })();
    if written.is_err() {
        // The row did not make it: the renamed file belongs to nobody.
        let _ = fs::remove_file(&target);
    } else {
        sync_dir(&job.dirs.attachments());
    }
    written.map(|()| view)
}
