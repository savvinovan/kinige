//! Запись для агента (ADR-0009): те же операции, что у интерфейса, одна транзакция писателя на вызов — проверка выключателя
//! и режима, видимость чата, изменение данных и строка журнала. Отказ откатывает всё, кроме строки журнала об отказе.

use crate::agent::{AgentError, AgentReply, Call, VISIBLE, load_settings};
use crate::agent_entry::to_entry;
use crate::agent_log::insert_log;
use crate::agent_read::chat_visible;
use crate::error::write_err;
use crate::hlc_state::next_hlc;
use crate::messages::{apply, edit_body, load};
use crate::send::insert_agent_message;
use crate::view::load_view;
use crate::{Now, StoreError};
use kinige_domain::agent::ToolArgs;
use kinige_domain::append::{AppendCandidate, can_append_origin, joined_body};
use kinige_domain::groups::{BodyError, INBOX_ID, ORIGIN_AGENT, normalize_body};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::message_ops::{self, FlagChange};
use kinige_domain::{AgentMode, MessageView};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{Value, json};

/// Итог исполнения внутри транзакции.
enum Fail {
    /// Отказ агенту: транзакция откатывается, в журнал идёт строка об отказе.
    Refused(AgentError),
    /// Сбой хранилища: откат, строки журнала нет.
    Store(StoreError),
}

impl From<StoreError> for Fail {
    fn from(error: StoreError) -> Self {
        Fail::Store(error)
    }
}

impl From<rusqlite::Error> for Fail {
    fn from(error: rusqlite::Error) -> Self {
        Fail::Store(write_err(error))
    }
}

struct Done {
    message_id: String,
    reply: Value,
}

/// Задача писателя для инструмента записи. Внешний `Err` — сбой хранилища (ничего не записано); внутренний — отказ агенту
/// (журнал об отказе записан).
pub(crate) fn run_write_job(
    conn: &mut Connection,
    call: &Call,
    args: &ToolArgs,
) -> Result<Result<AgentReply, AgentError>, StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    // Проверка внутри транзакции: выключатель и режим могли измениться после первой проверки.
    let settings = load_settings(&tx)?;
    let result = if !settings.enabled {
        Err(Fail::Refused(AgentError::Disabled))
    } else if settings.mode != AgentMode::ReadWrite {
        Err(Fail::Refused(AgentError::ReadOnly))
    } else {
        execute(&tx, call, args)
    };
    let tool = call.tool.name();
    match result {
        Ok(done) => {
            let ids = [done.message_id];
            insert_log(
                &tx,
                call.now.ms,
                &call.client,
                tool,
                &call.args_json,
                "ok",
                &ids,
            )
            .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(Ok(AgentReply {
                text: done.reply.to_string(),
                changed: true,
            }))
        }
        Err(Fail::Refused(error)) => {
            drop(tx);
            let tx = conn
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(write_err)?;
            insert_log(
                &tx,
                call.now.ms,
                &call.client,
                tool,
                &call.args_json,
                error.outcome(),
                &[],
            )
            .map_err(write_err)?;
            tx.commit().map_err(write_err)?;
            Ok(Err(error))
        }
        Err(Fail::Store(error)) => Err(error),
    }
}

fn execute(tx: &Transaction<'_>, call: &Call, args: &ToolArgs) -> Result<Done, Fail> {
    match args {
        ToolArgs::AddEntry {
            text,
            chat_id,
            task,
        } => add_entry(tx, call, text, chat_id.as_deref(), *task),
        ToolArgs::AppendToLastEntry { text, chat_id } => {
            append_to_last_entry(tx, call, text, chat_id.as_deref())
        }
        ToolArgs::SetTaskDone { id, done } => set_task_done(tx, call, id, *done),
        _ => Err(StoreError::Internal("чтение в записи".into()).into()),
    }
}

fn body_of(text: &str) -> Result<String, Fail> {
    normalize_body(text).map_err(|error| {
        Fail::Refused(AgentError::Invalid(match error {
            BodyError::Empty => "text is empty".into(),
            BodyError::TooLarge => "text is too long".into(),
        }))
    })
}

/// Чат по умолчанию — Inbox, и он проходит ту же проверку видимости, что любой другой.
fn target_chat<'a>(tx: &Transaction<'_>, chat_id: Option<&'a str>) -> Result<&'a str, Fail> {
    let chat = chat_id.unwrap_or(INBOX_ID);
    if chat_visible(tx, chat)? {
        Ok(chat)
    } else {
        Err(Fail::Refused(AgentError::NotFound))
    }
}

