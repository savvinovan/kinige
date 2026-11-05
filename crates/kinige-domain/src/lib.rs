//! Kinige domain rules. No IO; also builds for `wasm32` (PRD §3.2).

pub mod agent;
pub mod agent_api;
pub mod agent_reply;
pub mod api;
pub mod api_error;
pub mod append;
pub mod attachment;
pub mod backup;
pub mod chat;
pub mod chat_color;
pub mod draft;
pub mod emoji;
pub mod folders;
pub mod forest;
pub mod forest_year;
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
pub mod on_this_day;
pub mod reply;
pub mod rewrite;
pub mod search;
pub mod settings_file;
pub mod shortcuts;
pub mod sync;
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
    AttachOutcome, CaptureAction, CaptureOutcome, CaptureState, ChatDraft, ChatList, ChatSummary,
    FeedCursor, MessageFlag, MessagePage, MessageView, MirrorError, MirrorStatus, SearchFilters,
    SearchHit, SearchPage, Settings, TextSize, TrashItem,
};
pub use api_error::{ApiError, ErrorCode};
pub use attachment::{AttachmentKind, AttachmentState, AttachmentSummary, AttachmentView};
pub use backup::{BackupError, BackupInfo, BackupKind, BackupStatus, RestoreOutcome, StartupState};
pub use chat_color::ChatColor;
pub use forest::{FirstWeekday, Season, WeekdaySource};
pub use forest_year::{ForestDay, ForestMode, ForestTarget, ForestView, ForestYear};
pub use import::{
    ImportChatKind, ImportChatPreview, ImportOpened, ImportPreview, ImportProgress, ImportReport,
};
pub use lang::Language;
pub use on_this_day::{OnThisDay, OnThisDayEntry, OnThisDayYear};
pub use reply::{ReplyState, ReplyTo};
pub use settings_file::{
    FileChanges, FileProblem, FileProblemKind, KeyIssue, KeyIssueKind, SettingsFileState,
    UnsafeReason,
};
pub use shortcuts::{ShortcutKind, ShortcutState, ShortcutStatus};

/// App information that the frontend receives through `KinigeApi`.
///
/// The TS type is generated in `web/src/api/generated/` (see ADR-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export, export_to = "AppInfo.ts")]
pub struct AppInfo {
    /// The string for the window: "Kinige 0.1.0".
    pub title: String,
}

/// Builds the app information from the build version.
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
