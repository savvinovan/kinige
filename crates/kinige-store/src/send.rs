//! Отправка записи (ТЗ §5): одна транзакция — запись, HLC в `sync_state`, отметка для Markdown-копии.

use crate::error::write_err;
use crate::hlc_state::{get_state, load_clock, next_hlc, save_clock, set_state};
use crate::{Now, Store, StoreError};
use kinige_domain::MessageView;
use kinige_domain::groups::{self, ChatRow, MessageRow, NewMessage};
use kinige_domain::hlc::Clock;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use uuid::Uuid;

/// Первый запуск: `device_id`, часы HLC и Inbox. Повторный запуск ничего не меняет, кроме часов (они продолжаются, ADR §2).
pub(crate) fn ensure_initialized(conn: &mut Connection, now: &Now) -> Result<(), StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    if get_state(&tx, "device_id").map_err(write_err)?.is_none() {
        set_state(&tx, "device_id", &Uuid::new_v4().to_string()).map_err(write_err)?;
    }
    let pt = u64::try_from(now.ms).unwrap_or(0);
    let clock = match load_clock(&tx)? {
        Some(stored) => Clock::resume(stored.l, stored.c, pt),
        None => Clock { l: pt, c: 0 },
    };
    save_clock(&tx, clock).map_err(write_err)?;
    let inbox_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1)",
            [groups::INBOX_ID],
            |row| row.get(0),
        )
        .map_err(write_err)?;
    if !inbox_exists {
        let hlc = next_hlc(&tx, now.ms)?;
        let chat =
            groups::inbox_chat(&hlc, now.ms).map_err(|e| StoreError::Internal(e.to_string()))?;
        insert_chat(&tx, &chat).map_err(write_err)?;
    }
    tx.commit().map_err(write_err)
}

pub(crate) fn insert_chat(tx: &Transaction<'_>, chat: &ChatRow) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO chat (id, title, icon, color, sort_key, is_inbox, ai_excluded, pinned_at, archived_at, created_at, \
         updated_at, deleted_at, hlc_json, server_known, schema_version, groups_raw) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0, ?14, ?15)",
        rusqlite::params![
            chat.id,
            chat.title,
            chat.icon,
            chat.color,
            chat.sort_key,
            chat.is_inbox,
            chat.ai_excluded,
            chat.pinned_at,
            chat.archived_at,
            chat.created_at,
            chat.updated_at,
            chat.deleted_at,
            chat.hlc_json,
            chat.schema_version,
            chat.groups_raw
        ],
    )?;
    Ok(())
}

fn insert_message(tx: &Transaction<'_>, m: &MessageRow) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO message (id, chat_id, body_md, journal_date, created_at, created_tz, edited_at, is_task, \
         task_done_at, favorite_at, pinned_at, reply_to_id, origin, ext_json, deleted_at, body_base, body_supersedes, \
         placement_base, placement_supersedes, hlc_json, server_known, schema_version, groups_raw) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, 0, ?21, ?22)",
        rusqlite::params![
            m.id,
            m.chat_id,
            m.body_md,
            m.journal_date,
            m.created_at,
            m.created_tz,
            m.edited_at,
            m.is_task,
            m.task_done_at,
            m.favorite_at,
            m.pinned_at,
            m.reply_to_id,
            m.origin,
            m.ext_json,
            m.deleted_at,
            m.body_base,
            m.body_supersedes,
            m.placement_base,
            m.placement_supersedes,
            m.hlc_json,
            m.schema_version,
            m.groups_raw
        ],
    )?;
    Ok(())
}

/// Отмечает день чата как требующий перезаписи файла Markdown-копии. Номер растёт: фоновая задача удаляет отметку,
/// только если она не изменилась с момента чтения.
pub(crate) fn mark_dirty(
    tx: &Transaction<'_>,
    chat_id: &str,
    journal_date: &str,
) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO mirror_dirty (chat_id, journal_date, seq) \
         VALUES (?1, ?2, (SELECT COALESCE(MAX(seq), 0) + 1 FROM mirror_dirty)) \
         ON CONFLICT(chat_id, journal_date) DO UPDATE SET seq = excluded.seq",
        (chat_id, journal_date),
    )?;
    Ok(())
}

/// Новая запись в открытой транзакции: проверка чата, HLC, запись со всеми пятью группами, отметка копии. Фиксирует
/// транзакцию вызывающий; тот же путь у отправки из главного окна и из окна захвата.
pub(crate) fn insert_new_message(
    tx: &Transaction<'_>,
    chat_id: &str,
    body: &str,
    now: &Now,
) -> Result<MessageView, StoreError> {
    let chat_exists: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM chat WHERE id = ?1 AND deleted_at IS NULL)",
            [chat_id],
            |row| row.get(0),
        )
        .map_err(write_err)?;
    if !chat_exists {
        return Err(StoreError::ChatNotFound);
    }
    let id = Uuid::new_v4().to_string();
    let hlc = next_hlc(tx, now.ms)?;
    let date = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
    let row = groups::new_message(
        &NewMessage {
            id: &id,
            chat_id,
            body_md: body,
            created_at: now.ms,
            created_tz: &now.tz,
            journal_date: &date,
        },
        &hlc,
    )
    .map_err(|e| StoreError::Internal(e.to_string()))?;
    insert_message(tx, &row).map_err(write_err)?;
    crate::tags::replace_tags(tx, &row.id, &row.body_md).map_err(write_err)?;
    crate::search::reindex(tx, &row.id, &row.body_md).map_err(write_err)?;
    mark_dirty(tx, chat_id, &date).map_err(write_err)?;
    Ok(MessageView {
        tags: kinige_domain::tags::parse_tags(&row.body_md),
        id: row.id,
        chat_id: row.chat_id,
        body_md: row.body_md,
        journal_date: row.journal_date,
        created_at: row.created_at,
        created_tz: row.created_tz,
        is_task: false,
        task_done_at: None,
        favorite_at: None,
        read_only: false,
    })
}

impl Store {
    /// Отправка (ТЗ §5): текст нормализуется, затем одна транзакция: запись со всеми пятью группами, HLC, отметка копии,
    /// очистка черновика этого чата. Ответ — сохранённая запись; без оптимистичного показа: при ошибке ничего не записано.
    pub fn send_message(
        &self,
        chat_id: &str,
        body: &str,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let body = groups::normalize_body(body)?;
        let chat_id = chat_id.to_string();
        let now = now.clone();
        let view = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let view = insert_new_message(&tx, &chat_id, &body, &now)?;
            crate::draft::delete_draft(&tx, &crate::draft::chat_key(&chat_id))
                .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }
}

/// Помечает все дни с записями (кроме корзины): копия в новой папке пишется целиком.
pub(crate) fn mark_all_dirty(tx: &Transaction<'_>) -> rusqlite::Result<()> {
    tx.execute(
        "INSERT INTO mirror_dirty (chat_id, journal_date, seq) \
         SELECT DISTINCT chat_id, journal_date, (SELECT COALESCE(MAX(seq), 0) + 1 FROM mirror_dirty) \
         FROM message WHERE deleted_at IS NULL \
         ON CONFLICT(chat_id, journal_date) DO UPDATE SET seq = excluded.seq",
        [],
    )?;
    Ok(())
}
