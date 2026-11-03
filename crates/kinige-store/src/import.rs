//! Import from Telegram (slice 4, task 4.8): a preview and writing in batches of 500 messages.
//!
//! Each batch is one writer transaction: entries, `message_ext`, tags, the search index, Markdown copy marks and the HLC.
//! Between batches the writer is free for other writes. A stop or `kill -9` loses at most one uncommitted batch;
//! a repeated import is deduplicated by `message_ext` and brings in the rest. The database backup is taken before the
//! first write.

use crate::chats::insert_new_chat;
use crate::error::{read_err, write_err};
use crate::hlc_state::{get_state, load_clock, save_clock};
use crate::import_time::{Timeline, journal_date_of};
use crate::send::{insert_message, mark_dirty};
use crate::{Now, Store, StoreError};
use kinige_domain::Language;
use kinige_domain::chat::{DEFAULT_ICON, MAX_TITLE_CHARS, normalize_title};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::import::{
    EXT_SRC_TELEGRAM, ImportChatKind, ImportChatPreview, ImportPreview, ImportProgress,
    ImportReport, NewImportedMessage, new_imported_message,
};
use kinige_domain::telegram::{TgChat, TgChatKind, TgExport, TgMedia, TgMessage, render_body};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Messages in one batch (one writer transaction).
pub const IMPORT_BATCH: usize = 500;

fn supported_kind(chat: &TgChat) -> Option<ImportChatKind> {
    match chat.kind {
        TgChatKind::Saved => Some(ImportChatKind::Saved),
        TgChatKind::Personal => Some(ImportChatKind::Personal),
        TgChatKind::Other(_) => None,
    }
}

