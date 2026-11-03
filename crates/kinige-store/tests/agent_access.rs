//! Agent access (ADR-0009, slice 4a): the switch, the mode, the visible set of chats, the uniform `not_found`, the refusal log.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::agent::AgentTool;
use kinige_domain::groups::INBOX_ID;
use kinige_store::AgentError;
use serde_json::{Value, json};

const MISSING: &str = "0a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

/// The minimal valid arguments of every tool (the test fills in the chat and the entry).
fn minimal(tool: AgentTool, chat: &str, entry: &str) -> Value {
    match tool {
        AgentTool::ListChats => json!({}),
        AgentTool::Search => json!({"query": "заметка", "chat_id": chat}),
        AgentTool::GetEntries => json!({"date": "2026-10-21", "chat_id": chat}),
        AgentTool::OnThisDay => json!({"date": "2026-10-21"}),
        AgentTool::GetEntry => json!({"id": entry}),
        AgentTool::AddEntry => json!({"text": "заметка агента", "chat_id": chat}),
        AgentTool::AppendToLastEntry => json!({"text": "дополнение", "chat_id": chat}),
        AgentTool::SetTaskDone => json!({"id": entry, "done": true}),
    }
}

#[test]
fn access_is_off_by_default_and_every_tool_is_refused_and_logged() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for tool in AgentTool::ALL {
        let result = call(&store, tool.name(), minimal(tool, INBOX_ID, MISSING), T0);
        assert_eq!(result, Err(AgentError::Disabled), "{}", tool.name());
    }
    let rows = log_rows(dir.path());
    assert_eq!(rows.len(), AgentTool::ALL.len(), "exactly one row per call");
    assert!(rows.iter().all(|row| row.3 == "disabled" && row.4 == 0));
    assert_eq!(count(dir.path(), "message"), 0);
}

