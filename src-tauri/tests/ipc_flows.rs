//! Сценарии через IPC на mock-рантайме с настоящими capabilities: запись и настройки, быстрый захват, сочетания. Матрица
//! «окно → команды» — в `ipc.rs`, сценарии слайса 3 (чаты, корзина, правка, восстановление) — в `ipc_slice3.rs`.

mod common;

use common::*;
use kinige_lib::capture::ENTRIES_CHANGED;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::Listener;

const INBOX: &str = kinige_domain::groups::INBOX_ID;

#[test]
fn send_and_settings_work_end_to_end_over_ipc() {
    let fixture = fixture();
    let window = window(&fixture, "main");

    let list = call(&window, "list_chats", json!({})).unwrap();
    assert_eq!(list["chats"][0]["id"], INBOX);
    assert_eq!(list["chats"][0]["count"], 0);
    assert_eq!(list["trashCount"], 0);

    let sent = call(
        &window,
        "send_message",
        json!({"chatId": INBOX, "body": "Купить **молоко** #дом"}),
    )
    .unwrap();
    assert_eq!(sent["bodyMd"], "Купить **молоко** #дом");
    assert_eq!(sent["tags"], json!(["дом"]));
    assert_eq!(sent["isTask"], false);
    assert_eq!(sent["readOnly"], false);

    let page = call(
        &window,
        "list_messages",
        json!({"chatId": INBOX, "limit": 50}),
    )
    .unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert_eq!(page["older"], Value::Null);
    assert_eq!(
        call(&window, "list_chats", json!({})).unwrap()["chats"][0]["count"],
        1
    );

    let empty = call(
        &window,
        "send_message",
        json!({"chatId": INBOX, "body": "  \n "}),
    )
    .unwrap_err();
    assert_eq!(empty, json!({"code": "empty_message"}));

    let settings_window = self::window(&fixture, "settings");
    call(&settings_window, "set_text_size", json!({"size": "large"})).unwrap();
    let settings = call(&window, "get_settings", json!({})).unwrap();
    assert_eq!(settings["textSize"], "large");
    assert_eq!(settings["mirror"]["isDefault"], true);
    assert_eq!(settings["sidebarCollapsed"], false);
    assert_eq!(settings["recentEmoji"], json!([]));
    assert_eq!(settings["backup"]["count"], 0);

    let language = call(&window, "get_language", json!({})).unwrap();
    assert!(language == "en" || language == "ru");
}

