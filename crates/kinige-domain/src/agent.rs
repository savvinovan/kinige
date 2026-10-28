//! ИИ-агент через MCP (ADR-0009, слайс 4a): разбор и проверка аргументов инструментов, формы ответов, «журнал без
//! содержимого». Без IO. Права доступа здесь не проверяются: их каждый раз проверяет хранилище внутри запроса.

use crate::hlc::is_uuid;
use crate::journal::is_valid_journal_date;
use serde::Serialize;
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
/// Ответ не больше этого размера: страница обрывается на последней целой записи, которая помещается.
pub const RESPONSE_BYTES: usize = 1024 * 1024;
/// Запас на обёртку ответа (курсор, чаты): записи делят остальное.
const WRAPPER_BYTES: usize = 8 * 1024;

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
    SetTaskDone {
        id: String,
        done: bool,
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
        ToolArgs::SetTaskDone { done, .. } => {
            out.insert("done".into(), Value::Bool(*done));
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

/// Месяц-день и год даты для «в этот день»: записи с тем же `MM-DD` в **прошлые** годы. 29 февраля сравнивается только с
/// 29 февраля других годов (строгое равенство `MM-DD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthDay {
    pub month_day: String,
    pub year: i32,
}

pub fn month_day(date: &str) -> Option<MonthDay> {
    is_valid_journal_date(date).then(|| MonthDay {
        month_day: date[5..].to_string(),
        year: date[..4].parse().unwrap_or(0),
    })
}

/// Запись в ответе агента. `reply_to_id` не отдаётся (ответов в интерфейсе ещё нет).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentEntry {
    pub id: String,
    pub chat_id: String,
    pub chat: String,
    pub journal_date: String,
    /// Местное время создания с поясом на тот момент: `2026-03-14T21:05:09+03:00`.
    pub created: String,
    pub origin: String,
    pub task: bool,
    pub done: bool,
    pub favorite: bool,
    pub text: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

impl AgentEntry {
    /// Текст в списках обрезается до [`LIST_TEXT_CHARS`].
    pub fn shorten(mut self, max: usize) -> AgentEntry {
        if self.text.chars().take(max + 1).count() > max {
            self.text = truncate_chars(&self.text, max);
            self.truncated = true;
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentChat {
    pub id: String,
    pub title: String,
    pub emoji: Option<String>,
    pub entries: i64,
    pub archived: bool,
}

/// Страница записей в ответе.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentPage {
    pub entries: Vec<AgentEntry>,
    /// Чаты, чьё название совпало с запросом (первая страница поиска).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chats: Vec<AgentChat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Оставляет записи, чей JSON помещается в [`RESPONSE_BYTES`] вместе с обёрткой; вторая часть ответа — «страница
/// оборвана по размеру» (курсор тогда указывает на следующую запись). Одна запись больше предела обрезается с `truncated`.
pub fn fit_entries(mut entries: Vec<AgentEntry>) -> (Vec<AgentEntry>, bool) {
    let budget = RESPONSE_BYTES - WRAPPER_BYTES;
    let mut used = 0usize;
    let mut kept = 0usize;
    for entry in &entries {
        let size = serde_json::to_string(entry).map_or(usize::MAX, |json| json.len() + 1);
        if used.saturating_add(size) > budget {
            break;
        }
        used += size;
        kept += 1;
    }
    if kept == entries.len() {
        return (entries, false);
    }
    if kept == 0 {
        // Первая запись одна больше предела: режем текст, пока не поместится.
        let mut first = entries.swap_remove(0);
        let mut chars = first.text.chars().count();
        while chars > 1
            && serde_json::to_string(&first).map_or(usize::MAX, |json| json.len()) > budget
        {
            chars /= 2;
            first.text = truncate_chars(&first.text, chars);
            first.truncated = true;
        }
        return (vec![first], true);
    }
    entries.truncate(kept);
    (entries, true)
}

#[cfg(test)]
#[path = "agent_tests.rs"]
mod tests;
