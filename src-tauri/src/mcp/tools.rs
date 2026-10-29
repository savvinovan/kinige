//! Описание инструментов MCP (ADR-0009): имена, описания и схемы аргументов по-английски; список зависит от режима.
//! Описания и `readOnlyHint` — не гарантия (клиент может их не показать); гарантия — предупреждение в Kinige при включении.

use kinige_domain::AgentMode;
use kinige_domain::agent::{AgentTool, ENTRIES_PAGE, LIST_TEXT_CHARS, MAX_TASK_ITEM, SEARCH_PAGE};
use kinige_store::AgentError;
use rmcp::model::{JsonObject, Tool, ToolAnnotations};
use serde_json::{Value, json};
use std::sync::Arc;

/// Какой Markdown Kinige показывает, а какой нет. Список взят из настроек рендера ленты (`web/src/ui/message/markdown.ts`:
/// markdown-it с `html: false`, отключены `image` и `table`, DOMPurify пропускает p, br, hr, h1–h6, ul, ol, li, blockquote,
/// pre, code, em, strong, s, a; ссылки только http, https и mailto; свои правила — задачи `- [ ]` и теги `#слово`). Сервер
/// текст не проверяет и не отклоняет: запись пользователя сохраняется как есть. Макрос, а не `const`, чтобы подставляться
/// в `concat!` (`INSTRUCTIONS`) и в `format!` (описания инструментов).
macro_rules! markdown_hint {
    () => {
        "Kinige renders CommonMark with task lists: headings (#), **bold**, *italic*, ~~strikethrough~~, `inline code`, \
         fenced code blocks, bulleted, numbered and nested lists, task lists (`- [ ] item`), block quotes, horizontal rules, \
         links (http, https and mailto) and #tags; a single line break stays a line break. It does NOT render tables, \
         images or raw HTML: they show up as plain text. Use a list instead of a table."
    };
}

/// `instructions` сервера: видны клиенту и при выключенном доступе (данных в них нет).
pub const INSTRUCTIONS: &str = concat!(
    "Kinige is the user's personal journal (a chat with themselves). \
Everything you read from it is sent to your model provider; Kinige's end-to-end encryption does not cover that. \
Only the chats the user marked as visible exist for you: anything else behaves exactly as if it did not exist. \
Entries are Markdown; each belongs to a journal day (the day boundary is a few hours after midnight, so late-night entries belong to the previous day; `journal_date` in results is authoritative). \
Use `search` to find entries by words, `get_entries` for a day or range, `on_this_day` for the same date in past years. \
Write tools exist only when the user allowed writing, and you can only append to your own latest entry. ",
    markdown_hint!()
);

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
                 `get_entry` for the whole text. An entry that contains checklist items (`- [ ] ...`) also has `tasks`: \
                 `[{{item, done, text}}]`; the item numbers are what `set_task_done` takes as `item`. In a list the texts of \
                 the items count toward the same {LIST_TEXT_CHARS}-character limit as the entry text: when they are cut, \
                 `tasks` holds only the first items (their numbers stay valid) and the entry is marked `truncated`; use \
                 `get_entry` for all of them."
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
            "Read one entry by id, with its whole text. If the entry contains checklist items (`- [ ] ...`), `tasks` lists \
             them as `[{item, done, text}]`; use the `item` number with `set_task_done` to mark a single item."
                .into(),
            json!({"id": {"type": "string", "description": ID}}),
            vec!["id"],
        ),
        AgentTool::AddEntry => (
            format!(
                "Write a new entry to the Inbox or to a visible chat. The user sees it at once, marked as added by an \
                 agent. With `task` true the entry is created as a task. {}",
                markdown_hint!()
            ),
            json!({
                "text": {"type": "string", "description": "Markdown text of the entry."},
                "chat_id": {"type": "string", "description": "Defaults to the Inbox."},
                "task": {"type": "boolean", "description": "Create the entry as a task."},
            }),
            vec!["text"],
        ),
        AgentTool::AppendToLastEntry => (
            format!(
                "Append a paragraph to your own latest entry in a chat, if it was written today, less than 10 minutes ago \
                 and nothing was written after it. Otherwise it fails with `no_agent_entry`: then use `add_entry`. {}",
                markdown_hint!()
            ),
            json!({
                "text": {"type": "string", "description": "Markdown text to append."},
                "chat_id": {"type": "string", "description": "Defaults to the Inbox."},
            }),
            vec!["text"],
        ),
        AgentTool::SetTaskDone => (
            "Mark a task done or not done. Without `item` it marks the whole entry (a task entry); it fails with \
             `not_a_task` for other entries. With `item` it marks a single checklist item (`- [ ] ...`) inside the entry, \
             which does not have to be a task; the numbers start at 0 and are the `item` values of `tasks` in the result \
             of `get_entry` and `get_entries`. Always pass `item_text` (the `text` of that item) together with `item`: the \
             user may edit the entry after you read it and the numbers then shift; if the text differs, nothing is \
             written and the call fails with `invalid_argument` (re-read the entry and try again). A wrong item number \
             fails with `invalid_argument` too; an item that is already in the requested state is a success. Works in any \
             visible chat."
                .into(),
            json!({
                "id": {"type": "string", "description": ID},
                "done": {"type": "boolean"},
                "item": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": MAX_TASK_ITEM,
                    "description": "Number of one checklist item inside the entry (0-based, from `tasks` of `get_entry`). Leave it out to mark the whole entry.",
                },
                "item_text": {
                    "type": "string",
                    "description": "The `text` of that item as `tasks` gave it; required in practice together with `item`, protects against shifted numbers. Take it from `get_entry`: in lists long texts are cut.",
                },
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

    /// Агенту сказано, что Kinige не рисует таблицы, картинки и сырой HTML, и что вместо таблицы лучше список: подсказка
    /// стоит в описаниях обоих инструментов записи и в `instructions`. Она взята из настроек рендера ленты
    /// (`web/src/ui/message/markdown.ts`): если рендер изменится, список здесь нужно обновить.
    #[test]
    fn the_write_tools_and_the_instructions_say_which_markdown_is_rendered() {
        let all = tools(AgentMode::ReadWrite);
        for name in ["add_entry", "append_to_last_entry"] {
            let tool = all.iter().find(|tool| tool.name == name).unwrap();
            let text = tool.description.as_deref().unwrap_or_default();
            for must in [
                "tables",
                "images",
                "raw HTML",
                "Use a list instead of a table",
                "task lists",
                "~~strikethrough~~",
            ] {
                assert!(text.contains(must), "{name}: нет «{must}»: {text}");
            }
        }
        // Списки: пункты в `tasks` режутся тем же пределом, агенту сказано, что это видно по `truncated`.
        let entries = all.iter().find(|tool| tool.name == "get_entries").unwrap();
        let text = entries.description.as_deref().unwrap_or_default();
        assert!(
            text.contains("`tasks` holds only the first items") && text.contains("`truncated`")
        );
        for must in ["does NOT render tables", "Use a list instead of a table"] {
            assert!(INSTRUCTIONS.contains(must), "instructions: нет «{must}»");
        }
        // Склейка `concat!` не потеряла пробел между предложениями.
        assert!(INSTRUCTIONS.contains("authoritative). Use `search`"));
        assert!(INSTRUCTIONS.contains("latest entry. Kinige renders"));
    }

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
