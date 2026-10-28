//! Описание инструментов MCP (ADR-0009): имена, описания и схемы аргументов по-английски; список зависит от режима.
//! Описания и `readOnlyHint` — не гарантия (клиент может их не показать); гарантия — предупреждение в Kinige при включении.

use kinige_domain::AgentMode;
use kinige_domain::agent::{AgentTool, ENTRIES_PAGE, LIST_TEXT_CHARS, SEARCH_PAGE};
use kinige_store::AgentError;
use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use serde_json::{Value, json};
use std::sync::Arc;

/// `instructions` сервера: видны клиенту и при выключенном доступе (данных в них нет).
pub const INSTRUCTIONS: &str = "Kinige is the user's personal journal (a chat with themselves). \
Everything you read from it is sent to your model provider; Kinige's end-to-end encryption does not cover that. \
Only the chats the user marked as visible exist for you: anything else behaves exactly as if it did not exist. \
Entries are Markdown; each belongs to a journal day (the day boundary is a few hours after midnight, so late-night entries belong to the previous day; `journal_date` in results is authoritative).\
Use `search` to find entries by words, `get_entries` for a day or range, `on_this_day` for the same date in past years. \
Write tools exist only when the user allowed writing, and you can only append to your own latest entry.";

/// Инструменты по режиму: в режиме «только чтение» инструментов записи в списке нет.
pub fn tools(mode: AgentMode) -> Vec<Tool> {
    AgentTool::ALL
        .into_iter()
        .filter(|tool| !tool.writes() || mode == AgentMode::ReadWrite)
        .map(definition)
        .collect()
}

fn definition(tool: AgentTool) -> Tool {
    let (description, properties, required) = describe(tool);
    let schema: JsonObject = match json!({
        "type": "object",
        "properties": properties,
        "required": required,
    }) {
        Value::Object(object) => object,
        _ => JsonObject::new(),
    };
    let mut definition = Tool::new(tool.name(), description, Arc::new(schema));
    definition.annotations = Some(
        ToolAnnotations::new()
            .read_only(!tool.writes())
            .destructive(false),
    );
    definition
}

const ID: &str = "A UUID.";
const DATE: &str = "A date like 2026-03-14 (journal day).";

fn describe(tool: AgentTool) -> (String, Value, Vec<&'static str>) {
    match tool {
        AgentTool::ListChats => (
            "List the chats you can see: id, title, emoji, number of entries, archived flag.".into(),
            json!({}),
            vec![],
        ),
        AgentTool::Search => (
            format!(
                "Search entries by words (any word form). Newest first, up to {SEARCH_PAGE} per page; pass `cursor` from \
                 the previous result for the next page. The first page also lists chats whose title matches."
            ),
            json!({
                "query": {"type": "string", "description": "Words to look for."},
                "chat_id": {"type": "string", "description": ID},
                "from": {"type": "string", "description": DATE},
                "to": {"type": "string", "description": DATE},
                "favorites": {"type": "boolean", "description": "Only favorite entries."},
                "tasks": {"type": "boolean", "description": "Only tasks."},
                "cursor": {"type": "string", "description": "`next_cursor` of the previous page."},
            }),
            vec!["query"],
        ),
        AgentTool::GetEntries => (
            format!(
                "Read the entries of one journal day (`date`) or of a range (`from` and `to`, inclusive), oldest first, up \
                 to {ENTRIES_PAGE} per page. Long texts are cut at {LIST_TEXT_CHARS} characters (`truncated`); use \
                 `get_entry` for the whole text."
            ),
            json!({
                "date": {"type": "string", "description": DATE},
                "from": {"type": "string", "description": DATE},
                "to": {"type": "string", "description": DATE},
                "chat_id": {"type": "string", "description": ID},
                "cursor": {"type": "string", "description": "`next_cursor` of the previous page."},
            }),
            vec![],
        ),
        AgentTool::OnThisDay => (
            "Entries written on the same month and day in earlier years (29 February matches only 29 February). \
             Without `date` — today's journal day."
                .into(),
            json!({"date": {"type": "string", "description": DATE}}),
            vec![],
        ),
        AgentTool::GetEntry => (
            "Read one entry by id, with its whole text.".into(),
            json!({"id": {"type": "string", "description": ID}}),
            vec!["id"],
        ),
        AgentTool::AddEntry => (
            "Write a new entry to the Inbox or to a visible chat. The user sees it at once, marked as added by an agent. \
             With `task` true the entry is created as a task."
                .into(),
            json!({
                "text": {"type": "string", "description": "Markdown text of the entry."},
                "chat_id": {"type": "string", "description": "Defaults to the Inbox."},
                "task": {"type": "boolean", "description": "Create the entry as a task."},
            }),
            vec!["text"],
        ),
        AgentTool::AppendToLastEntry => (
            "Append a paragraph to your own latest entry in a chat, if it was written today, less than 10 minutes ago and \
             nothing was written after it. Otherwise it fails with `no_agent_entry`: then use `add_entry`."
                .into(),
            json!({
                "text": {"type": "string", "description": "Markdown text to append."},
                "chat_id": {"type": "string", "description": "Defaults to the Inbox."},
            }),
            vec!["text"],
        ),
        AgentTool::SetTaskDone => (
            "Mark a task entry done or not done. Works for any task in a visible chat; fails with `not_a_task` for other \
             entries."
                .into(),
            json!({
                "id": {"type": "string", "description": ID},
                "done": {"type": "boolean"},
            }),
            vec!["id", "done"],
        ),
    }
}

