//! ИИ-агент через MCP (ADR-0009, слайс 4a): разбор и проверка аргументов инструментов, формы ответов, «журнал без
//! содержимого». Без IO. Права доступа здесь не проверяются: их каждый раз проверяет хранилище внутри запроса.

use crate::hlc::is_uuid;
use crate::journal::is_valid_journal_date;
use serde_json::{Map, Value};

/// Длина поискового запроса в знаках: дальше запрос не имеет смысла для индекса слов.
pub const MAX_QUERY_CHARS: usize = 1000;
/// `query` в журнале обращений обрезается до этой длины (ADR-0009).
pub const LOG_QUERY_CHARS: usize = 128;
/// `client` в журнале: то, что клиент сообщил о себе, не длиннее.
pub const CLIENT_CHARS: usize = 64;
/// Записей на странице поиска.
pub const SEARCH_PAGE: usize = 50;
/// Записей на странице дня или диапазона.
pub const ENTRIES_PAGE: usize = 200;
/// Текст записи в списке: дальше обрезается с признаком `truncated`, целиком — в `get_entry`.
pub const LIST_TEXT_CHARS: usize = 2000;
/// Наибольший номер пункта `- [ ]` в `set_task_done(item)`: дальше запрос явно мусорный (пунктов в записи столько нет).
pub const MAX_TASK_ITEM: u64 = 1_000_000;
/// Наибольшая длина `item_text` в знаках: пункт длиннее строки записи не бывает, а мусор отсекается сразу.
pub const MAX_ITEM_TEXT_CHARS: usize = 100_000;
/// Ответ не больше этого размера: страница обрывается на последней целой записи, которая помещается.
pub const RESPONSE_BYTES: usize = 1024 * 1024;

/// Инструмент MCP. Имена — часть протокола с клиентами.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentTool {
    ListChats,
    Search,
    GetEntries,
    OnThisDay,
    GetEntry,
    AddEntry,
    AppendToLastEntry,
    SetTaskDone,
}

impl AgentTool {
    pub const ALL: [AgentTool; 8] = [
        AgentTool::ListChats,
        AgentTool::Search,
        AgentTool::GetEntries,
        AgentTool::OnThisDay,
        AgentTool::GetEntry,
        AgentTool::AddEntry,
        AgentTool::AppendToLastEntry,
        AgentTool::SetTaskDone,
    ];

    pub fn name(self) -> &'static str {
        match self {
            AgentTool::ListChats => "list_chats",
            AgentTool::Search => "search",
            AgentTool::GetEntries => "get_entries",
            AgentTool::OnThisDay => "on_this_day",
            AgentTool::GetEntry => "get_entry",
            AgentTool::AddEntry => "add_entry",
            AgentTool::AppendToLastEntry => "append_to_last_entry",
            AgentTool::SetTaskDone => "set_task_done",
        }
    }

    pub fn from_name(name: &str) -> Option<AgentTool> {
        AgentTool::ALL.into_iter().find(|tool| tool.name() == name)
    }

    /// Инструмент меняет данные: доступен только в режиме «чтение и запись».
    pub fn writes(self) -> bool {
        matches!(
            self,
            AgentTool::AddEntry | AgentTool::AppendToLastEntry | AgentTool::SetTaskDone
        )
    }
}

/// Аргумент не принят: текст для агента (по-английски), без содержимого аргументов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArgError(pub String);

/// Позиция в выдаче для постраничного чтения: `YYYY-MM-DD:<created_at>:<id>` (ключ ленты `journal_date, created_at, id`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCursor {
    pub journal_date: String,
    pub created_at: i64,
    pub id: String,
}

impl AgentCursor {
    pub fn encode(&self) -> String {
        format!("{}:{}:{}", self.journal_date, self.created_at, self.id)
    }

