//! Тесты чатов слайса 3 (продолжение `ops_tests.rs`): новый чат со всеми группами, перезапись `meta` и `state`, правила
//! названия и эмодзи, ключи порядка.

use crate::chat::{self, ChatInputError, NewChat, StateChange};
use crate::groups::{GroupError, parse_groups_raw};
use crate::rewrite::Groups;
use serde_json::{Value, json};
use std::collections::BTreeMap;

const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const NOW: i64 = 1_760_000_000_000;

fn hlc(l: u64) -> String {
    format!("{l:013}-00000-{DEVICE}")
}

fn group_text(groups_raw: &str, name: &str) -> String {
    parse_groups_raw(groups_raw).unwrap()[name]
        .get()
        .to_string()
}

fn hlcs(hlc_json: &str) -> BTreeMap<String, Value> {
    serde_json::from_str(hlc_json).unwrap()
}

#[test]
fn new_chat_has_all_six_groups_under_one_hlc() {
    let chat = chat::new_chat(
        &NewChat {
            id: "c1",
            title: "Книги",
            icon: "📚",
            sort_key: "a000001",
            created_at: NOW,
        },
        &hlc(NOW as u64),
    )
    .unwrap();
    let groups = parse_groups_raw(&chat.groups_raw).unwrap();
    assert_eq!(
        groups.keys().map(String::as_str).collect::<Vec<_>>(),
        ["core", "deleted", "meta", "order", "privacy", "state"]
    );
    assert_eq!(
        groups["core"].get(),
        format!(r#"{{"created_at":{NOW},"is_inbox":false}}"#)
    );
    assert_eq!(
        groups["meta"].get(),
        r#"{"color":null,"icon":"📚","title":"Книги"}"#
    );
    assert_eq!(groups["order"].get(), r#"{"sort_key":"a000001"}"#);
    assert_eq!(
        groups["state"].get(),
        r#"{"archived_at":null,"pinned_at":null}"#
    );
    assert_eq!(groups["privacy"].get(), r#"{"ai_excluded":false}"#);
    assert_eq!(groups["deleted"].get(), r#"{"deleted_at":null}"#);
    for entry in hlcs(&chat.hlc_json).values() {
        assert_eq!(*entry, json!({"h": hlc(NOW as u64), "sv": 1}));
    }
    assert_eq!(chat::chat_columns(&chat.groups_raw).unwrap().title, "Книги");
}

#[test]
fn chat_meta_and_state_rewrites_keep_the_rest_verbatim() {
    let chat = chat::new_chat(
        &NewChat {
            id: "c1",
            title: "Книги",
            icon: "📚",
            sort_key: "a000001",
            created_at: NOW,
        },
        &hlc(NOW as u64),
    )
    .unwrap();
    let cur = Groups {
        groups_raw: &chat.groups_raw,
        hlc_json: &chat.hlc_json,
    };
    let renamed = chat::set_meta(&cur, Some("Фильмы"), None, &hlc(NOW as u64 + 1)).unwrap();
    assert_eq!(
        group_text(&renamed.groups_raw, "meta"),
        r#"{"color":null,"icon":"📚","title":"Фильмы"}"#
    );
    let iconed = chat::set_meta(&cur, None, Some("🎬"), &hlc(NOW as u64 + 1)).unwrap();
    assert_eq!(
        group_text(&iconed.groups_raw, "meta"),
        r#"{"color":null,"icon":"🎬","title":"Книги"}"#
    );
    for other in ["core", "order", "state", "privacy", "deleted"] {
        assert_eq!(
            group_text(&renamed.groups_raw, other),
            group_text(&chat.groups_raw, other),
            "{other}"
        );
    }

    let pinned = chat::set_state(
        &cur,
        StateChange::Pinned(Some(NOW + 3)),
        &hlc(NOW as u64 + 2),
    )
    .unwrap();
    assert_eq!(
        group_text(&pinned.groups_raw, "state"),
        format!(r#"{{"archived_at":null,"pinned_at":{}}}"#, NOW + 3)
    );
    let archived = chat::set_state(
        &Groups {
            groups_raw: &pinned.groups_raw,
            hlc_json: &pinned.hlc_json,
        },
        StateChange::Archived(Some(NOW + 4)),
        &hlc(NOW as u64 + 3),
    )
    .unwrap();
    let columns = chat::chat_columns(&archived.groups_raw).unwrap();
    assert_eq!(
        (columns.pinned_at, columns.archived_at),
        (Some(NOW + 3), Some(NOW + 4))
    );
    assert_eq!(
        chat::set_state(&cur, StateChange::Archived(None), &hlc(NOW as u64)),
        Err(GroupError::HlcNotNewer)
    );
}

#[test]
fn chat_input_rules() {
    assert_eq!(chat::normalize_title("  Книги  "), Ok("Книги".to_string()));
    assert_eq!(chat::normalize_title(""), Err(ChatInputError::TitleEmpty));
    assert_eq!(
        chat::normalize_title(" \t "),
        Err(ChatInputError::TitleEmpty)
    );
    assert_eq!(
        chat::normalize_title("a\nb"),
        Err(ChatInputError::TitleEmpty)
    );
    assert!(chat::normalize_title(&"я".repeat(128)).is_ok());
    assert_eq!(
        chat::normalize_title(&"я".repeat(129)),
        Err(ChatInputError::TitleTooLong)
    );
    assert_eq!(chat::normalize_icon(None), Ok("💬".to_string()));
    assert_eq!(chat::normalize_icon(Some("  ")), Ok("💬".to_string()));
    assert_eq!(chat::normalize_icon(Some("📚")), Ok("📚".to_string()));
    assert_eq!(chat::normalize_icon(Some("👨‍👩‍👧‍👦")), Ok("👨‍👩‍👧‍👦".to_string()));
    assert_eq!(
        chat::normalize_icon(Some("a b")),
        Err(ChatInputError::IconInvalid)
    );
    assert_eq!(
        chat::normalize_icon(Some(&"😀".repeat(11))),
        Err(ChatInputError::IconInvalid)
    );
    // Самая длинная последовательность пикера (поцелуй с разными тонами кожи, 35 байт) помещается.
    let kiss = "👩\u{1F3FB}\u{200D}\u{2764}\u{FE0F}\u{200D}💋\u{200D}👨\u{1F3FF}";
    assert_eq!(chat::normalize_icon(Some(kiss)), Ok(kiss.to_string()));
}

#[test]
fn order_keys_grow_and_sort_after_everything_before() {
    let first = chat::key_after(None);
    assert_eq!(first, "a000001");
    assert!(first.as_str() > "a0", "после Inbox");
    assert_eq!(chat::key_after(Some("a0")), "a000001");
    let mut key = first;
    for _ in 0..2000 {
        let next = chat::key_after(Some(&key));
        assert!(next > key, "{next} > {key}");
        key = next;
    }
    assert_eq!(
        chat::key_after(Some("a00000z")),
        "a000010",
        "перенос разряда"
    );
    assert_eq!(
        chat::key_after(Some("azzzzzz")),
        "azzzzzz1",
        "переполнение — приставка"
    );
    assert!(chat::key_after(Some("azzzzzz")).as_str() > "azzzzzz");
    // Чужой формат: строго больше максимума.
    let foreign = chat::key_after(Some("m5"));
    assert!(foreign.as_str() > "m5", "{foreign}");
    assert_eq!(chat::key_after(Some("A")), "a000001");
}
