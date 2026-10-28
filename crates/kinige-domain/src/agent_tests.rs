//! Тесты слоя агента: байты записи агента, правило дописывания, «в этот день», «охотник за паникой» на разборе
//! аргументов, property-тест «журнал без содержимого» (ADR-0009, слайс 4a).

use super::*;
use crate::agent_reply::{AgentEntry, fit_entries, month_day};
use crate::append::{AppendCandidate, can_append, can_append_origin};
use crate::groups::{INBOX_ID, NewMessage, ORIGIN_AGENT, new_agent_message, parse_groups_raw};
use serde_json::json;

const HLC: &str = "1760000000000-00000-5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const ID: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";
const NOW: i64 = 1_760_000_000_000;

fn map(value: Value) -> Map<String, Value> {
    value.as_object().cloned().unwrap_or_default()
}

fn agent_row(task: bool) -> crate::groups::MessageRow {
    new_agent_message(
        &NewMessage {
            id: ID,
            chat_id: INBOX_ID,
            body_md: "Итоги встречи",
            created_at: NOW,
            created_tz: "Europe/Moscow",
            journal_date: "2026-10-09",
        },
        task,
        HLC,
    )
    .unwrap()
}

#[test]
fn agent_message_core_bytes_are_exact() {
    let row = agent_row(false);
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(
        groups["core"].get(),
        r#"{"created_at":1760000000000,"created_tz":"Europe/Moscow","origin":"agent","reply_to_id":null}"#
    );
    assert!(!groups["core"].get().contains("ext"));
    assert_eq!(row.origin, ORIGIN_AGENT);
    assert_eq!(row.ext_json, None);
    assert_eq!(
        groups["flags"].get(),
        r#"{"favorite_at":null,"is_task":false,"pinned_at":null,"task_done_at":null}"#
    );
}

#[test]
fn agent_task_message_has_flags_group_with_is_task() {
    let row = agent_row(true);
    let groups = parse_groups_raw(&row.groups_raw).unwrap();
    assert_eq!(
        groups["flags"].get(),
        r#"{"favorite_at":null,"is_task":true,"pinned_at":null,"task_done_at":null}"#
    );
    assert!(row.is_task);
    // Все пять групп под одним HLC, `sv = 1`.
    let hlcs: Value = serde_json::from_str(&row.hlc_json).unwrap();
    for group in ["core", "body", "flags", "placement", "deleted"] {
        assert_eq!(hlcs[group], json!({"h": HLC, "sv": 1}), "{group}");
    }
}

#[test]
fn append_rule_follows_the_expected_origin_and_user_rule_is_unchanged() {
    let row = agent_row(false);
    let candidate = AppendCandidate {
        origin: &row.origin,
        created_at: row.created_at,
        journal_date: &row.journal_date,
        hlc_json: &row.hlc_json,
    };
    let today = "2026-10-09";
    assert!(can_append_origin(&candidate, ORIGIN_AGENT, NOW + 60_000, today).unwrap());
    // Окно захвата (origin = user) к записи агента не дописывает.
    assert!(!can_append(&candidate, NOW + 60_000, today).unwrap());
    // Старше десяти минут и не сегодняшняя — нет.
    assert!(!can_append_origin(&candidate, ORIGIN_AGENT, NOW + 11 * 60_000, today).unwrap());
    assert!(!can_append_origin(&candidate, ORIGIN_AGENT, NOW + 60_000, "2026-10-10").unwrap());
    // Запись пользователя агент не дописывает.
    let user = AppendCandidate {
        origin: "user",
        ..candidate
    };
    assert!(!can_append_origin(&user, ORIGIN_AGENT, NOW + 60_000, today).unwrap());
}

#[test]
fn on_this_day_keeps_month_and_day_and_leap_day_matches_only_itself() {
    let md = month_day("2026-03-14").unwrap();
    assert_eq!((md.month_day.as_str(), md.year), ("03-14", 2026));
    let leap = month_day("2024-02-29").unwrap();
    assert_eq!(leap.month_day, "02-29");
    assert_ne!(leap.month_day, month_day("2025-02-28").unwrap().month_day);
    assert_ne!(leap.month_day, month_day("2025-03-01").unwrap().month_day);
    assert!(month_day("2025-02-29").is_none());
    assert!(month_day("garbage").is_none());
}

