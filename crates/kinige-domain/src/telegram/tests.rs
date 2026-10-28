//! Тесты разбора экспорта Telegram на синтетических фикстурах (формат не проверен на настоящем экспорте).

use super::*;

const SINGLE: &str = include_str!("fixtures/single_saved.json");
const FULL: &str = include_str!("fixtures/full_export.json");
const LEGACY: &str = include_str!("fixtures/legacy.json");

pub(super) fn single() -> TgChat {
    let mut export = parse_export(SINGLE).unwrap();
    assert_eq!(export.chats.len(), 1);
    export.chats.remove(0)
}

pub(super) fn message<'a>(chat: &'a TgChat, id: &str) -> &'a TgMessage {
    chat.messages
        .iter()
        .find(|m| m.id == id)
        .unwrap_or_else(|| panic!("нет сообщения {id}"))
}

pub(super) fn span(kind: TgSpanKind, text: &str) -> TgSpan {
    TgSpan {
        kind,
        text: text.to_owned(),
    }
}

/// Экспорт одного чата с одним сообщением, заданным JSON-объектом.
pub(super) fn one(message_json: &str) -> TgMessage {
    let json =
        format!(r#"{{"name":"x","type":"saved_messages","id":1,"messages":[{message_json}]}}"#);
    parse_export(&json)
        .unwrap()
        .chats
        .remove(0)
        .messages
        .remove(0)
}

#[test]
fn single_chat_form() {
    let export = parse_export(SINGLE).unwrap();
    // У «Избранного» id чата равен id владельца.
    assert_eq!(export.owner_id.as_deref(), Some("9223372036854775807"));
    let chat = &export.chats[0];
    assert_eq!(chat.id, "9223372036854775807");
    assert_eq!(chat.name.as_deref(), Some("Saved Messages"));
    assert_eq!(chat.kind, TgChatKind::Saved);
    assert_eq!(chat.messages.len(), 19);
    let first = &chat.messages[0];
    assert_eq!(first.id, "1");
    assert!(!first.service);
    assert_eq!(first.unix, Some(1_709_632_867));
    assert_eq!(first.local.as_deref(), Some("2024-03-05T09:41:07"));
    assert_eq!(first.from.as_deref(), Some("Ivan Test"));
    assert_eq!(first.from_id.as_deref(), Some("user1000001"));
    assert_eq!(first.text, vec![span(TgSpanKind::Plain, "Просто текст")]);
    assert_eq!(first.media, None);
}

#[test]
fn full_export_form() {
    let export = parse_export(FULL).unwrap();
    assert_eq!(export.owner_id.as_deref(), Some("1000001"));
    let kinds: Vec<_> = export
        .chats
        .iter()
        .map(|c| (c.id.as_str(), c.kind.clone()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("1000001", TgChatKind::Saved),
            ("2000002", TgChatKind::Personal),
            (
                "-1001234567890",
                TgChatKind::Other("private_group".to_owned())
            ),
        ]
    );
    let personal = &export.chats[1];
    assert_eq!(personal.name.as_deref(), Some("Пётр Тестов"));
    assert_eq!(personal.messages[0].from_id.as_deref(), Some("user2000002"));
    assert_eq!(personal.messages[1].reply_to.as_deref(), Some("1"));
    // Сообщения групп и каналов не разбираются: импортёр их не берёт.
    assert!(export.chats[2].messages.is_empty());
}

#[test]
fn full_export_entities_and_ordering() {
    let export = parse_export(FULL).unwrap();
    let saved = &export.chats[0];
    assert_eq!(
        saved.messages[0].text,
        vec![
            span(TgSpanKind::Plain, "Ссылка "),
            span(
                TgSpanKind::TextLink {
                    href: "https://example.org/a_(b)".to_owned()
                },
                "сайт"
            ),
            span(TgSpanKind::Plain, " "),
            span(TgSpanKind::Hashtag, "#дела"),
        ]
    );
    // text пуст, а text_entities нет: берутся сущности, порядок файла сохранён.
    assert_eq!(
        saved.messages[1].text,
        vec![
            span(TgSpanKind::Strike, "зачёркнуто"),
            span(TgSpanKind::Blockquote, "цитата"),
            span(TgSpanKind::BankCard, "4111 1111 1111 1111"),
            span(TgSpanKind::CustomEmoji, "🙂"),
        ]
    );
    let ids: Vec<_> = saved.messages.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, ["100", "101"]);
}

#[test]
fn legacy_without_unixtime_and_entities() {
    let export = parse_export(LEGACY).unwrap();
    let chat = &export.chats[0];
    assert_eq!(chat.id, "777");
    assert_eq!(chat.messages.len(), 4);
    let first = &chat.messages[0];
    assert_eq!(first.unix, None);
    assert_eq!(first.local.as_deref(), Some("2019-12-31T21:30:00"));
    // from_id числом превращается в строку.
    assert_eq!(first.from_id.as_deref(), Some("1000001"));
    assert_eq!(
        first.text,
        vec![span(TgSpanKind::Plain, "Старый экспорт, текст строкой")]
    );
    assert_eq!(
        chat.messages[1].text,
        vec![
            span(TgSpanKind::Plain, "Смесь "),
            span(TgSpanKind::Italic, "курсива"),
            span(TgSpanKind::Plain, " и "),
            span(TgSpanKind::Link, "https://example.org"),
            span(TgSpanKind::Pre { language: None }, "x = 1"),
        ]
    );
    assert!(chat.messages[2].text.is_empty());
    assert!(chat.messages[3].service);
}

