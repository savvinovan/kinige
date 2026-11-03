//! "Panic hunter" at the store level (slice 4a): every tool gets garbage arguments. The result is an error or
//! a normal reply, but never a panic; a database row appears only for calls that passed validation.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::agent::AgentTool;
use kinige_store::AgentError;
use serde_json::{Map, Value, json};

fn garbage() -> Vec<Value> {
    vec![
        Value::Null,
        json!(true),
        json!(0),
        json!(-1),
        json!(1e308),
        json!(u64::MAX),
        json!(""),
        json!(" "),
        json!("\u{0}\u{202e}\u{1b}[31m"),
        json!("x".repeat(1_200_000)),
        json!("😀".repeat(300_000)),
        json!([]),
        json!([[[[[1]]]]]),
        json!({}),
        json!({"a": {"b": null}}),
        json!("2026-02-30"),
        json!("0000-01-01"),
        json!("'; DROP TABLE message; --"),
        json!("\" OR 1=1 --"),
        json!("*"),
        json!("a AND OR NOT ( NEAR"),
        json!("2026-03-14:99999999999999999999:x"),
    ]
}

#[test]
fn every_tool_survives_garbage_arguments_and_only_valid_calls_write() {
    let a = agent(AgentMode::ReadWrite, true);
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
        "extra",
    ];
    let mut created = 0usize;
    for tool in AgentTool::ALL {
        for key in keys {
            for value in garbage() {
                let mut arguments = Map::new();
                arguments.insert(key.to_string(), value);
                let result = a.store.agent_call(
                    &kinige_store::AgentRequest {
                        client: "Fuzz",
                        tool: tool.name(),
                        arguments: Some(&arguments),
                        inbox_title: "Inbox",
                    },
                    &moscow(T0),
                );
                match result {
                    Ok(reply) => {
                        assert!(serde_json::from_str::<Value>(&reply.text).is_ok());
                        if tool == AgentTool::AddEntry {
                            created += 1;
                        }
                    }
                    Err(AgentError::Internal | AgentError::Unavailable) => {
                        panic!("{} {key}: internal error on garbage", tool.name())
                    }
                    Err(_) => {}
                }
            }
        }
        // No arguments at all.
        let _ = a.store.agent_call(
            &kinige_store::AgentRequest {
                client: "Fuzz",
                tool: tool.name(),
                arguments: None,
                inbox_title: "Inbox",
            },
            &moscow(T0),
        );
    }
    // The database is intact; there are exactly as many entries as successful add_entry calls; log: one row per call.
    assert_eq!(count(a.dir.path(), "message") as usize, created);
    // Revisions appear only from successful appends to entries that the garbage `text` has already created.
    assert!(count(a.dir.path(), "message_revision") as usize <= created);
    let calls = AgentTool::ALL.len() * (keys.len() * garbage().len() + 1);
    assert_eq!(count(a.dir.path(), "mcp_log") as usize, calls);
    let ok: String = raw(a.dir.path())
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ok, "ok");
}
