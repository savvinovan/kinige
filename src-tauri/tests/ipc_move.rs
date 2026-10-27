//! Перенос записи в другой чат через IPC (решение приёмки): команда `move_message` есть только у главного окна, ответ —
//! запись в новом чате, счётчики и фильтр по тегу следуют за ней, ошибки приходят кодами.

mod common;

use common::*;
use serde_json::json;

const INBOX: &str = kinige_domain::groups::INBOX_ID;

#[test]
fn move_over_ipc_updates_counters_and_the_tag_filter_and_errors_come_back_by_code() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let books = call(
        &main,
        "create_chat",
        json!({"title": "Книги", "icon": null}),
    )
    .unwrap();
    let books_id = books["id"].as_str().unwrap().to_string();
    let sent = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "перечитать #книги"}),
    )
    .unwrap();
    let id = sent["id"].as_str().unwrap();

    let moved = call(&main, "move_message", json!({"id": id, "chatId": books_id})).unwrap();
    assert_eq!(moved["chatId"], json!(books_id));
    assert_eq!(
        moved["journalDate"], sent["journalDate"],
        "день не меняется"
    );

    let list = call(&main, "list_chats", json!({})).unwrap();
    let count = |chat: &str| {
        list["chats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == chat)
            .unwrap()["count"]
            .clone()
    };
    assert_eq!((count(INBOX), count(&books_id)), (json!(0), json!(1)));
    let tagged = call(
        &main,
        "list_messages",
        json!({"chatId": books_id, "tag": "книги"}),
    )
    .unwrap();
    assert_eq!(tagged["messages"].as_array().unwrap().len(), 1);
    let old = call(&main, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(old["messages"], json!([]));

    for (args, code) in [
        (json!({"id": id, "chatId": "нет-такого"}), "chat_not_found"),
        (
            json!({"id": "нет", "chatId": books_id}),
            "message_not_found",
        ),
    ] {
        assert_eq!(
            call(&main, "move_message", args).unwrap_err(),
            json!({"code": code})
        );
    }
}

#[test]
fn the_capture_window_cannot_move_a_record() {
    let fixture = fixture();
    let capture = window(&fixture, "capture");
    let error = call(
        &capture,
        "move_message",
        json!({"id": "x", "chatId": INBOX}),
    )
    .unwrap_err();
    assert!(is_acl_denial(&error), "{error}");
}