/// Already imported messages of a Telegram chat: `ext_msg -> Kinige entry id`.
fn imported_messages(
    conn: &Connection,
    ext_chat: &str,
) -> Result<HashMap<String, String>, StoreError> {
    let mut statement = conn
        .prepare("SELECT ext_msg, message_id FROM message_ext WHERE src = ?1 AND ext_chat = ?2")
        .map_err(read_err)?;
    let rows = statement
        .query_map((EXT_SRC_TELEGRAM, ext_chat), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(read_err)?;
    rows.collect::<Result<_, _>>().map_err(read_err)
}

/// The personal-conversation chat on a repeated import is the chat of the freshest live imported entry of this Telegram
/// chat (decision 5): a not deleted entry in a not deleted chat. None -- `None`, the first batch with entries creates the chat.
fn find_import_chat(conn: &Connection, ext_chat: &str) -> Result<Option<String>, StoreError> {
    conn.query_row(
        "SELECT m.chat_id FROM message_ext e \
         JOIN message m ON m.id = e.message_id JOIN chat c ON c.id = m.chat_id \
         WHERE e.src = ?1 AND e.ext_chat = ?2 AND m.deleted_at IS NULL AND c.deleted_at IS NULL \
         ORDER BY m.created_at DESC, m.id DESC LIMIT 1",
        (EXT_SRC_TELEGRAM, ext_chat),
        |row| row.get(0),
    )
    .optional()
    .map_err(read_err)
}

/// The Kinige chat name for a personal conversation: from the export; no name -- `Telegram <id>`; longer than the limit -- cut.
fn chat_title(chat: &TgChat) -> String {
    let fallback = format!("Telegram {}", chat.id);
    let Some(name) = chat.name.as_deref() else {
        return fallback;
    };
    let cut: String = name.trim().chars().take(MAX_TITLE_CHARS).collect();
    normalize_title(&cut).unwrap_or(fallback)
}

/// Author: for the other person's message in a personal chat (`from_id` = `user<chat id>`, decision 6) -- the name; own messages and "Saved Messages" -- no name.
fn author_of<'a>(chat: &'a TgChat, message: &'a TgMessage) -> Option<&'a str> {
    if chat.kind != TgChatKind::Personal {
        return None;
    }
    let theirs = message.from_id.as_deref() == Some(format!("user{}", chat.id).as_str());
    if !theirs {
        return None;
    }
    message.from.as_deref().or(chat.name.as_deref())
}

/// An entry to insert, prepared outside the writer.
struct Prepared {
    id: String,
    ext_msg: String,
    body: String,
    created_at: i64,
    journal_date: String,
    reply_to_id: Option<String>,
}

/// What a batch returned.
struct BatchOutcome {
    /// The destination chat; `None` -- the personal chat was never created (all keys of the batch turned out to be taken).
    chat_id: Option<String>,
    created_chat: bool,
    /// Entries whose external id turned out to be taken inside the transaction itself: not inserted.
    skipped: i64,
    /// The prepared id of a skipped entry -> the id of the entry that owns its external id: replies to the skipped one go to it.
    replaced: HashMap<String, String>,
}

impl Store {
    /// What was found in the export: the chats that can be imported, the number of entries and how many are already imported.
    pub fn import_preview(&self, export: &TgExport) -> Result<ImportPreview, StoreError> {
        let mut chats = Vec::new();
        let mut skipped_chats = 0;
        for chat in &export.chats {
            let Some(kind) = supported_kind(chat) else {
                skipped_chats += 1;
                continue;
            };
            let existing = self.read(|conn| imported_messages(conn, &chat.id))?;
            let (mut total, mut have) = (0, 0);
            for message in &chat.messages {
                if message.service || render_body(message, None, Language::Ru).is_none() {
                    continue;
                }
                total += 1;
                have += i64::from(existing.contains_key(&message.id));
            }
            chats.push(ImportChatPreview {
                id: chat.id.clone(),
                name: chat.name.clone(),
                kind,
                total,
                existing: have,
            });
        }
        Ok(ImportPreview {
            chats,
            skipped_chats,
        })
    }

    /// Import of the selected chats (`chats` are their Telegram ids). A database backup is taken before the first write;
    /// its failure cancels the import (`ImportBackupFailed`). `progress` is called after each committed batch; `stop` is
    /// checked between batches. A batch write error leaves the previous batches in place: a repeated run brings in the rest.
    pub fn import_telegram(
        &self,
        export: Arc<TgExport>,
        chats: Vec<String>,
        language: Language,
        now: &Now,
        progress: impl Fn(ImportProgress) + Send + 'static,
        stop: Arc<AtomicBool>,
    ) -> Result<ImportReport, StoreError> {
        let selected: Vec<&TgChat> = export
            .chats
            .iter()
            .filter(|chat| supported_kind(chat).is_some() && chats.contains(&chat.id))
            .collect();
        // As in the preview: service and empty messages are not part of "N of M" (otherwise M is larger than the number in the chat list).
        let total: i64 = selected
            .iter()
            .flat_map(|c| c.messages.iter())
            .filter(|m| !m.service && render_body(m, None, language).is_some())
            .count() as i64;
        let started = Instant::now();
        let mut report = ImportReport::default();
        let mut done = 0i64;
        for chat in selected {
            let mut existing = self.read(|conn| imported_messages(conn, &chat.id))?;
            let mut dest = match chat.kind {
                TgChatKind::Saved => Some(INBOX_ID.to_string()),
                _ => self.read(|conn| find_import_chat(conn, &chat.id))?,
            };
            let mut timeline = Timeline::default();
            for batch in chat.messages.chunks(IMPORT_BATCH) {
                if stop.load(Ordering::SeqCst) {
                    report.stopped = true;
                    self.wake_mirror();
                    return Ok(report);
                }
                let mut rows: Vec<Prepared> = Vec::new();
                let mut new_ids: HashMap<String, String> = HashMap::new();
                let mut media = 0;
                // How many messages of the batch are part of "N of M": not service and not empty.
                let mut counted = 0i64;
                for message in batch {
                    let created_at = timeline.created_at(message, now);
                    if message.service {
                        report.skipped_service += 1;
                    } else if existing.contains_key(&message.id)
                        || new_ids.contains_key(&message.id)
                    {
                        report.skipped_existing += 1;
                        counted += 1;
                    } else if let Some(body) =
                        render_body(message, author_of(chat, message), language)
                    {
                        counted += 1;
                        let id = uuid::Uuid::new_v4().to_string();
                        let reply_to_id = message
                            .reply_to
                            .as_ref()
                            .and_then(|r| existing.get(r).or_else(|| new_ids.get(r)))
                            .cloned();
                        media += i64::from(
                            message
                                .media
                                .as_ref()
                                .is_some_and(|m| !matches!(m, TgMedia::Poll { .. })),
                        );
                        new_ids.insert(message.id.clone(), id.clone());
                        rows.push(Prepared {
                            id,
                            ext_msg: message.id.clone(),
                            body,
                            created_at,
                            journal_date: journal_date_of(created_at, now),
                            reply_to_id,
                        });
                    } else {
                        report.skipped_empty += 1;
                    }
                }
                if !rows.is_empty() {
                    if report.backup_name.is_none() {
                        report.backup_name = Some(self.backup_before_import(now)?);
                    }
                    let count = rows.len() as i64;
                    let pt_ms = now.ms.saturating_add(started.elapsed().as_millis() as i64);
                    let outcome =
                        self.write_batch(chat, dest.clone(), rows, pt_ms, now.tz.clone())?;
                    dest = outcome.chat_id;
                    report.chats_created += i64::from(outcome.created_chat);
                    report.imported += count - outcome.skipped;
                    report.skipped_existing += outcome.skipped;
                    report.media_placeholders += media;
                    existing.extend(new_ids);
                    // Replies of the following batches to a skipped entry go to the real owner of its external id.
                    for id in existing.values_mut() {
                        if let Some(owner) = outcome.replaced.get(id.as_str()) {
                            id.clone_from(owner);
                        }
                    }
                }
                done += counted;
                progress(ImportProgress { done, total });
            }
        }
        self.wake_mirror();
        Ok(report)
    }

    /// One batch in one transaction. `dest` is the destination chat; `None` -- the personal chat is not created yet, this batch creates it.
    fn write_batch(
        &self,
        chat: &TgChat,
        dest: Option<String>,
        rows: Vec<Prepared>,
        pt_ms: i64,
        tz: String,
    ) -> Result<BatchOutcome, StoreError> {
        let (ext_chat, title) = (chat.id.clone(), chat_title(chat));
        self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            // External id keys first: a taken key (the entry appeared after `existing` was read) means no entry will be
            // made, and replies to it must go to the real owner of the key, otherwise the id of a nonexistent entry would
            // land in the immutable `core`. If no new ones are left, the personal chat is not created: no empty duplicate chat.
            let mut fresh: Vec<&Prepared> = Vec::new();
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
                }
            }
            let skipped = replaced.len() as i64;
            if fresh.is_empty() {
                return Ok(BatchOutcome {
                    chat_id: dest,
                    created_chat: false,
                    skipped,
                    replaced,
                });
            }
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
                crate::tags::replace_tags(&tx, &row.id, &row.body).map_err(write_err)?;
                crate::search::reindex(&tx, &row.id, &row.body).map_err(write_err)?;
                days.insert(row.journal_date.clone());
            }
            save_clock(&tx, clock).map_err(write_err)?;
            for day in &days {
                mark_dirty(&tx, &chat_id, day).map_err(write_err)?;
            }
            tx.commit().map_err(write_err)?;
            Ok(BatchOutcome {
                chat_id: Some(chat_id),
                created_chat,
                skipped,
                replaced,
            })
        })
    }
}

#[cfg(test)]
mod tests;
