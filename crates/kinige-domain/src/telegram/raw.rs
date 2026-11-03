//! Raw `result.json` structures and their conversion into domain types. Only `parse` is visible outside the module.

use super::{TgChat, TgChatKind, TgExport, TgMedia, TgMessage, TgParseError, TgSpan, TgSpanKind};
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::value::RawValue;

/// Determines the export's shape (one chat or a full one) and builds the domain structures.
pub(super) fn parse(json: &str) -> Result<TgExport, TgParseError> {
    let top: RawTop = from_str(json)?;
    if let Some(list) = top.chats.and_then(|c| c.list) {
        let owner_id = top
            .personal_information
            .and_then(|p| p.user_id)
            .map(Flex::into_string);
        return Ok(TgExport {
            owner_id,
            chats: list
                .into_iter()
                .map(RawChat::into_chat)
                .collect::<Result<_, _>>()?,
        });
    }
    match (top.id, top.messages) {
        (Some(id), Some(messages)) => {
            let raw = RawChat {
                id,
                name: top.name,
                kind: top.kind,
                messages: Some(messages),
            };
            let chat = raw.into_chat()?;
            // Export of "Saved Messages": the chat id equals the owner id (verified on a real export).
            let owner_id = (chat.kind == TgChatKind::Saved).then(|| chat.id.clone());
            Ok(TgExport {
                owner_id,
                chats: vec![chat],
            })
        }
        _ => Err(TgParseError::NotTelegramExport),
    }
}

fn from_str<'a, T: Deserialize<'a>>(json: &'a str) -> Result<T, TgParseError> {
    use serde_json::error::Category;
    serde_json::from_str(json).map_err(|e| match e.classify() {
        Category::Data => TgParseError::Malformed(e.to_string()),
        _ => TgParseError::NotJson(e.to_string()),
    })
}

/// An identifier or a number that came as a string or as a number. `untagged` buffers a number as `i64`/`u64`, not `f64`,
/// so a fractional or too large number fits no variant, and an integer does not lose precision.
#[derive(Deserialize)]
#[serde(untagged)]
enum Flex {
    Str(String),
    Int(i64),
    Uint(u64),
}

impl Flex {
    fn into_string(self) -> String {
        match self {
            Flex::Str(s) => s,
            Flex::Int(n) => n.to_string(),
            Flex::Uint(n) => n.to_string(),
        }
    }
}

/// A field where the presence of the key itself matters: absent is `None`, `null` is `Some(None)`.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}

#[derive(Deserialize)]
struct RawTop<'a> {
    personal_information: Option<RawPersonal>,
    #[serde(borrow)]
    chats: Option<RawChats<'a>>,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    id: Option<Flex>,
    #[serde(borrow)]
    messages: Option<&'a RawValue>,
}

#[derive(Deserialize)]
struct RawPersonal {
    user_id: Option<Flex>,
}

#[derive(Deserialize)]
struct RawChats<'a> {
    #[serde(borrow)]
    list: Option<Vec<RawChat<'a>>>,
}

/// Messages are kept as a raw fragment: for groups and channels, which are not imported, they are not parsed at all
/// (faster, and a strange message of someone else's chat does not break the whole file).
#[derive(Deserialize)]
struct RawChat<'a> {
    id: Flex,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(borrow)]
    messages: Option<&'a RawValue>,
}

impl RawChat<'_> {
    fn into_chat(self) -> Result<TgChat, TgParseError> {
        let kind = match self.kind.as_deref() {
            Some("saved_messages") => TgChatKind::Saved,
            Some("personal_chat") => TgChatKind::Personal,
            other => TgChatKind::Other(other.unwrap_or_default().to_owned()),
        };
        let messages = match (&kind, self.messages) {
            (TgChatKind::Saved | TgChatKind::Personal, Some(raw)) => {
                from_str::<Vec<RawMessage>>(raw.get())?
                    .into_iter()
                    .map(RawMessage::into_message)
                    .collect()
            }
            _ => Vec::new(),
        };
        Ok(TgChat {
            id: self.id.into_string(),
            name: self.name,
            kind,
            messages,
        })
    }
}

