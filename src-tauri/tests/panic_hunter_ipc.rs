//! «Охотник за паникой» для IPC (KINIG-28): каждая команда каждого окна получает мусорные параметры — пустые, не того типа
//! и враждебные по значению (огромные строки, NUL, отрицательные и огромные числа, вложенность). Ответ приходит всегда, а на
//! неверные типы это ошибка. Паника в команде оборвала бы ответ, и тест упал бы, а не зависал.

// Корень интеграционных тестов целиком — тестовый код: `cfg(test)` даёт хелперам вне #[test] те же послабления
// clippy.toml (unwrap/expect/panic/индексация), что и тестам в src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use serde_json::{Map, Value, json};

/// Строка в 300 КБ: больше предела любого поля, но не настолько, чтобы тест шёл минутами.
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

/// Значения того же вида JSON, что у `original`, но враждебные.
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

/// Значение другого вида JSON, чем у `original`: десериализация аргумента обязана отказать.
fn wrong_kind(original: &Value) -> Value {
    match original {
        Value::String(_) => json!(5),
        Value::Number(_) => json!("не число"),
        Value::Bool(_) => json!("yes"),
        Value::Array(_) => json!("не массив"),
        Value::Object(_) | Value::Null => json!([[]]),
    }
}

/// Копии `template`, в каждой из которых одно поле заменено враждебным значением.
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

/// Глубокая вложенность: `serde_json` отказывает на разборе (предел 128), не падает на стеке.
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
        // Пустые аргументы: ответ приходит (Ok у команд без параметров, иначе Err).
        let empty = call(&window, command, json!({}));
        if let Err(error) = &empty {
            assert!(!is_acl_denial(error), "{label}/{command}: {error}");
        }
        // Не те типы: если у команды есть параметры, это ошибка.
        if template
            .as_object()
            .is_some_and(|fields| !fields.is_empty())
        {
            let result = call(&window, command, wrong_kinds(&template));
            assert!(
                result.is_err(),
                "{label}/{command}: не те типы приняты: {result:?}"
            );
            let nested = call(
                &window,
                command,
                json!({"body": deeply_nested(), "id": deeply_nested()}),
            );
            assert!(nested.is_err(), "{label}/{command}: вложенность принята");
        }
        // Враждебные значения нужных типов: ответ приходит, паники нет.
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
    // Приложение живо: обычный вызов после всего мусора работает.
    call(&window, "list_chats", json!({})).expect("после мусора команды работают");
}

#[test]
fn with_no_journal_open_every_command_answers_without_a_panic() {
    // Режим восстановления: хранилище не управляется. `State<Store>` не найден — ошибка команды, а не паника.
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
