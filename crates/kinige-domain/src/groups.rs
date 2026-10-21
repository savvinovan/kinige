//! Группы полей новой записи и чата (ADR-0001 §3.2, приложение §A.0): каждая запись создаётся сразу со всеми группами,
//! у каждой свой HLC и `sv = 1`. Колонки таблиц и каноничные байты групп строятся из одних и тех же значений, поэтому
//! расхождения между ними не бывает (инвариант `canonical(колонки) == groups_raw[g]`, приложение §B п. 38).

use crate::jcs::{self, JcsError};
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;
use std::fmt;

/// `sv` групп, которые пишет этот клиент (ADR-0001 §4).
pub const SCHEMA_VERSION: u32 = 1;

/// Inbox — одна константа на все аккаунты (ADR-0001, решение 2); определяется по id, а не по флагу (§3.6).
/// Значение в формате UUID v4 (версия 4, вариант 8), выбрано человеком и закреплено в приложении ADR-0001, §C.
pub const INBOX_ID: &str = "00000000-0000-4000-8000-000000000001";
/// Эмодзи Inbox в `meta`: данные чата, пользователь сможет сменить в слайсе 3.
pub const INBOX_ICON: &str = "📥";
/// Ключ порядка Inbox (дробный индекс, ТЗ §4.2): первый в списке.
pub const INBOX_SORT_KEY: &str = "a0";
/// Предел группы `body` в каноничных байтах JCS: конверт ограничен 1 МиБ до выравнивания (ADR-0001 §5.3), остальное место —
/// другим группам. Считаются именно байты группы, а не длина текста: экранирование (`"` → `\"`, перевод строки → `\n`)
/// удваивает размер, и текст в 512 КиБ мог бы дать группу в 1 МиБ.
pub const MAX_BODY_GROUP_BYTES: usize = 512 * 1024;
/// Кто создал запись: интерфейс и быстрый захват (ADR-0001, «origin»).
pub const ORIGIN_USER: &str = "user";

/// Ошибки сборки групп.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupError {
    Jcs(JcsError),
    /// `groups_raw` или `hlc_json` повреждены.
    Corrupt(String),
}

impl fmt::Display for GroupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GroupError::Jcs(error) => write!(f, "{error}"),
            GroupError::Corrupt(why) => write!(f, "повреждены группы записи: {why}"),
        }
    }
}

impl std::error::Error for GroupError {}

impl From<JcsError> for GroupError {
    fn from(error: JcsError) -> Self {
        GroupError::Jcs(error)
    }
}

/// Причина, по которой текст записи не принят.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyError {
    /// Только пробельные символы.
    Empty,
    /// Группа `body` длиннее [`MAX_BODY_GROUP_BYTES`] в каноничных байтах.
    TooLarge,
}

/// Текст записи перед сохранением: перевод строк приведён к `\n`, ведущие пустые строки и хвостовые пробелы убраны
/// (ведущие пробелы первой непустой строки остаются: отступ может быть значимым), пустой текст и слишком длинный — ошибка.
pub fn normalize_body(raw: &str) -> Result<String, BodyError> {
    let unified = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = unified
        .split('\n')
        .skip_while(|line| line.trim().is_empty());
    let first = lines.next();
    let Some(first) = first else {
        return Err(BodyError::Empty);
    };
    let mut text = String::from(first);
    for line in lines {
        text.push('\n');
        text.push_str(line);
    }
    let text = text.trim_end().to_string();
    if text.is_empty() {
        return Err(BodyError::Empty);
    }
    let group = jcs::canonical_bytes(&body_group(&text)).map_err(|_| BodyError::TooLarge)?;
    if group.len() > MAX_BODY_GROUP_BYTES {
        return Err(BodyError::TooLarge);
    }
    Ok(text)
}

/// Значение группы `body` новой записи (одно место и для проверки размера, и для байтов группы).
fn body_group(body_md: &str) -> Value {
    json!({"base": null, "body_md": body_md, "edited_at": null, "supersedes": []})
}

/// Исходные значения новой записи; текст уже нормализован.
pub struct NewMessage<'a> {
    pub id: &'a str,
    pub chat_id: &'a str,
    pub body_md: &'a str,
    pub created_at: i64,
    pub created_tz: &'a str,
    pub journal_date: &'a str,
}

/// Строка таблицы `message` целиком: разобранные колонки и сырые байты групп.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageRow {
    pub id: String,
    pub chat_id: String,
    pub body_md: String,
    pub journal_date: String,
    pub created_at: i64,
    pub created_tz: String,
    pub edited_at: Option<i64>,
    pub is_task: bool,
    pub task_done_at: Option<i64>,
    pub favorite_at: Option<i64>,
    pub pinned_at: Option<i64>,
    pub reply_to_id: Option<String>,
    pub origin: String,
    pub ext_json: Option<String>,
    pub deleted_at: Option<i64>,
    pub body_base: Option<String>,
    /// JSON-массив HLC, `[]` у новой записи.
    pub body_supersedes: String,
    pub placement_base: Option<String>,
    pub placement_supersedes: String,
    /// `{"<группа>":{"h":"HLC","sv":1}}`.
    pub hlc_json: String,
    pub schema_version: u32,
    /// `{"<группа>":<каноничные байты>}` — источник для переотправки (ADR-0001 §C).
    pub groups_raw: String,
}

