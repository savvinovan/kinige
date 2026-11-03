//! A command error as the frontend receives it: a code; the text lives in the frontend (`error.<code>` in the translation files).

use serde::Serialize;
use ts_rs::TS;

/// Command error code; the text lives in the frontend (`error.<code>` in the translation files).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ErrorCode.ts")]
pub enum ErrorCode {
    EmptyMessage,
    MessageTooLarge,
    ChatNotFound,
    StoreWriteFailed,
    StoreReadFailed,
    LinkNotAllowed,
    FolderPickFailed,
    /// The shortcut is not accepted: no modifier, reserved by the system, or written incorrectly.
    ShortcutInvalid,
    /// The shortcut is taken by another program or by the system.
    ShortcutTaken,
    /// The same shortcut is already assigned to another Kinige action.
    ShortcutDuplicate,
    /// The entry is not found (deleted permanently or the id is wrong).
    MessageNotFound,
    /// The entry is in the trash: moving to another chat is only for live entries, "Restore" first.
    MessageInTrash,
    /// The entry being replied to (slice 5) is not found: the id is wrong, it is in the trash, deleted permanently or in another chat.
    ReplyTargetNotFound,
    /// An entry with a group in an unsupported version: read-only (ADR-0001 §4).
    ReadOnly,
    /// Inbox cannot be renamed, archived or deleted.
    InboxProtected,
    /// The action is only for an entry in the trash (delete permanently).
    NotInTrash,
    /// The entry date does not exist.
    InvalidDate,
    /// The chat name is empty.
    ChatTitleEmpty,
    /// The chat name is longer than 128 characters.
    ChatTitleTooLong,
    /// The chat emoji is not accepted.
    ChatIconInvalid,
    /// There is no task item with this number in the text, or it is already in the needed state: the window showed stale text.
    TaskItemStale,
    /// The database is corrupt.
    DatabaseCorrupt,
    /// The database was created by a newer version of the app (§10.4): downgrade is not supported, Kinige must be updated.
    DatabaseTooNew,
    /// The backup is not found or cannot be opened.
    BackupNotFound,
    /// The Markdown copy folder could not be renamed (busy with another program, no permissions): the restore did not start,
    /// nothing was touched; close the folder and retry.
    MirrorFolderBusy,
    /// The chosen file is not a Telegram export (not JSON or someone else's JSON): nothing was written (slice 4).
    ImportNotTelegram,
    /// The chosen file could not be read.
    ImportFileUnreadable,
    /// The file is larger than the 1 GiB limit.
    ImportFileTooLarge,
    /// An import is already running: only one at a time.
    ImportInProgress,
    /// The parsed export was forgotten (the window was closed or another file was chosen): choose the file again.
    ImportSessionExpired,
    /// The database backup before the import was not taken: no entries were added.
    ImportBackupFailed,
    /// The action is not available on this platform: AI-agent access (MCP) on Windows until named pipes (slice 10a).
    Unsupported,
    Internal,
}

/// A command error as the frontend receives it: `{ "code": "store_write_failed" }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "ApiError.ts")]
pub struct ApiError {
    pub code: ErrorCode,
}

impl From<ErrorCode> for ApiError {
    fn from(code: ErrorCode) -> Self {
        ApiError { code }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_as_code_object() {
        assert_eq!(
            serde_json::to_string(&ApiError::from(ErrorCode::StoreWriteFailed)).unwrap(),
            r#"{"code":"store_write_failed"}"#
        );
        assert_eq!(
            serde_json::to_string(&ApiError::from(ErrorCode::LinkNotAllowed)).unwrap(),
            r#"{"code":"link_not_allowed"}"#
        );
    }
}
