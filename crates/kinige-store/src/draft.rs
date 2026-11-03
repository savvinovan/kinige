//! Drafts (PRD §5, §5.1): a local, non-synchronised table `draft(key, body, updated_at, reply_to_id)`. The keys are the capture
//! window (`capture`) and a chat of the main window (`chat:<id>`). A chat draft also carries the entry being replied to (slice 5,
//! migration 6). A draft is cleared in the same transaction as the send. Drafts do not reach the Markdown copy.

use crate::attachment::draft_views;
use crate::error::{read_err, write_err};
use crate::view::load_reply_to;
use crate::{Now, Store, StoreError};
use kinige_domain::hlc::is_uuid_v4;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::{ChatDraft, ReplyState};
use rusqlite::{OptionalExtension, Transaction, TransactionBehavior};

/// The key of the capture window draft.
pub(crate) const CAPTURE_KEY: &str = "capture";
/// The draft limit in bytes: a draft must not bloat the database; an entry longer than the limit cannot be sent anyway.
pub const MAX_DRAFT_BYTES: usize = 1024 * 1024;

pub(crate) fn chat_key(chat_id: &str) -> String {
    format!("chat:{chat_id}")
}

pub(crate) fn delete_draft(tx: &Transaction<'_>, key: &str) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM draft WHERE key = ?1", [key])?;
    Ok(())
}

/// Deletes the draft row if it holds nothing: no text, no reply and no attachment tiles (slice 7).
pub(crate) fn delete_empty_draft(tx: &Transaction<'_>, key: &str) -> rusqlite::Result<()> {
    tx.execute(
        "DELETE FROM draft WHERE key = ?1 AND body = '' AND reply_to_id IS NULL \
         AND NOT EXISTS (SELECT 1 FROM draft_attachment WHERE draft_key = ?1)",
        [key],
    )?;
    Ok(())
}

impl Store {
    /// The chat draft; an empty string if there is none.
    pub fn get_chat_draft(&self, chat_id: &str) -> Result<String, StoreError> {
        self.read_draft(&chat_key(chat_id))
    }

