//! Parsing of a "Saved Messages" export (`fixtures/real-saved.json`): 50 messages covering all forms of a real Telegram
//! Desktop file (attachments, forwards, a reply, reactions, an edit, a geotag, a contact, a gift, all entity kinds). The
//! file is fully synthetic (KINIG-45): names, texts, links, ids, dates, sizes and dimensions are invented, only
//! enumerations such as key names, entity types and MIME types remain from the real export.

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
    let json = fixture().expect("fixtures/real-saved.json is in the repository");
    let export = parse_export(&json).unwrap();
    assert_eq!(export.chats.len(), 1);
    let chat = &export.chats[0];
    assert_eq!(chat.kind, TgChatKind::Saved);
    assert_eq!(chat.name, None, "\"Saved Messages\" has no name");
    assert_eq!(export.owner_id.as_deref(), Some(chat.id.as_str()));
    let mut ids: Vec<&str> = chat.messages.iter().map(|m| m.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), chat.messages.len(), "ids are not repeated");
    assert!(chat.messages.iter().all(|m| m.unix.is_some()));
    // Each non-service message gives an entry or nothing (few are empty); no service messages among the "rendered" ones.
    let rendered = chat
        .messages
        .iter()
        .filter(|m| !m.service)
        .filter(|m| render_body(m, None, Language::Ru).is_some())
        .count();
    let not_service = chat.messages.iter().filter(|m| !m.service).count();
    assert!(
        rendered * 10 >= not_service * 8,
        "{rendered} of {not_service}"
    );
    // The kinds that exist in the real file are present.
    assert!(chat.messages.iter().any(|m| m.forwarded));
    assert!(chat.messages.iter().any(|m| m.reply_to.is_some()));
    assert!(chat.messages.iter().any(|m| m.media.is_some()));
}
