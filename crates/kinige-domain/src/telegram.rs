//! Разбор `result.json` экспорта Telegram Desktop в доменные структуры (слайс 4, задача 4.1). Без IO.
//!
//! Формат описан по памяти об исходнике Telegram Desktop и **не проверен** на настоящем экспорте (см. спецификацию
//! слайса 4, «Нужно от пользователя: настоящая фикстура»). Поэтому разбор терпимый: незнакомые поля игнорируются,
//! незнакомые сущности и медиа не теряются, а получают вид `Other`. Ошибкой считается только то, без чего запись
//! нельзя дедуплицировать: нет `id` сообщения или чата.
//!
//! Идентификаторы Telegram приходят то числом, то строкой. Здесь они всегда строки: число читается как `i64`/`u64`
//! без перехода через `f64`, поэтому 2⁶³−1 не теряет точность. Дробное число на месте `id` — ошибка формата.

use std::fmt;

mod raw;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_cases;

/// Разобранный экспорт: одного чата или всех чатов сразу.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgExport {
    /// `personal_information.user_id` полного экспорта; у экспорта одного чата его нет.
    pub owner_id: Option<String>,
    pub chats: Vec<TgChat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgChat {
    pub id: String,
    pub name: Option<String>,
    pub kind: TgChatKind,
    pub messages: Vec<TgMessage>,
}

/// Тип чата. Строки типов: `saved_messages`, `personal_chat` (не проверено).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgChatKind {
    Saved,
    Personal,
    /// Любой другой тип (группа, канал, бот…) вместе со строкой из экспорта; импортёр такие чаты пропускает.
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgMessage {
    pub id: String,
    /// `type = "service"`: вступление в группу, закрепление и т. п. Импортёр такие сообщения пропускает.
    pub service: bool,
    /// `date_unixtime`, секунды. У старых экспортов поля нет.
    pub unix: Option<i64>,
    /// `date` как есть, например `2024-03-05T09:41:07`, местное время пояса экспорта.
    pub local: Option<String>,
    pub from: Option<String>,
    pub from_id: Option<String>,
    /// `reply_to_message_id`.
    pub reply_to: Option<String>,
    /// Есть ключ `forwarded_from` или `saved_from`, даже со значением `null` (скрытый аккаунт).
    pub forwarded: bool,
    pub forwarded_from: Option<String>,
    pub saved_from: Option<String>,
    /// Текст кусками с видом форматирования. Пустой текст — пустой вектор.
    pub text: Vec<TgSpan>,
    pub media: Option<TgMedia>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgSpan {
    pub kind: TgSpanKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgSpanKind {
    Plain,
    Bold,
    Italic,
    Underline,
    Strike,
    Spoiler,
    Code,
    Pre {
        language: Option<String>,
    },
    TextLink {
        href: String,
    },
    Link,
    Email,
    Phone,
    Mention,
    MentionName,
    Hashtag,
    Cashtag,
    BotCommand,
    BankCard,
    Blockquote,
    CustomEmoji,
    /// Незнакомый тип сущности: текст сохраняется, вид — строка из экспорта.
    Other(String),
}

/// Вложение сообщения. Файлы не импортируются: от них остаётся только вид и пара подписей для заглушки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgMedia {
    /// Ключ `photo` есть и тогда, когда файл не выгружен («File not included…»).
    Photo,
    Sticker {
        emoji: Option<String>,
    },
    Animation,
    VideoFile,
    VideoMessage,
    VoiceMessage,
    AudioFile,
    File {
        name: Option<String>,
    },
    Location,
    /// Геопозиция с названием места (`place_name`).
    Venue {
        name: String,
    },
    Contact,
    Poll {
        question: String,
        answers: Vec<String>,
    },
    /// Вложение, которое разбор не узнал: ключ или значение `media_type` из экспорта.
    Other(String),
}

/// Почему экспорт не разобран.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgParseError {
    /// Не JSON (в том числе пустой файл); внутри — сообщение разборщика.
    NotJson(String),
    /// JSON, но не экспорт Telegram: не объект, нет `chats.list` или `messages`, нет `id`.
    NotTelegramExport,
    /// Похоже на экспорт, но поле не того вида (сообщение без `id`, дробный `id`…); внутри — сообщение разборщика
    /// с местом в файле.
    Malformed(String),
}

impl fmt::Display for TgParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TgParseError::NotJson(detail) => write!(f, "файл не является JSON: {detail}"),
            TgParseError::NotTelegramExport => f.write_str("файл не похож на экспорт Telegram"),
            TgParseError::Malformed(detail) => {
                write!(f, "файл не похож на экспорт Telegram: {detail}")
            }
        }
    }
}

impl std::error::Error for TgParseError {}

/// Разбирает `result.json` в одной из двух форм: экспорт одного чата (`name`, `type`, `id`, `messages`) или полный
/// (`personal_information.user_id`, `chats.list[]`). Сообщения остаются в порядке файла.
pub fn parse_export(json: &str) -> Result<TgExport, TgParseError> {
    raw::parse(json)
}
