//! The row of an imported entry (slice 4, task 4.3). The same five groups under one HLC as in `groups::new_message`,
//! but `core` carries the origin: `origin = "import_telegram"`, the external id `ext` and `reply_to_id`
//! (ADR-0001, appendix §C.3). The original Telegram time goes into `created_at` and `journal_date`, not into the HLC:
//! the HLC is the moment of the import (ADR-0001 §10, "except import").

use crate::groups::{GroupError, MessageRow, NewMessage, Provenance, build_message};
use crate::hlc::is_uuid_v4;
use serde::Serialize;
use serde_json::json;
use ts_rs::TS;

/// The `origin` of an entry imported from Telegram: immutable, `origin = 'import_telegram'` iff `ext.src = 'telegram'`.
pub const ORIGIN_IMPORT_TELEGRAM: &str = "import_telegram";
/// `ext.src` of entries from Telegram.
pub const EXT_SRC_TELEGRAM: &str = "telegram";

/// The source values of an imported entry; the text is already normalized (`groups::normalize_body`).
pub struct NewImportedMessage<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub body_md: &'a str,
    pub created_at: i64,
    pub created_tz: &'a str,
    pub journal_date: &'a str,
    /// The Telegram chat id, a decimal integer with or without a sign, up to 20 characters.
    pub ext_chat: &'a str,
    /// The Telegram message id, in the same form.
    pub ext_msg: &'a str,
    /// The id of the Kinige entry (UUID v4) this message replies to, if it is already imported.
    pub reply_to_id: Option<&'a str>,
}

/// `^-?[0-9]{1,20}$`: Telegram ids are 64-bit, in `core` they are stored as a string (JCS is limited to ±2⁵³).
fn is_external_id(text: &str) -> bool {
    let digits = text.strip_prefix('-').unwrap_or(text);
    (1..=20).contains(&digits.len()) && digits.bytes().all(|b| b.is_ascii_digit())
}

/// The row of an imported entry with all five groups under one `hlc`, `sv = 1`, `server_known = 0`.
pub fn new_imported_message(
    input: &NewImportedMessage<'_>,
    hlc: &str,
) -> Result<MessageRow, GroupError> {
    if !is_external_id(input.ext_chat) {
        return Err(GroupError::Invalid("ext.chat is not a Telegram number"));
    }
    if !is_external_id(input.ext_msg) {
        return Err(GroupError::Invalid("ext.msg is not a Telegram number"));
    }
    if input.reply_to_id.is_some_and(|id| !is_uuid_v4(id)) {
        return Err(GroupError::Invalid("reply_to_id is not a UUID v4"));
    }
    let provenance = Provenance {
        origin: ORIGIN_IMPORT_TELEGRAM,
        reply_to_id: input.reply_to_id,
        ext: Some(json!({
            "chat": input.ext_chat,
            "msg": input.ext_msg,
            "src": EXT_SRC_TELEGRAM,
        })),
        is_task: false,
    };
    let base = NewMessage {
        id: input.id,
        chat_id: input.chat_id,
        body_md: input.body_md,
        created_at: input.created_at,
        created_tz: input.created_tz,
        journal_date: input.journal_date,
    };
    build_message(&base, &provenance, hlc)
}

/// The kind of Telegram chat that is imported (groups and channels are not imported).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ImportChatKind.ts")]
pub enum ImportChatKind {
    /// "Saved Messages": entries go to Inbox.
    Saved,
    /// A personal chat: its own Kinige chat.
    Personal,
}

/// An export chat in the selection window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportChatPreview.ts")]
pub struct ImportChatPreview {
    /// The Telegram chat id (a string: a 64-bit number).
    pub id: String,
    /// The name from the export; "Saved Messages" has none.
    pub name: Option<String>,
    pub kind: ImportChatKind,
    /// How many entries there will be: messages without service and empty ones.
    #[ts(type = "number")]
    pub total: i64,
    /// Of them already imported (present in `message_ext`).
    #[ts(type = "number")]
    pub existing: i64,
    /// Files the export holds for these entries: photos, files, videos, voice messages, audio (slice 7).
    #[ts(type = "number")]
    pub media_files: i64,
    /// Their size in bytes as the export states it.
    #[ts(type = "number")]
    pub media_bytes: i64,
    /// Media the export says it did not download ("File not included"): they stay placeholders.
    #[ts(type = "number")]
    pub media_missing: i64,
}

/// What was found in the export: chats to choose from and how many chats were skipped (groups, channels, bots).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportPreview.ts")]
pub struct ImportPreview {
    pub chats: Vec<ImportChatPreview>,
    #[ts(type = "number")]
    pub skipped_chats: i64,
}

/// The answer to choosing a file: the session number (the parsed export waits in Rust) and what was found in it. The
/// file path does not reach the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportOpened.ts")]
pub struct ImportOpened {
    #[ts(type = "number")]
    pub session: u64,
    pub preview: ImportPreview,
}

/// Import progress after each committed batch: how many messages are processed out of how many, and, with media, how many bytes
/// of files are copied out of how many (slice 7: progress runs in gigabytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportProgress.ts")]
pub struct ImportProgress {
    #[ts(type = "number")]
    pub done: i64,
    #[ts(type = "number")]
    pub total: i64,
    /// Bytes of files copied so far; 0 when the import has no media.
    #[ts(type = "number")]
    pub bytes_done: i64,
    /// Bytes of files to copy, as the export states them.
    #[ts(type = "number")]
    pub bytes_total: i64,
}

/// The import result.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ImportReport.ts")]
pub struct ImportReport {
    #[ts(type = "number")]
    pub imported: i64,
    /// Were imported earlier (including those in the trash).
    #[ts(type = "number")]
    pub skipped_existing: i64,
    /// Service messages (joined, pinned...).
    #[ts(type = "number")]
    pub skipped_service: i64,
    /// Without text and without an attachment.
    #[ts(type = "number")]
    pub skipped_empty: i64,
    /// For how many imported entries a placeholder is left instead of an attachment.
    #[ts(type = "number")]
    pub media_placeholders: i64,
    /// Files attached (to new entries and to entries imported earlier, slice 7).
    #[ts(type = "number")]
    pub media_attached: i64,
    /// Bytes of the files attached.
    #[ts(type = "number")]
    pub media_bytes: i64,
    /// Files the export does not hold ("File not included", or the file is not where the export says).
    #[ts(type = "number")]
    pub media_missing: i64,
    /// Files not attached: a bad path, unreadable, over the limit or not a regular file.
    #[ts(type = "number")]
    pub media_skipped: i64,
    /// The import stopped because the disk is full; the batch in progress was rolled back, earlier batches stay.
    pub disk_full: bool,
    /// Kinige chats created (for personal chats).
    #[ts(type = "number")]
    pub chats_created: i64,
    /// The database backup file taken before the import; `null` means there was nothing to write and no backup was taken.
    pub backup_name: Option<String>,
    /// The import was stopped by the user; everything written before the stop stays.
    pub stopped: bool,
}

#[cfg(test)]
mod tests;
