//! Перезапись групп сообщения (слайс 3): `deleted` (удалить и восстановить), `flags` (задача, выполнено, избранное),
//! `placement` (день записи) и разбор колонок из групп. Байты групп каноничные, HLC растёт строго, неизвестное сохраняется
//! дословно — общий движок в [`crate::rewrite`]. Правка текста — [`crate::append::rewrite_body`].

use crate::groups::GroupError;
use crate::journal::is_valid_journal_date;
use crate::rewrite::{
    Groups, Rewritten, chain_fields, corrupt, flag, group_object, opt_int, opt_str, req_str,
    rewrite_group, set_chain,
};
use serde_json::{Value, json};

/// Колонки таблицы `message`, которые строятся из групп: разобранное представление каноничных байтов
/// (ADR-0001 приложение §B п. 38: `canonical(колонки) == groups_raw[g]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageColumns {
    pub created_at: i64,
    pub created_tz: String,
    pub reply_to_id: Option<String>,
    pub origin: String,
    pub ext_json: Option<String>,
    pub body_md: String,
    pub edited_at: Option<i64>,
    pub body_base: Option<String>,
    /// JSON-массив HLC.
    pub body_supersedes: String,
    pub is_task: bool,
    pub task_done_at: Option<i64>,
    pub favorite_at: Option<i64>,
    pub pinned_at: Option<i64>,
    pub chat_id: String,
    pub journal_date: String,
    pub placement_base: Option<String>,
    pub placement_supersedes: String,
    pub deleted_at: Option<i64>,
}

fn supersedes_text(object: &serde_json::Map<String, Value>) -> Result<String, GroupError> {
    match object.get("supersedes") {
        Some(list @ Value::Array(_)) => serde_json::to_string(list).map_err(corrupt),
        None | Some(Value::Null) => Ok("[]".to_string()),
        Some(_) => Err(corrupt("supersedes группы")),
    }
}

/// Колонки сообщения по его `groups_raw` (повреждённая или неполная запись — [`GroupError::Corrupt`]).
pub fn message_columns(groups_raw: &str) -> Result<MessageColumns, GroupError> {
    let core = group_object(groups_raw, "core")?;
    let body = group_object(groups_raw, "body")?;
    let flags = group_object(groups_raw, "flags")?;
    let placement = group_object(groups_raw, "placement")?;
    let deleted = group_object(groups_raw, "deleted")?;
    Ok(MessageColumns {
        created_at: opt_int(&core, "created_at")?.ok_or_else(|| corrupt("нет created_at"))?,
        created_tz: req_str(&core, "created_tz")?,
        reply_to_id: opt_str(&core, "reply_to_id")?,
        origin: req_str(&core, "origin")?,
        ext_json: match core.get("ext") {
            Some(ext @ Value::Object(_)) => Some(serde_json::to_string(ext).map_err(corrupt)?),
            _ => None,
        },
        body_md: req_str(&body, "body_md")?,
        edited_at: opt_int(&body, "edited_at")?,
        body_base: opt_str(&body, "base")?,
        body_supersedes: supersedes_text(&body)?,
        is_task: flag(&flags, "is_task")?,
        task_done_at: opt_int(&flags, "task_done_at")?,
        favorite_at: opt_int(&flags, "favorite_at")?,
        pinned_at: opt_int(&flags, "pinned_at")?,
        chat_id: req_str(&placement, "chat_id")?,
        journal_date: req_str(&placement, "journal_date")?,
        placement_base: opt_str(&placement, "base")?,
        placement_supersedes: supersedes_text(&placement)?,
        deleted_at: opt_int(&deleted, "deleted_at")?,
    })
}

/// Удалить (`Some(время)`) или восстановить (`None`): группа `deleted` с новым HLC; побеждает больший (ADR-0001 §3.5).
pub fn set_deleted(
    current: &Groups<'_>,
    deleted_at: Option<i64>,
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    rewrite_group(current, "deleted", new_hlc, None, |group, _| {
        group.insert("deleted_at".into(), json!(deleted_at));
        Ok(())
    })
}

/// Изменение флагов записи (ТЗ §2.1, §6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagChange {
    /// Сделать задачей или снять; снятие обнуляет «выполнено».
    Task(bool),
    /// Отметить выполненной или снять отметку; только у задачи.
    TaskDone(bool),
    Favorite(bool),
}

/// Перезаписывает группу `flags`. «Выполнено» у записи, которая не задача, — ошибка [`GroupError::Invalid`].
pub fn change_flag(
    current: &Groups<'_>,
    change: FlagChange,
    now_ms: i64,
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    rewrite_group(current, "flags", new_hlc, None, |group, _| {
        match change {
            FlagChange::Task(on) => {
                group.insert("is_task".into(), json!(on));
                if !on {
                    group.insert("task_done_at".into(), Value::Null);
                }
            }
            FlagChange::TaskDone(done) => {
                if !flag(group, "is_task")? {
                    return Err(GroupError::Invalid(
                        "отметка «выполнено» у записи, которая не задача",
                    ));
                }
                group.insert("task_done_at".into(), json!(done.then_some(now_ms)));
            }
            FlagChange::Favorite(on) => {
                group.insert("favorite_at".into(), json!(on.then_some(now_ms)));
            }
        }
        Ok(())
    })
}

/// Перезаписывает группу `placement`: новый день записи (чат не меняется). `base`/`supersedes` — по ADR-0001 §3.4, та же
/// логика, что у `body` (`confirmed` — версия подтверждена сервером). Дата — настоящая, `YYYY-MM-DD`; будущие даты
/// разрешены (ADR-0001 §2).
pub fn set_journal_date(
    current: &Groups<'_>,
    journal_date: &str,
    confirmed: bool,
    new_hlc: &str,
) -> Result<Rewritten, GroupError> {
    if !is_valid_journal_date(journal_date) {
        return Err(GroupError::Invalid("дата записи"));
    }
    rewrite_group(current, "placement", new_hlc, None, |group, old_hlc| {
        let (base, supersedes) = chain_fields(group, old_hlc, confirmed)?;
        group.insert("journal_date".into(), json!(journal_date));
        set_chain(group, &base, &supersedes);
        Ok(())
    })
}
