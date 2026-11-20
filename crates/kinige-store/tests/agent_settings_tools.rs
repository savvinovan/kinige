//! The two settings tools of MCP (slice 5d, ADR-0022): the pinned key set of `get_settings`, one writable setting, the refusals,
//! the log row and the one transaction.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::agent_prefs::SettingKey;
use kinige_store::{AgentError, AgentRequest};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

const MARKER: &str = "MARKER-7731";

/// Every `app_setting` row, as a sorted dump: "byte-identical before and after".
fn app_settings(dir: &Path) -> Vec<(String, String)> {
    let conn = raw(dir);
    let mut statement = conn
        .prepare("SELECT key, value FROM app_setting ORDER BY key")
        .unwrap();
    statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn keys_of(value: &Value) -> BTreeSet<&str> {
    value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect()
}

fn set(store: &kinige_store::Store, key: Value, value: Value) -> Result<Value, AgentError> {
    call(
        store,
        "set_setting",
        json!({"key": key, "value": value}),
        T0,
    )
}

#[test]
fn get_settings_has_exactly_the_pinned_keys_and_no_marker_of_anything_private() {
    let a = agent(AgentMode::Read, false);
    // Things an agent must never see: a path, the title of a chat it cannot see, a recent emoji, window state, the hash row.
    let conn = raw(a.dir.path());
    for (key, value) in [
        ("mirror_dir", format!("/{MARKER}/copy")),
        ("settings_file_hash", format!("hash-{MARKER}")),
        ("forest_view", "year".to_string()),
    ] {
        conn.execute(
            "INSERT INTO app_setting(key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            (key, value),
        )
        .unwrap();
    }
    a.store.add_recent_emoji("🦄").unwrap();
    a.store.set_sidebar_collapsed(true).unwrap();
    let reply = call(&a.store, "get_settings", json!({}), T0).unwrap();
    assert_eq!(
        keys_of(&reply),
        BTreeSet::from([
            "text_size",
            "shortcuts",
            "markdown_copy",
            "agent_access",
            "writable"
        ])
    );
    assert_eq!(
        keys_of(&reply["shortcuts"]),
        BTreeSet::from(["capture", "clipboard"])
    );
    assert_eq!(
        keys_of(&reply["shortcuts"]["capture"]),
        BTreeSet::from(["value", "default"])
    );
    assert_eq!(
        keys_of(&reply["agent_access"]),
        BTreeSet::from(["mode", "visible_chat_ids"])
    );
    assert_eq!(reply["text_size"], "normal");
    assert_eq!(reply["shortcuts"]["capture"]["value"], "Ctrl+Alt+KeyN");
    assert_eq!(reply["shortcuts"]["clipboard"]["default"], "Alt+Shift+KeyV");
    assert_eq!(reply["markdown_copy"], "chosen_folder");
    assert_eq!(reply["agent_access"]["mode"], "read");
    assert_eq!(reply["writable"], json!(["text_size"]));
    let text = reply.to_string();
    for private in [
        MARKER,
        "Секретный проект",
        "Дневник",
        "Работа",
        "🦄",
        "sidebar",
        "year",
    ] {
        assert!(!text.contains(private), "{private}: {text}");
    }
    // The log row of a read holds no arguments at all.
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(
        (rows[0].1.as_str(), rows[0].2.as_str(), rows[0].3.as_str()),
        ("get_settings", "{}", "ok")
    );
}

#[test]
fn the_copy_state_is_default_chosen_or_off_and_never_a_path() {
    let a = agent(AgentMode::Read, false);
    let default = call(&a.store, "get_settings", json!({}), T0).unwrap();
    assert_eq!(default["markdown_copy"], "default_folder");
    let dir = tempfile::tempdir().unwrap();
    let mut options = options(dir.path());
    options.default_mirror_dir = None;
    options.start_mirror = false;
    let store = kinige_store::Store::open_at(options, &moscow(T0)).unwrap();
    store.set_agent_enabled(true).unwrap();
    let off = call(&store, "get_settings", json!({}), T0).unwrap();
    assert_eq!(off["markdown_copy"], "off");
}

#[test]
fn visible_chat_ids_use_the_visibility_predicate_not_the_stored_list() {
    let a = agent(AgentMode::Read, false);
    let now = moscow(T0);
    let deleted = a.store.create_chat("Удалённый", None, &now).unwrap().id;
    raw(a.dir.path())
        .execute("UPDATE chat SET deleted_at = 1 WHERE id = ?1", [&deleted])
        .unwrap();
    // The stored list holds the excluded chat (ticked by a bypass), a deleted one and one that never existed.
    let stored = json!([
        a.work,
        a.excluded,
        deleted,
        "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d"
    ]);
    raw(a.dir.path())
        .execute(
            "UPDATE app_setting SET value = ?1 WHERE key = 'mcp_chats'",
            [stored.to_string()],
        )
        .unwrap();
    let reply = call(&a.store, "get_settings", json!({}), T0).unwrap();
    assert_eq!(reply["agent_access"]["visible_chat_ids"], json!([a.work]));
}

#[test]
fn off_gets_disabled_and_read_only_gets_the_write_refusal_for_set_setting() {
    let a = agent(AgentMode::Read, false);
    let before = app_settings(a.dir.path());
    assert!(call(&a.store, "get_settings", json!({}), T0).is_ok());
    assert_eq!(
        set(&a.store, json!("text_size"), json!("large")),
        Err(AgentError::ReadOnly)
    );
    a.store.set_agent_enabled(false).unwrap();
    for tool in ["get_settings", "set_setting"] {
        let result = call(
            &a.store,
            tool,
            json!({"key": "text_size", "value": "large"}),
            T0,
        );
        assert_eq!(result, Err(AgentError::Disabled), "{tool}");
    }
    let mut expected = before;
    expected.retain(|(key, _)| key != "mcp_enabled");
    let mut after = app_settings(a.dir.path());
    after.retain(|(key, _)| key != "mcp_enabled");
    assert_eq!(after, expected, "nothing was stored");
    let outcomes: Vec<String> = log_rows(a.dir.path())
        .into_iter()
        .map(|row| row.3)
        .collect();
    assert_eq!(outcomes, ["ok", "read_only", "disabled", "disabled"]);
}

#[test]
fn set_setting_changes_the_text_size_and_nothing_else_and_tells_the_windows() {
    let a = agent(AgentMode::ReadWrite, false);
    let before = app_settings(a.dir.path());
    let arguments = common::agent::args(json!({"key": "text_size", "value": "large"}));
    let reply = a
        .store
        .agent_call(
            &AgentRequest {
                client: "Claude Desktop",
                tool: "set_setting",
                arguments: Some(&arguments),
                inbox_title: "Inbox",
            },
            &moscow(T0),
        )
        .unwrap();
    assert!(reply.settings_changed && !reply.changed, "{reply:?}");
    let body: Value = serde_json::from_str(&reply.text).unwrap();
    assert_eq!(body["text_size"], "large");
    assert_eq!(body["writable"], json!(["text_size"]));
    let mut after = app_settings(a.dir.path());
    assert!(after.contains(&("text_size".into(), "large".into())));
    after.retain(|(key, _)| key != "text_size");
    assert_eq!(after, before, "no other setting moved");
    assert_eq!(
        a.store.get_settings().unwrap().text_size,
        kinige_domain::TextSize::Large
    );
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "set_setting");
    assert_eq!(rows[0].2, r#"{"key":"text_size","value":"large"}"#);
    assert_eq!((rows[0].3.as_str(), rows[0].4), ("ok", 0));
    // And back.
    set(&a.store, json!("text_size"), json!("normal")).unwrap();
    assert_eq!(
        a.store.get_settings().unwrap().text_size,
        kinige_domain::TextSize::Normal
    );
}

#[test]
fn every_name_only_the_user_may_change_is_refused_and_the_table_is_untouched() {
    let a = agent(AgentMode::ReadWrite, false);
    let before = app_settings(a.dir.path());
    let mut refused = 0;
    for key in SettingKey::ALL.into_iter().filter(|key| !key.writable()) {
        for value in [
            json!("large"),
            json!("MARKER-value"),
            json!(true),
            json!(["x"]),
            Value::Null,
        ] {
            let result = set(&a.store, json!(key.name()), value);
            assert_eq!(result, Err(AgentError::NotWritable), "{}", key.name());
            refused += 1;
        }
    }
    assert_eq!(AgentError::NotWritable.code(), "setting_not_writable");
    assert_eq!(app_settings(a.dir.path()), before);
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), refused, "one row per call");
    for row in &rows {
        assert_eq!(row.3, "not_writable");
        assert!(
            !row.2.contains("MARKER") && !row.2.contains("value"),
            "{}",
            row.2
        );
    }
    assert_eq!(rows[0].2, r#"{"key":"shortcuts.capture"}"#);
}