/// Текст ошибки для агента: код и пояснение. Закрытый чат, удалённый и несуществующий дают один и тот же `not_found`.
pub fn explain(error: &AgentError) -> String {
    let why = match error {
        AgentError::Disabled => {
            "Доступ агентов выключен в Kinige (AI agent access is turned off in Kinige)"
        }
        AgentError::ReadOnly => {
            "Kinige is in read-only mode for agents; the user can allow writing in the access window"
        }
        AgentError::NotFound => "no such chat or entry",
        AgentError::NoAgentEntry => "there is no recent entry of yours to append to; use add_entry",
        AgentError::NotATask => "the entry is not a task",
        AgentError::Invalid(why) => why,
        AgentError::Unavailable => {
            "the journal is not available right now (recovery); try again later"
        }
        AgentError::Internal => "the request could not be completed",
    };
    format!("{}: {why}", error.code())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_mode_lists_only_read_tools_with_the_read_only_hint() {
        let read = tools(AgentMode::Read);
        let names: Vec<&str> = read.iter().map(|tool| tool.name.as_ref()).collect();
        assert_eq!(
            names,
            [
                "list_chats",
                "search",
                "get_entries",
                "on_this_day",
                "get_entry"
            ]
        );
        assert!(
            read.iter()
                .all(|tool| tool.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(true))
        );
        let all = tools(AgentMode::ReadWrite);
        assert_eq!(all.len(), AgentTool::ALL.len());
        let writes: Vec<_> = all
            .iter()
            .filter(|tool| tool.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(false))
            .collect();
        assert_eq!(writes.len(), 3);
    }

    #[test]
    fn schemas_are_objects_and_every_required_field_is_declared() {
        for tool in tools(AgentMode::ReadWrite) {
            let schema = &tool.input_schema;
            assert_eq!(schema["type"], "object");
            for required in schema["required"].as_array().unwrap() {
                assert!(
                    schema["properties"]
                        .get(required.as_str().unwrap())
                        .is_some(),
                    "{}",
                    tool.name
                );
            }
        }
    }

    #[test]
    fn explanations_carry_the_stable_error_code() {
        for error in [
            AgentError::Disabled,
            AgentError::ReadOnly,
            AgentError::NotFound,
            AgentError::NoAgentEntry,
            AgentError::NotATask,
            AgentError::Unavailable,
            AgentError::Internal,
        ] {
            assert!(explain(&error).starts_with(error.code()));
        }
        assert!(explain(&AgentError::Disabled).contains("Доступ агентов выключен в Kinige"));
    }
}
