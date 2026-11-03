//! Reply shapes for the agent (ADR-0009, slice 4a): entry, chat, page, "on this day" and the reply size limit.
//! No IO. A reply is no larger than [`RESPONSE_BYTES`]; entry text in lists is truncated.

use crate::agent::{RESPONSE_BYTES, truncate_chars};
use crate::journal::{date_parts, is_valid_journal_date};
use crate::tasks::TaskItem;
use serde::Serialize;

/// Headroom for the reply wrapper (cursor, chats): entries share the rest.
const WRAPPER_BYTES: usize = 8 * 1024;

/// Month-day and year of a date for "on this day": entries with the same `MM-DD` in **past** years. February 29 is compared only with
/// February 29 of other years (strict equality of `MM-DD`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthDay {
    pub month_day: String,
    pub year: i32,
}

pub fn month_day(date: &str) -> Option<MonthDay> {
    if !is_valid_journal_date(date) {
        return None;
    }
    Some(MonthDay {
        month_day: date.get(5..)?.to_string(),
        year: date_parts(date)?.0,
    })
}

/// An entry in the agent's reply. `reply_to_id` is not exposed in slice 5 either, when replies appeared in the interface (ADR-0009).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentEntry {
    pub id: String,
    pub chat_id: String,
    pub chat: String,
    pub journal_date: String,
    /// Local creation time with the offset at that moment: `2026-03-14T21:05:09+03:00`.
    pub created: String,
    pub origin: String,
    pub task: bool,
    pub done: bool,
    pub favorite: bool,
    pub text: String,
    /// The `- [ ]` items inside the entry with numbers for `set_task_done(item)`; the field is absent if there are no items.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tasks: Vec<TaskItem>,
    /// What the entry holds (slice 7): kind, file name, size and, for a photo, its size in pixels. Never the bytes, a path or a key
    /// (ADR-0009); the field is absent for an entry without attachments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<AgentAttachment>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
}

/// One attachment as an agent sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentAttachment {
    pub kind: &'static str,
    pub name: String,
    pub bytes: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}

impl AgentEntry {
    /// Text in lists is truncated to [`crate::agent::LIST_TEXT_CHARS`]. Item texts count against the same limit (all
    /// together): the items stay at the start of the list, so their numbers are unchanged; those that do not fit are dropped, the last
    /// one that fits is cut.
    pub fn shorten(mut self, max: usize) -> AgentEntry {
        if self.text.chars().take(max + 1).count() > max {
            self.text = truncate_chars(&self.text, max);
            self.truncated = true;
        }
        let mut left = max;
        let mut kept = 0usize;
        for task in &mut self.tasks {
            let chars = task.text.chars().count();
            if chars > left {
                task.text = truncate_chars(&task.text, left);
                self.truncated = true;
                kept += 1;
                break;
            }
            left -= chars;
            kept += 1;
        }
        if kept < self.tasks.len() {
            self.tasks.truncate(kept);
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

/// A page of entries in the reply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentPage {
    pub entries: Vec<AgentEntry>,
    /// Chats whose name matched the query (the first page of a search).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chats: Vec<AgentChat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Keeps the entries whose JSON fits into [`RESPONSE_BYTES`] together with the wrapper; the second part of the reply is "the page
/// was cut by size" (the cursor then points to the next entry). A single entry above the limit is truncated with `truncated`.
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
        // The first entry alone is above the limit: cut the text until it fits.
        let mut first = entries.swap_remove(0);
        let mut chars = first.text.chars().count();
        // Items are cut together with the text (the start of the list is kept: numbers are unchanged).
        while (chars > 1 || !first.tasks.is_empty())
            && serde_json::to_string(&first).map_or(usize::MAX, |json| json.len()) > budget
        {
            chars = (chars / 2).max(1);
            first.text = truncate_chars(&first.text, chars);
            first.tasks.truncate(first.tasks.len() / 2);
            first.truncated = true;
        }
        return (vec![first], true);
    }
    entries.truncate(kept);
    (entries, true)
}