#[test]
fn capture_flow_over_ipc_creates_appends_keeps_the_draft_and_notifies_only_main() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let to_main = Arc::new(AtomicUsize::new(0));
    let to_capture = Arc::new(AtomicUsize::new(0));
    {
        let counter = to_main.clone();
        main.listen(ENTRIES_CHANGED, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let counter = to_capture.clone();
        capture.listen(ENTRIES_CHANGED, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }

    // Пустое состояние: ни контекста, ни цели дописывания, ни черновика, ни ошибки.
    let state = call(&capture, "capture_state", json!({})).unwrap();
    assert_eq!(state["context"], json!([]));
    assert_eq!(state["appendTarget"], Value::Null);
    assert_eq!(state["draft"], "");
    assert_eq!(state["error"], Value::Null);
    assert!(state["language"] == "en" || state["language"] == "ru");
    assert_eq!(state["textSize"], "normal");

    // Черновик окна не виден главному и наоборот.
    call(
        &capture,
        "capture_save_draft",
        json!({"body": "недописано"}),
    )
    .unwrap();
    assert_eq!(
        call(&capture, "capture_state", json!({})).unwrap()["draft"],
        "недописано"
    );
    assert_eq!(call(&main, "get_draft", args_for("get_draft")).unwrap(), "");
    call(&main, "save_draft", args_for("save_draft")).unwrap();
    assert_eq!(
        call(&main, "get_draft", args_for("get_draft")).unwrap(),
        "черновик"
    );
    assert_eq!(
        call(&capture, "capture_state", json!({})).unwrap()["draft"],
        "недописано"
    );

    // Enter: новая запись; главное окно её видит и получает событие; черновик окна очищен, черновик главного цел.
    let created = call(
        &capture,
        "capture_send",
        json!({"body": "Идея: попробовать"}),
    )
    .unwrap();
    assert_eq!(created["action"], "created");
    assert_eq!(created["message"]["bodyMd"], "Идея: попробовать");
    assert_eq!(created["message"]["chatId"], INBOX);
    assert_eq!(to_main.load(Ordering::SeqCst), 1);
    assert_eq!(
        to_capture.load(Ordering::SeqCst),
        0,
        "окно захвата события не получает"
    );
    let page = call(&main, "list_messages", args_for("list_messages")).unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert_eq!(
        call(&main, "get_draft", args_for("get_draft")).unwrap(),
        "черновик"
    );

    // Контекст и цель дописывания.
    let state = call(&capture, "capture_state", json!({})).unwrap();
    assert_eq!(state["draft"], "");
    assert_eq!(state["context"].as_array().unwrap().len(), 1);
    assert_eq!(state["appendTarget"], created["message"]["id"]);

    // ⌘Enter: дописано в ту же запись.
    let appended = call(&capture, "capture_append", json!({"body": "и ещё"})).unwrap();
    assert_eq!(appended["action"], "appended");
    assert_eq!(appended["message"]["id"], created["message"]["id"]);
    assert_eq!(appended["message"]["bodyMd"], "Идея: попробовать\n\nи ещё");
    assert_eq!(to_main.load(Ordering::SeqCst), 2);
    let page = call(&main, "list_messages", args_for("list_messages")).unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);
    assert_eq!(page["messages"][0]["bodyMd"], "Идея: попробовать\n\nи ещё");
    assert_eq!(
        call(&main, "list_chats", json!({})).unwrap()["chats"][0]["count"],
        1
    );

    // Ошибки уходят кодом, событий нет.
    let before = to_main.load(Ordering::SeqCst);
    assert_eq!(
        call(&capture, "capture_send", json!({"body": "  "})).unwrap_err(),
        json!({"code": "empty_message"})
    );
    assert_eq!(
        call(&capture, "capture_append", json!({"body": ""})).unwrap_err(),
        json!({"code": "empty_message"})
    );
    assert_eq!(to_main.load(Ordering::SeqCst), before);

    // Прятать окно, которого в mock-рантайме нет, безопасно.
    call(&capture, "capture_hide", json!({})).unwrap();
}

#[test]
fn capture_window_shares_the_recent_emoji_list_with_the_main_window() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let state = call(&capture, "capture_state", json!({})).unwrap();
    assert_eq!(state["recentEmoji"], json!([]));

    // Выбор в пикере главного окна виден окну захвата при следующем показе, выбор в окне захвата — главному.
    call(&main, "add_recent_emoji", json!({"emoji": "🎬"})).unwrap();
    assert_eq!(
        call(&capture, "capture_state", json!({})).unwrap()["recentEmoji"],
        json!(["🎬"])
    );
    let list = call(&capture, "capture_add_recent_emoji", json!({"emoji": "📚"})).unwrap();
    assert_eq!(list, json!(["📚", "🎬"]));
    assert_eq!(
        call(&main, "get_settings", json!({})).unwrap()["recentEmoji"],
        json!(["📚", "🎬"])
    );
    // Повтор переезжает вперёд; пустое значение отказано (те же правила, что у команды главного окна), список не меняется.
    let list = call(&capture, "capture_add_recent_emoji", json!({"emoji": "🎬"})).unwrap();
    assert_eq!(list, json!(["🎬", "📚"]));
    // Принимаются только последовательности эмодзи; обе команды (окна захвата и главного) отвечают одинаково.
    let too_long_ascii = "a".repeat(33);
    let too_long_emoji = "📚".repeat(11);
    for bad in [
        "",
        "  ",
        "a b",
        "📚 🎬",
        "<b>abc",
        "<b>📚",
        "abc",
        "📚\u{7}",
        "📚\n",
        too_long_ascii.as_str(),
        too_long_emoji.as_str(),
    ] {
        for (window, command) in [
            (&capture, "capture_add_recent_emoji"),
            (&main, "add_recent_emoji"),
        ] {
            let error = call(window, command, json!({"emoji": bad})).expect_err("не эмодзи");
            assert_eq!(error["code"], "chat_icon_invalid", "{command} {bad:?}");
        }
    }
    assert_eq!(
        call(&main, "get_settings", json!({})).unwrap()["recentEmoji"],
        json!(["🎬", "📚"])
    );
    // Команда главного окна окну захвата по-прежнему отказана.
    let error = call(&capture, "add_recent_emoji", json!({"emoji": "📚"})).unwrap_err();
    assert!(is_acl_denial(&error));
}

