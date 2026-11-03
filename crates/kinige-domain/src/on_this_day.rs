//! "On this day" (slice 5, PRD §2.1): a card at the start of today's Inbox day with entries of the same date from past years.
//! This module holds the dates of past years, the composition of the card and the response types. Selecting the entries
//! is in `kinige-store`. No IO.

use crate::api::MessageView;
use crate::journal::{date_parts, is_valid_journal_date};
use crate::search::snippet;
use chrono::{Datelike, NaiveDate};
use serde::Serialize;
use ts_rs::TS;

/// How many of the newest years are shown in detail; the rest get one line each ("User decision 3", slice 5).
pub const DETAILED_YEARS: usize = 3;
/// How many entries of a year are shown in detail; for the rest there is "N more entries of this day >".
pub const ENTRIES_PER_YEAR: usize = 2;

/// How many characters of the entry's plain text the card returns: the card shows up to three lines, CSS cuts the rest.
pub const PREVIEW_CHARS: usize = 300;

/// The "On this day" card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "OnThisDay.ts")]
pub struct OnThisDay {
    /// Today's `journal_date` the card belongs to (`dismiss_on_this_day` is called with it too).
    pub today: String,
    /// Years with entries, from newest to oldest; not empty.
    pub years: Vec<OnThisDayYear>,
}

/// One year of the card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "OnThisDayYear.ts")]
pub struct OnThisDayYear {
    /// The day of this year (`YYYY-MM-DD`).
    pub journal_date: String,
    /// How many full years ago (1, 3, 11...).
    #[ts(type = "number")]
    pub years_ago: i64,
    /// How many Inbox entries there are on this day, not counting the trash.
    #[ts(type = "number")]
    pub total: i64,
    /// The first entry of the day: the feed opens on it from "N more entries of this day >" and from the line of an old year.
    pub first_id: String,
    /// The first entries of the day (up to [`ENTRIES_PER_YEAR`]) for detailed years; empty for the others.
    pub entries: Vec<OnThisDayEntry>,
}

/// An entry in the card: the entry itself and its text as plain text on one line (without Markdown markup, up to
/// [`PREVIEW_CHARS`] characters with "…" at the end) for a compact line cut to three lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "OnThisDayEntry.ts")]
pub struct OnThisDayEntry {
    pub message: MessageView,
    pub text: String,
}

impl OnThisDayEntry {
    pub fn new(message: MessageView) -> OnThisDayEntry {
        let text = snippet(&message.body_md, &[], PREVIEW_CHARS);
        OnThisDayEntry { message, text }
    }
}

/// Dates of past years for `today` (`YYYY-MM-DD`): the same month and day, years from `oldest_year` to the year before
/// the current one, from newest to oldest. February 29 matches only February 29 (years without it are skipped), as with
/// `on_this_day` in MCP. If it is not a date, the result is empty.
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

/// The year from `journal_date` (`YYYY-MM-DD`); `None` if it is not a date.
pub fn year_of(journal_date: &str) -> Option<i32> {
    is_valid_journal_date(journal_date)
        .then(|| date_parts(journal_date).map(|(year, _, _)| year))
        .flatten()
}

/// How many years ago `date` was relative to `today` (by the year only: the dates share the month and day).
pub fn years_ago(today: &str, date: &NaiveDate) -> i64 {
    let year = year_of(today).unwrap_or(0);
    i64::from(year - date.year())
}

#[cfg(test)]
mod tests;
