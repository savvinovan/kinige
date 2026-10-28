use super::*;
use crate::groups::{INBOX_ID, ORIGIN_USER, parse_groups_raw};
use crate::jcs;
use crate::message_ops::message_columns;
use serde_json::Value;
use std::collections::BTreeMap;

const HLC: &str = "1760000000000-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const MSG_ID: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const REPLY_ID: &str = "7b2c9d10-1111-4222-8333-444455556666";

fn input<'a>() -> NewImportedMessage<'a> {
    NewImportedMessage {
        id: MSG_ID,
        chat_id: INBOX_ID,
        body_md: "Привет **мир**",
        created_at: 1_552_000_000_007,
        created_tz: "Europe/Moscow",
        journal_date: "2019-03-08",
        ext_chat: "9223372036854775807",
        ext_msg: "-42",
        reply_to_id: Some(REPLY_ID),
    }
}

#[test]
fn core_bytes_are_exact_with_big_id_as_string() {
    let row = new_imported_message(&input(), HLC).unwrap();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(
        groups["core"].get(),
        concat!(
            r#"{"created_at":1552000000007,"created_tz":"Europe/Moscow","#,
            r#""ext":{"chat":"9223372036854775807","msg":"-42","src":"telegram"},"#,
            r#""origin":"import_telegram","reply_to_id":"7b2c9d10-1111-4222-8333-444455556666"}"#
        )
    );
    assert_eq!(
        row.ext_json.as_deref(),
        Some(r#"{"chat":"9223372036854775807","msg":"-42","src":"telegram"}"#)
    );
    assert_eq!(row.origin, ORIGIN_IMPORT_TELEGRAM);
    assert_eq!(row.reply_to_id.as_deref(), Some(REPLY_ID));
}

#[test]
fn reply_to_id_is_null_without_target() {
    let mut without = input();
    without.reply_to_id = None;
    let row = new_imported_message(&without, HLC).unwrap();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert!(groups["core"].get().ends_with(r#""reply_to_id":null}"#));
    assert_eq!(row.reply_to_id, None);
}

#[test]
fn columns_parsed_from_group_bytes_equal_the_row() {
    let row = new_imported_message(&input(), HLC).unwrap();
    let columns = message_columns(&row.groups_raw).unwrap();
    assert_eq!(columns.origin, row.origin);
    assert_eq!(columns.ext_json, row.ext_json);
    assert_eq!(columns.reply_to_id, row.reply_to_id);
    assert_eq!(columns.created_at, row.created_at);
    assert_eq!(columns.created_tz, row.created_tz);
    assert_eq!(columns.body_md, row.body_md);
    assert_eq!(columns.chat_id, row.chat_id);
    assert_eq!(columns.journal_date, row.journal_date);
    assert_eq!(columns.deleted_at, None);
}

#[test]
fn every_group_is_canonical_and_shares_one_hlc_with_sv_1() {
    let row = new_imported_message(&input(), HLC).unwrap();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(
        groups.keys().map(String::as_str).collect::<Vec<_>>(),
        ["body", "core", "deleted", "flags", "placement"]
    );
    for (name, raw) in &groups {
        let value: Value = serde_json::from_str(raw.get()).unwrap();
        assert_eq!(
            jcs::canonical_bytes(&value).unwrap(),
            raw.get().as_bytes(),
            "{name} каноничен"
        );
    }
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(&row.hlc_json).unwrap();
    assert_eq!(hlcs.len(), 5);
    for group in ["core", "body", "flags", "placement", "deleted"] {
        assert_eq!(
            hlcs[group],
            serde_json::json!({"h": HLC, "sv": 1}),
            "{group}"
        );
    }
    assert_eq!(row.schema_version, 1);
}

#[test]
fn other_groups_are_those_of_a_user_message() {
    // Импорт отличается от записи пользователя только группой `core`: остальные четыре байт в байт те же.
    let imported = new_imported_message(&input(), HLC).unwrap();
    let user = crate::groups::new_message(
        &crate::groups::NewMessage {
            id: MSG_ID,
            chat_id: INBOX_ID,
            body_md: "Привет **мир**",
            created_at: 1_552_000_000_007,
            created_tz: "Europe/Moscow",
            journal_date: "2019-03-08",
        },
        HLC,
    )
    .unwrap();
    assert_eq!(user.origin, ORIGIN_USER);
    let (a, b) = (
        parse_groups_raw(&imported.groups_raw).unwrap(),
        parse_groups_raw(&user.groups_raw).unwrap(),
    );
    for group in ["body", "flags", "placement", "deleted"] {
        assert_eq!(a[group].get(), b[group].get(), "{group}");
    }
    assert_ne!(a["core"].get(), b["core"].get());
}

#[test]
fn invalid_external_ids_are_rejected() {
    for bad in [
        "",
        "-",
        "1.5",
        "abc",
        "12 3",
        "+5",
        "123456789012345678901",
        "--1",
        "１２",
    ] {
        let mut chat = input();
        chat.ext_chat = bad;
        assert!(
            matches!(
                new_imported_message(&chat, HLC),
                Err(GroupError::Invalid(_))
            ),
            "chat {bad:?}"
        );
        let mut msg = input();
        msg.ext_msg = bad;
        assert!(
            matches!(new_imported_message(&msg, HLC), Err(GroupError::Invalid(_))),
            "msg {bad:?}"
        );
    }
    // Границы: 20 знаков и отрицательный id чата (группы и каналы).
    for good in [
        "0",
        "-1001234567890",
        "12345678901234567890",
        "-9223372036854775808",
    ] {
        let mut ok = input();
        ok.ext_chat = good;
        assert!(new_imported_message(&ok, HLC).is_ok(), "{good}");
    }
}

#[test]
fn invalid_reply_to_id_is_rejected() {
    for bad in [
        "",
        "42",
        "7B2C9D10-1111-4222-8333-444455556666",
        "7b2c9d10-1111-1222-8333-444455556666",
    ] {
        let mut input = input();
        input.reply_to_id = Some(bad);
        assert!(
            matches!(
                new_imported_message(&input, HLC),
                Err(GroupError::Invalid(_))
            ),
            "{bad:?}"
        );
    }
}