/// Новая запись сразу со всеми пятью группами (`core`, `body`, `flags`, `placement`, `deleted`) под одним HLC: она создаётся одним событием.
pub fn new_message(input: &NewMessage<'_>, hlc: &str) -> Result<MessageRow, GroupError> {
    let groups: [(&str, Value); 5] = [
        (
            "core",
            json!({
                "created_at": input.created_at,
                "created_tz": input.created_tz,
                "origin": ORIGIN_USER,
                "reply_to_id": null,
            }),
        ),
        ("body", body_group(input.body_md)),
        (
            "flags",
            json!({"favorite_at": null, "is_task": false, "pinned_at": null, "task_done_at": null}),
        ),
        (
            "placement",
            json!({"base": null, "chat_id": input.chat_id, "journal_date": input.journal_date, "supersedes": []}),
        ),
        ("deleted", json!({"deleted_at": null})),
    ];
    let (groups_raw, hlc_json) = assemble(&groups, hlc)?;
    Ok(MessageRow {
        id: input.id.to_string(),
        chat_id: input.chat_id.to_string(),
        body_md: input.body_md.to_string(),
        journal_date: input.journal_date.to_string(),
        created_at: input.created_at,
        created_tz: input.created_tz.to_string(),
        edited_at: None,
        is_task: false,
        task_done_at: None,
        favorite_at: None,
        pinned_at: None,
        reply_to_id: None,
        origin: ORIGIN_USER.to_string(),
        ext_json: None,
        deleted_at: None,
        body_base: None,
        body_supersedes: "[]".to_string(),
        placement_base: None,
        placement_supersedes: "[]".to_string(),
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

/// Строка таблицы `chat` целиком.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatRow {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub sort_key: String,
    pub is_inbox: bool,
    pub ai_excluded: bool,
    pub pinned_at: Option<i64>,
    pub archived_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub deleted_at: Option<i64>,
    pub hlc_json: String,
    pub schema_version: u32,
    pub groups_raw: String,
}

/// Inbox: константный id и константная группа `core` (ADR-0001 §3.6) — `created_at = 0`, `is_inbox = true`, чтобы два
/// Inbox, созданные до привязки, сливались без конфликта. Хранимое имя пустое: интерфейс показывает переведённое.
pub fn inbox_chat(hlc: &str, now_ms: i64) -> Result<ChatRow, GroupError> {
    let groups: [(&str, Value); 6] = [
        ("core", json!({"created_at": 0, "is_inbox": true})),
        (
            "meta",
            json!({"color": null, "icon": INBOX_ICON, "title": ""}),
        ),
        ("order", json!({"sort_key": INBOX_SORT_KEY})),
        ("state", json!({"archived_at": null, "pinned_at": null})),
        ("privacy", json!({"ai_excluded": false})),
        ("deleted", json!({"deleted_at": null})),
    ];
    let (groups_raw, hlc_json) = assemble(&groups, hlc)?;
    Ok(ChatRow {
        id: INBOX_ID.to_string(),
        title: String::new(),
        icon: Some(INBOX_ICON.to_string()),
        color: None,
        sort_key: INBOX_SORT_KEY.to_string(),
        is_inbox: true,
        ai_excluded: false,
        pinned_at: None,
        archived_at: None,
        created_at: 0,
        updated_at: now_ms,
        deleted_at: None,
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

/// Группы из `groups_raw` с дословными байтами (для проверки инварианта и для слияния).
pub fn parse_groups_raw(groups_raw: &str) -> Result<BTreeMap<String, Box<RawValue>>, GroupError> {
    serde_json::from_str(groups_raw).map_err(|e| GroupError::Corrupt(e.to_string()))
}

/// Собирает `groups_raw` (каноничные байты групп дословно) и `hlc_json` (один HLC на все группы события).
fn assemble(groups: &[(&str, Value)], hlc: &str) -> Result<(String, String), GroupError> {
    let mut raw: BTreeMap<String, Box<RawValue>> = BTreeMap::new();
    let mut hlcs: BTreeMap<&str, Value> = BTreeMap::new();
    for (name, value) in groups {
        let bytes = jcs::canonical_bytes(value)?;
        let text = String::from_utf8(bytes).expect("JCS выдаёт UTF-8");
        raw.insert(
            (*name).to_string(),
            RawValue::from_string(text).map_err(|e| GroupError::Corrupt(e.to_string()))?,
        );
        hlcs.insert(name, json!({"h": hlc, "sv": SCHEMA_VERSION}));
    }
    let groups_raw = serde_json::to_string(&raw).map_err(|e| GroupError::Corrupt(e.to_string()))?;
    let hlc_json = serde_json::to_string(&hlcs).map_err(|e| GroupError::Corrupt(e.to_string()))?;
    Ok((groups_raw, hlc_json))
}

#[cfg(test)]
mod tests {
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
}
