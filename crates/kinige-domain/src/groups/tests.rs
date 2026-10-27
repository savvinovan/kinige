use super::*;

const HLC: &str = "1760000000000-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const HLC2: &str = "1760000005000-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const MSG_ID: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

fn sample() -> MessageRow {
    new_message(
        &NewMessage {
            id: MSG_ID,
            chat_id: INBOX_ID,
            body_md: "Купить **молоко**\n- [ ] позвонить маме",
            created_at: 1_760_000_000_123,
            created_tz: "Europe/Moscow",
            journal_date: "2026-10-09",
        },
        HLC,
    )
    .unwrap()
}

#[test]
fn inbox_id_is_a_valid_v4_uuid() {
    assert!(crate::hlc::is_uuid(INBOX_ID));
    assert_eq!(&INBOX_ID[14..15], "4", "версия");
    assert!(
        matches!(&INBOX_ID[19..20], "8" | "9" | "a" | "b"),
        "вариант"
    );
}

#[test]
fn new_message_group_bytes_are_exact() {
    // Точные байты групп новой записи: ключи по JCS, без пробелов, `sv = 1`, все пять групп (ADR-0001 §A.0).
    let row = sample();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    let get = |name: &str| groups[name].get().to_string();
    assert_eq!(
        get("core"),
        r#"{"created_at":1760000000123,"created_tz":"Europe/Moscow","origin":"user","reply_to_id":null}"#
    );
    assert_eq!(
        get("body"),
        r#"{"base":null,"body_md":"Купить **молоко**\n- [ ] позвонить маме","edited_at":null,"supersedes":[]}"#
    );
    assert_eq!(
        get("flags"),
        r#"{"favorite_at":null,"is_task":false,"pinned_at":null,"task_done_at":null}"#
    );
    assert_eq!(
        get("placement"),
        format!(
            r#"{{"base":null,"chat_id":"{INBOX_ID}","journal_date":"2026-10-09","supersedes":[]}}"#
        )
    );
    assert_eq!(get("deleted"), r#"{"deleted_at":null}"#);
    assert_eq!(
        groups.keys().map(String::as_str).collect::<Vec<_>>(),
        ["body", "core", "deleted", "flags", "placement"]
    );
}

#[test]
fn new_message_has_hlc_and_sv_for_every_group() {
    let row = sample();
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(&row.hlc_json).unwrap();
    assert_eq!(hlcs.len(), 5);
    for group in ["core", "body", "flags", "placement", "deleted"] {
        assert_eq!(hlcs[group], json!({"h": HLC, "sv": 1}), "{group}");
    }
    assert_eq!(row.schema_version, 1);
    assert_eq!(row.origin, "user");
    assert_eq!(row.body_supersedes, "[]");
}

#[test]
fn columns_match_canonical_group_bytes() {
    // Инвариант приложения §B п. 38: колонки — разобранное представление байтов групп.
    let row = sample();
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    let value = |name: &str| -> Value { serde_json::from_str(groups[name].get()).unwrap() };
    assert_eq!(value("core")["created_at"], json!(row.created_at));
    assert_eq!(value("core")["created_tz"], json!(row.created_tz));
    assert_eq!(value("core")["origin"], json!(row.origin));
    assert_eq!(value("body")["body_md"], json!(row.body_md));
    assert_eq!(value("placement")["chat_id"], json!(row.chat_id));
    assert_eq!(value("placement")["journal_date"], json!(row.journal_date));
    assert_eq!(value("flags")["is_task"], json!(row.is_task));
    assert_eq!(value("deleted")["deleted_at"], json!(row.deleted_at));
    for name in ["core", "body", "flags", "placement", "deleted"] {
        let reparsed: Value = serde_json::from_str(groups[name].get()).unwrap();
        assert_eq!(
            jcs::canonical_bytes(&reparsed).unwrap(),
            groups[name].get().as_bytes(),
            "{name} каноничен"
        );
    }
}

#[test]
fn inbox_core_is_constant_and_independent_of_time_and_device() {
    let a = inbox_chat(HLC, 1_000).unwrap();
    let b = inbox_chat(HLC2, 9_999_999).unwrap();
    let core = |row: &ChatRow| {
        parse_groups_raw(&row.groups_raw).unwrap()["core"]
            .get()
            .to_string()
    };
    assert_eq!(core(&a), r#"{"created_at":0,"is_inbox":true}"#);
    assert_eq!(core(&a), core(&b));
    assert_eq!(a.id, INBOX_ID);
    assert_eq!(
        a.title, "",
        "хранимое имя пустое, интерфейс показывает переведённое"
    );
    let groups = parse_groups_raw(&a.groups_raw).unwrap();
    assert_eq!(
        groups.keys().map(String::as_str).collect::<Vec<_>>(),
        ["core", "deleted", "meta", "order", "privacy", "state"]
    );
    assert_eq!(
        groups["meta"].get(),
        r#"{"color":null,"icon":"📥","title":""}"#
    );
    assert_eq!(groups["order"].get(), r#"{"sort_key":"a0"}"#);
    assert_eq!(
        groups["state"].get(),
        r#"{"archived_at":null,"pinned_at":null}"#
    );
    assert_eq!(groups["privacy"].get(), r#"{"ai_excluded":false}"#);
}

#[test]
fn body_is_normalized() {
    assert_eq!(
        normalize_body("  \n\n  Привет\r\nмир \n\n").unwrap(),
        "  Привет\nмир"
    );
    assert_eq!(normalize_body("a\rb").unwrap(), "a\nb");
    assert_eq!(
        normalize_body("- [ ] a\n- [x] b\n").unwrap(),
        "- [ ] a\n- [x] b"
    );
    assert_eq!(normalize_body(""), Err(BodyError::Empty));
    assert_eq!(normalize_body(" \n\t\r\n"), Err(BodyError::Empty));
}

/// Каноничные байты группы `body` для текста.
fn body_group_len(text: &str) -> usize {
    jcs::canonical_bytes(&body_group(text)).unwrap().len()
}

#[test]
fn body_limit_counts_canonical_group_bytes_not_text_length() {
    // Кавычка и перевод строки в JCS занимают по два байта: 256 Ки пар `"\n` — это 512 КиБ текста,
    // но 1 МиБ группы. По длине текста такая запись проходила бы.
    let escaped = "\"\n".repeat(256 * 1024);
    assert!(escaped.len() <= MAX_BODY_GROUP_BYTES);
    assert_eq!(normalize_body(&escaped), Err(BodyError::TooLarge));

    // Обычный текст: граница ровно по байтам группы (накладные — ключи и `null` вокруг текста).
    let overhead = body_group_len("");
    let fits = "я".repeat((MAX_BODY_GROUP_BYTES - overhead) / 2);
    assert_eq!(
        body_group_len(&fits),
        MAX_BODY_GROUP_BYTES - ((MAX_BODY_GROUP_BYTES - overhead) % 2)
    );
    assert!(normalize_body(&fits).is_ok());
    assert_eq!(
        normalize_body(&"я".repeat(fits.chars().count() + 1)),
        Err(BodyError::TooLarge)
    );
}
