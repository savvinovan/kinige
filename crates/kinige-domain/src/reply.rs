//! A reply to an entry (slice 5, PRD §4.2): the `core.reply_to_id` link, the quote in the plate and the "N years later" caption. No IO.
//! The link has no foreign key: the old entry may be in the trash, deleted for good or not yet arrived through sync.

use crate::attachment::AttachmentSummary;
use crate::search::snippet;
use chrono::NaiveDate;
use serde::Serialize;
use ts_rs::TS;

/// The quote length in characters: the plate shows one line, CSS cuts the rest.
pub const EXCERPT_CHARS: usize = 120;

/// The state of the entry that was replied to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ReplyState.ts")]
pub enum ReplyState {
    /// The entry is in place.
    Live,
    /// The entry is in the trash: the quote is captioned "Entry in trash", jumping to it is impossible.
    Trash,
    /// There is no entry: deleted for good or not arrived yet. The reply stays, the quote is captioned "Original entry deleted".
    Missing,
    /// Draft only: the entry is alive but was moved to another chat. A reply can be written only to an entry of the same chat
    /// (`reply_target_not_found`), so the line above the field says "moved", and the entry is sent without a reply.
    Moved,
}

/// The entry that was replied to: what the quote in the plate and the "Reply to entry" line above the input field need.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ReplyTo.ts")]
pub struct ReplyTo {
    pub id: String,
    /// The chat of the old entry; `null` if there is none (`state = missing`).
    pub chat_id: Option<String>,
    /// The day of the old entry (`YYYY-MM-DD`); `null` if there is none.
    pub journal_date: Option<String>,
    /// Plain text of the start of the old entry on one line, up to [`EXCERPT_CHARS`] characters; empty if there is no entry.
    pub excerpt: String,
    pub state: ReplyState,
    /// Full years between the day of the old entry and the day of the reply; 0 if less than a year or the reply is earlier than the original. With 0 the
    /// "N years later" caption is not shown.
    #[ts(type = "number")]
    pub years_later: i64,
    /// What the old entry holds (slice 7): an entry with no text is quoted as "Photo", "3 photos" or its file name.
    pub attachments: AttachmentSummary,
}

/// What is known about the target entry from the query row: chat, day, text, the trash flag and its attachments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplyTarget {
    pub chat_id: String,
    pub journal_date: String,
    pub body_md: String,
    pub in_trash: bool,
    pub attachments: AttachmentSummary,
}

/// The quote for the reply `reply_id` on the day `reply_date`; `target = None` means there is no entry.
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
            attachments: old.attachments,
        },
        None => ReplyTo {
            id: reply_id.to_string(),
            chat_id: None,
            journal_date: None,
            excerpt: String::new(),
            state: ReplyState::Missing,
            years_later: 0,
            attachments: AttachmentSummary::default(),
        },
    }
}

/// The start of an entry as plain text on one line: no Markdown markup, line breaks turned into spaces, no longer than
/// [`EXCERPT_CHARS`] characters with "…" at the end. In a Telegram reply from the other party the first line is the author's name in bold: the quote
/// gets the name and the start of the text.
pub fn excerpt(body_md: &str) -> String {
    snippet(body_md, &[], EXCERPT_CHARS)
}

/// Full years between `old` (the day of the old entry) and `reply` (the day of the reply). Less than a year, a reply earlier than the original (the
/// entry date was changed) or not a date gives 0. February 29 in a non-leap year counts as earlier than March 1 (`chrono::NaiveDate::years_since`).
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
