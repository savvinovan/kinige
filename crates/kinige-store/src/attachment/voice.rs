//! Sending a voice note (slice 7a, ADR-0015): `Store::send_voice`. The recorder's bytes cross IPC in one raw body; stage 1 is the
//! copy pipeline of every attachment (`.tmp/`, hash, `fsync`), stage 2 is one writer job that writes the entry (empty body, valid
//! with an attachment, appendix §C.6), renames the file and writes the `attachment` and `attachment_local` rows. No migration:
//! `attachment.duration_ms` exists since slice 7. A crash between the rename and the commit leaves an orphan for the sweep.

use super::copy::{Hints, Source, stage};
use super::{Dirs, Faults, NewFile, insert_attachment};
use crate::draft::{chat_key, delete_empty_draft};
use crate::error::{read_err, write_err};
use crate::send::{insert_new_message, insert_reply_message};
use crate::{Now, Store, StoreError};
use kinige_domain::MessageView;
use kinige_domain::attachment::{
    AttachmentKind, MAX_PASTE_BYTES, MAX_VOICE_MS, MIN_VOICE_MS, VoiceContainer, voice_file_name,
};
use rusqlite::{Connection, TransactionBehavior};
use std::fs;

/// How many leading bytes decide the container.
const HEAD_BYTES: usize = 12;

impl Store {
    /// Saves a recording as a new entry of the chat: no text, one `audio` attachment. The container is decided by the first bytes
    /// (`ftyp` at offset 4 is MP4, the EBML header is WebM, anything else is `AttachmentUnreadable`); what the recorder says about
    /// itself is never trusted. `duration_ms` is the recorder's own clock: 1 s to 30 min (+5 s), otherwise `VoiceTooShort` or
    /// `VoiceTooLong`. More than 64 MiB is `AttachmentTooLarge`. With `reply_to_id` the target is checked in the same transaction
    /// (`ReplyTargetNotFound`); the draft's reply line is cleared, its text and tiles are left alone. Nothing is written on an error.
    pub fn send_voice(
        &self,
        chat_id: &str,
        reply_to_id: Option<&str>,
        bytes: &[u8],
        duration_ms: i64,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        if bytes.len() > MAX_PASTE_BYTES {
            return Err(StoreError::AttachmentTooLarge);
        }
        if duration_ms < MIN_VOICE_MS {
            return Err(StoreError::VoiceTooShort);
        }
        if duration_ms > MAX_VOICE_MS {
            return Err(StoreError::VoiceTooLong);
        }
        let head = bytes.get(..HEAD_BYTES).unwrap_or(bytes);
        let container = VoiceContainer::detect(head).ok_or(StoreError::AttachmentUnreadable)?;
        let filename = voice_file_name(now.ms, now.utc_offset_secs, container)
            .ok_or_else(|| StoreError::Internal("the time is out of range".into()))?;
        // Refuse before copying: no such chat.
        self.read(|conn| check_chat(conn, chat_id))?;

        let dirs = self.dirs();
        let mut staged = stage(
            &dirs,
            self.faults(),
            Source::Bytes(bytes),
            &Hints {
                name: Some(&filename),
                mime: None,
            },
        )?;
        // `classify` maps `webm` to video; a recording is audio whatever its container.
        staged.kind = AttachmentKind::Audio;
        staged.mime = container.mime().to_string();

        let part = staged.part.clone();
        let job = Job {
            chat_id: chat_id.to_string(),
            reply_to_id: reply_to_id.map(str::to_string),
            now: now.clone(),
            dirs,
            file: NewFile {
                id: staged.id.clone(),
                file_name: staged.file_name.clone(),
                sha256: staged.sha256.clone(),
                byte_size: i64::try_from(staged.byte_size).unwrap_or(i64::MAX),
                filename: staged.filename.clone(),
                kind: staged.kind,
                mime: staged.mime.clone(),
                width: None,
                height: None,
                duration_ms: Some(duration_ms),
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

/// What stage 2 needs, moved into the writer.
struct Job {
    chat_id: String,
    reply_to_id: Option<String>,
    now: Now,
    dirs: Dirs,
    file: NewFile,
    part: std::path::PathBuf,
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

/// Stage 2, in the writer: `BEGIN IMMEDIATE`; the chat and the reply target are checked; the entry; the rename; the rows; the
/// search index; the copy mark (inside the entry insert); the reply line of the draft; commit; `sync_dir`.
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
    super::make_dir(&job.dirs.attachments()).map_err(super::io_error)?;
    fs::rename(&job.part, &target).map_err(super::io_error)?;
    let written = (|| {
        faults.run_after_rename();
        let attachment = insert_attachment(&tx, &view.id, 0, &job.file, job.now.ms)?;
        view.attachments = vec![attachment];
        // The file name is part of the entry's search text: index again, now that the row exists.
        crate::search::reindex(&tx, &view.id, &view.body_md).map_err(write_err)?;
        // The note carries the reply like a text reply; the typed text and the pending tiles stay where they are. Only a reply the note
        // took leaves the draft: a note sent without one (the target is in the trash) leaves the draft's reply line alone.
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
        super::sync_dir(&job.dirs.attachments());
    }
    written.map(|()| view)
}