#[test]
fn shortcuts_over_ipc_show_state_validate_and_roll_back() {
    let fixture = fixture_with_taken(&["Ctrl+Alt+KeyN", "Alt+Cmd+KeyN"]);
    let main = window(&fixture, "main");
    let prefs = window(&fixture, "settings");

    let settings = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(
        settings["shortcuts"],
        json!([
            {"kind": "capture", "accelerator": "Ctrl+Alt+KeyN", "isDefault": true, "state": "taken"},
            {"kind": "clipboard", "accelerator": "Alt+Shift+KeyV", "isDefault": true, "state": "assigned"},
        ]),
        "занятое сочетание видно в настройках, остальное работает"
    );

    // Занято — отказ до сохранения, в настройках прежнее.
    assert_eq!(
        call(
            &prefs,
            "set_shortcut",
            json!({"kind": "capture", "accelerator": "Cmd+Alt+KeyN"})
        )
        .unwrap_err(),
        json!({"code": "shortcut_taken"})
    );
    // Неверное, зарезервированное и совпадающее с другим действием — свои коды.
    for (accelerator, code) in [
        ("Space", "shortcut_invalid"),
        ("Cmd+KeyQ", "shortcut_invalid"),
        ("???", "shortcut_invalid"),
        ("Alt+Shift+KeyV", "shortcut_duplicate"),
    ] {
        assert_eq!(
            call(
                &prefs,
                "set_shortcut",
                json!({"kind": "capture", "accelerator": accelerator})
            )
            .unwrap_err(),
            json!({"code": code}),
            "{accelerator}"
        );
    }
    let after = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(after["shortcuts"][0]["accelerator"], "Ctrl+Alt+KeyN");

    // Свободное сочетание принято и сохранено в каноничном виде; ответ — настройки целиком.
    let changed = call(
        &prefs,
        "set_shortcut",
        json!({"kind": "capture", "accelerator": "Ctrl+Alt+KeyK"}),
    )
    .unwrap();
    assert_eq!(
        changed["shortcuts"][0],
        json!({"kind": "capture", "accelerator": "Ctrl+Alt+KeyK", "isDefault": false, "state": "assigned"})
    );
    // Сброс: умолчание занято другой программой — отказ, действующее остаётся.
    assert_eq!(
        call(&prefs, "reset_shortcut", json!({"kind": "capture"})).unwrap_err(),
        json!({"code": "shortcut_taken"})
    );
    fixture.registrar.taken.lock().unwrap().clear();
    let reset = call(&prefs, "reset_shortcut", json!({"kind": "capture"})).unwrap();
    assert_eq!(
        reset["shortcuts"][0],
        json!({"kind": "capture", "accelerator": "Ctrl+Alt+KeyN", "isDefault": true, "state": "assigned"})
    );

    // Режим записи включается и выключается без ошибок.
    call(&prefs, "shortcut_recording", json!({"active": true})).unwrap();
    call(&prefs, "shortcut_recording", json!({"active": false})).unwrap();
}
