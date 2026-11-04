//! Import from Telegram (slice 4, task 4.8): a preview and writing in batches of up to 500 messages.
//!
//! Each batch is one writer transaction: entries, `message_ext`, tags, the search index, Markdown copy marks and the HLC.
//! Between batches the writer is free for other writes. A stop or `kill -9` loses at most one uncommitted batch;
//! a repeated import is deduplicated by `message_ext` and brings in the rest. The database backup is taken before the
//! first write.
//!
//! Media (slice 7, ADR-0014): with the folder of `result.json` the files of photos, files, videos, voice messages and audio are
//! copied in (streamed, outside the writer) and attached; a batch is cut after 100 entries with files or 256 MiB copied. The
//! files of entries imported earlier are attached too, once (an entry that has any attachment, live or in the trash, is skipped).

use crate::import_batch::Row;
use crate::import_lookup::{
    author_of, entries_with_files, file_of, find_import_chat, imported_messages, renderings,
    supported_kind,
};
use crate::import_media::{Late, Prepared, StagedMedia, prepare};
use crate::import_time::{Timeline, journal_date_of};
use crate::{Now, Store, StoreError};
use kinige_domain::Language;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::import::{ImportChatPreview, ImportPreview, ImportProgress, ImportReport};
use kinige_domain::telegram::{
    TgChat, TgChatKind, TgExport, TgMedia, render_body, render_body_attached,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Messages in one batch (one writer transaction).
pub const IMPORT_BATCH: usize = 500;
/// A batch is also cut after this many entries with files (slice 7).
const BATCH_MEDIA_ENTRIES: usize = 100;
/// ... or after this many bytes of files copied.
const BATCH_MEDIA_BYTES: i64 = 256 * 1024 * 1024;

/// Counters of a run that are not part of the report until the end.
#[derive(Default)]
struct Run {
    report: ImportReport,
    bytes_done: i64,
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
            let (mut media_files, mut media_bytes, mut media_missing) = (0i64, 0i64, 0i64);
            for message in &chat.messages {
                if message.service || render_body(message, None, Language::Ru).is_none() {
                    continue;
                }
                total += 1;
                have += i64::from(existing.contains_key(&message.id));
                match file_of(message) {
                    Some((true, size)) => {
                        media_files += 1;
                        media_bytes = media_bytes.saturating_add(i64::try_from(size).unwrap_or(0));
                    }
                    Some((false, _)) => media_missing += 1,
                    None => {}
                }
            }
            chats.push(ImportChatPreview {
                id: chat.id.clone(),
                name: chat.name.clone(),
                kind,
                total,
                existing: have,
                media_files,
                media_bytes,
                media_missing,
            });
        }
        Ok(ImportPreview {
            chats,
            skipped_chats,
        })
    }

    /// Import of the selected chats without files: the attachments stay placeholders.
    pub fn import_telegram(
        &self,
        export: Arc<TgExport>,
        chats: Vec<String>,
        language: Language,
        now: &Now,
        progress: impl Fn(ImportProgress) + Send + 'static,
        stop: Arc<AtomicBool>,
    ) -> Result<ImportReport, StoreError> {
        self.import_telegram_with_media(export, chats, language, now, progress, stop, None)
    }

    /// Import of the selected chats (`chats` are their Telegram ids). A database backup is taken before the first write;
    /// its failure cancels the import (`ImportBackupFailed`). `progress` is called after each committed batch; `stop` is
    /// checked between batches and between files. A batch write error leaves the previous batches in place: a repeated run
    /// brings in the rest. `media_root` is the folder of `result.json` (slice 7): with it the files are copied and attached;
    /// without it they stay placeholders. A full disk stops the import (`report.disk_full`) and rolls the batch back.
    #[allow(clippy::too_many_arguments)]
    pub fn import_telegram_with_media(
        &self,
        export: Arc<TgExport>,
        chats: Vec<String>,
        language: Language,
        now: &Now,
        progress: impl Fn(ImportProgress) + Send + 'static,
        stop: Arc<AtomicBool>,
        media_root: Option<&Path>,
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
        let root = media_root.and_then(|folder| std::fs::canonicalize(folder).ok());
        let bytes_total: i64 = root.as_ref().map_or(0, |_| {
            selected
                .iter()
                .flat_map(|c| c.messages.iter())
                .filter_map(file_of)
                .filter(|(included, _)| *included)
                .map(|(_, size)| i64::try_from(size).unwrap_or(0))
                .sum()
        });
        let started = Instant::now();
        let mut run = Run::default();
        let mut done = 0i64;
        for chat in selected {
            let mut existing = self.read(|conn| imported_messages(conn, &chat.id))?;
            let with_files = match root {
                Some(_) => self.read(|conn| entries_with_files(conn, &chat.id))?,
                None => HashSet::new(),
            };
            let mut dest = match chat.kind {
                TgChatKind::Saved => Some(INBOX_ID.to_string()),
                _ => self.read(|conn| find_import_chat(conn, &chat.id))?,
            };
            let mut timeline = Timeline::default();
            let mut cursor = 0usize;
            while cursor < chat.messages.len() {
                if stop.load(Ordering::SeqCst) {
                    run.report.stopped = true;
                    self.wake_mirror();
                    return Ok(run.report);
                }
                let mut rows: Vec<Row> = Vec::new();
                let mut late: Vec<Late> = Vec::new();
                let mut new_ids: HashMap<String, String> = HashMap::new();
                let mut placeholders = 0;
                // How many messages of the batch are part of "N of M": not service and not empty.
                let mut counted = 0i64;
                let (mut batch_files, mut batch_bytes) = (0usize, 0i64);
                let (mut stopping, mut disk_full) = (false, false);
                let begin = cursor;
                while let Some(message) = chat.messages.get(cursor) {
                    if cursor - begin >= IMPORT_BATCH
                        || batch_files >= BATCH_MEDIA_ENTRIES
                        || batch_bytes >= BATCH_MEDIA_BYTES
                    {
                        break;
                    }
                    if batch_files > 0 && stop.load(Ordering::SeqCst) {
                        // The stop flag is checked between files: what is prepared is committed, then the import ends.
                        stopping = true;
                        break;
                    }
                    cursor += 1;
                    let created_at = timeline.created_at(message, now);
                    if message.service {
                        run.report.skipped_service += 1;
                    } else if existing.contains_key(&message.id)
                        || new_ids.contains_key(&message.id)
                    {
                        run.report.skipped_existing += 1;
                        counted += 1;
                        let owner = existing
                            .get(&message.id)
                            .filter(|id| !with_files.contains(*id));
                        if let (Some(root), Some(file), Some(owner)) =
                            (root.as_deref(), message.file.as_ref(), owner)
                        {
                            match prepare(&self.dirs(), self.faults(), root, file) {
                                Prepared::Ready(media) => {
                                    batch_files += 1;
                                    batch_bytes += media.bytes();
                                    run.bytes_done += media.bytes();
                                    late.push(Late {
                                        message_id: owner.clone(),
                                        media,
                                        renderings: renderings(message, author_of(chat, message)),
                                    });
                                }
                                Prepared::Missing => run.report.media_missing += 1,
                                Prepared::Skipped => run.report.media_skipped += 1,
                                Prepared::DiskFull => {
                                    disk_full = true;
                                    break;
                                }
                            }
                        }
                    } else {
                        let author = author_of(chat, message);
                        let mut media: Option<StagedMedia> = None;
                        if let (Some(root), Some(file)) = (root.as_deref(), message.file.as_ref()) {
                            match prepare(&self.dirs(), self.faults(), root, file) {
                                Prepared::Ready(staged) => media = Some(staged),
                                Prepared::Missing => run.report.media_missing += 1,
                                Prepared::Skipped => run.report.media_skipped += 1,
                                Prepared::DiskFull => {
                                    disk_full = true;
                                    break;
                                }
                            }
                        }
                        // With the file attached its placeholder line is not written; if that rendering fails, the file is dropped.
                        let attached_body = media
                            .as_ref()
                            .and_then(|_| render_body_attached(message, author, language));
                        let body = match attached_body {
                            Some(body) => Some(body),
                            None => {
                                if let Some(unused) = media.take() {
                                    unused.discard();
                                }
                                render_body(message, author, language)
                            }
                        };
                        let Some(body) = body else {
                            run.report.skipped_empty += 1;
                            continue;
                        };
                        counted += 1;
                        let id = uuid::Uuid::new_v4().to_string();
                        let reply_to_id = message
                            .reply_to
                            .as_ref()
                            .and_then(|r| existing.get(r).or_else(|| new_ids.get(r)))
                            .cloned();
                        placeholders += i64::from(
                            media.is_none()
                                && message
                                    .media
                                    .as_ref()
                                    .is_some_and(|m| !matches!(m, TgMedia::Poll { .. })),
                        );
                        if let Some(staged) = &media {
                            batch_files += 1;
                            batch_bytes += staged.bytes();
                            run.bytes_done += staged.bytes();
                        }
                        new_ids.insert(message.id.clone(), id.clone());
                        rows.push(Row {
                            id,
                            ext_msg: message.id.clone(),
                            body,
                            created_at,
                            journal_date: journal_date_of(created_at, now),
                            reply_to_id,
                            media,
                        });
                    }
                }
                if disk_full {
                    // The batch is rolled back: nothing of it was written, the copies made for it are removed.
                    for staged in rows.iter().filter_map(|row| row.media.as_ref()) {
                        staged.discard();
                    }
                    for item in &late {
                        item.media.discard();
                    }
                    run.report.disk_full = true;
                    self.wake_mirror();
                    return Ok(run.report);
                }
                if !rows.is_empty() || !late.is_empty() {
                    if run.report.backup_name.is_none() {
                        run.report.backup_name = Some(self.backup_before_import(now)?);
                    }
                    let count = rows.len() as i64;
                    let pt_ms = now.ms.saturating_add(started.elapsed().as_millis() as i64);
                    let copies: Vec<PathBuf> = rows
                        .iter()
                        .filter_map(|row| row.media.as_ref())
                        .map(|media| media.staged.part.clone())
                        .chain(late.iter().map(|item| item.media.staged.part.clone()))
                        .collect();
                    let result = self.write_batch_with(
                        chat,
                        dest.clone(),
                        rows,
                        late,
                        pt_ms,
                        now.tz.clone(),
                    );
                    let outcome = match result {
                        Ok(outcome) => outcome,
                        Err(error) => {
                            // Copies not renamed into place are of no use; renamed ones are found by the next sweep.
                            for part in copies {
                                let _ = std::fs::remove_file(part);
                            }
                            return Err(error);
                        }
                    };
                    dest = outcome.chat_id;
                    run.report.chats_created += i64::from(outcome.created_chat);
                    run.report.imported += count - outcome.skipped;
                    run.report.skipped_existing += outcome.skipped;
                    run.report.media_placeholders += placeholders;
                    run.report.media_attached += outcome.attached;
                    run.report.media_bytes += outcome.attached_bytes;
                    existing.extend(new_ids);
                    // Replies of the following batches to a skipped entry go to the real owner of its external id.
                    for id in existing.values_mut() {
                        if let Some(owner) = outcome.replaced.get(id.as_str()) {
                            id.clone_from(owner);
                        }
                    }
                }
                done += counted;
                progress(ImportProgress {
                    done,
                    total,
                    bytes_done: run.bytes_done,
                    bytes_total,
                });
                if stopping {
                    run.report.stopped = true;
                    self.wake_mirror();
                    return Ok(run.report);
                }
            }
        }
        self.wake_mirror();
        Ok(run.report)
    }
}

#[cfg(test)]
mod tests;
