//! «Охотник за паникой» для MCP (KINIG-28): каждый инструмент получает мусорные аргументы — не объект, пустые, не тех типов
//! и враждебные по значению. Ответ приходит всегда, на неверные типы это `isError`, а сервер после всего мусора отвечает на
//! обычный вызов.

// Корень интеграционных тестов целиком — тестовый код: `cfg(test)` даёт хелперам вне #[test] те же послабления
// clippy.toml (unwrap/expect/panic/индексация), что и тестам в src/.
#![cfg(test)]

mod common;

use common::mcp::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use serde_json::{Map, Value, json};

const BIG: usize = 300_000;

fn hostile_for(kind: &str) -> Vec<Value> {
    match kind {
        "string" => vec![
            json!(""),
            json!("\u{0}"),
            json!("\u{202e}\u{301}😀"),
            json!("../../../etc/passwd"),
            json!("9999-99-99"),
            json!("'; DROP TABLE message; --"),
            json!("x".repeat(BIG)),
        ],
        "integer" | "number" => vec![
            json!(-1),
            json!(0),
            json!(0.5),
            json!(4_294_967_296_u64),
            json!(i64::MIN),
            json!(u64::MAX),
            json!(1.0e308),
        ],
        "boolean" => vec![json!(true), json!(false)],
        "array" => vec![
            json!([]),
            json!([null, 1, "a"]),
            Value::Array(vec![json!("\u{0}"); 2_000]),
        ],
        _ => vec![json!({}), Value::Null],
    }
}

/// Сырой ответ на `tools/call`: ответ приходит всегда (`request` ждёт его и падает, если сервер замолчал).
fn call_raw(client: &mut Client, tool: &str, arguments: Value) -> Value {
    client.request("tools/call", json!({"name": tool, "arguments": arguments}))
}

/// Отказ: ошибка JSON-RPC (аргументы не разобраны) или результат с `isError`.
fn failed(reply: &Value) -> bool {
    reply.get("error").is_some() || reply["result"]["isError"].as_bool().unwrap_or(false)
}

fn benign_for(kind: &str) -> Value {
    match kind {
        "string" => json!("x"),
        "integer" | "number" => json!(1),
        "boolean" => json!(true),
        "array" => json!([]),
        _ => Value::Null,
    }
}

fn wrong_for(kind: &str) -> Value {
    match kind {
        "string" => json!(5),
        "integer" | "number" => json!("не число"),
        "boolean" => json!("yes"),
        _ => json!("не то"),
    }
}

#[test]
fn every_tool_survives_garbage_arguments_and_the_server_lives_on() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let mut client = mcp.client();
    client.handshake("Hunter");
    let listed = client.request("tools/list", json!({}));
    let tools = listed["result"]["tools"]
        .as_array()
        .expect("список инструментов");
    assert_eq!(tools.len(), 8, "охотник должен видеть все инструменты");

    for tool in tools {
        let name = tool["name"].as_str().expect("имя");
        let schema = &tool["inputSchema"];
        let properties: Map<String, Value> = schema["properties"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let required = schema["required"].as_array().map_or(0, Vec::len);

        // Не объект и пустые аргументы: ответ приходит; если есть обязательные поля, это ошибка.
        for arguments in [json!(5), json!("x"), json!([]), Value::Null, json!({})] {
            let reply = call_raw(&mut client, name, arguments.clone());
            if required > 0 {
                assert!(failed(&reply), "{name}({arguments}): {reply}");
            }
        }
        // Не те типы у каждого поля.
        if required > 0 {
            let wrong: Map<String, Value> = properties
                .iter()
                .map(|(key, spec)| (key.clone(), wrong_for(spec["type"].as_str().unwrap_or(""))))
                .collect();
            let reply = call_raw(&mut client, name, Value::Object(wrong));
            assert!(failed(&reply), "{name}: не те типы приняты: {reply}");
        }
        // Враждебные значения нужного типа, по одному полю при безобидных остальных: ответ приходит, паники нет.
        for (key, spec) in &properties {
            for value in hostile_for(spec["type"].as_str().unwrap_or("")) {
                let mut arguments: Map<String, Value> = properties
                    .iter()
                    .map(|(other, spec)| {
                        (
                            other.clone(),
                            benign_for(spec["type"].as_str().unwrap_or("")),
                        )
                    })
                    .collect();
                arguments.insert(key.clone(), value);
                call_raw(&mut client, name, Value::Object(arguments));
            }
        }
    }

    // Сервер жив, и мусор ничего не записал.
    let (is_error, _) = client.call("list_chats", json!({}));
    assert!(!is_error, "после мусора инструменты работают");
}
