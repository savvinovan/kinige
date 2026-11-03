//! The "panic hunter" for IPC (KINIG-28): every command of every window gets garbage parameters -- empty, of the wrong type
//! and hostile by value (huge strings, NUL, negative and huge numbers, nesting). A reply always comes, and for
//! wrong types it is an error. A panic in a command would cut the reply, and the test would fail rather than hang.

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use serde_json::{Map, Value, json};

/// A string of 300 KB: more than the limit of any field, but not so much that the test takes minutes.
const BIG: usize = 300_000;

fn hostile_strings() -> Vec<Value> {
    vec![
        json!(""),
        json!(" "),
        json!("\u{0}"),
        json!("a\u{0}b"),
        json!("\u{202e}rtl\u{200f}"),
        json!("\u{301}"),
        json!("е\u{301}😀👨\u{200d}👩\u{200d}👧"),
        json!("../../../etc/passwd"),
        json!("'; DROP TABLE message; --"),
        json!("%s%n{}{0}"),
        json!("9999-99-99"),
        json!("0000-00-00"),
        json!("x".repeat(BIG)),
        json!("я".repeat(BIG / 2)),
    ]
}

fn hostile_numbers() -> Vec<Value> {
    vec![
        json!(-1),
        json!(0),
        json!(0.5),
        json!(4_294_967_296_u64),
        json!(9_007_199_254_740_993_u64),
        json!(i64::MIN),
        json!(u64::MAX),
        json!(1.0e308),
        json!(-1.0e308),
    ]
}

fn hostile_arrays() -> Vec<Value> {
    vec![
        json!([]),
        json!([null]),
        json!([1, "a", null, [], {}]),
        json!(["\u{0}", ""]),
        Value::Array((0..5_000).map(|n| json!(format!("id-{n}"))).collect()),
    ]
}

/// Values of the same JSON kind as `original`, but hostile.
fn hostile_like(original: &Value) -> Vec<Value> {
    match original {
        Value::String(_) => hostile_strings(),
        Value::Number(_) => hostile_numbers(),
        Value::Bool(_) => vec![json!(true), json!(false)],
        Value::Array(_) => hostile_arrays(),
        Value::Object(_) => vec![json!({}), json!({"a": null}), json!({"": ""})],
        Value::Null => vec![Value::Null],
    }
}

/// A value of a different JSON kind than `original`: deserializing the argument must refuse.
fn wrong_kind(original: &Value) -> Value {
    match original {
        Value::String(_) => json!(5),
        Value::Number(_) => json!("не число"),
        Value::Bool(_) => json!("yes"),
        Value::Array(_) => json!("не массив"),
        Value::Object(_) | Value::Null => json!([[]]),
    }
}

/// Copies of `template`, in each of which one field is replaced by a hostile value.
fn hostile_variants(template: &Value) -> Vec<Value> {
    let Some(fields) = template.as_object() else {
        return Vec::new();
    };
    let mut variants = Vec::new();
    for (key, original) in fields {
        for value in hostile_like(original) {
            let mut copy: Map<String, Value> = fields.clone();
            copy.insert(key.clone(), value);
            variants.push(Value::Object(copy));
        }
    }
    variants
}

fn wrong_kinds(template: &Value) -> Value {
    let fields = template.as_object().cloned().unwrap_or_default();
    Value::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key, wrong_kind(&value)))
            .collect(),
    )
}

/// Deep nesting: `serde_json` refuses on parsing (limit 128), it does not overflow the stack.
fn deeply_nested() -> Value {
    let mut value = json!(1);
    for _ in 0..400 {
        value = Value::Array(vec![value]);
    }
    value
}

fn hunt(fixture: &Fixture, label: &str, commands: &[&str]) {
    let window = window(fixture, label);
    for command in commands
        .iter()
        .filter(|command| !NEEDS_NATIVE_UI.contains(command))
    {
        let template = args_for(command);
        // Empty arguments: a reply comes (Ok for commands without parameters, otherwise Err).
        let empty = call(&window, command, json!({}));
        if let Err(error) = &empty {
            assert!(!is_acl_denial(error), "{label}/{command}: {error}");
        }
        // The wrong types: if the command has parameters, this is an error.
        if template
            .as_object()
            .is_some_and(|fields| !fields.is_empty())
        {
            let result = call(&window, command, wrong_kinds(&template));
            assert!(
                result.is_err(),
                "{label}/{command}: wrong types were accepted: {result:?}"
            );
            let nested = call(
                &window,
                command,
                json!({"body": deeply_nested(), "id": deeply_nested()}),
            );
            assert!(nested.is_err(), "{label}/{command}: nesting was accepted");
        }
        // Hostile values of the right types: a reply comes, no panic.
        for variant in hostile_variants(&template) {
            if let Err(error) = call(&window, command, variant) {
                assert!(!is_acl_denial(&error), "{label}/{command}: {error}");
            }
        }
    }
}

