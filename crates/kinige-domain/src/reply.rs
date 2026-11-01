//! Ответ на запись (слайс 5, ТЗ §4.2): ссылка `core.reply_to_id`, цитата в плашке и подпись «N лет спустя». Без IO.
//! Ссылка без внешнего ключа: старая запись может быть в корзине, удалена навсегда или ещё не прийти при синхронизации.

use crate::search::snippet;
use chrono::NaiveDate;
use serde::Serialize;
use ts_rs::TS;

/// Длина цитаты в знаках: плашка показывает одну строку, остальное обрезает CSS.
pub const EXCERPT_CHARS: usize = 120;

/// В каком состоянии запись, на которую ответили.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ReplyState.ts")]
pub enum ReplyState {
    /// Запись на месте.
    Live,
    /// Запись в корзине: цитата подписана «Запись в корзине», переход невозможен.
    Trash,
    /// Записи нет: удалена навсегда или ещё не пришла. Ответ остаётся, цитата подписана «Исходная запись удалена».
    Missing,
    /// Только у черновика: запись жива, но её перенесли в другой чат. Ответ можно писать только на запись того же чата
    /// (`reply_target_not_found`), поэтому строка над полем говорит «перенесена», а запись уходит без ответа.
    Moved,
}

/// Запись, на которую ответили: то, что нужно цитате в плашке и строке «Ответ на запись» над полем ввода.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ReplyTo.ts")]
pub struct ReplyTo {
    pub id: String,
    /// Чат старой записи; `null`, если её нет (`state = missing`).
    pub chat_id: Option<String>,
    /// День старой записи (`YYYY-MM-DD`); `null`, если её нет.
    pub journal_date: Option<String>,
    /// Простой текст начала старой записи в одну строку, до [`EXCERPT_CHARS`] знаков; пустой, если записи нет.
    pub excerpt: String,
    pub state: ReplyState,
    /// Полные годы между днём старой записи и днём ответа; 0, если меньше года или ответ раньше оригинала. При 0 подпись
    /// «N лет спустя» не показывается.
    #[ts(type = "number")]
    pub years_later: i64,
}

/// Что известно о записи-адресате по строке запроса: чат, день, текст и признак корзины.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTarget {
    pub chat_id: String,
    pub journal_date: String,
    pub body_md: String,
    pub in_trash: bool,
}

/// Цитата для ответа `reply_id` в день `reply_date`; `target = None` — записи нет.
pub fn reply_to(reply_id: &str, reply_date: &str, target: Option<ReplyTarget>) -> ReplyTo {
    match target {
        Some(old) => ReplyTo {
            id: reply_id.to_string(),
            years_later: years_later(&old.journal_date, reply_date),
            excerpt: excerpt(&old.body_md),
            state: if old.in_trash {
                ReplyState::Trash
            } else {
                ReplyState::Live
            },
            chat_id: Some(old.chat_id),
            journal_date: Some(old.journal_date),
        },
        None => ReplyTo {
            id: reply_id.to_string(),
            chat_id: None,
            journal_date: None,
            excerpt: String::new(),
            state: ReplyState::Missing,
            years_later: 0,
        },
    }
}

/// Начало записи простым текстом в одну строку: без разметки Markdown, переводы строк стали пробелами, не длиннее
/// [`EXCERPT_CHARS`] знаков с «…» в конце. У реплики собеседника из Telegram первая строка — имя автора жирным: в цитату
/// попадают имя и начало текста.
pub fn excerpt(body_md: &str) -> String {
    snippet(body_md, &[], EXCERPT_CHARS)
}

/// Полные годы между `old` (день старой записи) и `reply` (день ответа). Меньше года, ответ раньше оригинала (дату
/// записи сменили) или не дата — 0. 29 февраля в невисокосный год считается раньше 1 марта (`chrono::NaiveDate::years_since`).
pub fn years_later(old: &str, reply: &str) -> i64 {
    let (Some(old), Some(reply)) = (parse_date(old), parse_date(reply)) else {
        return 0;
    };
    reply.years_since(old).map_or(0, i64::from)
}

fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests;
