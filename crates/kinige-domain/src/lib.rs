//! Правила предметной области Kinige. Без IO; собирается и под `wasm32` (§3.2 ТЗ).

pub mod api;
pub mod append;
pub mod groups;
pub mod hlc;
pub mod jcs;
pub mod journal;
pub mod lang;
pub mod link;
pub mod mirror;
pub mod shortcuts;

use serde::Serialize;
use ts_rs::TS;

pub use api::{
    ApiError, CaptureAction, CaptureOutcome, CaptureState, ChatSummary, ErrorCode, FeedCursor,
    MessagePage, MessageView, MirrorError, MirrorStatus, Settings, TextSize,
};
pub use lang::Language;
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
