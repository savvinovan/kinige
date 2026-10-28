//! Записи в ответах агенту (ADR-0009): запись, страница с курсором и ограничением размера, местное время.

use crate::StoreError;
use crate::agent_read::{ReadCtx, ReadFail, ReadOutput};
use chrono::{DateTime, FixedOffset};
use kinige_domain::MessageView;
use kinige_domain::agent::{AgentCursor, LIST_TEXT_CHARS};
use kinige_domain::agent_reply::{AgentChat, AgentEntry, AgentPage, fit_entries};
use kinige_domain::groups::INBOX_ID;

/// Страница записей: текст в списке обрезается, ответ ограничивается по размеру целыми записями, курсор указывает на
/// следующую запись. В журнал идут только записи, которые вошли в ответ.
pub(crate) fn page(
    rows: Vec<(MessageView, String)>,
    has_more: bool,
    chats: Vec<AgentChat>,
    ctx: &ReadCtx<'_>,
) -> Result<ReadOutput, ReadFail> {
    let positions: Vec<AgentCursor> = rows
        .iter()
        .map(|(view, _)| AgentCursor {
            journal_date: view.journal_date.clone(),
            created_at: view.created_at,
            id: view.id.clone(),
        })
        .collect();
    let entries = rows
        .iter()
        .map(|(view, title)| {
            to_entry(view, &chat_title(view, title, ctx.inbox_title)).shorten(LIST_TEXT_CHARS)
        })
        .collect();
    let (entries, cut) = fit_entries(entries);
    let next_cursor = (has_more || cut)
        .then(|| {
            entries
                .len()
                .checked_sub(1)
                .map(|last| positions[last].encode())
        })
        .flatten();
    let ids = entries.iter().map(|entry| entry.id.clone()).collect();
    let text = serde_json::to_string(&AgentPage {
        entries,
        chats,
        next_cursor,
    })
    .map_err(|e| StoreError::Internal(e.to_string()))?;
    Ok(ReadOutput { text, ids })
}

/// Название чата для ответа: Inbox хранится с пустым названием, интерфейс показывает переведённое.
pub(crate) fn chat_title(view: &MessageView, title: &str, inbox_title: &str) -> String {
    if view.chat_id == INBOX_ID {
        inbox_title.to_string()
    } else {
        title.to_string()
    }
}

/// Запись для ответа агенту. `reply_to_id` не отдаётся.
pub(crate) fn to_entry(view: &MessageView, chat_title: &str) -> AgentEntry {
    AgentEntry {
        id: view.id.clone(),
        chat_id: view.chat_id.clone(),
        chat: chat_title.to_string(),
        journal_date: view.journal_date.clone(),
        created: local_time(view.created_at, &view.created_tz),
        origin: view.origin.clone(),
        task: view.is_task,
        done: view.task_done_at.is_some(),
        favorite: view.favorite_at.is_some(),
        text: view.body_md.clone(),
        truncated: false,
    }
}

/// Местное время создания с поясом на тот момент: `2026-03-14T21:05:09+03:00`.
fn local_time(created_at: i64, tz: &str) -> String {
    let offset = crate::offset_secs(tz, created_at)
        .and_then(FixedOffset::east_opt)
        .or_else(|| FixedOffset::east_opt(0));
    DateTime::from_timestamp_millis(created_at)
        .zip(offset)
        .map(|(utc, offset)| {
            utc.with_timezone(&offset)
                .format("%Y-%m-%dT%H:%M:%S%:z")
                .to_string()
        })
        .unwrap_or_default()
}