#[test]
fn raw_key_names_and_variants_and_wrong_values_are_invalid_and_write_nothing() {
    let a = agent(AgentMode::ReadWrite, false);
    let before = app_settings(a.dir.path());
    let names = [
        "mcp_enabled",
        "mcp_mode",
        "mcp_chats",
        "mirror_dir",
        "shortcut_capture",
        "shortcut_clipboard",
        "settings_file_hash",
        "sidebar_collapsed",
        "recent_emoji",
        "forest_view",
        "forest_mode",
        "on_this_day_dismissed",
        "search_index_version",
        "Text_Size",
        "TEXT_SIZE",
        " text_size",
        "text_size ",
        "text_size\n",
        "textSize",
        "text-size",
        "shortcuts",
        "agent_access",
        "mirror",
        "",
        MARKER,
    ];
    let mut calls = 0;
    for name in names {
        let result = set(&a.store, json!(name), json!("large"));
        assert!(
            matches!(result, Err(AgentError::Invalid(_))),
            "{name:?}: {result:?}"
        );
        calls += 1;
    }
    for value in [
        json!("huge"),
        json!("Large"),
        json!(" large"),
        json!(""),
        json!(1),
        json!(true),
        json!(["large"]),
        json!({"a": 1}),
        json!(MARKER),
        Value::Null,
    ] {
        let result = set(&a.store, json!("text_size"), value.clone());
        assert!(
            matches!(result, Err(AgentError::Invalid(_))),
            "{value}: {result:?}"
        );
        calls += 1;
    }
    assert_eq!(app_settings(a.dir.path()), before);
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), calls);
    for row in &rows {
        assert_eq!(row.3, "invalid_argument");
        assert!(
            row.2 == r#"{"key":"other"}"# || row.2 == r#"{"key":"text_size"}"#,
            "{}",
            row.2
        );
    }
}

