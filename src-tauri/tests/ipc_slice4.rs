//! Сценарии слайса 4 через IPC на mock-рантайме с настоящими capabilities: импорт из Telegram, поиск, страницы ленты
//! вокруг записи и новее. Новые команды есть только у главного окна; набор окна захвата не изменился.

mod common;

use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::telegram::parse_export;
use kinige_lib::CAPTURE_COMMANDS;
use kinige_lib::import::{IMPORT_PROGRESS, ImportState, open_with, read_export};
use kinige_store::Store;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{Listener, Manager};

const INBOX: &str = kinige_domain::groups::INBOX_ID;

/// Команды слайса 4. Окну захвата — отказ на каждую.
const SLICE_FOUR: &[&str] = &[
    "import_telegram_open",
    "import_telegram_run",
    "import_telegram_stop",
    "import_telegram_close",
    "search_messages",
];

/// «Избранное» из `n` сообщений: по секунде, текст `запись {i} книга`.
fn saved(n: u64) -> kinige_domain::telegram::TgExport {
    let messages: Vec<String> = (1..=n)
        .map(|i| {
            format!(
                r#"{{"id":{i},"type":"message","date_unixtime":"{}","from_id":"user1","text":"запись {i} книга","text_entities":[{{"type":"plain","text":"запись {i} книга"}}]}}"#,
                1_552_035_600 + i
            )
        })
        .collect();
    parse_export(&format!(
        r#"{{"type":"saved_messages","id":1,"messages":[{}]}}"#,
        messages.join(",")
    ))
    .unwrap()
}

#[test]
fn slice_four_commands_are_main_only_and_the_capture_set_is_unchanged() {
    assert_eq!(
        CAPTURE_COMMANDS,
        [
            "capture_state",
            "capture_send",
            "capture_append",
            "capture_save_draft",
            "capture_hide",
            "capture_resize",
            "capture_add_recent_emoji"
        ],
        "у окна захвата по-прежнему семь команд"
    );
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    for command in SLICE_FOUR {
        assert!(kinige_lib::MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        let error = call(&capture, command, args_for(command))
            .expect_err(&format!("{command} недоступна окну захвата"));
        assert!(is_acl_denial(&error), "{command}: {error}");
        if !NEEDS_NATIVE_UI.contains(command)
            && let Err(error) = call(&main, command, args_for(command))
        {
            assert!(!is_acl_denial(&error), "{command}: {error}");
        }
    }
}

#[test]
fn run_with_a_foreign_or_forgotten_session_is_expired() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let store = fixture.app.state::<Store>();
    let state = fixture.app.state::<ImportState>();
    let opened = open_with(&store, &state, saved(3)).unwrap();
    // Чужой номер.
    let error = call(
        &main,
        "import_telegram_run",
        json!({"session": opened.session + 41, "chatIds": ["1"]}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "import_session_expired"}));
    // Забытая: окно импорта закрыто.
    call(&main, "import_telegram_close", json!({})).unwrap();
    let error = call(
        &main,
        "import_telegram_run",
        json!({"session": opened.session, "chatIds": ["1"]}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "import_session_expired"}));
    // Новая сессия заменяет прежнюю.
    let first = open_with(&store, &state, saved(1)).unwrap();
    let second = open_with(&store, &state, saved(2)).unwrap();
    assert_ne!(first.session, second.session);
    let error = call(
        &main,
        "import_telegram_run",
        json!({"session": first.session, "chatIds": ["1"]}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "import_session_expired"}));
}

#[test]
fn a_second_run_during_the_first_is_refused() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let store = fixture.app.state::<Store>();
    let state = fixture.app.state::<ImportState>();
    let opened = open_with(&store, &state, saved(3)).unwrap();
    // Первый импорт «идёт»: его защёлку держит тест.
    let running = state.begin(opened.session).unwrap();
    let error = call(
        &main,
        "import_telegram_run",
        json!({"session": opened.session, "chatIds": ["1"]}),
    )
    .unwrap_err();
    assert_eq!(error, json!({"code": "import_in_progress"}));
    assert!(state.is_running());
    drop(running);
    assert!(!state.is_running());
    let report = call(
        &main,
        "import_telegram_run",
        json!({"session": opened.session, "chatIds": ["1"]}),
    )
    .unwrap();
    assert_eq!(report["imported"], 3);
}

