//! One batch of a Telegram import in one writer transaction (slice 4, extended in slice 7 with the media): the entries, their
//! external ids, tags, the search index, the Markdown copy marks, the HLC, the files of the new entries and the files of
//! entries imported before. The files were copied outside the writer; here they are renamed into place and their rows written.

use crate::chats::insert_new_chat;
use crate::error::write_err;
use crate::hlc_state::{get_state, load_clock, save_clock};
use crate::import_lookup::chat_title;
use crate::import_media::{Late, LateOutcome, StagedMedia, attach_late, attach_to_new};
use crate::send::{insert_message, mark_dirty};
use crate::{Now, Store, StoreError};
use kinige_domain::chat::DEFAULT_ICON;
use kinige_domain::import::{EXT_SRC_TELEGRAM, NewImportedMessage, new_imported_message};
use kinige_domain::telegram::TgChat;
use rusqlite::TransactionBehavior;
use std::collections::{BTreeSet, HashMap};

/// An entry to insert, prepared outside the writer.
pub(crate) struct Row {
    pub id: String,
    pub ext_msg: String,
    pub body: String,
    pub created_at: i64,
    pub journal_date: String,
    pub reply_to_id: Option<String>,
    /// The file copied for it, if the message has one and the export holds it.
    pub media: Option<StagedMedia>,
}

/// What a batch returned.
pub(crate) struct BatchOutcome {
    /// The destination chat; `None` -- the personal chat was never created (all keys of the batch turned out to be taken).
    pub chat_id: Option<String>,
    pub created_chat: bool,
    /// Entries whose external id turned out to be taken inside the transaction itself: not inserted.
    pub skipped: i64,
    /// The prepared id of a skipped entry -> the id of the entry that owns its external id: replies to the skipped one go to it.
    pub replaced: HashMap<String, String>,
    /// Files attached in this batch (to new entries and to entries imported before) and their size.
    pub attached: i64,
    pub attached_bytes: i64,
}

impl Store {
    /// A batch of new entries only (no files for entries imported before); the tests of the batch itself use it.
    #[cfg(test)]
    pub(crate) fn write_batch(
        &self,
        chat: &TgChat,
        dest: Option<String>,
        rows: Vec<Row>,
        pt_ms: i64,
        tz: String,
    ) -> Result<BatchOutcome, StoreError> {
        self.write_batch_with(chat, dest, rows, Vec::new(), pt_ms, tz)
    }

    /// One batch in one transaction. `dest` is the destination chat; `None` -- the personal chat is not created yet, this batch
    /// creates it. `late` are files for entries imported before.
    pub(crate) fn write_batch_with(
        &self,
        chat: &TgChat,
        dest: Option<String>,
        rows: Vec<Row>,
        late: Vec<Late>,
        pt_ms: i64,
        tz: String,
    ) -> Result<BatchOutcome, StoreError> {
        let (ext_chat, title) = (chat.id.clone(), chat_title(chat));
        let dirs = self.dirs();
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            // External id keys first: a taken key (the entry appeared after `existing` was read) means no entry will be
            // made, and replies to it must go to the real owner of the key, otherwise the id of a nonexistent entry would
            // land in the immutable `core`. If no new ones are left, the personal chat is not created: no empty duplicate chat.
            let mut fresh: Vec<&Row> = Vec::new();
            let mut replaced: HashMap<String, String> = HashMap::new();
            for row in &rows {
                if crate::ext::insert(&tx, EXT_SRC_TELEGRAM, &ext_chat, &row.ext_msg, &row.id)
                    .map_err(write_err)?
                {
                    fresh.push(row);
                } else {
                    let owner: String = tx
                        .query_row(
                            "SELECT message_id FROM message_ext WHERE src = ?1 AND ext_chat = ?2 AND ext_msg = ?3",
                            (EXT_SRC_TELEGRAM, &ext_chat, &row.ext_msg),
                            |r| r.get(0),
                        )
                        .map_err(write_err)?;
                    replaced.insert(row.id.clone(), owner);
                    if let Some(media) = &row.media {
                        media.discard();
                    }
                }
            }
            let skipped = replaced.len() as i64;
            let mut outcome = BatchOutcome {
                chat_id: dest.clone(),
                created_chat: false,
                skipped,
                replaced: HashMap::new(),
                attached: 0,
                attached_bytes: 0,
            };
            if !fresh.is_empty() {
                let (chat_id, created_chat) = match dest {
                    Some(id) => {
                        let alive: bool = tx
                            .query_row(
                                "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
                                [&id],
                                |row| row.get(0),
                            )
                            .map_err(write_err)?;
                        if !alive {
                            return Err(StoreError::ChatNotFound);
                        }
                        (id, false)
                    }
                    None => (insert_new_chat(&tx, &title, DEFAULT_ICON, pt_ms)?, true),
                };
                // The clock is read after the chat is created, which moves it by itself. Then the tick goes in memory, saved once.
                let device = get_state(&tx, "device_id")
                    .map_err(write_err)?
                    .ok_or_else(|| StoreError::Internal("no device_id".into()))?;
                let mut clock = load_clock(&tx)?
                    .ok_or_else(|| StoreError::Internal("no clock state".into()))?;
                let mut days: BTreeSet<String> = BTreeSet::new();
                for row in fresh {
                    let reply_to_id = row
                        .reply_to_id
                        .as_deref()
                        .map(|target| replaced.get(target).map_or(target, String::as_str));
                    clock = clock
                        .tick(u64::try_from(pt_ms).unwrap_or(0))
                        .map_err(|e| StoreError::Internal(e.to_string()))?;
                    let message = new_imported_message(
                        &NewImportedMessage {
                            id: &row.id,
                            chat_id: &chat_id,
                            body_md: &row.body,
                            created_at: row.created_at,
                            created_tz: &tz,
                            journal_date: &row.journal_date,
                            ext_chat: &ext_chat,
                            ext_msg: &row.ext_msg,
                            reply_to_id,
                        },
                        &clock.format(&device),
                    )?;
                    insert_message(&tx, &message).map_err(write_err)?;
                    if let Some(media) = &row.media {
                        // The file's rows take the next stamp of the same in-memory clock.
                        clock = clock
                            .tick(u64::try_from(pt_ms).unwrap_or(0))
                            .map_err(|e| StoreError::Internal(e.to_string()))?;
                        attach_to_new(&tx, &dirs, &row.id, media, &clock.format(&device))?;
                        outcome.attached += 1;
                        outcome.attached_bytes += media.bytes();
                    }
                    crate::tags::replace_tags(&tx, &row.id, &row.body).map_err(write_err)?;
                    crate::search::reindex(&tx, &row.id, &row.body).map_err(write_err)?;
                    days.insert(row.journal_date.clone());
                }
                save_clock(&tx, clock).map_err(write_err)?;
                for day in &days {
                    mark_dirty(&tx, &chat_id, day).map_err(write_err)?;
                }
                outcome.chat_id = Some(chat_id);
                outcome.created_chat = created_chat;
            }
            // Files for entries imported before use the stored clock, which is saved above.
            let now = Now::at(pt_ms, &tz);
            for item in &late {
                if let LateOutcome::Attached { bytes } = attach_late(&tx, &dirs, item, &now)? {
                    outcome.attached += 1;
                    outcome.attached_bytes += bytes;
                }
            }
            tx.commit().map_err(write_err)?;
            outcome.replaced = replaced;
            Ok(outcome)
        })
    }
}
