//! The year table of the forest (slice 6, PRD §7, ADR-0013): every day of one year with its cell, season, number of
//! notes, growth stage and the start of its first entry. The store collects the per-day facts, this module applies the
//! rules (`forest.rs`), so the frontend only draws and looks days up. No IO.

use crate::forest::{
    DEFAULT_W_MAX, FirstWeekday, NoteStat, Season, day_position, day_weight, grid_dims,
    season_of_month, stage_of,
};
use crate::reply::excerpt;
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use ts_rs::TS;

/// The years the forest answers for (the same range as a journal date, `journal::is_valid_journal_date`); anything else
/// is an invalid date (the store maps it to `InvalidDate`).
pub const FOREST_YEARS: std::ops::RangeInclusive<i32> = 1000..=9999;

/// One year of the forest, as the main window receives it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ForestYear.ts")]
pub struct ForestYear {
    pub year: i32,
    pub first_weekday: FirstWeekday,
    /// Columns of the board: 53, or 54 in the leap years that need it (ADR-0013).
    pub cols: u8,
    /// The year of the oldest live entry, or the current year when there is none: the lower end of "‹ year ›".
    pub min_year: i32,
    /// The current journal day (`YYYY-MM-DD`): later days of the board are ground without interaction.
    pub today: String,
    /// Every day of the year in calendar order, 365 or 366 rows.
    pub days: Vec<ForestDay>,
}

/// One day of the board.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ForestDay.ts")]
pub struct ForestDay {
    /// `YYYY-MM-DD`.
    pub date: String,
    pub row: u8,
    pub col: u8,
    pub season: Season,
    /// Live entries of the day, all chats not in the trash, short ones included.
    pub notes: u32,
    /// Growth stage 0..=6 (PRD §7.2).
    pub stage: u8,
    /// The start of the day's first entry as plain text (the same text as a reply quote); `null` for an empty day.
    pub excerpt: Option<String>,
}

/// The entry a click on a day opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ForestTarget.ts")]
pub struct ForestTarget {
    pub chat_id: String,
    pub message_id: String,
}

/// How much of the forest is shown above the feed (`app_setting.forest_view`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ForestView.ts")]
pub enum ForestView {
    Hidden,
    Strip,
    Expanded,
}

/// What an expanded forest shows (`app_setting.forest_mode`): the isometric board or the plain calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ForestMode.ts")]
pub enum ForestMode {
    Forest,
    Calendar,
}

impl ForestView {
    pub fn as_setting(self) -> &'static str {
        match self {
            ForestView::Hidden => "hidden",
            ForestView::Strip => "strip",
            ForestView::Expanded => "expanded",
        }
    }

    /// An unknown or missing stored value is the default, the strip (user decision 1 of slice 6).
    pub fn from_setting(value: Option<&str>) -> ForestView {
        match value {
            Some("hidden") => ForestView::Hidden,
            Some("expanded") => ForestView::Expanded,
            _ => ForestView::Strip,
        }
    }
}

impl ForestMode {
    pub fn as_setting(self) -> &'static str {
        match self {
            ForestMode::Forest => "forest",
            ForestMode::Calendar => "calendar",
        }
    }

    pub fn from_setting(value: Option<&str>) -> ForestMode {
        match value {
            Some("calendar") => ForestMode::Calendar,
            _ => ForestMode::Forest,
        }
    }
}

/// What the store found out about one day. The store may sum the weight in SQL (one row per day instead of one per
/// entry); the per-entry rule of `day_weight` is the definition and the store tests that its SQL gives the same number.
#[derive(Debug, Clone, PartialEq)]
pub struct DayStats {
    pub date: NaiveDate,
    /// How many live entries the day has.
    pub notes: u32,
    /// `forest::day_weight` of those entries.
    pub weight: f64,
    /// Markdown of the day's first entry (by journal date, creation time, id).
    pub first_body: Option<String>,
}

impl DayStats {
    /// The facts of a day from its entries one by one (the weight by `day_weight`).
    pub fn from_notes(date: NaiveDate, notes: &[NoteStat], first_body: Option<String>) -> DayStats {
        DayStats {
            date,
            notes: u32::try_from(notes.len()).unwrap_or(u32::MAX),
            weight: day_weight(notes),
            first_body,
        }
    }
}

/// The board of one year.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YearTable {
    pub cols: u8,
    pub days: Vec<ForestDay>,
}

/// Builds the table of `year`: every day once, in calendar order. Facts for dates outside the year are ignored; two
/// facts for one date are merged (notes added, the earlier first body wins by order of arrival). `None` for a year
/// outside [`FOREST_YEARS`].
pub fn year_table(year: i32, first: FirstWeekday, stats: &[DayStats]) -> Option<YearTable> {
    if !FOREST_YEARS.contains(&year) {
        return None;
    }
    let dims = grid_dims(year, first)?;
    let mut by_date: BTreeMap<NaiveDate, (u32, f64, Option<&str>)> = BTreeMap::new();
    for stat in stats.iter().filter(|stat| stat.date.year() == year) {
        let entry = by_date.entry(stat.date).or_default();
        entry.0 = entry.0.saturating_add(stat.notes);
        entry.1 += stat.weight;
        if entry.2.is_none() {
            entry.2 = stat.first_body.as_deref();
        }
    }
    let mut days = Vec::with_capacity(366);
    for ordinal in 1..=366 {
        let Some(date) = NaiveDate::from_yo_opt(year, ordinal) else {
            break;
        };
        let (notes, weight, body) = by_date.get(&date).copied().unwrap_or_default();
        let pos = day_position(date, first);
        days.push(ForestDay {
            date: date.format("%Y-%m-%d").to_string(),
            row: pos.row,
            col: pos.col,
            season: season_of_month(date.month(), false),
            notes,
            stage: stage_of(weight, DEFAULT_W_MAX),
            excerpt: body.map(excerpt).filter(|text| !text.is_empty()),
        });
    }
    Some(YearTable {
        cols: dims.cols,
        days,
    })
}

/// The chat a click on a day opens (user decision 2 of slice 6): the open chat if it has entries that day, otherwise
/// the chat with the most entries, the Inbox on a tie, and among tied chats without the Inbox the smaller id (so the
/// answer never depends on row order). `counts` is `(chat id, live entries that day)`; chats with no entries do not
/// count. `None` for an empty day.
pub fn day_chat<'a>(
    counts: &'a [(String, usize)],
    current: Option<&str>,
    inbox: &str,
) -> Option<&'a str> {
    let live = || counts.iter().filter(|(_, count)| *count > 0);
    if let Some(open) = current
        && let Some((id, _)) = live().find(|(id, _)| id == open)
    {
        return Some(id);
    }
    live()
        .max_by(|(a_id, a), (b_id, b)| {
            a.cmp(b)
                .then_with(|| (a_id == inbox).cmp(&(b_id == inbox)))
                .then_with(|| b_id.cmp(a_id))
        })
        .map(|(id, _)| id.as_str())
}

#[cfg(test)]
mod tests;
