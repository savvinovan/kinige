//! Writing for the agent (ADR-0009): the same operations as the interface, one writer transaction per call -- the check of
//! the switch and the mode, chat visibility, the data change and the log row. A refusal rolls back everything except the refusal log row.

use crate::agent::{AgentError, AgentReply, Call, VISIBLE, load_settings};
use crate::agent_entry::to_entry;
use crate::agent_log::insert_log;
use crate::agent_read::chat_visible;
use crate::error::write_err;
use crate::hlc_state::next_hlc;
use crate::message_body::edit_body;
use crate::messages::{apply, load};
use crate::send::insert_agent_message;
use crate::view::load_view;
use crate::{Now, StoreError};
use kinige_domain::agent::ToolArgs;
use kinige_domain::agent_reply::fit_entries;
use kinige_domain::append::{AppendCandidate, can_append_origin, joined_body};
use kinige_domain::groups::{BodyError, INBOX_ID, ORIGIN_AGENT, normalize_body};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::message_ops::{self, FlagChange};
use kinige_domain::tasks::{TaskItemError, set_task_item, task_items};
use kinige_domain::{AgentMode, MessageView};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{Value, json};

/// The outcome of execution inside the transaction.
enum Fail {
    /// A refusal to the agent: the transaction is rolled back, a refusal row goes to the log.
    Refused(AgentError),
    /// A storage failure: rollback, no log row.
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

/// The writer job for a write tool. The outer `Err` is a storage failure (nothing written); the inner one is a refusal
/// to the agent (the refusal log row is written).
pub(crate) fn run_write_job(
    conn: &mut Connection,
    call: &Call,
    args: &ToolArgs,
) -> Result<Result<AgentReply, AgentError>, StoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(write_err)?;
    // The check inside the transaction: the switch and the mode may have changed after the first check.
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
        ToolArgs::SetTaskDone {
            id,
            done,
            item,
            item_text,
        } => set_task_done(tx, call, id, *done, *item, item_text.as_deref()),
        _ => Err(StoreError::Internal("read tool in a write".into()).into()),
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

/// The default chat is Inbox, and it passes the same visibility check as any other.
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
    // The target is the last live entry of the chat; it must be an agent entry, from today and no older than ten minutes.
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
    // The user may have edited the entry: the version before the append is kept as an `edit` revision (an exception to §A.0).
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

/// Without `item` the task is the whole entry. With `item` it is one `- [ ]` item inside an entry (the entry need not be
/// a task): the text is edited by the same `set_task_item` as a click in the feed and is written with the same `edit`
/// revision. No such item gives an `invalid_argument` refusal (the entry is already visible to the agent, so this is an
/// error of its item number, not a hidden entry); an item already in the wanted state is a success with no write. Item
/// numbers shift if the user edits the entry between the read and the mark: with `item_text` (the item text as `tasks`
/// returned it) the item found by number is compared, and on a mismatch the result is `invalid_argument` and nothing is
/// written; without `item_text` the number is taken as is.
fn set_task_done(
    tx: &Transaction<'_>,
    call: &Call,
    id: &str,
    done_flag: bool,
    item: Option<usize>,
    item_text: Option<&str>,
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
    if let Some(index) = item {
        let items = task_items(&loaded.columns.body_md);
        let Some(found) = items.get(index) else {
            return Err(Fail::Refused(AgentError::Invalid(
                "no such checklist item in this entry".into(),
            )));
        };
        if item_text.is_some_and(|expected| expected != found.text) {
            return Err(Fail::Refused(AgentError::Invalid(
                "checklist item changed; re-read the entry".into(),
            )));
        }
        match set_task_item(&loaded.columns.body_md, index, done_flag) {
            Ok(body) => match edit_body(tx, &loaded, &body, &call.now) {
                Ok(()) => {}
                Err(StoreError::TooLarge) => {
                    return Err(Fail::Refused(AgentError::Invalid(
                        "the entry would become too long".into(),
                    )));
                }
                Err(error) => return Err(error.into()),
            },
            Err(TaskItemError::AlreadyThere) => {}
            Err(TaskItemError::NotFound) => {
                return Err(Fail::Refused(AgentError::Invalid(
                    "no such checklist item in this entry".into(),
                )));
            }
        }
        let view = load_view(tx, id)?;
        return Ok(done("updated", &view, tx, call)?);
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
    // The write reply is also capped at `RESPONSE_BYTES`, as in `get_entry`: an entry with tens of thousands of items gives `tasks`
    // twice the size of the text itself. The entry in the database stays whole, only the reply is cut (the `truncated` flag).
    let (entries, _) = fit_entries(vec![to_entry(view, &title)]);
    Ok(Done {
        message_id: view.id.clone(),
        reply: json!({ "action": action, "entry": entries.first() }),
    })
}

fn today(now: &Now) -> String {
    journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES)
}
