//! Sending an entry (PRD §5): one transaction -- the entry, the HLC in `sync_state`, the mark for the Markdown copy.

use crate::error::write_err;
use crate::hlc_state::{get_state, load_clock, next_hlc, save_clock, set_state};
use crate::view::load_reply_to;
use crate::{Now, Store, StoreError};
use kinige_domain::MessageView;
use kinige_domain::groups::{self, ChatRow, GroupError, MessageRow, NewMessage};
use kinige_domain::hlc::{Clock, is_uuid_v4};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use rusqlite::{Connection, Transaction, TransactionBehavior};
use uuid::Uuid;

/// First launch: `device_id`, the HLC clock and the Inbox. A repeated launch changes nothing except the clock (it continues, ADR §2).
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

pub(crate) fn insert_message(tx: &Transaction<'_>, m: &MessageRow) -> rusqlite::Result<()> {
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

/// Marks a chat day as needing its Markdown copy file rewritten. The number grows: the background task removes the mark
/// only if it has not changed since it was read.
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

/// A new entry in an open transaction: chat check, HLC, the entry with all five groups, the copy mark. The caller
/// commits the transaction; sending from the main window and from the capture window takes the same path.
pub(crate) fn insert_new_message(
    tx: &Transaction<'_>,
    chat_id: &str,
    body: &str,
    now: &Now,
) -> Result<MessageView, StoreError> {
    insert_built_message(tx, chat_id, body, now, groups::new_message)
}

/// A reply to an entry (slice 5): the same path, `core.reply_to_id` = the id of the target entry. The target is checked
/// here, in the writer transaction: no such entry in this chat, it is in the trash or the id is not a UUID v4 --
/// `ReplyTargetNotFound`, nothing is written.
pub(crate) fn insert_reply_message(
    tx: &Transaction<'_>,
    chat_id: &str,
    body: &str,
    reply_to_id: &str,
    now: &Now,
) -> Result<MessageView, StoreError> {
    check_reply_target(tx, chat_id, reply_to_id)?;
    insert_built_message(tx, chat_id, body, now, |input, hlc| {
        groups::new_reply_message(input, reply_to_id, hlc)
    })
}

/// The reply target is alive and lies in the same chat as the reply. A reply from the capture window and from an agent has no target.
fn check_reply_target(
    tx: &Transaction<'_>,
    chat_id: &str,
    reply_to_id: &str,
) -> Result<(), StoreError> {
    if !is_uuid_v4(reply_to_id) {
        return Err(StoreError::ReplyTargetNotFound);
    }
    let alive: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM message WHERE id = ?1 AND chat_id = ?2 AND deleted_at IS NULL)",
            (reply_to_id, chat_id),
            |row| row.get(0),
        )
        .map_err(write_err)?;
    if alive {
        Ok(())
    } else {
        Err(StoreError::ReplyTargetNotFound)
    }
}

/// A new agent entry (ADR-0009): the same path, `origin = 'agent'`; with `task` the `flags` group is created as a task at once.
/// The chat was already checked by the caller (the set visible to the agent), here only its existence.
pub(crate) fn insert_agent_message(
    tx: &Transaction<'_>,
    chat_id: &str,
    body: &str,
    task: bool,
    now: &Now,
) -> Result<MessageView, StoreError> {
    insert_built_message(tx, chat_id, body, now, |input, hlc| {
        groups::new_agent_message(input, task, hlc)
    })
}

fn insert_built_message(
    tx: &Transaction<'_>,
    chat_id: &str,
    body: &str,
    now: &Now,
    build: impl FnOnce(&NewMessage<'_>, &str) -> Result<MessageRow, GroupError>,
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
    let row = build(
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
    let reply_to = match row.reply_to_id.as_deref() {
        Some(id) => Some(load_reply_to(tx, id, &date)?),
        None => None,
    };
    Ok(MessageView {
        reply_to,
        tags: kinige_domain::tags::parse_tags(&row.body_md),
        id: row.id,
        chat_id: row.chat_id,
        body_md: row.body_md,
        journal_date: row.journal_date,
        created_at: row.created_at,
        created_tz: row.created_tz,
        is_task: row.is_task,
        task_done_at: None,
        favorite_at: None,
        read_only: false,
        origin: row.origin,
    })
}

impl Store {
    /// Send (PRD §5): the text is normalized, then one transaction: the entry with all five groups, HLC, the copy mark,
    /// clearing this chat's draft. The result is the saved entry; no optimistic display: on error nothing is written.
    pub fn send_message(
        &self,
        chat_id: &str,
        body: &str,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        self.send_reply(chat_id, body, None, now)
    }

    /// Send, possibly as a reply to an entry (slice 5): with `reply_to_id` the target is checked in the same transaction
    /// (`ReplyTargetNotFound` if it is missing). The chat draft, including the "Reply to entry" line, is cleared there too.
    pub fn send_reply(
        &self,
        chat_id: &str,
        body: &str,
        reply_to_id: Option<&str>,
        now: &Now,
    ) -> Result<MessageView, StoreError> {
        let body = groups::normalize_body(body)?;
        let chat_id = chat_id.to_string();
        let reply_to_id = reply_to_id.map(str::to_string);
        let now = now.clone();
        let view = self.write(move |conn| {
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            let view = match &reply_to_id {
                Some(target) => insert_reply_message(&tx, &chat_id, &body, target, &now)?,
                None => insert_new_message(&tx, &chat_id, &body, &now)?,
            };
            crate::draft::delete_draft(&tx, &crate::draft::chat_key(&chat_id))
                .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(view)
        })?;
        self.wake_mirror();
        Ok(view)
    }
}

/// Marks all days that have entries (except the trash): the copy in a new folder is written in full.
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
