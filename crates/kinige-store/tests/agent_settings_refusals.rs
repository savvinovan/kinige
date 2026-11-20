//! `set_setting` refusals (slice 5d, ADR-0022): the six names only the user may change, raw key names, variants and wrong values, and
//! garbage arguments. In every case `app_setting` is byte-identical afterwards and the log gets one row without the agent's text.

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
use std::path::Path;

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

fn set(store: &kinige_store::Store, key: Value, value: Value) -> Result<Value, AgentError> {
    call(
        store,
        "set_setting",
        json!({"key": key, "value": value}),
        T0,
    )
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