#[test]
fn main_window_commands_survive_garbage_parameters() {
    let fixture = fixture();
    hunt(&fixture, "main", MAIN_COMMANDS);
}

#[test]
fn settings_window_commands_survive_garbage_parameters() {
    let fixture = fixture();
    hunt(&fixture, "settings", SETTINGS_COMMANDS);
}

#[test]
fn capture_window_commands_survive_garbage_parameters() {
    let fixture = fixture();
    hunt(&fixture, "capture", CAPTURE_COMMANDS);
}

#[test]
fn the_feed_and_search_survive_hostile_cursors_limits_and_tags() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    let inbox = kinige_domain::groups::INBOX_ID;
    let cursors = [
        json!({"journalDate": "x", "createdAt": 0, "id": ""}),
        json!({"journalDate": "9999-99-99", "createdAt": i64::MIN, "id": "\u{0}"}),
        json!({"journalDate": "", "createdAt": i64::MAX, "id": "я".repeat(1000)}),
    ];
    let limits = [
        json!(0),
        json!(1),
        json!(usize::MAX),
        json!(u64::MAX),
        json!(-1),
    ];
    for cursor in &cursors {
        for key in ["before", "after"] {
            let args = json!({"chatId": inbox, key: cursor});
            if let Err(error) = call(&window, "list_messages", args) {
                assert!(!is_acl_denial(&error), "{error}");
            }
        }
    }
    for limit in &limits {
        let _ = call(
            &window,
            "list_messages",
            json!({"chatId": inbox, "limit": limit}),
        );
    }
    for tag in hostile_strings() {
        let _ = call(
            &window,
            "list_messages",
            json!({"chatId": inbox, "tag": tag}),
        );
    }
    for around in hostile_strings() {
        let _ = call(
            &window,
            "list_messages",
            json!({"chatId": inbox, "around": around}),
        );
    }
    for query in hostile_strings() {
        let _ = call(&window, "search_messages", json!({"query": query}));
    }
    for extra in [
        json!({"limit": 0}),
        json!({"cursor": "x"}),
        json!({"chatId": "\u{0}"}),
    ] {
        let mut args = json!({"query": "проверка"});
        if let (Some(args), Some(extra)) = (args.as_object_mut(), extra.as_object()) {
            args.extend(extra.clone());
        }
        let _ = call(&window, "search_messages", args);
    }
    // The app is alive: a regular call works after all the garbage.
    call(&window, "list_chats", json!({})).expect("the commands work after the garbage");
}

#[test]
fn with_no_journal_open_every_command_answers_without_a_panic() {
    // Recovery mode: the store is not managed. `State<Store>` is not found -- a command error, not a panic.
    let data = tempfile::tempdir().unwrap();
    let fixture = fixture_without_store(&data.path().join("kinige.db"));
    for (label, commands) in [
        ("main", MAIN_COMMANDS),
        ("settings", SETTINGS_COMMANDS),
        ("capture", CAPTURE_COMMANDS),
    ] {
        let window = window(&fixture, label);
        for command in commands
            .iter()
            .filter(|command| !NEEDS_NATIVE_UI.contains(command))
        {
            let _ = call(&window, command, args_for(command));
        }
    }
}

#[test]
fn a_panic_in_the_command_handler_rejects_the_promise_with_internal() {
    use tauri::test::{mock_builder, mock_context, noop_assets};
    use tauri::{WebviewWindowBuilder, ipc::Invoke};

    // A handler that panics on one command is wrapped the same way as the real one in `builder`.
    let handler = kinige_lib::startup::guarded(|invoke: Invoke<tauri::test::MockRuntime>| {
        assert!(
            invoke.message.command() != "boom",
            "check: panic in the handler"
        );
        false
    });
    let app = mock_builder()
        .invoke_handler(handler)
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();

    let error =
        call(&window, "boom", json!({})).expect_err("a panic is an error, not a hanging promise");
    assert_eq!(error["code"], "internal", "{error}");
    // The handler is alive: the next call does not panic (there is no such command -- a tauri error, not an `internal` from a panic).
    let next = call(&window, "other", json!({})).expect_err("no such command");
    assert_ne!(next["code"], "internal", "{next}");
}