    pub fn decode(text: &str) -> Option<AgentCursor> {
        let mut parts = text.splitn(3, ':');
        let (date, created, id) = (parts.next()?, parts.next()?, parts.next()?);
        let created_at = created.parse::<i64>().ok().filter(|ms| *ms >= 0)?;
        (is_valid_journal_date(date) && is_uuid(id)).then(|| AgentCursor {
            journal_date: date.to_string(),
            created_at,
            id: id.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchArgs {
    pub query: String,
    pub chat_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub favorites: bool,
    pub tasks: bool,
    pub cursor: Option<AgentCursor>,
}

/// День или диапазон дней (включительно); один день — `from == to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntriesArgs {
    pub from: String,
    pub to: String,
    pub chat_id: Option<String>,
    pub cursor: Option<AgentCursor>,
}

/// Проверенные аргументы вызова. Права и существование чатов и записей проверяет хранилище.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolArgs {
    ListChats,
    Search(SearchArgs),
    GetEntries(EntriesArgs),
    OnThisDay {
        date: Option<String>,
    },
    GetEntry {
        id: String,
    },
    AddEntry {
        text: String,
        chat_id: Option<String>,
        task: bool,
    },
    AppendToLastEntry {
        text: String,
        chat_id: Option<String>,
    },
    /// Без `item` — отметка задачи-записи целиком; с `item` — один пункт `- [ ]` внутри записи (номер с нуля, как в
    /// `tasks::task_items`).
    ///
    /// `item_text` — страховка от сдвинувшихся номеров (пользователь добавил пункт выше после чтения): текст пункта так,
    /// как его отдаёт `tasks`; не совпал с текстом найденного пункта — отказ, ничего не записано. Без `item_text` номер
    /// принимается как есть.
    SetTaskDone {
        id: String,
        done: bool,
        item: Option<usize>,
        item_text: Option<String>,
    },
}

/// Разбор `arguments` вызова инструмента. Лишние поля игнорируются (клиенты добавляют свои), типы и форматы строгие.
pub fn parse_args(
    tool: AgentTool,
    arguments: Option<&Map<String, Value>>,
) -> Result<ToolArgs, ArgError> {
    let empty = Map::new();
    let args = arguments.unwrap_or(&empty);
    Ok(match tool {
        AgentTool::ListChats => ToolArgs::ListChats,
        AgentTool::Search => ToolArgs::Search(SearchArgs {
            query: required_text(args, "query", MAX_QUERY_CHARS)?,
            chat_id: uuid_field(args, "chat_id")?,
            from: date_field(args, "from")?,
            to: date_field(args, "to")?,
            favorites: bool_field(args, "favorites")?.unwrap_or(false),
            tasks: bool_field(args, "tasks")?.unwrap_or(false),
            cursor: cursor_field(args)?,
        }),
        AgentTool::GetEntries => {
            let (date, from, to) = (
                date_field(args, "date")?,
                date_field(args, "from")?,
                date_field(args, "to")?,
            );
            let (from, to) = match (date, from, to) {
                (Some(day), None, None) => (day.clone(), day),
                (None, Some(from), Some(to)) if from <= to => (from, to),
                (None, Some(_), Some(_)) => return Err(bad("from must not be after to")),
                _ => return Err(bad("pass either date, or both from and to")),
            };
            ToolArgs::GetEntries(EntriesArgs {
                from,
                to,
                chat_id: uuid_field(args, "chat_id")?,
                cursor: cursor_field(args)?,
            })
        }
        AgentTool::OnThisDay => ToolArgs::OnThisDay {
            date: date_field(args, "date")?,
        },
        AgentTool::GetEntry => ToolArgs::GetEntry {
            id: uuid_field(args, "id")?.ok_or_else(|| bad("id is required"))?,
        },
        AgentTool::AddEntry => ToolArgs::AddEntry {
            text: required_text(args, "text", usize::MAX)?,
            chat_id: uuid_field(args, "chat_id")?,
            task: bool_field(args, "task")?.unwrap_or(false),
        },
        AgentTool::AppendToLastEntry => ToolArgs::AppendToLastEntry {
            text: required_text(args, "text", usize::MAX)?,
            chat_id: uuid_field(args, "chat_id")?,
        },
        AgentTool::SetTaskDone => ToolArgs::SetTaskDone {
            id: uuid_field(args, "id")?.ok_or_else(|| bad("id is required"))?,
            done: bool_field(args, "done")?.ok_or_else(|| bad("done is required"))?,
            item: item_field(args)?,
            item_text: item_text_field(args)?,
        },
    })
}

fn bad(why: &str) -> ArgError {
    ArgError(why.to_string())
}

fn field<'a>(args: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    args.get(key).filter(|value| !value.is_null())
}

fn text_field<'a>(args: &'a Map<String, Value>, key: &str) -> Result<Option<&'a str>, ArgError> {
    match field(args, key) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text)),
        Some(_) => Err(ArgError(format!("{key} must be a string"))),
    }
}