#[test]
fn text_as_string_and_as_array_agree_with_entities() {
    let chat = single();
    let m = message(&chat, "2");
    let from_entities = m.text.clone();
    assert_eq!(m.text.len(), 4);
    assert_eq!(
        m.text[3].kind,
        TgSpanKind::Pre {
            language: Some("rust".to_owned())
        }
    );
    // То же сообщение без text_entities разбирается из массива text так же.
    let legacy = one(
        r#"{"id":2,"text":["Жирный ",{"type":"bold","text":"акцент"}," и ",{"type":"pre","text":"fn main() {}","language":"rust"}]}"#,
    );
    assert_eq!(legacy.text, from_entities);
    assert_eq!(
        one(r#"{"id":3,"text":"строка"}"#).text,
        vec![span(TgSpanKind::Plain, "строка")]
    );
    assert!(one(r#"{"id":4}"#).text.is_empty());
}

#[test]
fn service_message() {
    let chat = single();
    let m = message(&chat, "13");
    assert!(m.service);
    assert_eq!(m.from, None);
    assert!(m.text.is_empty());
    assert_eq!(m.media, None);
    assert!(!message(&chat, "1").service);
}

#[test]
fn photo_without_file_is_still_photo() {
    let chat = single();
    assert_eq!(message(&chat, "5").media, Some(TgMedia::Photo));
    let with_file = one(r#"{"id":1,"photo":"photos/photo_1.jpg","width":1,"height":1}"#);
    assert_eq!(with_file.media, Some(TgMedia::Photo));
}

#[test]
fn every_media_kind() {
    let chat = single();
    let media = |id: &str| message(&chat, id).media.clone();
    assert_eq!(media("6"), Some(TgMedia::VoiceMessage));
    assert_eq!(
        media("7"),
        Some(TgMedia::Sticker {
            emoji: Some("🙂".to_owned())
        })
    );
    assert_eq!(
        media("8"),
        Some(TgMedia::File {
            name: Some("report.pdf".to_owned())
        })
    );
    assert_eq!(media("9"), Some(TgMedia::Location));
    assert_eq!(
        media("10"),
        Some(TgMedia::Venue {
            name: "Кофейня".to_owned()
        })
    );
    assert_eq!(media("11"), Some(TgMedia::Contact));
    assert_eq!(
        media("12"),
        Some(TgMedia::Poll {
            question: "Что на ужин?".to_owned(),
            answers: vec!["Суп".to_owned(), "Паста".to_owned()]
        })
    );
    assert_eq!(media("14"), Some(TgMedia::VideoFile));
    assert_eq!(media("15"), Some(TgMedia::VideoMessage));
    assert_eq!(media("16"), Some(TgMedia::Animation));
    assert_eq!(media("17"), Some(TgMedia::AudioFile));
    assert_eq!(media("1"), None);
}

#[test]
fn media_edge_cases() {
    assert_eq!(
        one(r#"{"id":1,"file":"files/a.bin"}"#).media,
        Some(TgMedia::File { name: None })
    );
    assert_eq!(
        one(r#"{"id":1,"file":"s.webp","media_type":"sticker"}"#).media,
        Some(TgMedia::Sticker { emoji: None })
    );
    assert_eq!(
        one(r#"{"id":1,"file":"x","media_type":"hologram"}"#).media,
        Some(TgMedia::Other("hologram".to_owned()))
    );
    assert_eq!(
        one(r#"{"id":1,"game_title":"Chess"}"#).media,
        Some(TgMedia::Other("game_title".to_owned()))
    );
    assert_eq!(
        one(r#"{"id":1,"poll":{"answers":[{}]}}"#).media,
        Some(TgMedia::Poll {
            question: String::new(),
            answers: vec![String::new()]
        })
    );
}

#[test]
fn reply_forward_and_saved_from() {
    let chat = single();
    assert_eq!(message(&chat, "3").reply_to.as_deref(), Some("1"));
    assert_eq!(message(&chat, "3").id, "3");
    let forwarded = message(&chat, "4");
    assert!(forwarded.forwarded);
    assert_eq!(forwarded.forwarded_from.as_deref(), Some("Канал Пример"));
    assert_eq!(forwarded.saved_from.as_deref(), Some("Канал Пример"));
    assert_eq!(message(&chat, "1").reply_to, None);
    assert!(!message(&chat, "1").forwarded);
}

#[test]
fn forwarded_key_counts_even_when_null() {
    // Скрытый аккаунт: ключи есть, значения `null` (настоящий экспорт, спецификация 4.1).
    let hidden = one(r#"{"id":1,"forwarded_from":null,"saved_from":null}"#);
    assert!(hidden.forwarded);
    assert_eq!((hidden.forwarded_from, hidden.saved_from), (None, None));
    let only_saved = one(r#"{"id":1,"saved_from":"Канал"}"#);
    assert!(only_saved.forwarded);
    assert_eq!(only_saved.saved_from.as_deref(), Some("Канал"));
    assert!(!one(r#"{"id":1,"from":"Я"}"#).forwarded);
}

#[test]
fn saved_chat_without_name_takes_owner_from_chat_id() {
    // Настоящий экспорт «Избранного»: верхний уровень — только `type`, `id`, `messages`.
    let export = parse_export(r#"{"type":"saved_messages","id":777,"messages":[]}"#).unwrap();
    assert_eq!(export.owner_id.as_deref(), Some("777"));
    assert_eq!(export.chats[0].name, None);
    assert_eq!(export.chats[0].kind, TgChatKind::Saved);
    // У личного чата id владельца в форме одного чата неизвестен.
    let personal = parse_export(r#"{"type":"personal_chat","id":5,"messages":[]}"#).unwrap();
    assert_eq!(personal.owner_id, None);
}