    /// The chat draft with the entry being replied to (slice 5): `reply_to` is a quote with the state `live`, `trash` or
    /// `missing` if the entry is already gone. No draft means an empty text and no reply.
    pub fn chat_draft(&self, chat_id: &str, now: &Now) -> Result<ChatDraft, StoreError> {
        let key = chat_key(chat_id);
        let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
        self.read(|conn| {
            let stored: Option<(String, Option<String>)> = conn
                .query_row(
                    "SELECT body, reply_to_id FROM draft WHERE key = ?1",
                    [&key],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
                .map_err(read_err)?;
            let attachments = draft_views(conn, &key)?;
            let Some((body, reply_id)) = stored else {
                return Ok(ChatDraft {
                    body: String::new(),
                    reply_to: None,
                    attachments,
                });
            };
            let reply_to = match reply_id {
                Some(id) => {
                    let mut quote = load_reply_to(conn, &id, &today)?;
                    // The entry is alive but in another chat: Rust will not accept a reply to it, the line above the field will show that.
                    if quote.state == ReplyState::Live && quote.chat_id.as_deref() != Some(chat_id)
                    {
                        quote.state = ReplyState::Moved;
                    }
                    Some(quote)
                }
                None => None,
            };
            Ok(ChatDraft {
                body,
                reply_to,
                attachments,
            })
        })
    }

    /// Saves the chat draft without a reply; an empty text deletes it. No such chat gives `ChatNotFound`.
    pub fn set_chat_draft(&self, chat_id: &str, body: &str, now: &Now) -> Result<(), StoreError> {
        self.save_chat_draft(chat_id, body, None, now)
    }

    /// Saves the chat draft and the entry being replied to. The row is deleted only when both the text and the reply are empty.
    /// A `reply_to_id` that is not a UUID v4 gives `ReplyTargetNotFound` and nothing is written; the existence of the entry is checked by the send:
    /// while the draft sits, the entry may be moved to the trash, and the draft must neither vanish nor become an error because of that.
    pub fn save_chat_draft(
        &self,
        chat_id: &str,
        body: &str,
        reply_to_id: Option<&str>,
        now: &Now,
    ) -> Result<(), StoreError> {
        let chat_id = chat_id.to_string();
        self.write_draft(
            Some(chat_id.clone()),
            chat_key(&chat_id),
            body,
            reply_to_id,
            now,
        )
    }

    /// The capture window draft.
    pub fn get_capture_draft(&self) -> Result<String, StoreError> {
        self.read_draft(CAPTURE_KEY)
    }

    pub fn set_capture_draft(&self, body: &str, now: &Now) -> Result<(), StoreError> {
        self.write_draft(None, CAPTURE_KEY.to_string(), body, None, now)
    }

    /// A clipboard hotkey failure: the clipboard text goes into the capture window draft after the previous non-empty draft
    /// (`kinige_domain::draft::joined_draft`), nothing is overwritten. Reading and writing happen in one writer transaction.
    /// Returns the whole field text and a "written to the database" flag: if the result is longer than [`MAX_DRAFT_BYTES`], the database
    /// stays as it was, but the text is still returned (the window will show it from memory).
    pub fn merge_clipboard_into_capture_draft(
        &self,
        clipboard: &str,
        now: &Now,
    ) -> Result<(String, bool), StoreError> {
        let clipboard = clipboard.to_string();
        let now_ms = now.ms;
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let existing: String = tx
                .query_row(
                    "SELECT body FROM draft WHERE key = ?1",
                    [CAPTURE_KEY],
                    |row| row.get(0),
                )
                .optional()
                .map_err(write_err)?
                .unwrap_or_default();
            let joined = kinige_domain::draft::joined_draft(&existing, &clipboard);
            if joined.len() > MAX_DRAFT_BYTES {
                return Ok((joined, false));
            }
            tx.execute(
                "INSERT INTO draft(key, body, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(key) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at",
                rusqlite::params![CAPTURE_KEY, joined, now_ms],
            )
            .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok((joined, true))
        })
    }

    fn read_draft(&self, key: &str) -> Result<String, StoreError> {
        self.read(|conn| {
            Ok(conn
                .query_row("SELECT body FROM draft WHERE key = ?1", [key], |row| {
                    row.get::<_, String>(0)
                })
                .optional()
                .map_err(read_err)?
                .unwrap_or_default())
        })
    }

    fn write_draft(
        &self,
        require_chat: Option<String>,
        key: String,
        body: &str,
        reply_to_id: Option<&str>,
        now: &Now,
    ) -> Result<(), StoreError> {
        if body.len() > MAX_DRAFT_BYTES {
            return Err(StoreError::TooLarge);
        }
        if reply_to_id.is_some_and(|id| !is_uuid_v4(id)) {
            return Err(StoreError::ReplyTargetNotFound);
        }
        let body = body.to_string();
        let reply_to_id = reply_to_id.map(str::to_string);
        let now_ms = now.ms;
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            if let Some(chat_id) = require_chat {
                let exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
                        [&chat_id],
                        |row| row.get(0),
                    )
                    .map_err(write_err)?;
                if !exists {
                    return Err(StoreError::ChatNotFound);
                }
            }
            let has_tiles: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM draft_attachment WHERE draft_key = ?1)",
                    [&key],
                    |row| row.get(0),
                )
                .map_err(write_err)?;
            // Deleted only when the text, the reply and the attachment tiles are all empty.
            if body.is_empty() && reply_to_id.is_none() && !has_tiles {
                delete_draft(&tx, &key).map_err(write_err)?;
            } else {
                tx.execute(
                    "INSERT INTO draft(key, body, updated_at, reply_to_id) VALUES (?1, ?2, ?3, ?4) \
                     ON CONFLICT(key) DO UPDATE SET body = excluded.body, updated_at = excluded.updated_at, \
                     reply_to_id = excluded.reply_to_id",
                    rusqlite::params![key, body, now_ms, reply_to_id],
                )
                .map_err(write_err)?;
            }
            tx.commit().map_err(write_err)
        })
    }
}
