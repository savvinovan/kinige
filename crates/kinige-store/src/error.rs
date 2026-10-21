use kinige_domain::ErrorCode;
use kinige_domain::groups::BodyError;
use std::fmt;

/// Ошибка хранилища. Фронтенд получает только код (`ErrorCode`), подробности остаются в журнале процесса.
#[derive(Debug)]
pub enum StoreError {
    /// Пустой текст записи.
    Empty,
    /// Текст длиннее предела.
    TooLarge,
    ChatNotFound,
    /// Сочетание клавиш записано неверно, без модификатора или зарезервировано системой.
    ShortcutInvalid,
    /// Такое сочетание уже назначено другому действию Kinige.
    ShortcutDuplicate,
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
            StoreError::ShortcutInvalid => ErrorCode::ShortcutInvalid,
            StoreError::ShortcutDuplicate => ErrorCode::ShortcutDuplicate,
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
            StoreError::ShortcutInvalid => "shortcut_invalid",
            StoreError::ShortcutDuplicate => "shortcut_duplicate",
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
            StoreError::ShortcutInvalid => f.write_str("неверное сочетание клавиш"),
            StoreError::ShortcutDuplicate => f.write_str("сочетание уже назначено"),
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
        ] {
            let category = error.category();
            assert!(["write", "read", "internal"].contains(&category));
            assert!(!category.contains("личное") && !category.contains("example"));
        }
    }
}