#[test]
fn import_over_ipc_reports_progress_only_to_main_then_search_and_pages_work() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let to_main = Arc::new(AtomicUsize::new(0));
    let to_capture = Arc::new(AtomicUsize::new(0));
    {
        let counter = to_main.clone();
        main.listen(IMPORT_PROGRESS, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        let counter = to_capture.clone();
        capture.listen(IMPORT_PROGRESS, move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        });
    }
    let store = fixture.app.state::<Store>();
    let state = fixture.app.state::<ImportState>();
    let opened = open_with(&store, &state, saved(1_100)).unwrap();
    assert_eq!(opened.preview.chats.len(), 1);
    assert_eq!(opened.preview.chats[0].total, 1_100);

    let report = call(
        &main,
        "import_telegram_run",
        json!({"session": opened.session, "chatIds": ["1"]}),
    )
    .unwrap();
    assert_eq!(report["imported"], 1_100);
    assert_eq!(report["stopped"], false);
    assert!(
        report["backupName"]
            .as_str()
            .unwrap()
            .starts_with("pre-import-")
    );
    assert_eq!(to_main.load(Ordering::SeqCst), 3, "три пачки — три события");
    assert_eq!(
        to_capture.load(Ordering::SeqCst),
        0,
        "окно захвата прогресса не получает"
    );

    // Поиск словоформы, фильтры и курсор.
    let page = call(
        &main,
        "search_messages",
        json!({"query": "книги", "limit": 40}),
    )
    .unwrap();
    assert_eq!(page["hits"].as_array().unwrap().len(), 40);
    assert!(page["older"].is_object());
    let hit = &page["hits"][0];
    assert_eq!(hit["terms"], json!(["книга"]));
    assert!(hit["snippet"].as_str().unwrap().contains("книга"));
    let next = call(
        &main,
        "search_messages",
        json!({"query": "книги", "limit": 40, "before": page["older"]}),
    )
    .unwrap();
    assert_ne!(next["hits"][0]["message"]["id"], hit["message"]["id"]);
    let none = call(
        &main,
        "search_messages",
        json!({"query": "книги", "filters": {"favorite": true}}),
    )
    .unwrap();
    assert_eq!(none["hits"], json!([]));
    let blank = call(&main, "search_messages", json!({"query": "  "})).unwrap();
    assert_eq!(blank, json!({"chats": [], "hits": [], "older": null}));
    let bad_date = call(
        &main,
        "search_messages",
        json!({"query": "книги", "filters": {"from": "вчера"}}),
    )
    .unwrap_err();
    assert_eq!(bad_date, json!({"code": "invalid_date"}));

    // Страницы ленты: вокруг записи и новее.
    let middle = &page["hits"][20]["message"]["id"];
    let around = call(
        &main,
        "list_messages",
        json!({"chatId": INBOX, "around": middle, "limit": 11}),
    )
    .unwrap();
    let ids: Vec<&Value> = around["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| &m["id"])
        .collect();
    assert_eq!(ids.len(), 11);
    assert!(ids.contains(&middle));
    assert!(around["older"].is_object() && around["newer"].is_object());
    let newer = call(
        &main,
        "list_messages",
        json!({"chatId": INBOX, "after": around["newer"], "limit": 5}),
    )
    .unwrap();
    assert_eq!(newer["messages"].as_array().unwrap().len(), 5);
    let missing = call(
        &main,
        "list_messages",
        json!({"chatId": INBOX, "around": "нет-такой"}),
    )
    .unwrap_err();
    assert_eq!(missing, json!({"code": "message_not_found"}));
}

#[test]
fn stop_over_ipc_without_a_running_import_is_harmless_and_run_resets_it() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let store = fixture.app.state::<Store>();
    let state = fixture.app.state::<ImportState>();
    let opened = open_with(&store, &state, saved(5)).unwrap();
    call(&main, "import_telegram_stop", json!({})).unwrap();
    // Остановка, пришедшая до запуска, запуску не мешает: флаг сбрасывается при старте.
    let report = call(
        &main,
        "import_telegram_run",
        json!({"session": opened.session, "chatIds": ["1"]}),
    )
    .unwrap();
    assert_eq!(report["imported"], 5);
    assert_eq!(report["stopped"], false);
}

#[test]
fn files_are_read_in_rust_and_bad_ones_have_their_own_codes() {
    let dir = tempfile::tempdir().unwrap();
    let write = |name: &str, bytes: &[u8]| {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let good = write(
        "ok.json",
        br#"{"type":"saved_messages","id":1,"messages":[]}"#,
    );
    assert_eq!(read_export(&good).unwrap().chats.len(), 1);
    for (name, bytes) in [
        ("empty.json", &b""[..]),
        ("text.json", b"hello"),
        ("array.json", b"[1,2,3]"),
        ("other.json", br#"{"hello":"world"}"#),
        ("binary.json", &[0xff, 0xfe, 0x00]),
    ] {
        assert_eq!(
            read_export(&write(name, bytes)).unwrap_err(),
            ErrorCode::ImportNotTelegram,
            "{name}"
        );
    }
    assert_eq!(
        read_export(&dir.path().join("нет.json")).unwrap_err(),
        ErrorCode::ImportFileUnreadable
    );
    assert_eq!(
        read_export(dir.path()).unwrap_err(),
        ErrorCode::ImportFileUnreadable,
        "папка — не файл"
    );
}
