//! Тесты разбора, продолжение: идентификаторы, сущности, время и ошибки.

use super::tests::{message, one, single, span};
use super::*;

#[test]
fn big_ids_keep_precision() {
    let chat = single();
    assert_eq!(chat.id, i64::MAX.to_string());
    assert_eq!(
        message(&chat, "9223372036854775807").text[0].text,
        "Самый большой id"
    );
    let m = one(
        r#"{"id":18446744073709551615,"reply_to_message_id":"9223372036854775807","from_id":-9223372036854775808}"#,
    );
    assert_eq!(m.id, u64::MAX.to_string());
    assert_eq!(m.reply_to.as_deref(), Some("9223372036854775807"));
    assert_eq!(m.from_id.as_deref(), Some("-9223372036854775808"));
    // Целое число, приходящее строкой, и число совпадают.
    assert_eq!(one(r#"{"id":"42"}"#).id, one(r#"{"id":42}"#).id);
}

#[test]
fn fractional_or_huge_id_is_rejected() {
    let json = |id: &str| {
        format!(r#"{{"name":"x","type":"saved_messages","id":1,"messages":[{{"id":{id}}}]}}"#)
    };
    for id in ["1.5", "1e20", "340282366920938463463374607431768211456"] {
        let result = parse_export(&json(id));
        assert!(matches!(result, Err(TgParseError::Malformed(_))), "{id}");
    }
}

#[test]
fn unknown_entity_keeps_text() {
    let chat = single();
    let m = message(&chat, "18");
    assert_eq!(m.text.len(), 2);
    assert_eq!(
        m.text[1],
        span(TgSpanKind::Other("future_entity".to_owned()), "что-то")
    );
    assert_eq!(
        one(r#"{"id":1,"text_entities":[{"text":"без типа"}]}"#).text[0].kind,
        TgSpanKind::Other(String::new())
    );
    assert_eq!(
        one(r#"{"id":1,"text_entities":[{"type":"text_link","text":"без href"}]}"#).text[0].kind,
        TgSpanKind::Other("text_link".to_owned())
    );
}

#[test]
fn every_entity_type() {
    let cases = [
        ("plain", TgSpanKind::Plain),
        ("bold", TgSpanKind::Bold),
        ("italic", TgSpanKind::Italic),
        ("underline", TgSpanKind::Underline),
        ("strikethrough", TgSpanKind::Strike),
        ("spoiler", TgSpanKind::Spoiler),
        ("code", TgSpanKind::Code),
        ("pre", TgSpanKind::Pre { language: None }),
        ("link", TgSpanKind::Link),
        ("email", TgSpanKind::Email),
        ("phone", TgSpanKind::Phone),
        ("mention", TgSpanKind::Mention),
        ("mention_name", TgSpanKind::MentionName),
        ("hashtag", TgSpanKind::Hashtag),
        ("cashtag", TgSpanKind::Cashtag),
        ("bot_command", TgSpanKind::BotCommand),
        ("bank_card", TgSpanKind::BankCard),
        ("blockquote", TgSpanKind::Blockquote),
        ("custom_emoji", TgSpanKind::CustomEmoji),
    ];
    for (name, kind) in cases {
        let m = one(&format!(
            r#"{{"id":1,"text_entities":[{{"type":"{name}","text":"t"}}]}}"#
        ));
        assert_eq!(m.text, vec![span(kind, "t")], "тип {name}");
    }
    let empty_lang = one(r#"{"id":1,"text_entities":[{"type":"pre","text":"t","language":""}]}"#);
    assert_eq!(empty_lang.text[0].kind, TgSpanKind::Pre { language: None });
}

#[test]
fn unixtime_forms() {
    assert_eq!(
        one(r#"{"id":1,"date_unixtime":"1709632867"}"#).unix,
        Some(1_709_632_867)
    );
    assert_eq!(
        one(r#"{"id":1,"date_unixtime":1709632867}"#).unix,
        Some(1_709_632_867)
    );
    assert_eq!(one(r#"{"id":1,"date_unixtime":"не число"}"#).unix, None);
}

#[test]
fn chat_without_name_or_type() {
    let export = parse_export(r#"{"id":5,"messages":[]}"#).unwrap();
    assert_eq!(export.chats[0].name, None);
    assert_eq!(export.chats[0].kind, TgChatKind::Other(String::new()));
    assert!(export.chats[0].messages.is_empty());
}

#[test]
fn empty_input_is_not_json() {
    assert!(matches!(parse_export(""), Err(TgParseError::NotJson(_))));
    assert!(matches!(
        parse_export("   \n"),
        Err(TgParseError::NotJson(_))
    ));
    assert!(matches!(
        parse_export("{\"id\": 1,"),
        Err(TgParseError::NotJson(_))
    ));
    assert!(matches!(
        parse_export("не json"),
        Err(TgParseError::NotJson(_))
    ));
}

#[test]
fn foreign_json_is_not_telegram() {
    // Объекты без нужных ключей.
    for json in [
        "{}",
        r#"{"name":"x","items":[]}"#,
        r#"{"id":1}"#,
        r#"{"messages":[]}"#,
    ] {
        assert_eq!(
            parse_export(json),
            Err(TgParseError::NotTelegramExport),
            "{json}"
        );
    }
    // Не объект, `chats` не объект, сообщение без id: ошибка с подробностью от разборщика.
    for json in [
        "[]",
        "[1,2]",
        "null",
        "42",
        "\"text\"",
        r#"{"chats":[]}"#,
        r#"{"type":"saved_messages","id":1,"messages":[{"text":"x"}]}"#,
    ] {
        let Err(TgParseError::Malformed(detail)) = parse_export(json) else {
            panic!("{json}");
        };
        assert!(!detail.is_empty(), "{json}");
    }
    let Err(error) = parse_export(r#"{"type":"saved_messages","id":1,"messages":[{"text":"x"}]}"#)
    else {
        panic!("ошибка");
    };
    assert!(error.to_string().contains("missing field `id`"), "{error}");
}

#[test]
fn broken_message_of_a_group_does_not_break_the_file() {
    // Сообщения групп не разбираются: странное сообщение чужого чата не мешает импорту «Избранного».
    let json = r#"{"chats":{"list":[
        {"type":"private_group","id":1,"messages":[{"no_id":true}]},
        {"type":"saved_messages","id":2,"messages":[{"id":1,"text":"x"}]}]}}"#;
    let export = parse_export(json).unwrap();
    assert_eq!(export.chats[1].messages.len(), 1);
    // А у импортируемого чата такое сообщение — ошибка файла.
    let bad = r#"{"chats":{"list":[{"type":"personal_chat","id":1,"messages":[{"no_id":true}]}]}}"#;
    assert!(matches!(parse_export(bad), Err(TgParseError::Malformed(_))));
}

#[test]
fn full_form_without_personal_information() {
    let export = parse_export(r#"{"chats":{"list":[]}}"#).unwrap();
    assert_eq!(export.owner_id, None);
    assert!(export.chats.is_empty());
}

#[test]
fn error_messages_are_in_russian() {
    assert_eq!(
        TgParseError::NotTelegramExport.to_string(),
        "файл не похож на экспорт Telegram"
    );
    assert!(
        TgParseError::NotJson("x".to_owned())
            .to_string()
            .starts_with("файл не является JSON")
    );
}
