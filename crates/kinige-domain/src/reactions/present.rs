//! What reactions look like outside the group (slice 5a): the entry's view, the Markdown copy line, MCP's `put` time and the quick set
//! of the hover bar. Dates need the zone database, which the domain does not have (it builds for wasm), so the store passes in the
//! journal date of the moment a reaction was put, computed in the zone that reaction carries.

use super::{Reaction, reaction_key};
use crate::emoji::is_emoji_sequence;
use crate::journal::{date_parts, is_valid_journal_date};
use chrono::NaiveDate;
use serde::Serialize;
use std::collections::HashSet;
use ts_rs::TS;

/// How many recent reactions the device remembers.
pub const MAX_RECENT: usize = 8;
/// The fixed quick set that fills the hover bar after the recent reactions, in this order.
pub const QUICK_DEFAULTS: [&str; 3] = ["\u{2764}\u{FE0F}", "😂", "😢"];
/// How many quick emoji the hover bar and the popover show.
pub const QUICK_COUNT: usize = 3;
/// The separator of the Markdown copy line.
const LINE_SEPARATOR: &str = " · ";

/// One reaction on an entry, as the interface shows it (slice 5a). The list is in the order the reactions were put.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ReactionView.ts")]
pub struct ReactionView {
    /// The emoji in the form the picker gave.
    pub emoji: String,
    /// When it was put (UTC, ms).
    #[ts(type = "number")]
    pub put_at: i64,
    /// The device's IANA zone when it was put: the day it was put on is counted in it.
    pub put_tz: String,
    /// Full years between the entry's day and the day the reaction was put on; 0 for a reaction put the same year, never negative.
    pub years_later: u32,
}

impl ReactionView {
    /// `put_date` is the reaction's `journal_date` in its own zone (04:00 boundary), `entry_date` the entry's.
    pub fn new(reaction: Reaction, entry_date: &str, put_date: &str) -> ReactionView {
        ReactionView {
            years_later: years_later(entry_date, put_date),
            emoji: reaction.emoji,
            put_at: reaction.put_at,
            put_tz: reaction.put_tz,
        }
    }
}

/// Full years from the entry's day to the day the reaction was put on, by the calendar (`chrono`); 0 when either date is not a date or
/// the reaction is earlier than the entry's day.
pub fn years_later(entry_date: &str, put_date: &str) -> u32 {
    let (Some(entry), Some(put)) = (parse_day(entry_date), parse_day(put_date)) else {
        return 0;
    };
    put.years_since(entry).unwrap_or(0)
}

fn parse_day(text: &str) -> Option<NaiveDate> {
    let (year, month, day) = date_parts(text)?;
    if !is_valid_journal_date(text) {
        return None;
    }
    NaiveDate::from_ymd_opt(year, month, day)
}

/// The Markdown copy line: `<emoji> <YYYY-MM-DD>` for each reaction in the order put, joined by ` · `; `None` without reactions.
pub fn markdown_line(reactions: &[(String, String)]) -> Option<String> {
    if reactions.is_empty() {
        return None;
    }
    let parts: Vec<String> = reactions
        .iter()
        .map(|(emoji, date)| format!("{emoji} {date}"))
        .collect();
    Some(parts.join(LINE_SEPARATOR))
}

/// The three quick emoji: the recent reactions first, then the fixed set, without repeats by [`reaction_key`].
pub fn quick_reactions(recent: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    recent
        .iter()
        .map(String::as_str)
        .chain(QUICK_DEFAULTS)
        .filter(|emoji| seen.insert(reaction_key(emoji)))
        .take(QUICK_COUNT)
        .map(str::to_string)
        .collect()
}

/// The stored recent list: a broken value is an empty list, entries that are not emoji or repeat by key are dropped, the freshest first,
/// at most [`MAX_RECENT`].
pub fn parse_recent(stored: Option<&str>) -> Vec<String> {
    let list: Vec<String> = stored
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();
    let mut seen = HashSet::new();
    list.into_iter()
        .filter(|emoji| is_emoji_sequence(emoji) && seen.insert(reaction_key(emoji)))
        .take(MAX_RECENT)
        .collect()
}

/// `emoji` goes first; an earlier entry with the same key goes out; the list is capped at [`MAX_RECENT`].
pub fn push_recent(list: &[String], emoji: &str) -> Vec<String> {
    let key = reaction_key(emoji);
    std::iter::once(emoji.to_string())
        .chain(
            list.iter()
                .filter(|known| reaction_key(known) != key)
                .cloned(),
        )
        .take(MAX_RECENT)
        .collect()
}
