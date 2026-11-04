use kinige_domain::ErrorCode;
use kinige_domain::chat::ChatInputError;
use kinige_domain::groups::{BodyError, GroupError};
use std::fmt;

/// Store error. The frontend gets only the code (`ErrorCode`); details stay in the process log.
#[derive(Debug)]
pub enum StoreError {
    /// Empty entry text.
    Empty,
    /// Text longer than the limit.
    TooLarge,
    ChatNotFound,
    /// The entry does not exist (deleted forever or the id is wrong).
    MessageNotFound,
    /// The entry is in the trash: moving to another chat is for live entries only (restore it first).
    MessageInTrash,
    /// The entry being replied to is not found: wrong id, in the trash, deleted forever or in another chat.
    ReplyTargetNotFound,
    /// An entry with a group of an unsupported version: read only (ADR-0001 §4).
    ReadOnly,
    /// Inbox cannot be renamed, archived or deleted.
    InboxProtected,
    /// "Delete forever" is only for an entry from the trash.
    NotInTrash,
    /// The entry date does not exist.
    InvalidDate,
    /// The chat name or emoji was not accepted.
    ChatInput(ChatInputError),
    /// There is no task item with this number, or it is already in the wanted state.
    TaskItemStale,
    /// The shortcut is written wrongly, has no modifier or is reserved by the system.
    ShortcutInvalid,
    /// This shortcut is already assigned to another Kinige action.
    ShortcutDuplicate,
    /// The database is corrupt: the journal is not open, a backup is needed.
    Corrupt(String),
    /// Only the search table (a derived one) is corrupt: healed by a rebuild from the entries, the database is intact.
    SearchIndex(String),
    /// The backup is not found, does not open or is corrupt.
    BackupNotFound,
    /// The previous Markdown copy folder could not be renamed before the restore: nothing was touched.
    MirrorFolderBusy,
    /// The database backup before the import was not taken: the import did not start, nothing was written.
    ImportBackupFailed,
    /// The attachment is larger than the limit (slice 7).
    AttachmentTooLarge,
    /// The entry would have more attachments than allowed.
    AttachmentLimit,
    /// The file could not be read or changed while it was copied.
    AttachmentUnreadable,
    /// No space left on the disk: nothing was written.
    DiskFull,
    /// A draft attachment of this chat with this id does not exist.
    AttachmentNotFound,
    /// The recording is shorter than one second (slice 7a): not saved.
    VoiceTooShort,
    /// The recording is longer than the recorder allows (30 minutes): not saved.
    VoiceTooLong,
    /// The database was created by a newer app version (schema `found`): downgrade is not supported (PRD §10.4).
    TooNew(i64),
    /// Writing to the database failed: nothing was written (the transaction rolled back).
    Write(String),
    /// Reading failed.
    Read(String),
    /// Everything else: a broken stream, corrupt groups, a database newer than the app.
    Internal(String),
}

impl StoreError {
    pub fn code(&self) -> ErrorCode {
        match self {
            StoreError::Empty => ErrorCode::EmptyMessage,
            StoreError::TooLarge => ErrorCode::MessageTooLarge,
            StoreError::ChatNotFound => ErrorCode::ChatNotFound,
            StoreError::MessageNotFound => ErrorCode::MessageNotFound,
            StoreError::MessageInTrash => ErrorCode::MessageInTrash,
            StoreError::ReplyTargetNotFound => ErrorCode::ReplyTargetNotFound,
            StoreError::ReadOnly => ErrorCode::ReadOnly,
            StoreError::InboxProtected => ErrorCode::InboxProtected,
            StoreError::NotInTrash => ErrorCode::NotInTrash,
            StoreError::InvalidDate => ErrorCode::InvalidDate,
            StoreError::ChatInput(ChatInputError::TitleEmpty) => ErrorCode::ChatTitleEmpty,
            StoreError::ChatInput(ChatInputError::TitleTooLong) => ErrorCode::ChatTitleTooLong,
            StoreError::ChatInput(ChatInputError::IconInvalid) => ErrorCode::ChatIconInvalid,
            StoreError::TaskItemStale => ErrorCode::TaskItemStale,
            StoreError::ShortcutInvalid => ErrorCode::ShortcutInvalid,
            StoreError::ShortcutDuplicate => ErrorCode::ShortcutDuplicate,
            StoreError::Corrupt(_) => ErrorCode::DatabaseCorrupt,
            StoreError::SearchIndex(_) => ErrorCode::Internal,
            StoreError::BackupNotFound => ErrorCode::BackupNotFound,
            StoreError::MirrorFolderBusy => ErrorCode::MirrorFolderBusy,
            StoreError::ImportBackupFailed => ErrorCode::ImportBackupFailed,
            StoreError::AttachmentTooLarge => ErrorCode::AttachmentTooLarge,
            StoreError::AttachmentLimit => ErrorCode::AttachmentLimit,
            StoreError::AttachmentUnreadable => ErrorCode::AttachmentUnreadable,
            StoreError::DiskFull => ErrorCode::DiskFull,
            StoreError::AttachmentNotFound => ErrorCode::AttachmentNotFound,
            StoreError::VoiceTooShort => ErrorCode::VoiceTooShort,
            StoreError::VoiceTooLong => ErrorCode::VoiceTooLong,
            StoreError::TooNew(_) => ErrorCode::DatabaseTooNew,
            StoreError::Write(_) => ErrorCode::StoreWriteFailed,
            StoreError::Read(_) => ErrorCode::StoreReadFailed,
            StoreError::Internal(_) => ErrorCode::Internal,
        }
    }
}