#[test]
fn nothing_is_visible_until_chats_are_marked_even_the_inbox() {
    let a = agent(AgentMode::ReadWrite, false);
    let now = moscow(T0);
    a.store
        .send_message(INBOX_ID, "пароль от банка", &now)
        .unwrap();
    // Inbox is not ticked: the list and the search are empty, a write without chat_id is not_found, no entry is created.
    let chats = call(&a.store, "list_chats", json!({}), T0).unwrap();
    let ids: Vec<&str> = chats["chats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, [a.work.as_str()]);
    assert_eq!(
        entry_ids(&call(&a.store, "search", json!({"query": "пароль"}), T0).unwrap()),
        Vec::<String>::new()
    );
    let before = count(a.dir.path(), "message");
    for tool in ["add_entry", "append_to_last_entry"] {
        assert_eq!(
            call(&a.store, tool, json!({"text": "x"}), T0),
            Err(AgentError::NotFound),
            "{tool}"
        );
    }
    assert_eq!(count(a.dir.path(), "message"), before);
}

#[test]
fn not_found_matrix_closed_excluded_deleted_and_missing_are_indistinguishable() {
    let a = agent(AgentMode::ReadWrite, true);
    let now = moscow(T0);
    // Entries: in a closed chat, in an excluded one, in the trash of a visible chat, a live task in a visible chat.
    let in_secret = a
        .store
        .send_message(&a.secret, "заметка про секрет", &now)
        .unwrap()
        .id;
    let in_excluded = a
        .store
        .send_message(&a.excluded, "заметка дневника", &now)
        .unwrap()
        .id;
    let trashed = a
        .store
        .send_message(&a.work, "заметка в корзине", &now)
        .unwrap()
        .id;
    a.store.delete_message(&trashed, &now).unwrap();
    let deleted_chat = a.store.create_chat("Удалённый", None, &now).unwrap().id;
    raw(a.dir.path())
        .execute(
            "UPDATE chat SET deleted_at = 1 WHERE id = ?1",
            [&deleted_chat],
        )
        .unwrap();

    let chat_cases = [&a.secret, &a.excluded, &deleted_chat, &MISSING.to_string()];
    for chat in chat_cases {
        for tool in [
            AgentTool::Search,
            AgentTool::GetEntries,
            AgentTool::AddEntry,
            AgentTool::AppendToLastEntry,
        ] {
            let result = call(&a.store, tool.name(), minimal(tool, chat, MISSING), T0 + 1);
            assert_eq!(result, Err(AgentError::NotFound), "{} {chat}", tool.name());
        }
    }
    for entry in [&in_secret, &in_excluded, &trashed, &MISSING.to_string()] {
        for tool in [AgentTool::GetEntry, AgentTool::SetTaskDone] {
            let result = call(
                &a.store,
                tool.name(),
                minimal(tool, INBOX_ID, entry),
                T0 + 1,
            );
            assert_eq!(result, Err(AgentError::NotFound), "{} {entry}", tool.name());
        }
    }
    // The reply is the same: the code and the text do not depend on the reason.
    assert_eq!(AgentError::NotFound.code(), "not_found");
    // Nothing is written, every refusal is in the log.
    assert_eq!(count(a.dir.path(), "message"), 3);
    let rows = log_rows(a.dir.path());
    assert!(
        rows.iter().all(|row| row.3 == "not_found" && row.4 == 0),
        "{rows:?}"
    );
    assert_eq!(rows.len(), 4 * 4 + 4 * 2);
}

#[test]
fn closed_chat_title_is_found_neither_by_search_nor_in_the_list() {
    let a = agent(AgentMode::Read, true);
    let search = call(&a.store, "search", json!({"query": "Секретный"}), T0).unwrap();
    assert!(
        search.get("chats").is_none() && entry_ids(&search).is_empty(),
        "{search}"
    );
    let list = call(&a.store, "list_chats", json!({}), T0)
        .unwrap()
        .to_string();
    assert!(
        !list.contains("Секретный") && !list.contains("Дневник"),
        "{list}"
    );
    // A visible chat is found by name (first page), the following pages do not carry the "Chats" row.
    let found = call(&a.store, "search", json!({"query": "Работа"}), T0).unwrap();
    assert_eq!(found["chats"][0]["id"], json!(a.work));
}

#[test]
fn mcp_chats_value_cases() {
    let a = agent(AgentMode::Read, false);
    let set = |value: Option<&str>| {
        let conn = raw(a.dir.path());
        match value {
            Some(text) => conn.execute(
                "INSERT INTO app_setting (key, value) VALUES ('mcp_chats', ?1) ON CONFLICT(key) DO UPDATE SET value = ?1",
                [text],
            ),
            None => conn.execute("DELETE FROM app_setting WHERE key = 'mcp_chats'", []),
        }
        .unwrap();
    };
    // No key: the set is empty, everything is not_found / empty.
    set(None);
    assert_eq!(
        call(&a.store, "get_entry", json!({"id": MISSING}), T0),
        Err(AgentError::NotFound)
    );
    assert_eq!(
        call(&a.store, "list_chats", json!({}), T0).unwrap()["chats"],
        json!([])
    );
    // Ids that are not in the database are ignored; an existing one is visible.
    set(Some(&format!(r#"["{MISSING}","{}"]"#, a.work)));
    let ids = call(&a.store, "list_chats", json!({}), T0).unwrap();
    assert_eq!(ids["chats"].as_array().unwrap().len(), 1);
    // A corrupted value: an error without data, no entry.
    for broken in ["{broken", r#"{"a":"x"}"#, "[1,2]", "null"] {
        set(Some(broken));
        let before = count(a.dir.path(), "message");
        assert_eq!(
            call(&a.store, "list_chats", json!({}), T0),
            Err(AgentError::Internal),
            "{broken}"
        );
        assert_eq!(count(a.dir.path(), "message"), before);
    }
}

#[test]
fn unchecking_between_calls_refuses_the_second_and_disabling_refuses_writes() {
    let a = agent(AgentMode::ReadWrite, true);
    let id = a
        .store
        .send_message(&a.work, "заметка", &moscow(T0))
        .unwrap()
        .id;
    assert!(call(&a.store, "get_entry", json!({"id": id}), T0).is_ok());
    a.store.set_agent_chats(vec![]).unwrap();
    assert_eq!(
        call(&a.store, "get_entry", json!({"id": id}), T0),
        Err(AgentError::NotFound)
    );
    assert_eq!(
        call(
            &a.store,
            "add_entry",
            json!({"text": "x", "chat_id": a.work}),
            T0
        ),
        Err(AgentError::NotFound)
    );
    a.store.set_agent_enabled(false).unwrap();
    assert_eq!(
        call(&a.store, "get_entry", json!({"id": id}), T0),
        Err(AgentError::Disabled)
    );
    assert_eq!(count(a.dir.path(), "message"), 1);
}

#[test]
fn read_only_mode_refuses_every_write_and_writes_nothing() {
    let a = agent(AgentMode::Read, true);
    let id = a
        .store
        .send_message(&a.work, "- [ ] задача", &moscow(T0))
        .unwrap()
        .id;
    for tool in [
        AgentTool::AddEntry,
        AgentTool::AppendToLastEntry,
        AgentTool::SetTaskDone,
    ] {
        assert_eq!(
            call(&a.store, tool.name(), minimal(tool, &a.work, &id), T0),
            Err(AgentError::ReadOnly)
        );
    }
    assert_eq!(count(a.dir.path(), "message"), 1);
    assert_eq!(count(a.dir.path(), "message_revision"), 0);
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|row| row.3 == "read_only"));
    // Reading works in this mode.
    assert!(call(&a.store, "get_entry", json!({"id": id}), T0).is_ok());
}

#[test]
fn set_agent_chats_takes_only_existing_chats_without_ai_exclusion() {
    let a = agent(AgentMode::Read, false);
    use kinige_store::StoreError;
    for bad in [
        vec![a.excluded.clone()],
        vec![MISSING.to_string()],
        vec![a.work.clone(), MISSING.to_string()],
    ] {
        assert!(matches!(
            a.store.set_agent_chats(bad),
            Err(StoreError::ChatNotFound)
        ));
    }
    a.store
        .set_agent_chats(vec![a.work.clone(), a.work.clone(), a.secret.clone()])
        .unwrap();
    let access = a.store.agent_access().unwrap();
    let visible: Vec<&str> = access
        .chats
        .iter()
        .filter(|c| c.visible)
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(visible.len(), 2);
    assert!(
        access
            .chats
            .iter()
            .any(|c| c.ai_excluded && !c.visible && c.id == a.excluded)
    );
    assert!(access.enabled && access.mode == AgentMode::Read);
}

#[test]
fn unknown_tool_and_bad_arguments_are_logged_once_each() {
    let a = agent(AgentMode::ReadWrite, true);
    assert!(matches!(
        call(&a.store, "drop_tables", json!({}), T0),
        Err(AgentError::Invalid(_))
    ));
    assert!(matches!(
        call(&a.store, "search", json!({"query": 5}), T0),
        Err(AgentError::Invalid(_))
    ));
    assert!(matches!(
        call(&a.store, "add_entry", json!({"text": "  "}), T0),
        Err(AgentError::Invalid(_))
    ));
    let rows = log_rows(a.dir.path());
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].1, "unknown");
    assert!(rows.iter().all(|row| row.3 == "invalid_argument"));
}
