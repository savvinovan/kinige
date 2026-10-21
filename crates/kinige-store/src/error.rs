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
    MessageNotFound,
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
            StoreError::Write(_) => ErrorCode::StoreWriteFailed,
            StoreError::Read(_) => ErrorCode::StoreReadFailed,
            StoreError::Internal(_) => ErrorCode::Internal,
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
