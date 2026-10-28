//! Разбор анонимизированного настоящего экспорта «Избранного» (`fixtures/real-saved.json`, слайс 4, 28.10.2026): 50
//! сообщений, покрывающих все формы настоящего файла, с вымышленными строками. Файл кладётся в папку фикстур после
//! просмотра пользователем; пока его нет, тест молча проходит.

use super::*;
use crate::lang::Language;

fn fixture() -> Option<String> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/telegram/fixtures/real-saved.json"
    );
    std::fs::read_to_string(path).ok()
}

#[test]
fn real_shaped_export_parses_and_every_message_renders() {
    let Some(json) = fixture() else {
        eprintln!("fixtures/real-saved.json нет: тест пропущен");
        return;
    };
    let export = parse_export(&json).unwrap();
    assert_eq!(export.chats.len(), 1);
    let chat = &export.chats[0];
    assert_eq!(chat.kind, TgChatKind::Saved);
    assert_eq!(chat.name, None, "у «Избранного» нет name");
    assert_eq!(export.owner_id.as_deref(), Some(chat.id.as_str()));
    let mut ids: Vec<&str> = chat.messages.iter().map(|m| m.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), chat.messages.len(), "id не повторяются");
    assert!(chat.messages.iter().all(|m| m.unix.is_some()));
    // Каждое не служебное сообщение даёт запись или пусто (пустых мало), служебных нет среди «рендерящихся».
    let rendered = chat
        .messages
        .iter()
        .filter(|m| !m.service)
        .filter(|m| render_body(m, None, Language::Ru).is_some())
        .count();
    let not_service = chat.messages.iter().filter(|m| !m.service).count();
    assert!(
        rendered * 10 >= not_service * 8,
        "{rendered} из {not_service}"
    );
    // Встретились виды, которые есть в настоящем файле.
    assert!(chat.messages.iter().any(|m| m.forwarded));
    assert!(chat.messages.iter().any(|m| m.reply_to.is_some()));
    assert!(chat.messages.iter().any(|m| m.media.is_some()));
}
