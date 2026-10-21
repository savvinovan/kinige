//! Типы, которыми обмениваются Rust и фронтенд (`KinigeApi`). TS-типы генерируются в `web/src/api/generated/`
//! (`cargo test`, ADR-0002). Тексты ошибок — во фронтенде: Rust возвращает только коды.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Чат в сайдбаре. У Inbox хранимое имя пустое: интерфейс показывает переведённое.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "ChatSummary.ts")]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub is_inbox: bool,
    /// Записи чата, кроме корзины.
    #[ts(type = "number")]
    pub count: i64,
}

/// Запись в ленте.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MessageView.ts")]
pub struct MessageView {
    pub id: String,
    pub chat_id: String,
    pub body_md: String,
    /// `YYYY-MM-DD`, граница дня 04:00 (ТЗ §4.1).
    pub journal_date: String,
    /// UTC, мс.
    #[ts(type = "number")]
    pub created_at: i64,
    /// IANA-пояс устройства в момент создания.
    pub created_tz: String,
}

/// Позиция в ленте для постраничной загрузки (keyset): `(journal_date, created_at, id)` последней загруженной записи.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "FeedCursor.ts")]
pub struct FeedCursor {
    pub journal_date: String,
    #[ts(type = "number")]
    pub created_at: i64,
    pub id: String,
}

/// Страница ленты: записи по возрастанию `(journal_date, created_at, id)`, самая новая — последней.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MessagePage.ts")]
pub struct MessagePage {
    pub messages: Vec<MessageView>,
    /// Позиция для следующей (более старой) страницы; `null` — больше записей нет.
    pub older: Option<FeedCursor>,
    /// Сегодняшний `journal_date` (с границей дня 04:00): от него считаются метки «Сегодня» и «Вчера».
    pub today: String,
}

/// Результат «Отменить»: текст возвращается в поле ввода.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "UndoResult.ts")]
pub struct UndoResult {
    pub body_md: String,
}

/// Размер текста (ADR-0004): «Обычный» или «Крупный» ×1,25.
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

    /// Значение из `app_setting`; незнакомое — обычный размер.
    pub fn from_setting(value: &str) -> TextSize {
        if value == "large" {
            TextSize::Large
        } else {
            TextSize::Normal
        }
    }
}

/// Почему Markdown-копия сейчас не пишется.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "MirrorError.ts")]
pub enum MirrorError {
    /// Папку нельзя создать или открыть (нет доступа, диска, это файл).
    FolderUnavailable,
    /// Файл дня записать не удалось.
    WriteFailed,
}

/// Состояние Markdown-копии для окна «Настройки».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "MirrorStatus.ts")]
pub struct MirrorStatus {
    /// Путь папки; `null`, если папка не выбрана и по умолчанию её нет — копия выключена.
    pub folder: Option<String>,
    /// Папка по умолчанию (`~/Documents/Kinige`), пользователь её не менял.
    pub is_default: bool,
    pub error: Option<MirrorError>,
    /// Дней, которые ждут записи в копию.
    #[ts(type = "number")]
    pub pending_days: i64,
}

/// Настройки (не синхронизируются, `app_setting`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "Settings.ts")]
pub struct Settings {
    pub text_size: TextSize,
    pub mirror: MirrorStatus,
}

/// Код ошибки команды; текст — во фронтенде (`error.<код>` в файлах переводов).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "ErrorCode.ts")]
pub enum ErrorCode {
    EmptyMessage,
    MessageTooLarge,
    ChatNotFound,
    MessageNotFound,
    StoreWriteFailed,
    StoreReadFailed,
    LinkNotAllowed,
    FolderPickFailed,
    Internal,
}

/// Ошибка команды, как её получает фронтенд: `{ "code": "store_write_failed" }`.
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

    #[test]
    fn views_serialize_in_camel_case() {
        let view = MessageView {
            id: "i".into(),
            chat_id: "c".into(),
            body_md: "b".into(),
            journal_date: "2026-10-21".into(),
            created_at: 1,
            created_tz: "UTC".into(),
        };
        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            r#"{"id":"i","chatId":"c","bodyMd":"b","journalDate":"2026-10-21","createdAt":1,"createdTz":"UTC"}"#
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