impl StoreError {
    /// Category for the process log: without the inner error text (SQLite, serde, paths, addresses) -- it may contain
    /// pieces of the user's data.
    pub fn category(&self) -> &'static str {
        match self {
            StoreError::Empty => "empty",
            StoreError::TooLarge => "too_large",
            StoreError::ChatNotFound => "chat_not_found",
            StoreError::MessageNotFound => "message_not_found",
            StoreError::MessageInTrash => "message_in_trash",
            StoreError::ReplyTargetNotFound => "reply_target_not_found",
            StoreError::ReadOnly => "read_only",
            StoreError::InboxProtected => "inbox_protected",
            StoreError::NotInTrash => "not_in_trash",
            StoreError::InvalidDate => "invalid_date",
            StoreError::ChatInput(_) => "chat_input",
            StoreError::TaskItemStale => "task_item_stale",
            StoreError::ShortcutInvalid => "shortcut_invalid",
            StoreError::ShortcutDuplicate => "shortcut_duplicate",
            StoreError::Corrupt(_) => "corrupt",
            StoreError::SearchIndex(_) => "search_index",
            StoreError::BackupNotFound => "backup_not_found",
            StoreError::MirrorFolderBusy => "mirror_folder_busy",
            StoreError::ImportBackupFailed => "import_backup_failed",
            StoreError::AttachmentTooLarge => "attachment_too_large",
            StoreError::AttachmentLimit => "attachment_limit",
            StoreError::AttachmentUnreadable => "attachment_unreadable",
            StoreError::DiskFull => "disk_full",
            StoreError::AttachmentNotFound => "attachment_not_found",
            StoreError::VoiceTooShort => "voice_too_short",
            StoreError::VoiceTooLong => "voice_too_long",
            StoreError::TooNew(_) => "too_new",
            StoreError::Write(_) => "write",
            StoreError::Read(_) => "read",
            StoreError::Internal(_) => "internal",
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Empty => f.write_str("empty entry"),
            StoreError::TooLarge => f.write_str("entry is too long"),
            StoreError::ChatNotFound => f.write_str("chat not found"),
            StoreError::MessageNotFound => f.write_str("entry not found"),
            StoreError::MessageInTrash => f.write_str("entry is in the trash"),
            StoreError::ReplyTargetNotFound => f.write_str("reply target entry not found"),
            StoreError::ReadOnly => f.write_str("entry is read-only"),
            StoreError::InboxProtected => f.write_str("Inbox cannot be changed this way"),
            StoreError::NotInTrash => f.write_str("entry is not in the trash"),
            StoreError::InvalidDate => f.write_str("entry date does not exist"),
            StoreError::ChatInput(why) => write!(f, "{why}"),
            StoreError::TaskItemStale => f.write_str("task item changed"),
            StoreError::ShortcutInvalid => f.write_str("invalid shortcut"),
            StoreError::ShortcutDuplicate => f.write_str("shortcut already assigned"),
            StoreError::Corrupt(why) => write!(f, "database is corrupt: {why}"),
            StoreError::SearchIndex(why) => write!(f, "search index is corrupt: {why}"),
            StoreError::BackupNotFound => f.write_str("backup not found or corrupt"),
            StoreError::MirrorFolderBusy => {
                f.write_str("could not rename the Markdown copy folder")
            }
            StoreError::ImportBackupFailed => f.write_str("backup before the import failed"),
            StoreError::AttachmentTooLarge => f.write_str("attachment is too large"),
            StoreError::AttachmentLimit => f.write_str("too many attachments"),
            StoreError::AttachmentUnreadable => f.write_str("file could not be read"),
            StoreError::DiskFull => f.write_str("not enough disk space"),
            StoreError::AttachmentNotFound => f.write_str("draft attachment not found"),
            StoreError::VoiceTooShort => f.write_str("recording is too short"),
            StoreError::VoiceTooLong => f.write_str("recording is too long"),
            StoreError::TooNew(found) => {
                write!(
                    f,
                    "database was created by a newer app version (schema {found})"
                )
            }
            StoreError::Write(why) => write!(f, "writing to the database failed: {why}"),
            StoreError::Read(why) => write!(f, "reading the database failed: {why}"),
            StoreError::Internal(why) => write!(f, "internal error: {why}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<BodyError> for StoreError {
    fn from(error: BodyError) -> Self {
        match error {
            BodyError::Empty => StoreError::Empty,
            BodyError::TooLarge => StoreError::TooLarge,
        }
    }
}

impl From<ChatInputError> for StoreError {
    fn from(error: ChatInputError) -> Self {
        StoreError::ChatInput(error)
    }
}

impl From<GroupError> for StoreError {
    /// A group newer than supported means a read-only entry; too long is `TooLarge`; anything else is corruption.
    fn from(error: GroupError) -> Self {
        match error {
            GroupError::TooLarge => StoreError::TooLarge,
            GroupError::UnsupportedVersion => StoreError::ReadOnly,
            other => StoreError::Internal(other.to_string()),
        }
    }
}

/// SQLite error on write; "not a database" and "database is corrupt" are `Corrupt`, not a write failure.
pub(crate) fn write_err(error: rusqlite::Error) -> StoreError {
    if is_search_index_error(&error) {
        return StoreError::SearchIndex(error.to_string());
    }
    if is_corruption(&error) {
        return StoreError::Corrupt(error.to_string());
    }
    StoreError::Write(error.to_string())
}

/// SQLite error on read.
pub(crate) fn read_err(error: rusqlite::Error) -> StoreError {
    if is_search_index_error(&error) {
        return StoreError::SearchIndex(error.to_string());
    }
    if is_corruption(&error) {
        return StoreError::Corrupt(error.to_string());
    }
    StoreError::Read(error.to_string())
}

/// An error of the search table `search_fts` itself, not of the database: `SQLITE_CORRUPT_VTAB` (extended code 267, how
/// FTS5 reports corruption of its data) and "invalid fts5 file format ... run 'rebuild'" (`SQLITE_ERROR`, corrupt index
/// settings). The table is derived from `message.body_md`: healed by a rebuild, not by a restore from a backup.
pub(crate) fn is_search_index_error(error: &rusqlite::Error) -> bool {
    const SQLITE_CORRUPT_VTAB: i32 = 267;
    match error {
        rusqlite::Error::SqliteFailure(failure, message) => {
            failure.extended_code == SQLITE_CORRUPT_VTAB
                || (failure.code == rusqlite::ErrorCode::Unknown
                    && message.as_deref().is_some_and(|m| m.contains("fts5")))
        }
        _ => false,
    }
}

fn is_corruption(error: &rusqlite::Error) -> bool {
    matches!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_category_never_contains_the_inner_text() {
        let secret =
            "expected value at line 1: {\"body_md\":\"личное\"} https://example.com/?token=x";
        for error in [
            StoreError::Write(secret.into()),
            StoreError::Read(secret.into()),
            StoreError::Internal(secret.into()),
            StoreError::Corrupt(secret.into()),
        ] {
            let category = error.category();
            assert!(["write", "read", "internal", "corrupt"].contains(&category));
            assert!(!category.contains("личное") && !category.contains("example"));
        }
    }

    #[test]
    fn unsupported_group_version_is_read_only_and_too_large_stays_too_large() {
        assert!(matches!(
            StoreError::from(GroupError::UnsupportedVersion),
            StoreError::ReadOnly
        ));
        assert!(matches!(
            StoreError::from(GroupError::TooLarge),
            StoreError::TooLarge
        ));
        assert_eq!(
            StoreError::from(GroupError::HlcNotNewer).code(),
            ErrorCode::Internal
        );
    }
}
