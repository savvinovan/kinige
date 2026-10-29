//! Пункты `- [ ]` по настоящему сокету MCP (слайс 4a, приёмка): схема `set_task_done` называет `item`, `get_entry` отдаёт
//! `tasks` с номерами, отметка пункта доходит до записи, мусорные `item` — `invalid_argument`.

mod common;

use common::mcp::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Now;
use serde_json::{Value, json};

const BODY: &str =
    "Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [ ] Почитать книгу";

#[test]
fn an_agent_can_mark_one_checklist_item_by_the_number_it_read() {
    let mcp = mcp(AgentMode::ReadWrite, &[INBOX_ID]);
    let id = mcp
        .store()
        .send_message(INBOX_ID, BODY, &Now::system())
        .unwrap()
        .id;
    let mut client = mcp.client();
    client.handshake("Claude");
    // Схема: `item` — целое от нуля, и описание объясняет, откуда номера.
    let list = client.request("tools/list", json!({}));
    let tool = list["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "set_task_done")
        .unwrap()
        .clone();
    assert_eq!(tool["inputSchema"]["properties"]["item"]["type"], "integer");
    assert_eq!(tool["inputSchema"]["properties"]["item"]["minimum"], 0);
    assert_eq!(
        tool["inputSchema"]["properties"]["item"]["maximum"],
        1_000_000
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["item_text"]["type"],
        "string"
    );
    // Описание просит передавать текст пункта вместе с номером и говорит, что будет при расхождении.
    let description = tool["description"].as_str().unwrap();
    assert!(description.contains("`item_text`") && description.contains("numbers then shift"));
    assert!(
        tool["description"]
            .as_str()
            .unwrap()
            .contains("checklist item")
    );
    assert!(
        !tool["inputSchema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!("item"))
    );
    // Агент читает запись и находит «Почитать книгу» по номеру.
    let (error, text) = client.call("get_entry", json!({"id": id}));
    assert!(!error, "{text}");
    let entry: Value = serde_json::from_str(&text).unwrap();
    let book = entry["entry"]["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["text"] == "Почитать книгу")
        .unwrap()
        .clone();
    assert_eq!(
        book,
        json!({"item": 2, "done": false, "text": "Почитать книгу"})
    );
    // Отмечает ровно его.
    let (error, text) = client.call(
        "set_task_done",
        json!({"id": id, "done": true, "item": book["item"], "item_text": book["text"]}),
    );
    assert!(!error, "{text}");
    let body = mcp
        .store()
        .list_messages(INBOX_ID, None, 10, &Now::system())
        .unwrap()
        .messages[0]
        .body_md
        .clone();
    assert_eq!(
        body,
        "Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [x] Почитать книгу"
    );
    // Мусорный номер — `invalid_argument`, текст не тронут.
    for junk in [json!(-1), json!("2"), json!(7)] {
        let (error, text) = client.call(
            "set_task_done",
            json!({"id": id, "done": true, "item": junk}),
        );
        assert!(
            error && text.starts_with("invalid_argument"),
            "{junk}: {text}"
        );
    }
}
