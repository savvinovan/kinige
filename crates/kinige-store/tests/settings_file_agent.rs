//! Доступ ИИ-агентов из файла настроек (слайс 4b, ADR-0010): сопоставление чатов, строка журнала, подпись по инструменту.

mod common;

use common::settings_file::*;
use common::*;
use kinige_domain::{AgentMode, KeyIssueKind};
use serde_json::json;

#[test]
fn agent_access_from_the_file_resolves_chats_logs_one_row_and_never_logs_titles() {
    let rig = rig();
    let (work, diary) = chats(&rig.store, rig.dir.path());
    rig.write(
        &json!({"agent_access": {"enabled": true, "mode": "read_write", "chats": [
            {"id": work, "title": "Старое название"},
            {"title": "  Inbox  "},
            {"title": "Дневник"},
            {"id": diary}
        ]}})
        .to_string(),
    );
    let access = rig.store.agent_access().unwrap();
    assert!(access.enabled);
    assert_eq!(access.mode, AgentMode::ReadWrite);
    let mut visible: Vec<&str> = access
        .chats
        .iter()
        .filter(|c| c.visible)
        .map(|c| c.id.as_str())
        .collect();
    visible.sort();
    let mut expected = vec![work.as_str(), kinige_domain::groups::INBOX_ID];
    expected.sort();
    assert_eq!(visible, expected);
    let issues = rig.file.state().issues;
    assert_eq!(
        issues
            .iter()
            .map(|i| (i.key.as_str(), i.kind))
            .collect::<Vec<_>>(),
        vec![
            ("agent_access.chats[2]", KeyIssueKind::ChatExcluded),
            ("agent_access.chats[3]", KeyIssueKind::ChatExcluded)
        ]
    );
    assert!(rig.file.state().changed.access.is_some());

    let rows = rig.log_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "settings.json");
    assert_eq!(rows[0].1, "settings_file");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&rows[0].2).unwrap(),
        json!({"enabled": true, "mode": "read_write", "chats": 2})
    );
    assert!(!rows[0].2.contains("Работа") && !rows[0].2.contains("Inbox"));

    // Тот же доступ, другой ключ файла: строки журнала не прибавляется.
    let mut doc = rig.json();
    doc["text_size"] = json!("large");
    rig.write(&doc.to_string());
    assert_eq!(rig.log_rows().len(), 1);
    // Окно меняет доступ — строки «settings_file» нет.
    rig.file
        .edit(|store| store.set_agent_enabled(false))
        .unwrap()
        .unwrap();
    assert_eq!(rig.log_rows().len(), 1);
    assert_eq!(rig.json()["agent_access"]["enabled"], false);
}

#[test]
fn inbox_is_matched_by_title_in_any_language_and_a_user_chat_named_inbox_makes_it_ambiguous() {
    let rig = rig();
    rig.write(r#"{"agent_access":{"chats":[{"title":"Inbox"}]}}"#);
    assert!(
        rig.store
            .agent_access()
            .unwrap()
            .chats
            .iter()
            .any(|c| c.is_inbox && c.visible)
    );
    // Файл, который записало приложение, называет Inbox так же.
    rig.file
        .edit(|store| store.set_agent_enabled(true))
        .unwrap()
        .unwrap();
    assert_eq!(
        rig.json()["agent_access"]["chats"],
        json!([{"id": kinige_domain::groups::INBOX_ID, "title": "Inbox"}])
    );
    let twin = rig
        .store
        .create_chat("Inbox", None, &moscow(T0))
        .unwrap()
        .id;
    rig.write(r#"{"agent_access":{"chats":[{"title":"Inbox"}]}}"#);
    let issues = rig.file.state().issues;
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].kind, KeyIssueKind::ChatAmbiguous);
    // Элемент с id не зависит от названия.
    rig.write(&json!({"agent_access":{"chats":[{"id": twin}]}}).to_string());
    assert!(rig.file.state().issues.is_empty());
}

#[test]
fn a_missing_id_is_an_error_even_when_the_title_exists_and_archived_chats_take_part() {
    let rig = rig();
    let (work, _) = chats(&rig.store, rig.dir.path());
    rig.write(r#"{"agent_access":{"chats":[{"id":"no-such-id","title":"Работа"}]}}"#);
    assert_eq!(rig.file.state().issues[0].kind, KeyIssueKind::ChatNotFound);
    assert!(
        rig.store
            .agent_access()
            .unwrap()
            .chats
            .iter()
            .all(|c| !c.visible)
    );
    rig.store
        .set_chat_archived(&work, true, &moscow(T0))
        .unwrap();
    rig.write(r#"{"agent_access":{"chats":[{"title":"Работа"}]}}"#);
    assert!(rig.file.state().issues.is_empty());
    assert!(
        rig.store
            .agent_access()
            .unwrap()
            .chats
            .iter()
            .any(|c| c.id == work && c.visible)
    );
}

#[test]
fn an_mcp_client_cannot_forge_the_settings_file_log_row_by_name_or_by_tool() {
    // Строку «доступ изменён» подписывает инструмент `settings_file`, который пишет только поток файла: клиент MCP может
    // назвать себя `settings.json` сколько угодно, а неизвестный инструмент в журнале всегда `unknown`.
    use common::agent::{agent, call_as};
    use kinige_domain::AgentMode;
    let agent = agent(AgentMode::Read, true);
    let _ = call_as(
        &agent.store,
        "settings.json",
        "search",
        json!({"query": "x"}),
        T0,
    );
    let _ = call_as(
        &agent.store,
        "settings.json",
        "settings_file",
        json!({}),
        T0,
    );
    let conn = raw(agent.dir.path());
    let mut statement = conn
        .prepare("SELECT client, tool FROM mcp_log ORDER BY id")
        .unwrap();
    let rows: Vec<(String, String)> = statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(!rows.is_empty());
    assert!(
        rows.iter()
            .all(|(client, tool)| client == "settings.json" && tool != "settings_file"),
        "{rows:?}"
    );
    assert!(rows.iter().any(|(_, tool)| tool == "unknown"));
}

#[test]
fn stale_ids_in_the_stored_set_do_not_make_an_unchanged_access_look_changed() {
    // В базе остался id чата, которого уже нет: агент его не видит. Файл просит те же «ничего не видно» — строки журнала нет.
    let rig = rig();
    raw(rig.dir.path())
        .execute(
            "INSERT INTO app_setting(key, value) VALUES ('mcp_chats', '[\"ghost\"]') ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [],
        )
        .unwrap();
    rig.write(r#"{"agent_access":{"chats":[]}}"#);
    assert!(rig.log_rows().is_empty(), "{:?}", rig.log_rows());
    assert!(rig.file.state().changed.access.is_none());
}
