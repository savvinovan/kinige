//! Тесты переноса записи в другой чат (`message_ops::move_to_chat`): байты группы `placement`, обе ветки §3.4, день не
//! меняется, остальные группы дословно, запись с неподдерживаемой `sv` не переносится.

use crate::groups::{GroupError, INBOX_ID, MessageRow, NewMessage, new_message, parse_groups_raw};
use crate::message_ops::{self, message_columns};
use crate::rewrite::{Groups, is_read_only};
use serde_json::Value;
use std::collections::BTreeMap;

const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const NOW: i64 = 1_760_000_000_000;
const BOOKS: &str = "7d6e5f40-3b2a-4c19-8e07-6a5b4c3d2e1f";

fn hlc(l: u64) -> String {
    format!("{l:013}-00000-{DEVICE}")
}

fn message() -> MessageRow {
    new_message(
        &NewMessage {
            id: "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d",
            chat_id: INBOX_ID,
            body_md: "Идея #заметка",
            created_at: NOW,
            created_tz: "Europe/Moscow",
            journal_date: "2026-10-09",
        },
        &hlc(NOW as u64),
    )
    .unwrap()
}

fn groups(row: &MessageRow) -> Groups<'_> {
    Groups {
        groups_raw: &row.groups_raw,
        hlc_json: &row.hlc_json,
    }
}

fn hlcs(hlc_json: &str) -> BTreeMap<String, Value> {
    serde_json::from_str(hlc_json).unwrap()
}

#[test]
fn placement_bytes_for_a_move_in_both_branches_of_the_chain() {
    let row = message();
    let confirmed =
        message_ops::move_to_chat(&groups(&row), BOOKS, true, &hlc(NOW as u64 + 1000)).unwrap();
    let placement = parse_groups_raw(&confirmed.groups_raw).unwrap()["placement"]
        .get()
        .to_string();
    assert_eq!(
        placement,
        format!(
            r#"{{"base":"{}","chat_id":"{BOOKS}","journal_date":"2026-10-09","supersedes":[]}}"#,
            hlc(NOW as u64)
        )
    );

    let own =
        message_ops::move_to_chat(&groups(&row), BOOKS, false, &hlc(NOW as u64 + 1000)).unwrap();
    let columns = message_columns(&own.groups_raw).unwrap();
    assert_eq!(columns.chat_id, BOOKS);
    assert_eq!(columns.journal_date, "2026-10-09", "день не меняется");
    assert_eq!(columns.placement_base, None);
    assert_eq!(
        columns.placement_supersedes,
        format!(r#"["{}"]"#, hlc(NOW as u64))
    );

    // Перенос обратно накапливает supersedes (как вторая смена дня).
    let back = message_ops::move_to_chat(
        &Groups {
            groups_raw: &own.groups_raw,
            hlc_json: &own.hlc_json,
        },
        INBOX_ID,
        false,
        &hlc(NOW as u64 + 2000),
    )
    .unwrap();
    let columns = message_columns(&back.groups_raw).unwrap();
    assert_eq!(columns.chat_id, INBOX_ID);
    assert_eq!(
        columns.placement_supersedes,
        format!(r#"["{}","{}"]"#, hlc(NOW as u64), hlc(NOW as u64 + 1000))
    );
}

#[test]
fn only_placement_changes_and_its_hlc_grows() {
    let row = message();
    let moved =
        message_ops::move_to_chat(&groups(&row), BOOKS, true, &hlc(NOW as u64 + 1)).unwrap();
    let before = parse_groups_raw(&row.groups_raw).unwrap();
    let after = parse_groups_raw(&moved.groups_raw).unwrap();
    for (name, raw) in &before {
        if name != "placement" {
            assert_eq!(after[name].get(), raw.get(), "{name} дословно");
            assert_eq!(hlcs(&moved.hlc_json)[name], hlcs(&row.hlc_json)[name]);
        }
    }
    assert_ne!(
        hlcs(&moved.hlc_json)["placement"],
        hlcs(&row.hlc_json)["placement"]
    );
    // Не растущий HLC — ошибка, ничего не переписано.
    assert!(matches!(
        message_ops::move_to_chat(&groups(&row), BOOKS, true, &hlc(NOW as u64 - 1)),
        Err(GroupError::HlcNotNewer)
    ));
}

#[test]
fn a_bad_chat_id_is_refused_and_a_newer_group_makes_the_record_read_only() {
    let row = message();
    for bad in ["", "с\nпереводом", &"я".repeat(65)] {
        assert!(
            matches!(
                message_ops::move_to_chat(&groups(&row), bad, true, &hlc(NOW as u64 + 1)),
                Err(GroupError::Invalid(_))
            ),
            "{bad:?}"
        );
    }
    // Группа из будущей версии: запись только для чтения, перенос не предлагается (проверяет вызывающий через is_read_only).
    let mut hlc_map: BTreeMap<String, Value> = hlcs(&row.hlc_json);
    hlc_map.insert(
        "flags".into(),
        serde_json::json!({"h": hlc(NOW as u64), "sv": 2}),
    );
    let newer = serde_json::to_string(&hlc_map).unwrap();
    assert!(is_read_only(&newer).unwrap());
    assert!(!is_read_only(&row.hlc_json).unwrap());
}