#[test]
fn set_setting_survives_garbage_arguments() {
    let a = agent(AgentMode::ReadWrite, false);
    let before = app_settings(a.dir.path());
    let big = Value::String("x".repeat(1_048_576));
    let cases = [
        json!({}),
        json!({"value": "large"}),
        json!({"key": null, "value": "large"}),
        json!({"key": ["text_size"], "value": "large"}),
        json!({"key": {"a": 1}, "value": "large"}),
        json!({"key": 5, "value": "large"}),
        json!({"key": "text_size"}),
        json!({"key": "text_size", "value": {"a": 1}}),
        json!({"key": "text_size", "value": ["large"]}),
        json!({"key": big, "value": "large"}),
        json!({"key": "text_size", "value": big}),
        json!({"key": "shortcuts.capture", "value": big}),
        json!([]),
    ];
    for arguments in cases {
        let map: Map<String, Value> = arguments.as_object().cloned().unwrap_or_default();
        let result = a.store.agent_call(
            &AgentRequest {
                client: "Fuzz",
                tool: "set_setting",
                arguments: Some(&map),
                inbox_title: "Inbox",
            },
            &moscow(T0),
        );
        assert!(
            matches!(
                result,
                Err(AgentError::Invalid(_) | AgentError::NotWritable)
            ),
            "{result:?}"
        );
    }
    assert_eq!(app_settings(a.dir.path()), before);
    for row in log_rows(a.dir.path()) {
        assert!(row.2.len() < 64, "a refused value never reaches the log");
    }
}

#[test]
fn a_failing_log_row_leaves_the_text_size_alone() {
    let a = agent(AgentMode::ReadWrite, false);
    raw(a.dir.path())
        .execute_batch(
            "CREATE TRIGGER mcp_log_broken BEFORE INSERT ON mcp_log BEGIN SELECT RAISE(ABORT, 'log is broken'); END;",
        )
        .unwrap();
    let before = app_settings(a.dir.path());
    assert!(set(&a.store, json!("text_size"), json!("large")).is_err());
    assert_eq!(app_settings(a.dir.path()), before);
    assert_eq!(
        a.store.get_settings().unwrap().text_size,
        kinige_domain::TextSize::Normal
    );
}

#[test]
fn switching_access_off_before_the_save_stores_nothing_and_logs_one_refusal() {
    let a = agent(AgentMode::ReadWrite, false);
    let before = app_settings(a.dir.path());
    // The first check (outside the transaction) passes; then access goes off, as if from the settings window.
    let store = a.store.clone();
    a.store.set_agent_gap_hook_for_test(Some(Arc::new(move || {
        store.set_agent_enabled(false).unwrap();
    })));
    let result = set(&a.store, json!("text_size"), json!("large"));
    a.store.set_agent_gap_hook_for_test(None);
    assert_eq!(result, Err(AgentError::Disabled));
    let mut after = app_settings(a.dir.path());
    let mut expected = before;
    after.retain(|(key, _)| key != "mcp_enabled");
    expected.retain(|(key, _)| key != "mcp_enabled");
    assert_eq!(after, expected, "the text size was not stored");
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), 1, "one refusal row");
    assert_eq!(
        (rows[0].2.as_str(), rows[0].3.as_str()),
        (r#"{"key":"text_size"}"#, "disabled")
    );
}

#[test]
fn switching_the_mode_to_read_before_the_save_is_refused_too() {
    let a = agent(AgentMode::ReadWrite, false);
    let store = a.store.clone();
    a.store.set_agent_gap_hook_for_test(Some(Arc::new(move || {
        store.set_agent_mode(AgentMode::Read).unwrap();
    })));
    let result = set(&a.store, json!("text_size"), json!("large"));
    a.store.set_agent_gap_hook_for_test(None);
    assert_eq!(result, Err(AgentError::ReadOnly));
    assert_eq!(
        a.store.get_settings().unwrap().text_size,
        kinige_domain::TextSize::Normal
    );
}
