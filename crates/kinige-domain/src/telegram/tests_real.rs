//! Разбор экспорта «Избранного» (`fixtures/real-saved.json`): 50 сообщений, покрывающих все формы настоящего файла Telegram
//! Desktop (вложения, пересылки, ответ, реакции, правка, геометка, контакт, подарок, все виды сущностей). Файл полностью
//! синтетический (KINIG-45): имена, тексты, ссылки, id, даты, размеры и габариты выдуманы, от настоящего экспорта остались
//! только перечисления вроде имён ключей, типов сущностей и MIME.

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
    let json = fixture().expect("fixtures/real-saved.json лежит в репозитории");
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