#[test]
fn argument_limits_and_formats() {
    let ok = |tool, value: Value| parse_args(tool, Some(&map(value)));
    assert!(ok(AgentTool::Search, json!({"query": "Raft"})).is_ok());
    assert!(
        ok(
            AgentTool::Search,
            json!({"query": "x".repeat(MAX_QUERY_CHARS)})
        )
        .is_ok()
    );
    assert!(
        ok(
            AgentTool::Search,
            json!({"query": "x".repeat(MAX_QUERY_CHARS + 1)})
        )
        .is_err()
    );
    assert!(ok(AgentTool::Search, json!({"query": 5})).is_err());
    assert!(ok(AgentTool::Search, json!({})).is_err());
    assert!(
        ok(
            AgentTool::Search,
            json!({"query": "a", "from": "2026-02-30"})
        )
        .is_err()
    );
    assert!(
        ok(
            AgentTool::Search,
            json!({"query": "a", "chat_id": "not-an-id"})
        )
        .is_err()
    );
    assert!(ok(AgentTool::GetEntries, json!({"date": "2026-03-14"})).is_ok());
    assert!(ok(AgentTool::GetEntries, json!({"from": "2026-03-14"})).is_err());
    assert!(
        ok(
            AgentTool::GetEntries,
            json!({"from": "2026-03-15", "to": "2026-03-14"})
        )
        .is_err()
    );
    assert!(
        ok(
            AgentTool::GetEntries,
            json!({"date": "2026-03-14", "from": "2026-03-14", "to": "2026-03-14"})
        )
        .is_err()
    );
    assert!(ok(AgentTool::GetEntry, json!({"id": ID})).is_ok());
    assert!(ok(AgentTool::GetEntry, json!({"id": "x"})).is_err());
    assert!(ok(AgentTool::AddEntry, json!({"text": "   "})).is_err());
    assert!(ok(AgentTool::AddEntry, json!({"text": "ok", "task": "yes"})).is_err());
    assert!(ok(AgentTool::SetTaskDone, json!({"id": ID})).is_err());
    assert!(ok(AgentTool::SetTaskDone, json!({"id": ID, "done": true})).is_ok());
    // Лишние поля игнорируются, `null` — «нет поля».
    assert!(
        ok(
            AgentTool::GetEntry,
            json!({"id": ID, "extra": [1], "chat_id": null})
        )
        .is_ok()
    );
}

#[test]
fn cursor_round_trips_and_rejects_garbage() {
    let cursor = AgentCursor {
        journal_date: "2026-03-14".into(),
        created_at: NOW,
        id: ID.into(),
    };
    assert_eq!(AgentCursor::decode(&cursor.encode()), Some(cursor));
    for text in [
        "",
        ":::",
        "2026-03-14:x:y",
        "2026-13-01:1:0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d",
        "2026-03-14:-5:0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d",
    ] {
        assert_eq!(AgentCursor::decode(text), None, "{text:?}");
    }
}

/// Мусорные значения: пустые, неверного типа, огромные, со служебными символами.
fn garbage() -> Vec<Value> {
    vec![
        Value::Null,
        json!(true),
        json!(false),
        json!(0),
        json!(-1),
        json!(1e308),
        json!(u64::MAX),
        json!(""),
        json!(" "),
        json!("\u{0}\u{202e}\u{1b}[31m"),
        json!("x".repeat(1_500_000)),
        json!("я".repeat(600_000)),
        json!([]),
        json!([[[[[[[[1]]]]]]]]),
        json!({}),
        json!({"a": {"b": {"c": null}}}),
        json!("2026-02-30"),
        json!("9999-12-31"),
        json!("0000-01-01"),
        json!(ID),
        json!("2026-03-14:99999999999999999999:x"),
    ]
}

#[test]
fn panic_hunt_every_tool_survives_garbage_arguments() {
    let keys = [
        "query",
        "chat_id",
        "from",
        "to",
        "date",
        "id",
        "text",
        "task",
        "done",
        "favorites",
        "tasks",
        "cursor",
    ];
    for tool in AgentTool::ALL {
        assert!(parse_args(tool, None).is_ok() || parse_args(tool, None).is_err());
        for key in keys {
            for value in garbage() {
                let mut args = Map::new();
                args.insert(key.to_string(), value);
                // Результат не важен: важно, что нет паники, а принятые аргументы после разбора корректны.
                if let Ok(parsed) = parse_args(tool, Some(&args)) {
                    let _ = args_json(&parsed);
                }
            }
        }
        // Все поля сразу мусорные.
        for value in garbage() {
            let args: Map<String, Value> = keys
                .iter()
                .map(|k| (k.to_string(), value.clone()))
                .collect();
            let _ = parse_args(tool, Some(&args));
        }
    }
}

/// Детерминированный генератор для property-теста (без внешних крейтов).
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn text(&mut self, marker: &str) -> String {
        const ALPHABET: &[char] = &[
            'a', 'я', ' ', '\n', '"', '\\', '{', '}', '😀', '1', '-', ':',
        ];
        let mut out = String::new();
        for _ in 0..(self.next() % 40) {
            out.push(ALPHABET[(self.next() % ALPHABET.len() as u64) as usize]);
        }
        out.push_str(marker);
        for _ in 0..(self.next() % 40) {
            out.push(ALPHABET[(self.next() % ALPHABET.len() as u64) as usize]);
        }
        out
    }
}

