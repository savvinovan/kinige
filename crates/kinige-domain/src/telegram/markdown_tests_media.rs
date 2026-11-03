//! Tests of `render_body`: placeholder attachments, a poll, a forward, the author line, an empty message.

use super::markdown_tests::{msg, plain, ru};
use super::*;
use crate::lang::Language;

fn render(message: &TgMessage, author: Option<&str>, language: Language) -> String {
    render_body(message, author, language).unwrap()
}

#[test]
fn media_placeholders_ru_and_en() {
    let file = |name: Option<&str>| TgMedia::File {
        name: name.map(str::to_owned),
    };
    let cases: Vec<(TgMedia, &str, &str)> = vec![
        (
            TgMedia::Photo,
            "_[Фото не импортировано]_",
            "_[Photo not imported]_",
        ),
        (
            TgMedia::Sticker {
                emoji: Some("🙂".to_owned()),
            },
            "_[Стикер 🙂 не импортирован]_",
            "_[Sticker 🙂 not imported]_",
        ),
        (
            TgMedia::Sticker { emoji: None },
            "_[Стикер не импортирован]_",
            "_[Sticker not imported]_",
        ),
        (
            TgMedia::Animation,
            "_[Анимация не импортирована]_",
            "_[Animation not imported]_",
        ),
        (
            TgMedia::VideoFile,
            "_[Видео не импортировано]_",
            "_[Video not imported]_",
        ),
        (
            TgMedia::VideoMessage,
            "_[Видеосообщение не импортировано]_",
            "_[Video message not imported]_",
        ),
        (
            TgMedia::VoiceMessage,
            "_[Голосовое сообщение не импортировано]_",
            "_[Voice message not imported]_",
        ),
        (
            TgMedia::AudioFile,
            "_[Аудио не импортировано]_",
            "_[Audio not imported]_",
        ),
        (
            file(Some("report_1.pdf")),
            "_[Файл report\\_1.pdf не импортирован]_",
            "_[File report\\_1.pdf not imported]_",
        ),
        (
            file(None),
            "_[Файл не импортирован]_",
            "_[File not imported]_",
        ),
        (
            TgMedia::Location,
            "_[Геопозиция не импортирована]_",
            "_[Location not imported]_",
        ),
        (
            TgMedia::Venue {
                name: "Кофейня".to_owned(),
            },
            "_[Место Кофейня не импортировано]_",
            "_[Place Кофейня not imported]_",
        ),
        (
            TgMedia::Contact,
            "_[Контакт не импортирован]_",
            "_[Contact not imported]_",
        ),
        (
            TgMedia::Other("game_title".to_owned()),
            "_[Вложение не импортировано]_",
            "_[Attachment not imported]_",
        ),
    ];
    for (media, expected_ru, expected_en) in cases {
        let mut message = msg(Vec::new());
        message.media = Some(media.clone());
        assert_eq!(
            render(&message, None, Language::Ru),
            expected_ru,
            "{media:?}"
        );
        assert_eq!(
            render(&message, None, Language::En),
            expected_en,
            "{media:?}"
        );
    }
}

#[test]
fn media_goes_before_the_caption() {
    let mut message = msg(vec![plain("подпись")]);
    message.media = Some(TgMedia::Photo);
    assert_eq!(
        render(&message, None, Language::Ru),
        "_[Фото не импортировано]_\nподпись"
    );
}

#[test]
fn poll_becomes_question_and_items() {
    let mut message = msg(Vec::new());
    message.media = Some(TgMedia::Poll {
        question: "Что на ужин?".to_owned(),
        answers: vec!["Суп".to_owned(), "- Паста".to_owned()],
    });
    assert_eq!(
        render(&message, None, Language::Ru),
        "**Опрос:** Что на ужин?\n- Суп\n- - Паста"
    );
    assert_eq!(
        render(&message, None, Language::En),
        "**Poll:** Что на ужин?\n- Суп\n- - Паста"
    );
}

#[test]
fn forwarded_line_ru_en_saved_from_and_hidden_account() {
    let mut message = msg(vec![plain("текст")]);
    message.forwarded = true;
    message.forwarded_from = Some("Канал_1".to_owned());
    message.saved_from = Some("Другой".to_owned());
    assert_eq!(
        render(&message, None, Language::Ru),
        "_Переслано от Канал\\_1_\nтекст"
    );
    assert_eq!(
        render(&message, None, Language::En),
        "_Forwarded from Канал\\_1_\nтекст"
    );
    // No forwarded_from: saved_from is taken.
    message.forwarded_from = None;
    assert_eq!(
        render(&message, None, Language::Ru),
        "_Переслано от Другой_\nтекст"
    );
    // Hidden account: the key is present, there are no names.
    message.saved_from = None;
    assert_eq!(render(&message, None, Language::Ru), "_Переслано_\nтекст");
    assert_eq!(render(&message, None, Language::En), "_Forwarded_\nтекст");
    // An empty name counts as absent.
    message.forwarded_from = Some("  ".to_owned());
    assert_eq!(render(&message, None, Language::Ru), "_Переслано_\nтекст");
}

#[test]
fn author_line_goes_first() {
    let mut message = msg(vec![plain("привет")]);
    assert_eq!(
        render(&message, Some("Пётр *Т*"), Language::Ru),
        "**Пётр \\*Т\\***\nпривет"
    );
    message.forwarded = true;
    message.forwarded_from = Some("Канал".to_owned());
    message.media = Some(TgMedia::Photo);
    assert_eq!(
        render(&message, Some("Пётр"), Language::En),
        "**Пётр**\n_Forwarded from Канал_\n_[Photo not imported]_\nпривет"
    );
    // An empty author name gives no line.
    assert_eq!(
        render(&message, Some("  "), Language::En).lines().next(),
        Some("_Forwarded from Канал_")
    );
}

#[test]
fn empty_message_is_none() {
    assert_eq!(ru(Vec::new()), None);
    assert_eq!(ru(vec![plain("  \n ")]), None);
    // An author and a forward without text and attachment do not become an entry: they are not message content.
    let mut message = msg(Vec::new());
    message.forwarded = true;
    assert_eq!(render_body(&message, Some("Пётр"), Language::Ru), None);
}
