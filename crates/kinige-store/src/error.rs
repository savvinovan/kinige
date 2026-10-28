use kinige_domain::ErrorCode;
use kinige_domain::chat::ChatInputError;
use kinige_domain::groups::{BodyError, GroupError};
use std::fmt;

/// Ошибка хранилища. Фронтенд получает только код (`ErrorCode`), подробности остаются в журнале процесса.
#[derive(Debug)]
pub enum StoreError {
    /// Пустой текст записи.
    Empty,
    /// Текст длиннее предела.
    TooLarge,
    ChatNotFound,
    /// Записи нет (удалена навсегда или id неверен).
    MessageNotFound,
    /// Запись в корзине: перенос в другой чат только для живых записей (сначала «Вернуть»).
    MessageInTrash,
    /// Запись с группой в неподдерживаемой версии: только чтение (ADR-0001 §4).
    ReadOnly,
    /// Inbox нельзя переименовать, архивировать и удалить.
    InboxProtected,
    /// «Удалить навсегда» только для записи из корзины.
    NotInTrash,
    /// Дата записи не существует.
    InvalidDate,
    /// Имя или эмодзи чата не приняты.
    ChatInput(ChatInputError),
    /// Пункта задачи с таким номером нет или он уже в нужном состоянии.
    TaskItemStale,
    /// Сочетание клавиш записано неверно, без модификатора или зарезервировано системой.
    ShortcutInvalid,
    /// Такое сочетание уже назначено другому действию Kinige.
    ShortcutDuplicate,
    /// База повреждена: журнал не открыт, нужна копия.
    Corrupt(String),
    /// Копия не найдена, не открывается или повреждена.
    BackupNotFound,
    /// Прежнюю папку Markdown-копии не удалось переименовать перед восстановлением: ничего не тронуто.
    MirrorFolderBusy,
    /// Копия базы перед импортом не снялась: импорт не начат, ничего не записано.
    ImportBackupFailed,
    /// База создана более новой версией приложения (схема `found`): откат не поддерживается (ТЗ §10.4).
    TooNew(i64),
    /// Запись в базу не удалась: ничего не записано (транзакция откатилась).
    Write(String),
    /// Чтение не удалось.
    Read(String),
    /// Всё остальное: сломанный поток, повреждённые группы, база новее приложения.
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
            StoreError::BackupNotFound => ErrorCode::BackupNotFound,
            StoreError::MirrorFolderBusy => ErrorCode::MirrorFolderBusy,
            StoreError::ImportBackupFailed => ErrorCode::ImportBackupFailed,
            StoreError::TooNew(_) => ErrorCode::DatabaseTooNew,
            StoreError::Write(_) => ErrorCode::StoreWriteFailed,
            StoreError::Read(_) => ErrorCode::StoreReadFailed,
            StoreError::Internal(_) => ErrorCode::Internal,
        }
    }
}

impl StoreError {
    /// Категория для журнала процесса: без текста внутренней ошибки (SQLite, serde, пути, адреса) — в нём могут оказаться
    /// куски данных пользователя.
    pub fn category(&self) -> &'static str {
        match self {
            StoreError::Empty => "empty",
            StoreError::TooLarge => "too_large",
            StoreError::ChatNotFound => "chat_not_found",
            StoreError::MessageNotFound => "message_not_found",
            StoreError::MessageInTrash => "message_in_trash",
            StoreError::ReadOnly => "read_only",
            StoreError::InboxProtected => "inbox_protected",
            StoreError::NotInTrash => "not_in_trash",
            StoreError::InvalidDate => "invalid_date",
            StoreError::ChatInput(_) => "chat_input",
            StoreError::TaskItemStale => "task_item_stale",
            StoreError::ShortcutInvalid => "shortcut_invalid",
            StoreError::ShortcutDuplicate => "shortcut_duplicate",
            StoreError::Corrupt(_) => "corrupt",
            StoreError::BackupNotFound => "backup_not_found",
            StoreError::MirrorFolderBusy => "mirror_folder_busy",
            StoreError::ImportBackupFailed => "import_backup_failed",
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
            StoreError::Empty => f.write_str("пустая запись"),
            StoreError::TooLarge => f.write_str("запись слишком длинная"),
            StoreError::ChatNotFound => f.write_str("чат не найден"),
            StoreError::MessageNotFound => f.write_str("запись не найдена"),
            StoreError::MessageInTrash => f.write_str("запись в корзине"),
            StoreError::ReadOnly => f.write_str("запись только для чтения"),
            StoreError::InboxProtected => f.write_str("Inbox нельзя изменить так"),
            StoreError::NotInTrash => f.write_str("запись не в корзине"),
            StoreError::InvalidDate => f.write_str("дата записи не существует"),
            StoreError::ChatInput(why) => write!(f, "{why}"),
            StoreError::TaskItemStale => f.write_str("пункт задачи изменился"),
            StoreError::ShortcutInvalid => f.write_str("неверное сочетание клавиш"),
            StoreError::ShortcutDuplicate => f.write_str("сочетание уже назначено"),
            StoreError::Corrupt(why) => write!(f, "база повреждена: {why}"),
            StoreError::BackupNotFound => f.write_str("копия не найдена или повреждена"),
            StoreError::MirrorFolderBusy => {
                f.write_str("папку Markdown-копии не удалось переименовать")
            }
            StoreError::ImportBackupFailed => f.write_str("копия базы перед импортом не снялась"),
            StoreError::TooNew(found) => {
                write!(
                    f,
                    "база создана более новой версией приложения (схема {found})"
                )
            }
            StoreError::Write(why) => write!(f, "запись в базу не удалась: {why}"),
            StoreError::Read(why) => write!(f, "чтение базы не удалось: {why}"),
            StoreError::Internal(why) => write!(f, "внутренняя ошибка: {why}"),
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
    /// Группа новее поддерживаемой — запись только для чтения; слишком длинная — `TooLarge`; остальное — повреждение.
    fn from(error: GroupError) -> Self {
        match error {
            GroupError::TooLarge => StoreError::TooLarge,
            GroupError::UnsupportedVersion => StoreError::ReadOnly,
            other => StoreError::Internal(other.to_string()),
        }
    }
}

/// Ошибка SQLite при записи; «не база данных» и «база повреждена» — это `Corrupt`, а не сбой записи.
pub(crate) fn write_err(error: rusqlite::Error) -> StoreError {
    if is_corruption(&error) {
        return StoreError::Corrupt(error.to_string());
    }
    StoreError::Write(error.to_string())
}

/// Ошибка SQLite при чтении.
pub(crate) fn read_err(error: rusqlite::Error) -> StoreError {
    if is_corruption(&error) {
        return StoreError::Corrupt(error.to_string());
    }
    StoreError::Read(error.to_string())
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
