//! Types exchanged between Rust and the frontend (`KinigeApi`). TS types are generated in `web/src/api/generated/`
//! (`cargo test`, ADR-0002). Error texts live in the frontend: Rust returns only codes.

use crate::api_error::ErrorCode;
use crate::attachment::AttachmentView;
use crate::backup::BackupStatus;
use crate::forest_year::{ForestMode, ForestView};
use crate::lang::Language;
use crate::reply::ReplyTo;
use crate::shortcuts::ShortcutStatus;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A chat in the sidebar. Inbox has an empty stored name: the interface shows the translated one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ChatSummary.ts")]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub is_inbox: bool,
    /// The chat's entries, except the trash.
    #[ts(type = "number")]
    pub count: i64,
    /// When pinned (ms); `null` -- not pinned. Pinned chats go in the "Pinned" section in the order they were pinned.
    #[ts(type = "number | null")]
    pub pinned_at: Option<i64>,
    /// When sent to the archive (ms); `null` -- not archived.
    #[ts(type = "number | null")]
    pub archived_at: Option<i64>,
}

/// The sidebar chats in display order and the trash size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ChatList.ts")]
pub struct ChatList {
    /// Pinned first (by pin time), then the rest and the archive (by `sort_key`, ties by `id`).
    pub chats: Vec<ChatSummary>,
    /// Entries in the trash.
    #[ts(type = "number")]
    pub trash_count: i64,
}

/// An entry in the feed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MessageView.ts")]
pub struct MessageView {
    pub id: String,
    pub chat_id: String,
    pub body_md: String,
    /// `YYYY-MM-DD`, the day boundary is 04:00 (PRD §4.1).
    pub journal_date: String,
    /// UTC, ms.
    #[ts(type = "number")]
    pub created_at: i64,
    /// The device's IANA time zone at the moment of creation.
    pub created_tz: String,
    /// The entry is a task (the flag on the bubble).
    pub is_task: bool,
    /// When the task was marked done (ms); `null` -- not done.
    #[ts(type = "number | null")]
    pub task_done_at: Option<i64>,
    /// When added to favorites (ms); `null` -- not a favorite.
    #[ts(type = "number | null")]
    pub favorite_at: Option<i64>,
    /// The entry's tags in lower case, without `#` (parsing -- `kinige_domain::tags`).
    pub tags: Vec<String>,
    /// One of the entry's groups is newer than the supported version: the entry is read-only, actions are unavailable (ADR-0001 §4).
    pub read_only: bool,
    /// Who created the entry (`core.origin`): `user`, `import_telegram`, `agent`. The feed marks agent entries.
    pub origin: String,
    /// The entry this one replies to (slice 5): set for new replies and for replies imported from Telegram; `null` -- not a reply.
    pub reply_to: Option<ReplyTo>,
    /// The live attachments in their order (slice 7). Empty for most entries.
    pub attachments: Vec<AttachmentView>,
}

/// The draft of the chat input field (slice 5): the text and the entry being replied to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ChatDraft.ts")]
pub struct ChatDraft {
    pub body: String,
    /// The "Reply to entry" line above the field; `null` -- a plain entry. If the entry no longer exists, `state = missing`.
    pub reply_to: Option<ReplyTo>,
    /// The tiles above the field (slice 7): attachments not yet sent, in their order.
    pub attachments: Vec<AttachmentView>,
}

/// Which entry flag the `set_message_flag` command changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "MessageFlag.ts")]
pub enum MessageFlag {
    /// The entry is a task (the flag). Clearing it resets "done".
    Task,
    /// The task is done; only for a task.
    TaskDone,
    /// Favorite.
    Favorite,
}

/// An entry in the trash (PRD §2.1): chat, date, how long is left until auto-deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TrashItem.ts")]
pub struct TrashItem {
    pub message: MessageView,
    pub chat_title: String,
    pub chat_icon: Option<String>,
    pub chat_is_inbox: bool,
    /// When deleted (ms).
    #[ts(type = "number")]
    pub deleted_at: i64,
    /// How many whole days are left until auto-deletion (a partial day counts as a day).
    #[ts(type = "number")]
    pub days_left: i64,
}

/// A feed position for paged loading (keyset): `(journal_date, created_at, id)` of the last loaded entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FeedCursor.ts")]
pub struct FeedCursor {
    pub journal_date: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub id: String,
}

/// A feed page: entries in ascending `(journal_date, created_at, id)` order, the newest last.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MessagePage.ts")]
pub struct MessagePage {
    pub messages: Vec<MessageView>,
    /// The position for the next (older) page; `null` -- there are no more entries.
    pub older: Option<FeedCursor>,
    /// The position for the next (newer) page: the last entry of this one; `null` -- nothing is newer (slice 4).
    pub newer: Option<FeedCursor>,
    /// Today's `journal_date` (with the 04:00 day boundary): the "Today" and "Yesterday" labels are counted from it.
    pub today: String,
}

/// Search filters (slice 4, §9.1): all optional and combined with "and".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export, export_to = "SearchFilters.ts")]
pub struct SearchFilters {
    /// Only entries of this chat; `null` -- everywhere.
    pub chat_id: Option<String>,
    /// Favorites only.
    pub favorite: bool,
    /// Tasks only.
    pub tasks: bool,
    /// From this `journal_date`, inclusive (`YYYY-MM-DD`).
    pub from: Option<String>,
    /// Up to this `journal_date`, inclusive.
    pub to: Option<String>,
}

