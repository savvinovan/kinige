//! Parsing of the Telegram Desktop export `result.json` into domain structures (slice 4, task 4.1). No IO.
//!
//! The format is described from memory of the Telegram Desktop source and is **not verified** on a real export (see the
//! slice 4 specification, "Needed from the user: a real fixture"). So the parsing is tolerant: unknown fields are
//! ignored, unknown entities and media are not lost but get the kind `Other`. Only what an entry cannot be
//! deduplicated without counts as an error: no message `id` or chat `id`.
//!
//! Telegram identifiers come sometimes as a number, sometimes as a string. Here they are always strings: a number is
//! read as `i64`/`u64` without going through `f64`, so 2^63-1 does not lose precision. A fractional number in place of
//! an `id` is a format error.

use std::fmt;

pub use markdown::render_body;

mod markdown;
#[cfg(test)]
mod markdown_tests;
#[cfg(test)]
mod markdown_tests_media;
mod raw;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_cases;
#[cfg(test)]
mod tests_real;

/// A parsed export: of one chat or of all chats at once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgExport {
    /// `personal_information.user_id` of a full export; a single-chat export does not have it.
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

/// Chat type. Type strings: `saved_messages`, `personal_chat` (not verified).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgChatKind {
    Saved,
    Personal,
    /// Any other type (group, channel, bot...) together with the string from the export; the importer skips such chats.
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TgMessage {
    pub id: String,
    /// `type = "service"`: joining a group, pinning and the like. The importer skips such messages.
    pub service: bool,
    /// `date_unixtime`, seconds. Old exports do not have the field.
    pub unix: Option<i64>,
    /// `date` as is, for example `2024-03-05T09:41:07`, local time of the export's time zone.
    pub local: Option<String>,
    pub from: Option<String>,
    pub from_id: Option<String>,
    /// `reply_to_message_id`.
    pub reply_to: Option<String>,
    /// The key `forwarded_from` or `saved_from` is present, even with the value `null` (a hidden account).
    pub forwarded: bool,
    pub forwarded_from: Option<String>,
    pub saved_from: Option<String>,
    /// Text in pieces with a formatting kind. Empty text is an empty vector.
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
    /// Unknown entity type: the text is kept, the kind is the string from the export.
    Other(String),
}

/// A message attachment. Files are not imported: only the kind and a couple of captions for a placeholder remain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgMedia {
    /// The `photo` key is present even when the file is not exported ("File not included...").
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
    /// Geolocation with a place name (`place_name`).
    Venue {
        name: String,
    },
    Contact,
    Poll {
        question: String,
        answers: Vec<String>,
    },
    /// An attachment the parser did not recognize: the key or the `media_type` value from the export.
    Other(String),
}

/// Why the export was not parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TgParseError {
    /// Not JSON (including an empty file); inside is the parser's message.
    NotJson(String),
    /// JSON, but not a Telegram export: not an object, no `chats.list` or `messages`, no `id`.
    NotTelegramExport,
    /// Looks like an export, but a field is of the wrong kind (a message without `id`, a fractional `id`...); inside
    /// is the parser's message with the place in the file.
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

/// Parses `result.json` in one of two forms: an export of one chat (`name`, `type`, `id`, `messages`) or a full one
/// (`personal_information.user_id`, `chats.list[]`). Messages stay in file order.
pub fn parse_export(json: &str) -> Result<TgExport, TgParseError> {
    raw::parse(json)
}