#[test]
fn log_args_never_contain_entry_text() {
    let mut random = Xorshift(0x9E37_79B9_7F4A_7C15);
    for round in 0..400 {
        let marker = format!("SECRETMARKER{round}Z");
        let text = random.text(&marker);
        let chat = random.next().is_multiple_of(2).then(|| ID.to_string());
        for args in [
            ToolArgs::AddEntry {
                text: text.clone(),
                chat_id: chat.clone(),
                task: round % 2 == 0,
            },
            ToolArgs::AppendToLastEntry {
                text: text.clone(),
                chat_id: chat.clone(),
            },
            ToolArgs::GetEntry { id: ID.into() },
            ToolArgs::SetTaskDone {
                id: ID.into(),
                done: true,
            },
        ] {
            let json = args_json(&args);
            assert!(!json.contains(&marker), "{json}");
            assert!(!json.contains(&text), "{json}");
            assert!(
                !json.contains(ID) || chat.is_some(),
                "id записи в args_json: {json}"
            );
        }
        // Через разбор настоящих аргументов инструмента записи.
        let parsed = parse_args(
            AgentTool::AddEntry,
            Some(&map(
                json!({"text": text, "chat_id": chat, "task": true, "extra": marker}),
            )),
        )
        .unwrap();
        assert!(!args_json(&parsed).contains(&marker));
    }
}

#[test]
fn search_query_in_log_is_cut_to_128_chars() {
    let query = format!("{}HIDDENTAIL", "я".repeat(LOG_QUERY_CHARS));
    let parsed = parse_args(AgentTool::Search, Some(&map(json!({"query": query})))).unwrap();
    let json: Value = serde_json::from_str(&args_json(&parsed)).unwrap();
    assert_eq!(
        json["query"].as_str().unwrap().chars().count(),
        LOG_QUERY_CHARS
    );
    assert!(!args_json(&parsed).contains("HIDDENTAIL"));
}

#[test]
fn client_name_is_cleaned_and_cut() {
    assert_eq!(clean_client("Claude Desktop"), "Claude Desktop");
    assert_eq!(clean_client("a\u{0}b\nc\u{1b}[0m"), "abc[0m");
    assert_eq!(
        clean_client("я".repeat(100).as_str()).chars().count(),
        CLIENT_CHARS
    );
    assert_eq!(clean_client(" \n "), "unknown");
    assert_eq!(clean_client(""), "unknown");
}

fn entry(text: &str) -> AgentEntry {
    AgentEntry {
        id: ID.into(),
        chat_id: INBOX_ID.into(),
        chat: "Inbox".into(),
        journal_date: "2026-03-14".into(),
        created: "2026-03-14T21:05:09+03:00".into(),
        origin: "user".into(),
        task: false,
        done: false,
        favorite: false,
        text: text.into(),
        truncated: false,
    }
}

#[test]
fn list_text_is_cut_with_a_flag() {
    let short = entry("abc").shorten(LIST_TEXT_CHARS);
    assert!(!short.truncated);
    assert!(!serde_json::to_string(&short).unwrap().contains("truncated"));
    let long = entry(&"я".repeat(LIST_TEXT_CHARS + 5)).shorten(LIST_TEXT_CHARS);
    assert!(long.truncated);
    assert_eq!(long.text.chars().count(), LIST_TEXT_CHARS);
}

#[test]
fn response_is_cut_by_whole_entries_and_one_huge_entry_is_shortened() {
    let big = "я".repeat(LIST_TEXT_CHARS);
    let many: Vec<AgentEntry> = (0..400)
        .map(|_| entry(&big).shorten(LIST_TEXT_CHARS))
        .collect();
    let (kept, cut) = fit_entries(many);
    assert!(cut);
    assert!(!kept.is_empty() && kept.len() < 400);
    assert!(serde_json::to_string(&kept).unwrap().len() <= RESPONSE_BYTES);
    let (kept, cut) = fit_entries(vec![entry("a"), entry("b")]);
    assert_eq!((kept.len(), cut), (2, false));
    let (kept, cut) = fit_entries(vec![entry(&"x".repeat(3 * RESPONSE_BYTES)), entry("b")]);
    assert!(cut);
    assert_eq!(kept.len(), 1);
    assert!(kept[0].truncated);
    assert!(serde_json::to_string(&kept[0]).unwrap().len() <= RESPONSE_BYTES);
}