/// A message as in the export. Media fields are listed explicitly: only the fact that the key exists and a couple of captions are needed.
#[derive(Deserialize)]
struct RawMessage {
    id: Flex,
    #[serde(rename = "type")]
    kind: Option<String>,
    date: Option<String>,
    date_unixtime: Option<Flex>,
    from: Option<String>,
    from_id: Option<Flex>,
    reply_to_message_id: Option<Flex>,
    // `Some(None)`: the key is present, the value is `null` (a hidden account): this is a forward too.
    #[serde(default, deserialize_with = "present")]
    forwarded_from: Option<Option<String>>,
    #[serde(default, deserialize_with = "present")]
    saved_from: Option<Option<String>>,
    text: Option<RawText>,
    text_entities: Option<Vec<RawEntity>>,
    photo: Option<IgnoredAny>,
    media_type: Option<String>,
    file: Option<IgnoredAny>,
    file_name: Option<String>,
    sticker_emoji: Option<String>,
    location_information: Option<IgnoredAny>,
    place_name: Option<String>,
    contact_information: Option<IgnoredAny>,
    poll: Option<RawPoll>,
    // Keys of other attachments (not verified): a message with them must not look empty.
    game_title: Option<IgnoredAny>,
    invoice_information: Option<IgnoredAny>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawText {
    Str(String),
    Parts(Vec<RawPart>),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawPart {
    Str(String),
    Entity(RawEntity),
}

#[derive(Deserialize)]
struct RawEntity {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
    href: Option<String>,
    language: Option<String>,
}

#[derive(Deserialize)]
struct RawPoll {
    question: Option<String>,
    #[serde(default)]
    answers: Vec<RawAnswer>,
}

#[derive(Deserialize)]
struct RawAnswer {
    text: Option<String>,
}

impl RawMessage {
    fn into_message(self) -> TgMessage {
        let media = self.media();
        let text = match (self.text_entities, self.text) {
            (Some(entities), _) => entities.into_iter().map(RawEntity::into_span).collect(),
            (None, Some(RawText::Str(s))) => plain(s).into_iter().collect(),
            (None, Some(RawText::Parts(parts))) => parts
                .into_iter()
                .filter_map(|part| match part {
                    RawPart::Str(s) => plain(s),
                    RawPart::Entity(e) => Some(e.into_span()),
                })
                .collect(),
            (None, None) => Vec::new(),
        };
        TgMessage {
            id: self.id.into_string(),
            service: self.kind.as_deref() == Some("service"),
            unix: self.date_unixtime.and_then(|v| match v {
                Flex::Str(s) => s.trim().parse().ok(),
                Flex::Int(n) => Some(n),
                Flex::Uint(n) => i64::try_from(n).ok(),
            }),
            local: self.date,
            from: self.from,
            from_id: self.from_id.map(Flex::into_string),
            reply_to: self.reply_to_message_id.map(Flex::into_string),
            forwarded: self.forwarded_from.is_some() || self.saved_from.is_some(),
            forwarded_from: self.forwarded_from.flatten(),
            saved_from: self.saved_from.flatten(),
            text,
            media,
        }
    }

    /// The order of checks matters: a photo and a sticker can have both `file` and `media_type`.
    fn media(&self) -> Option<TgMedia> {
        if self.photo.is_some() {
            return Some(TgMedia::Photo);
        }
        if let Some(media_type) = &self.media_type {
            return Some(match media_type.as_str() {
                "sticker" => TgMedia::Sticker {
                    emoji: self.sticker_emoji.clone().filter(|e| !e.is_empty()),
                },
                "animation" => TgMedia::Animation,
                "video_file" => TgMedia::VideoFile,
                "video_message" => TgMedia::VideoMessage,
                "voice_message" => TgMedia::VoiceMessage,
                "audio_file" => TgMedia::AudioFile,
                other => TgMedia::Other(other.to_owned()),
            });
        }
        if self.file.is_some() {
            return Some(TgMedia::File {
                name: self.file_name.clone(),
            });
        }
        if self.location_information.is_some() {
            return Some(match &self.place_name {
                Some(name) => TgMedia::Venue { name: name.clone() },
                None => TgMedia::Location,
            });
        }
        if self.contact_information.is_some() {
            return Some(TgMedia::Contact);
        }
        if let Some(poll) = &self.poll {
            return Some(TgMedia::Poll {
                question: poll.question.clone().unwrap_or_default(),
                answers: poll
                    .answers
                    .iter()
                    .map(|a| a.text.clone().unwrap_or_default())
                    .collect(),
            });
        }
        if self.game_title.is_some() {
            return Some(TgMedia::Other("game_title".to_owned()));
        }
        self.invoice_information
            .is_some()
            .then(|| TgMedia::Other("invoice_information".to_owned()))
    }
}

/// A plain piece from a string; an empty string gives no piece.
fn plain(text: String) -> Option<TgSpan> {
    (!text.is_empty()).then_some(TgSpan {
        kind: TgSpanKind::Plain,
        text,
    })
}

impl RawEntity {
    fn into_span(self) -> TgSpan {
        let kind = match self.kind.as_deref().unwrap_or_default() {
            "plain" => TgSpanKind::Plain,
            "bold" => TgSpanKind::Bold,
            "italic" => TgSpanKind::Italic,
            "underline" => TgSpanKind::Underline,
            "strikethrough" | "strike" => TgSpanKind::Strike,
            "spoiler" => TgSpanKind::Spoiler,
            "code" => TgSpanKind::Code,
            "pre" => TgSpanKind::Pre {
                language: self.language.filter(|l| !l.is_empty()),
            },
            "text_link" => match self.href {
                Some(href) => TgSpanKind::TextLink { href },
                None => TgSpanKind::Other("text_link".to_owned()),
            },
            "link" => TgSpanKind::Link,
            "email" => TgSpanKind::Email,
            "phone" => TgSpanKind::Phone,
            "mention" => TgSpanKind::Mention,
            "mention_name" => TgSpanKind::MentionName,
            "hashtag" => TgSpanKind::Hashtag,
            "cashtag" => TgSpanKind::Cashtag,
            "bot_command" => TgSpanKind::BotCommand,
            "bank_card" | "bankcard" => TgSpanKind::BankCard,
            "blockquote" => TgSpanKind::Blockquote,
            "custom_emoji" => TgSpanKind::CustomEmoji,
            other => TgSpanKind::Other(other.to_owned()),
        };
        TgSpan {
            kind,
            text: self.text.unwrap_or_default(),
        }
    }
}