fn add_entry(
    tx: &Transaction<'_>,
    call: &Call,
    text: &str,
    chat_id: Option<&str>,
    task: bool,
) -> Result<Done, Fail> {
    let body = body_of(text)?;
    let chat = target_chat(tx, chat_id)?;
    let view = insert_agent_message(tx, chat, &body, task, &call.now)?;
    Ok(done("created", &view, tx, call)?)
}

fn append_to_last_entry(
    tx: &Transaction<'_>,
    call: &Call,
    text: &str,
    chat_id: Option<&str>,
) -> Result<Done, Fail> {
    let addition = body_of(text)?;
    let chat = target_chat(tx, chat_id)?;
    // Цель — последняя живая запись чата; она должна быть записью агента, сегодняшней и не старше десяти минут.
    let last: Option<(String, String, i64, String, String)> = tx
        .query_row(
            "SELECT id, origin, created_at, journal_date, hlc_json FROM message \
             WHERE chat_id = ?1 AND deleted_at IS NULL \
             ORDER BY journal_date DESC, created_at DESC, id DESC LIMIT 1",
            [chat],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .optional()?;
    let Some((id, origin, created_at, date, hlc_json)) = last else {
        return Err(Fail::Refused(AgentError::NoAgentEntry));
    };
    let today = today(&call.now);
    let candidate = AppendCandidate {
        origin: &origin,
        created_at,
        journal_date: &date,
        hlc_json: &hlc_json,
    };
    let allowed = can_append_origin(&candidate, ORIGIN_AGENT, call.now.ms, &today)
        .map_err(StoreError::from)?;
    if !allowed {
        return Err(Fail::Refused(AgentError::NoAgentEntry));
    }
    let loaded = load(tx, &id)?;
    // Запись мог поправить пользователь: версия до дописывания сохраняется ревизией `edit` (исключение из §A.0).
    let combined = joined_body(&loaded.columns.body_md, &addition);
    match edit_body(tx, &loaded, &combined, &call.now) {
        Ok(()) => {}
        Err(StoreError::TooLarge) => {
            return Err(Fail::Refused(AgentError::Invalid(
                "the entry would become too long".into(),
            )));
        }
        Err(error) => return Err(error.into()),
    }
    let view = load_view(tx, &id)?;
    Ok(done("appended", &view, tx, call)?)
}

fn set_task_done(
    tx: &Transaction<'_>,
    call: &Call,
    id: &str,
    done_flag: bool,
) -> Result<Done, Fail> {
    let visible: bool = tx.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM message m JOIN chat c ON c.id = m.chat_id \
             WHERE m.id = ?1 AND m.deleted_at IS NULL AND {VISIBLE})"
        ),
        [id],
        |row| row.get(0),
    )?;
    if !visible {
        return Err(Fail::Refused(AgentError::NotFound));
    }
    let loaded = load(tx, id)?;
    if loaded.ensure_writable().is_err() {
        return Err(Fail::Refused(AgentError::Invalid(
            "the entry is read-only".into(),
        )));
    }
    if !loaded.columns.is_task {
        return Err(Fail::Refused(AgentError::NotATask));
    }
    if loaded.columns.task_done_at.is_some() != done_flag {
        let hlc = next_hlc(tx, call.now.ms)?;
        let rewritten = message_ops::change_flag(
            &loaded.groups(),
            FlagChange::TaskDone(done_flag),
            call.now.ms,
            &hlc,
        )
        .map_err(StoreError::from)?;
        apply(tx, id, &rewritten)?;
    }
    let view = load_view(tx, id)?;
    Ok(done("updated", &view, tx, call)?)
}

fn done(
    action: &str,
    view: &MessageView,
    tx: &Transaction<'_>,
    call: &Call,
) -> Result<Done, StoreError> {
    let title = if view.chat_id == INBOX_ID {
        call.inbox_title.clone()
    } else {
        tx.query_row(
            "SELECT title FROM chat WHERE id = ?1",
            [&view.chat_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(write_err)?
    };
    Ok(Done {
        message_id: view.id.clone(),
        reply: json!({ "action": action, "entry": to_entry(view, &title) }),
    })
}

fn today(now: &Now) -> String {
    journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES)
}