fn required_text(
    args: &Map<String, Value>,
    key: &str,
    max_chars: usize,
) -> Result<String, ArgError> {
    let text = text_field(args, key)?.ok_or_else(|| ArgError(format!("{key} is required")))?;
    if text.trim().is_empty() && key != "query" {
        return Err(ArgError(format!("{key} must not be empty")));
    }
    if max_chars != usize::MAX && text.chars().take(max_chars + 1).count() > max_chars {
        return Err(ArgError(format!(
            "{key} is longer than {max_chars} characters"
        )));
    }
    Ok(text.to_string())
}

fn uuid_field(args: &Map<String, Value>, key: &str) -> Result<Option<String>, ArgError> {
    match text_field(args, key)? {
        None => Ok(None),
        Some(id) if is_uuid(id) => Ok(Some(id.to_string())),
        Some(_) => Err(ArgError(format!("{key} is not a valid id"))),
    }
}

fn date_field(args: &Map<String, Value>, key: &str) -> Result<Option<String>, ArgError> {
    match text_field(args, key)? {
        None => Ok(None),
        Some(date) if is_valid_journal_date(date) => Ok(Some(date.to_string())),
        Some(_) => Err(ArgError(format!("{key} must be a date like 2026-03-14"))),
    }
}

fn bool_field(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, ArgError> {
    match field(args, key) {
        None => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(ArgError(format!("{key} must be true or false"))),
    }
}

/// Номер пункта: целое неотрицательное число (не строка, не дробное, не больше [`MAX_TASK_ITEM`]).
fn item_field(args: &Map<String, Value>) -> Result<Option<usize>, ArgError> {
    match field(args, "item") {
        None => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .filter(|value| *value <= MAX_TASK_ITEM)
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| bad("item must be a whole number from 0 to 1000000")),
        Some(_) => Err(bad("item must be a whole number from 0 to 1000000")),
    }
}

/// Текст пункта для сверки: строка, без пробелов по краям (так же обрезан `text` в `tasks`), не пустая и не длиннее
/// [`MAX_ITEM_TEXT_CHARS`]; без `item` не имеет смысла.
fn item_text_field(args: &Map<String, Value>) -> Result<Option<String>, ArgError> {
    let Some(text) = text_field(args, "item_text")? else {
        return Ok(None);
    };
    if field(args, "item").is_none() {
        return Err(bad("item_text needs item"));
    }
    let text = text.trim();
    if text.is_empty() {
        return Err(bad("item_text must not be empty"));
    }
    if text.chars().take(MAX_ITEM_TEXT_CHARS + 1).count() > MAX_ITEM_TEXT_CHARS {
        return Err(bad("item_text is too long"));
    }
    Ok(Some(text.to_string()))
}

fn cursor_field(args: &Map<String, Value>) -> Result<Option<AgentCursor>, ArgError> {
    match text_field(args, "cursor")? {
        None => Ok(None),
        Some(text) => AgentCursor::decode(text)
            .map(Some)
            .ok_or_else(|| bad("cursor is not valid")),
    }
}

/// `args_json` журнала обращений — **белый список** полей разобранных аргументов: чат, даты, флаги и `query`, обрезанный
/// до [`LOG_QUERY_CHARS`]. Текста записи (`text`) и id записей здесь нет никогда: id идут в `mcp_log_message`.
pub fn args_json(args: &ToolArgs) -> String {
    let mut out = Map::new();
    match args {
        ToolArgs::ListChats | ToolArgs::GetEntry { .. } => {}
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
            // Только номер пункта: его текст в журнал не попадает никогда.
            if let Some(item) = item {
                out.insert("item".into(), Value::from(*item));
            }
        }
    }
    Value::Object(out).to_string()
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

/// Имя клиента из `clientInfo` для журнала: управляющие символы убраны, не длиннее [`CLIENT_CHARS`]. Пустое имя —
/// `unknown`. Это слова клиента, а не проверенная личность (ADR-0009).
pub fn clean_client(raw: &str) -> String {
    let cleaned: String = raw.chars().filter(|c| !c.is_control()).collect();
    let cleaned = truncate_chars(cleaned.trim(), CLIENT_CHARS);
    if cleaned.is_empty() {
        "unknown".to_string()
    } else {
        cleaned
    }
}

/// Первые `max` знаков (по границе символа).
pub fn truncate_chars(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
