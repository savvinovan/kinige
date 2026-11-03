//! Types of the `settings.json` settings file for the "Settings file" window (slice 4b, ADR-0010): the path, what is wrong
//! with the file, key errors and when a section was changed through the file. No IO. The TS types are generated into
//! `web/src/api/generated/` (ADR-0002).

use serde::Serialize;
use ts_rs::TS;

/// The state of the file for the window.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "SettingsFileState.ts")]
pub struct SettingsFileState {
    /// The absolute path of the file: shown and copied ("Copy path").
    pub path: String,
    /// The file did not parse or could not be read; no setting from it was applied.
    pub problem: Option<FileProblem>,
    /// Why the file is unsafe: the `agent_access.*` and `mirror.folder` keys from it are not applied.
    pub unsafe_reason: Option<UnsafeReason>,
    /// Errors of individual keys: the other keys were applied.
    pub issues: Vec<KeyIssue>,
    /// Unknown keys: a warning, they are kept in the file.
    pub unknown_keys: Vec<String>,
    /// How many unknown keys did not fit into the list ("and N more").
    pub unknown_more: u32,
    /// When a section was changed through the file.
    pub changed: FileChanges,
}

/// The file as a whole is unusable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FileProblem.ts")]
pub struct FileProblem {
    pub kind: FileProblemKind,
    /// Line and column (1-based) for `parse`.
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "FileProblemKind.ts")]
pub enum FileProblemKind {
    /// Not JSON.
    Parse,
    /// JSON, but not an object.
    NotObject,
    /// Not UTF-8.
    NotUtf8,
    /// Starts with a BOM.
    Bom,
    /// Larger than 1 MiB.
    TooLarge,
    /// Could not be read or written (permissions, disk).
    Io,
}

/// Why the file is unsafe (ADR-0010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "UnsafeReason.ts")]
pub enum UnsafeReason {
    /// The file is a symbolic link.
    Symlink,
    /// Not a regular file (a pipe, a device).
    NotRegular,
    /// The owner is not the current user.
    Owner,
    /// The file is writable by the group or by everyone.
    Permissions,
    /// The data directory is writable by the group or by everyone (or it is a link).
    Directory,
}

/// An error of one key (or array element): `key` is a path like `agent_access.chats[2]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "KeyIssue.ts")]
pub struct KeyIssue {
    pub key: String,
    pub kind: KeyIssueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "KeyIssueKind.ts")]
pub enum KeyIssueKind {
    /// The wrong type of value.
    Type,
    /// An invalid value.
    Value,
    /// The shortcut is written incorrectly or is reserved.
    ShortcutInvalid,
    /// The shortcut is already assigned to another action.
    ShortcutDuplicate,
    /// The system did not accept the shortcut.
    ShortcutTaken,
    FolderNotAbsolute,
    FolderMissing,
    /// The folder is inside the data directory.
    FolderInsideData,
    /// The file is unsafe: the key was not applied.
    UnsafeIgnored,
    ChatNotFound,
    ChatAmbiguous,
    /// The chat is excluded from AI.
    ChatExcluded,
    /// The key is not available on this platform (`agent_access` on Windows until named pipes, slice 10a): not applied.
    Unsupported,
}

/// When a section was changed through the file (UTC, ms); `null` means it was not changed in this run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FileChanges.ts")]
pub struct FileChanges {
    #[ts(type = "number | null")]
    pub appearance: Option<i64>,
    #[ts(type = "number | null")]
    pub shortcuts: Option<i64>,
    #[ts(type = "number | null")]
    pub mirror: Option<i64>,
    #[ts(type = "number | null")]
    pub access: Option<i64>,
}