/// A found entry: the entry itself, its chat and what the result row shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SearchHit.ts")]
pub struct SearchHit {
    pub message: MessageView,
    pub chat_title: String,
    pub chat_icon: Option<String>,
    pub chat_is_inbox: bool,
    /// Plain text around the first match, up to 200 characters, with "…" at the trimmed edges.
    pub snippet: String,
    /// The entry's words that matched the query, as written in it: the frontend highlights them whole.
    pub terms: Vec<String>,
}

/// A page of search results: new entries on top (feed order, the newest first).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SearchPage.ts")]
pub struct SearchPage {
    /// Chats whose name matched the query; only on the first page. The frontend translates Inbox's name.
    pub chats: Vec<ChatSummary>,
    pub hits: Vec<SearchHit>,
    /// The position for the next page (the last entry of this one); `null` -- there are no more.
    pub older: Option<FeedCursor>,
}

/// Text size (ADR-0004): "Normal" or "Large" at ×1.25.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export, export_to = "TextSize.ts")]
pub enum TextSize {
    Normal,
    Large,
}

impl TextSize {
    pub fn as_setting(self) -> &'static str {
        match self {
            TextSize::Normal => "normal",
            TextSize::Large => "large",
        }
    }

    /// The value from `app_setting`; an unknown one means the normal size.
    pub fn from_setting(value: &str) -> TextSize {
        if value == "large" {
            TextSize::Large
        } else {
            TextSize::Normal
        }
    }
}

/// Why the Markdown copy is not being written right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "MirrorError.ts")]
pub enum MirrorError {
    /// The folder cannot be created or opened (no access, no disk, it is a file).
    FolderUnavailable,
    /// The day's file could not be written.
    WriteFailed,
}

/// The state of the Markdown copy for the "Settings" window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MirrorStatus.ts")]
pub struct MirrorStatus {
    /// The folder path; `null` if no folder is chosen and there is no default one -- the copy is off.
    pub folder: Option<String>,
    /// The default folder (`~/Documents/Kinige`), the user did not change it.
    pub is_default: bool,
    pub error: Option<MirrorError>,
    /// Days waiting to be written to the copy.
    #[ts(type = "number")]
    pub pending_days: i64,
}

/// Settings (not synced, `app_setting`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "Settings.ts")]
pub struct Settings {
    pub text_size: TextSize,
    pub mirror: MirrorStatus,
    /// Global keyboard shortcuts (slice 2): one for each action.
    pub shortcuts: Vec<ShortcutStatus>,
    /// The sidebar is collapsed to an emoji strip (slice 3).
    pub sidebar_collapsed: bool,
    /// The picker's recent emoji, the freshest first (slice 3).
    pub recent_emoji: Vec<String>,
    /// The database file backup (slice 3).
    pub backup: BackupStatus,
    /// How much of the forest is shown above the feed (slice 6); local, not synced, not in `settings.json`.
    pub forest_view: ForestView,
    /// What an expanded forest shows: the board or the calendar (slice 6).
    pub forest_mode: ForestMode,
}

/// What the quick capture did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "CaptureAction.ts")]
pub enum CaptureAction {
    /// A new entry was created.
    Created,
    /// The text was appended to the last entry.
    Appended,
}

/// The reply to the capture window: what was done and the entry as it looks now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "CaptureOutcome.ts")]
pub struct CaptureOutcome {
    pub action: CaptureAction,
    pub message: MessageView,
}

/// Everything the capture window needs when shown, in one reply (§5.1, slice 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "CaptureState.ts")]
pub struct CaptureState {
    pub language: Language,
    pub text_size: TextSize,
    /// Today's `journal_date` (with the 04:00 day boundary).
    pub today: String,
    /// Up to three last non-deleted Inbox entries of today, ascending: the newest last.
    pub context: Vec<MessageView>,
    /// The id of the entry that ⌘Enter appends the text to; `null` -- ⌘Enter creates a new entry.
    pub append_target: Option<String>,
    /// The capture window's draft.
    pub draft: String,
    /// The error the window was opened with (a failure writing from the clipboard): shown once.
    pub error: Option<ErrorCode>,
    /// The picker's "Recent" emoji (a list shared with the main window), the freshest first.
    pub recent_emoji: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn views_serialize_in_camel_case() {
        let view = MessageView {
            id: "i".into(),
            chat_id: "c".into(),
            body_md: "b".into(),
            journal_date: "2026-10-21".into(),
            created_at: 1,
            created_tz: "UTC".into(),
            is_task: true,
            task_done_at: None,
            favorite_at: Some(5),
            tags: vec!["идея".into()],
            read_only: false,
            origin: "agent".into(),
            reply_to: None,
            attachments: Vec::new(),
        };
        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            r#"{"id":"i","chatId":"c","bodyMd":"b","journalDate":"2026-10-21","createdAt":1,"createdTz":"UTC","isTask":true,"taskDoneAt":null,"favoriteAt":5,"tags":["идея"],"readOnly":false,"origin":"agent","replyTo":null,"attachments":[]}"#
        );
    }

    #[test]
    fn text_size_round_trips_through_the_setting() {
        for size in [TextSize::Normal, TextSize::Large] {
            assert_eq!(TextSize::from_setting(size.as_setting()), size);
        }
        assert_eq!(TextSize::from_setting("huge"), TextSize::Normal);
    }
}
