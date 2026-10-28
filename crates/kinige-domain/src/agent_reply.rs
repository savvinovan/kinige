//! Формы ответов агенту (ADR-0009, слайс 4a): запись, чат, страница, «в этот день» и ограничение размера ответа.
//! Без IO. Ответ не больше [`RESPONSE_BYTES`]; текст записи в списках обрезается.

use crate::agent::{RESPONSE_BYTES, truncate_chars};
use crate::journal::is_valid_journal_date;
use serde::Serialize;

/// Запас на обёртку ответа (курсор, чаты): записи делят остальное.
const WRAPPER_BYTES: usize = 8 * 1024;

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
    /// Текст в списках обрезается до [`crate::agent::LIST_TEXT_CHARS`].
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
