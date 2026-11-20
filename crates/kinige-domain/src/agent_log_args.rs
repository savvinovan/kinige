//! `args_json` of the agent call log (ADR-0009): an allowlist of the parsed arguments, never the content. No IO.

use crate::agent::{LOG_QUERY_CHARS, ToolArgs, truncate_chars};
use crate::agent_prefs::set_setting_args_json;
use serde_json::{Map, Value};

/// `args_json` of the call log is an **allow-list** of the parsed arguments' fields: chat, dates, flags and `query`, truncated
/// to [`LOG_QUERY_CHARS`]. The entry text (`text`) and entry ids are never here: ids go to `mcp_log_message`. For `set_setting` it
/// is the setting's name and the canonical value (the settings window's wording, no user text).
pub fn args_json(args: &ToolArgs) -> String {
    let mut out = Map::new();
    match args {
        ToolArgs::ListChats | ToolArgs::GetEntry { .. } | ToolArgs::GetSettings => {}
        ToolArgs::SetSetting { key, value } => {
            return set_setting_args_json(Some(key.name()), *value);
        }
        ToolArgs::Search(search) => {
            put(
                &mut out,
                "query",
                Some(&truncate_chars(&search.query, LOG_QUERY_CHARS)),
            );
            put(&mut out, "chat_id", search.chat_id.as_ref());
            put(&mut out, "from", search.from.as_ref());
            put(&mut out, "to", search.to.as_ref());
            flag(&mut out, "favorites", search.favorites);
            flag(&mut out, "tasks", search.tasks);
            flag(&mut out, "page", search.cursor.is_some());
        }
        ToolArgs::GetEntries(entries) => {
            put(&mut out, "from", Some(&entries.from));
            put(&mut out, "to", Some(&entries.to));
            put(&mut out, "chat_id", entries.chat_id.as_ref());
            flag(&mut out, "page", entries.cursor.is_some());
        }
        ToolArgs::OnThisDay { date } => put(&mut out, "date", date.as_ref()),
        ToolArgs::AddEntry { chat_id, task, .. } => {
            put(&mut out, "chat_id", chat_id.as_ref());
            flag(&mut out, "task", *task);
        }
        ToolArgs::AppendToLastEntry { chat_id, .. } => put(&mut out, "chat_id", chat_id.as_ref()),
        ToolArgs::SetTaskDone { done, item, .. } => {
            out.insert("done".into(), Value::Bool(*done));
            // Only the item number: its text never reaches the log.
            if let Some(item) = item {
                out.insert("item".into(), Value::from(*item));
            }
        }
    }
    Value::Object(out).to_string()
}

/// The same for a call that did not succeed: a `set_setting` value is the agent's, so it is left out (a refused value never
/// reaches the log); everything else is as in [`args_json`].
pub fn refusal_args_json(args: &ToolArgs) -> String {
    match args {
        ToolArgs::SetSetting { key, .. } => set_setting_args_json(Some(key.name()), None),
        other => args_json(other),
    }
}

fn put(out: &mut Map<String, Value>, key: &str, value: Option<&String>) {
    if let Some(value) = value {
        out.insert(key.into(), Value::String(value.clone()));
    }
}

fn flag(out: &mut Map<String, Value>, key: &str, on: bool) {
    if on {
        out.insert(key.into(), Value::Bool(true));
    }
}
