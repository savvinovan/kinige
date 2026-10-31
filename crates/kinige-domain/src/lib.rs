//! Правила предметной области Kinige. Без IO; собирается и под `wasm32` (§3.2 ТЗ).

pub mod agent;
pub mod agent_api;
pub mod agent_reply;
pub mod api;
pub mod api_error;
pub mod append;
pub mod backup;
pub mod chat;
pub mod draft;
pub mod emoji;
pub mod folders;
pub mod groups;
pub mod history;
pub mod hlc;
pub mod import;
pub mod jcs;
pub mod journal;
pub mod lang;
pub mod link;
pub mod message_ops;
pub mod mirror;
pub mod rewrite;
pub mod search;
pub mod settings_file;
pub mod shortcuts;
pub mod tags;
pub mod tasks;
pub mod telegram;

#[cfg(test)]
mod chat_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod move_tests;
#[cfg(test)]
mod ops_tests;

use serde::Serialize;
use ts_rs::TS;

pub use agent_api::{
    AgentAccess, AgentChatChoice, AgentLogEntry, AgentLogPage, AgentLogRow, AgentMode, AgentSocket,
};
pub use api::{
    CaptureAction, CaptureOutcome, CaptureState, ChatList, ChatSummary, FeedCursor, MessageFlag,
    MessagePage, MessageView, MirrorError, MirrorStatus, SearchFilters, SearchHit, SearchPage,
    Settings, TextSize, TrashItem,
};
pub use api_error::{ApiError, ErrorCode};
pub use backup::{BackupError, BackupInfo, BackupKind, BackupStatus, RestoreOutcome, StartupState};
pub use import::{
    ImportChatKind, ImportChatPreview, ImportOpened, ImportPreview, ImportProgress, ImportReport,
};
pub use lang::Language;
pub use settings_file::{
    FileChanges, FileProblem, FileProblemKind, KeyIssue, KeyIssueKind, SettingsFileState,
    UnsafeReason,
};
pub use shortcuts::{ShortcutKind, ShortcutState, ShortcutStatus};

/// Сведения о приложении, которые фронтенд получает через `KinigeApi`.
///
/// TS-тип генерируется в `web/src/api/generated/` (см. ADR-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "AppInfo.ts")]
pub struct AppInfo {
    /// Строка для окна: «Kinige 0.1.0».
    pub title: String,
}

/// Собирает сведения о приложении из версии сборки.
pub fn app_info(version: &str) -> AppInfo {
    AppInfo {
        title: format!("Kinige {version}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_contains_name_and_version() {
        assert_eq!(app_info("0.1.0").title, "Kinige 0.1.0");
    }

    #[test]
    fn title_keeps_prerelease_suffix() {
        assert_eq!(app_info("1.2.3-rc.1").title, "Kinige 1.2.3-rc.1");
    }
}
