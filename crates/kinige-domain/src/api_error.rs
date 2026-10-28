//! Ошибка команды, как её получает фронтенд: код, текст — во фронтенде (`error.<код>` в файлах переводов).

use serde::Serialize;
use ts_rs::TS;

/// Код ошибки команды; текст — во фронтенде (`error.<код>` в файлах переводов).
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
    /// Сочетание не принято: нет модификатора, зарезервировано системой или записано неверно.
    ShortcutInvalid,
    /// Сочетание занято другой программой или системой.
    ShortcutTaken,
    /// Такое же сочетание уже назначено другому действию Kinige.
    ShortcutDuplicate,
    /// Запись не найдена (удалена навсегда или id неверен).
    MessageNotFound,
    /// Запись в корзине: перенос в другой чат только для живых записей, сначала «Вернуть».
    MessageInTrash,
    /// Запись с группой в неподдерживаемой версии: только чтение (ADR-0001 §4).
    ReadOnly,
    /// Inbox нельзя переименовать, архивировать и удалить.
    InboxProtected,
    /// Действие только для записи в корзине (удалить навсегда).
    NotInTrash,
    /// Дата записи не существует.
    InvalidDate,
    /// Имя чата пустое.
    ChatTitleEmpty,
    /// Имя чата длиннее 128 знаков.
    ChatTitleTooLong,
    /// Эмодзи чата не принято.
    ChatIconInvalid,
    /// Пункта задачи с таким номером в тексте нет или он уже в нужном состоянии: окно показывало устаревший текст.
    TaskItemStale,
    /// База повреждена.
    DatabaseCorrupt,
    /// База создана более новой версией приложения (§10.4): откат не поддерживается, нужно обновить Kinige.
    DatabaseTooNew,
    /// Копия не найдена или не открывается.
    BackupNotFound,
    /// Папку Markdown-копии не удалось переименовать (занята другой программой, нет прав): восстановление не начато,
    /// ничего не тронуто; закрыть папку и повторить.
    MirrorFolderBusy,
    /// Выбранный файл не экспорт Telegram (не JSON или чужой JSON): ничего не записано (слайс 4).
    ImportNotTelegram,
    /// Выбранный файл не прочитался.
    ImportFileUnreadable,
    /// Файл больше предела 1 ГиБ.
    ImportFileTooLarge,
    /// Импорт уже идёт: одновременно один.
    ImportInProgress,
    /// Разобранный экспорт забыт (окно закрыто или выбран другой файл): выбрать файл заново.
    ImportSessionExpired,
    /// Копия базы перед импортом не снялась: ни одной записи не добавлено.
    ImportBackupFailed,
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
}
