//! Импорт из Telegram (слайс 4, задача 4.8): предпросмотр и запись пачками по 500 сообщений.
//!
//! Каждая пачка — одна транзакция писателя: записи, `message_ext`, теги, индекс поиска, отметки Markdown-копии и HLC. Между
//! пачками писатель свободен для других записей. Остановка и `kill -9` теряют не больше одной незафиксированной пачки;
//! повторный импорт дедуплицируется по `message_ext` и доносит остаток. Копия базы снимается перед первой записью.

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

/// Сообщений в одной пачке (одной транзакции писателя).
pub const IMPORT_BATCH: usize = 500;

fn supported_kind(chat: &TgChat) -> Option<ImportChatKind> {
    match chat.kind {
        TgChatKind::Saved => Some(ImportChatKind::Saved),
        TgChatKind::Personal => Some(ImportChatKind::Personal),
        TgChatKind::Other(_) => None,
    }
}

/// Уже импортированные сообщения чата Telegram: `ext_msg → id записи Kinige`.
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

/// Чат личной переписки при повторном импорте — чат самой свежей живой импортированной записи этого чата Telegram
/// (решение 5): не удалённая запись в не удалённом чате. Нет такой — `None`, чат создаст первая пачка с записями.
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

/// Имя чата Kinige для личной переписки: из экспорта; нет имени — `Telegram <id>`; длиннее предела — обрезается.
fn chat_title(chat: &TgChat) -> String {
    let fallback = format!("Telegram {}", chat.id);
    let Some(name) = chat.name.as_deref() else {
        return fallback;
    };
    let cut: String = name.trim().chars().take(MAX_TITLE_CHARS).collect();
    normalize_title(&cut).unwrap_or(fallback)
}

/// Автор: у чужого сообщения личного чата (`from_id` = `user<id чата>`, решение 6) — имя; свои и «Избранное» — без имени.
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

/// Запись к вставке, подготовленная вне писателя.
struct Prepared {
    id: String,
    ext_msg: String,
    body: String,
    created_at: i64,
    journal_date: String,
    reply_to_id: Option<String>,
}

/// Что вернула пачка.
struct BatchOutcome {
    chat_id: String,
    created_chat: bool,
    /// Записи, чей внешний id оказался занят в самой транзакции: не вставлены.
    skipped: i64,
}

impl Store {
    /// Что нашлось в экспорте: чаты, которые можно импортировать, число записей и сколько уже импортировано.
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

    /// Импорт выбранных чатов (`chats` — их id Telegram). Перед первой записью снимается копия базы; её сбой отменяет
    /// импорт (`ImportBackupFailed`). `progress` вызывается после каждой зафиксированной пачки; `stop` проверяется между
    /// пачками. Ошибка записи пачки оставляет предыдущие пачки на месте: повторный запуск доносит остаток.
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
        let total: i64 = selected.iter().map(|c| c.messages.len() as i64).sum();
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
                for message in batch {
                    let created_at = timeline.created_at(message, now);
                    if message.service {
                        report.skipped_service += 1;
                    } else if existing.contains_key(&message.id)
                        || new_ids.contains_key(&message.id)
                    {
                        report.skipped_existing += 1;
                    } else if let Some(body) =
                        render_body(message, author_of(chat, message), language)
                    {
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
                    dest = Some(outcome.chat_id);
                    report.chats_created += i64::from(outcome.created_chat);
                    report.imported += count - outcome.skipped;
                    report.skipped_existing += outcome.skipped;
                    report.media_placeholders += media;
                    existing.extend(new_ids);
                }
                done += batch.len() as i64;
                progress(ImportProgress { done, total });
            }
        }
        self.wake_mirror();
        Ok(report)
    }

    /// Одна пачка одной транзакцией. `dest` — чат-адресат; `None` — личный чат ещё не создан, его создаёт эта пачка.
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
            // Часы читаются после создания чата, которое двигает их само. Дальше тик идёт в памяти, сохранение — один раз.
            let device = get_state(&tx, "device_id")
                .map_err(write_err)?
                .ok_or_else(|| StoreError::Internal("нет device_id".into()))?;
            let mut clock = load_clock(&tx)?
                .ok_or_else(|| StoreError::Internal("нет состояния часов".into()))?;
            let mut days: BTreeSet<String> = BTreeSet::new();
            let mut skipped = 0i64;
            for row in &rows {
                // Ключ внешнего id первым: он уже занят (запись появилась после чтения `existing`) — записи не будет.
                if !crate::ext::insert(&tx, EXT_SRC_TELEGRAM, &ext_chat, &row.ext_msg, &row.id)
                    .map_err(write_err)?
                {
                    skipped += 1;
                    continue;
                }
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
                        reply_to_id: row.reply_to_id.as_deref(),
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
                chat_id,
                created_chat,
                skipped,
            })
        })
    }
}

#[cfg(test)]
mod tests;
