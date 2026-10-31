//! «В этот день» (слайс 5, ТЗ §2.1): карточка в начале сегодняшнего дня Inbox с записями этого же числа прошлых лет.
//! Здесь — даты прошлых лет, состав карточки и типы ответа. Отбор записей — в `kinige-store`. Без IO.

use crate::api::MessageView;
use crate::journal::is_valid_journal_date;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use ts_rs::TS;

/// Сколько самых новых лет показываются подробно; остальные — одной строкой («Решение пользователя 3», слайс 5).
pub const DETAILED_YEARS: usize = 3;
/// Сколько записей года показывается подробно; про остальные — «ещё N записей этого дня ›».
pub const ENTRIES_PER_YEAR: usize = 2;

/// Карточка «В этот день».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "OnThisDay.ts")]
pub struct OnThisDay {
    /// Сегодняшний `journal_date`, к которому относится карточка (с ним же вызывается `dismiss_on_this_day`).
    pub today: String,
    /// Годы с записями, от новых к старым; не пусто.
    pub years: Vec<OnThisDayYear>,
}

/// Один год карточки.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "OnThisDayYear.ts")]
pub struct OnThisDayYear {
    /// День этого года (`YYYY-MM-DD`).
    pub journal_date: String,
    /// Сколько полных лет назад (1, 3, 11…).
    #[ts(type = "number")]
    pub years_ago: i64,
    /// Сколько записей Inbox в этот день, кроме корзины.
    #[ts(type = "number")]
    pub total: i64,
    /// Первая запись дня: на неё открывается лента по «ещё N записей этого дня ›» и по строке старого года.
    pub first_id: String,
    /// Первые записи дня (до [`ENTRIES_PER_YEAR`]) у подробных лет; у остальных пусто.
    pub messages: Vec<MessageView>,
}

/// Даты прошлых лет для `today` (`YYYY-MM-DD`): тот же месяц и день, годы от `oldest_year` до года перед сегодняшним,
/// от новых к старым. 29 февраля совпадает только с 29 февраля (года без него пропускаются), как у `on_this_day` в MCP.
/// Не дата — пусто.
pub fn past_dates(today: &str, oldest_year: i32) -> Vec<NaiveDate> {
    if !is_valid_journal_date(today) {
        return Vec::new();
    }
    let Ok(today) = NaiveDate::parse_from_str(today, "%Y-%m-%d") else {
        return Vec::new();
    };
    (oldest_year..today.year())
        .rev()
        .filter_map(|year| NaiveDate::from_ymd_opt(year, today.month(), today.day()))
        .collect()
}

/// Год из `journal_date` (`YYYY-MM-DD`); `None`, если это не дата.
pub fn year_of(journal_date: &str) -> Option<i32> {
    is_valid_journal_date(journal_date)
        .then(|| journal_date[..4].parse().ok())
        .flatten()
}

/// Сколько лет назад был `date` относительно `today` (только по году: даты одного месяца и дня).
pub fn years_ago(today: &str, date: &NaiveDate) -> i64 {
    let year = year_of(today).unwrap_or(0);
    i64::from(year - date.year())
}

#[cfg(test)]
mod tests;
