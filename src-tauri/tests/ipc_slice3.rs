//! Сценарии слайса 3 через IPC на mock-рантайме с настоящими capabilities: удаление и корзина, чаты, правка, флаги, день,
//! теги, состояние запуска и восстановление. Матрица «окно → команды» — в `ipc.rs`, прочие сценарии — в `ipc_flows.rs`.

mod common;

use common::*;
use kinige_store::Store;
use serde_json::json;
use tauri::Manager;

const INBOX: &str = kinige_domain::groups::INBOX_ID;

fn chat_titles(window: &tauri::WebviewWindow<tauri::test::MockRuntime>) -> Vec<String> {
    call(window, "list_chats", json!({})).unwrap()["chats"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn delete_updates_the_counter_and_errors_come_back_by_code() {
    // Долг слайса 1: «удалить → счётчик → ошибка по коду».
    let fixture = fixture();
    let window = window(&fixture, "main");
    let sent = call(
        &window,
        "send_message",
        json!({"chatId": INBOX, "body": "в корзину"}),
    )
    .unwrap();
    let id = sent["id"].as_str().unwrap();
    call(
        &window,
        "send_message",
        json!({"chatId": INBOX, "body": "остаётся"}),
    )
    .unwrap();
    assert_eq!(
        call(&window, "list_chats", json!({})).unwrap()["chats"][0]["count"],
        2
    );

    call(&window, "delete_message", json!({"id": id})).unwrap();
    let list = call(&window, "list_chats", json!({})).unwrap();
    assert_eq!(list["chats"][0]["count"], 1, "счётчик уменьшился на 1");
    assert_eq!(list["trashCount"], 1);
    let page = call(&window, "list_messages", json!({"chatId": INBOX})).unwrap();
    assert_eq!(page["messages"].as_array().unwrap().len(), 1);

    let trash = call(&window, "list_trash", json!({})).unwrap();
    assert_eq!(trash[0]["message"]["id"], id);
    assert_eq!(trash[0]["chatIsInbox"], true);
    assert_eq!(trash[0]["daysLeft"], 30);

    call(&window, "restore_message", json!({"id": id})).unwrap();
    assert_eq!(
        call(&window, "list_chats", json!({})).unwrap()["chats"][0]["count"],
        2
    );

    // Ошибки — кодами.
    for (command, args, code) in [
        ("delete_message", json!({"id": "нет"}), "message_not_found"),
        ("restore_message", json!({"id": "нет"}), "message_not_found"),
        ("purge_message", json!({"id": id}), "not_in_trash"),
        ("purge_message", json!({"id": "нет"}), "message_not_found"),
        (
            "edit_message",
            json!({"id": id, "body": "  "}),
            "empty_message",
        ),
        (
            "edit_message",
            json!({"id": "нет", "body": "x"}),
            "message_not_found",
        ),
        (
            "set_message_date",
            json!({"id": id, "journalDate": "2026-02-30"}),
            "invalid_date",
        ),
        (
            "toggle_task_item",
            json!({"id": id, "index": 0, "checked": true}),
            "task_item_stale",
        ),
        (
            "set_message_flag",
            json!({"id": id, "flag": "task_done", "on": true}),
            "internal",
        ),
    ] {
        assert_eq!(
            call(&window, command, args.clone()).unwrap_err(),
            json!({"code": code}),
            "{command} {args}"
        );
    }
    // Неверный вид аргументов (неизвестный флаг) отвергается до обработчика.
    assert!(
        call(
            &window,
            "set_message_flag",
            json!({"id": id, "flag": "pinned", "on": true})
        )
        .is_err()
    );
    // «Удалить навсегда» у записи из корзины: запись исчезает насовсем.
    call(&window, "delete_message", json!({"id": id})).unwrap();
    call(&window, "purge_message", json!({"id": id})).unwrap();
    assert_eq!(call(&window, "list_trash", json!({})).unwrap(), json!([]));
    assert_eq!(
        call(&window, "list_chats", json!({})).unwrap()["trashCount"],
        0
    );
}

#[test]
fn chats_over_ipc_create_rename_pin_archive_and_protect_inbox() {
    let fixture = fixture();
    let window = window(&fixture, "main");

    let books = call(
        &window,
        "create_chat",
        json!({"title": " Книги ", "icon": "📚"}),
    )
    .unwrap();
    assert_eq!(
        (books["title"].as_str(), books["icon"].as_str()),
        (Some("Книги"), Some("📚"))
    );
    let films = call(
        &window,
        "create_chat",
        json!({"title": "Фильмы", "icon": null}),
    )
    .unwrap();
    assert_eq!(films["icon"], "💬");
    assert_eq!(chat_titles(&window), ["", "Книги", "Фильмы"]);

    for (args, code) in [
        (json!({"title": "", "icon": null}), "chat_title_empty"),
        (
            json!({"title": "я".repeat(129), "icon": null}),
            "chat_title_too_long",
        ),
        (
            json!({"title": "ок", "icon": "не эмодзи"}),
            "chat_icon_invalid",
        ),
    ] {
        assert_eq!(
            call(&window, "create_chat", args).unwrap_err(),
            json!({"code": code})
        );
    }

    let id = books["id"].as_str().unwrap();
    let renamed = call(&window, "rename_chat", json!({"id": id, "title": "Проза"})).unwrap();
    assert_eq!(renamed["title"], "Проза");
    let iconed = call(&window, "set_chat_icon", json!({"id": id, "icon": "🎬"})).unwrap();
    assert_eq!(iconed["icon"], "🎬");
    let pinned = call(
        &window,
        "set_chat_pinned",
        json!({"id": id, "pinned": true}),
    )
    .unwrap();
    assert!(pinned["pinnedAt"].is_number());
    assert_eq!(
        chat_titles(&window),
        ["Проза", "", "Фильмы"],
        "закреплённый первым"
    );
    let archived = call(
        &window,
        "set_chat_archived",
        json!({"id": films["id"], "archived": true}),
    )
    .unwrap();
    assert!(archived["archivedAt"].is_number());
    call(
        &window,
        "set_chat_archived",
        json!({"id": films["id"], "archived": false}),
    )
    .unwrap();

    // Inbox: закрепить можно, остальное нельзя.
    call(
        &window,
        "set_chat_pinned",
        json!({"id": INBOX, "pinned": true}),
    )
    .unwrap();
    for (command, args) in [
        ("rename_chat", json!({"id": INBOX, "title": "Другое"})),
        ("set_chat_icon", json!({"id": INBOX, "icon": "🎬"})),
        ("set_chat_archived", json!({"id": INBOX, "archived": true})),
    ] {
        assert_eq!(
            call(&window, command, args).unwrap_err(),
            json!({"code": "inbox_protected"})
        );
    }
    assert_eq!(
        call(&window, "rename_chat", json!({"id": "нет", "title": "X"})).unwrap_err(),
        json!({"code": "chat_not_found"})
    );

    // Запись уходит в выбранный чат; черновик у каждого чата свой.
    call(
        &window,
        "send_message",
        json!({"chatId": id, "body": "Дюна"}),
    )
    .unwrap();
    let list = call(&window, "list_chats", json!({})).unwrap();
    let count_of = |title: &str| {
        list["chats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["title"] == title)
            .unwrap()["count"]
            .clone()
    };
    assert_eq!((count_of("Проза"), count_of("")), (json!(1), json!(0)));
    call(
        &window,
        "save_draft",
        json!({"chatId": id, "body": "черновик книг"}),
    )
    .unwrap();
    assert_eq!(
        call(&window, "get_draft", json!({"chatId": INBOX})).unwrap(),
        ""
    );
    assert_eq!(
        call(&window, "get_draft", json!({"chatId": id})).unwrap(),
        "черновик книг"
    );

    // Настройки сайдбара и недавние эмодзи.
    call(&window, "set_sidebar_collapsed", json!({"collapsed": true})).unwrap();
    assert_eq!(
        call(&window, "add_recent_emoji", json!({"emoji": "📚"})).unwrap(),
        json!(["📚"])
    );
    assert_eq!(
        call(&window, "add_recent_emoji", json!({"emoji": "🎬"})).unwrap(),
        json!(["🎬", "📚"])
    );
    assert_eq!(
        call(&window, "add_recent_emoji", json!({"emoji": ""})).unwrap_err(),
        json!({"code": "chat_icon_invalid"})
    );
    let settings = call(&window, "get_settings", json!({})).unwrap();
    assert_eq!(settings["sidebarCollapsed"], true);
    assert_eq!(settings["recentEmoji"], json!(["🎬", "📚"]));
}

#[test]
fn edit_flags_date_task_items_and_tags_over_ipc() {
    let fixture = fixture();
    let window = window(&fixture, "main");
    let sent = call(
        &window,
        "send_message",
        json!({"chatId": INBOX, "body": "План #работа\n- [ ] первый\n- [ ] второй"}),
    )
    .unwrap();
    let id = sent["id"].as_str().unwrap();

    let edited = call(
        &window,
        "edit_message",
        json!({"id": id, "body": "План #дом\n- [ ] первый\n- [ ] второй"}),
    )
    .unwrap();
    assert_eq!(edited["tags"], json!(["дом"]));
    let toggled = call(
        &window,
        "toggle_task_item",
        json!({"id": id, "index": 1, "checked": true}),
    )
    .unwrap();
    assert_eq!(toggled["bodyMd"], "План #дом\n- [ ] первый\n- [x] второй");
    let task = call(
        &window,
        "set_message_flag",
        json!({"id": id, "flag": "task", "on": true}),
    )
    .unwrap();
    assert_eq!(task["isTask"], true);
    let done = call(
        &window,
        "set_message_flag",
        json!({"id": id, "flag": "task_done", "on": true}),
    )
    .unwrap();
    assert!(done["taskDoneAt"].is_number());
    let fav = call(
        &window,
        "set_message_flag",
        json!({"id": id, "flag": "favorite", "on": true}),
    )
    .unwrap();
    assert!(fav["favoriteAt"].is_number());
    let moved = call(
        &window,
        "set_message_date",
        json!({"id": id, "journalDate": "2099-01-01"}),
    )
    .unwrap();
    assert_eq!(moved["journalDate"], "2099-01-01");

    // Фильтр по тегу: только записи чата с этим тегом.
    call(
        &window,
        "send_message",
        json!({"chatId": INBOX, "body": "без тега"}),
    )
    .unwrap();
    let filtered = call(
        &window,
        "list_messages",
        json!({"chatId": INBOX, "tag": "ДОМ"}),
    )
    .unwrap();
    assert_eq!(filtered["messages"].as_array().unwrap().len(), 1);
    assert_eq!(filtered["messages"][0]["id"], id);
    let none = call(
        &window,
        "list_messages",
        json!({"chatId": INBOX, "tag": "работа"}),
    )
    .unwrap();
    assert_eq!(none["messages"], json!([]), "после правки старого тега нет");
}

#[test]
fn startup_state_reports_recovery_and_refuses_restore_when_the_journal_is_open() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    assert_eq!(
        call(&main, "startup_state", json!({})).unwrap(),
        json!({"needsRecovery": false, "backups": [], "databaseTooNew": false, "corruptionFound": false})
    );
    // «Перезапустить для восстановления» без найденного повреждения (отметки нет) отвергается и отметку не ставит.
    assert_eq!(
        call(&main, "restart_for_recovery", json!({})).unwrap_err(),
        json!({"code": "internal"})
    );
    let db_path = fixture.app.state::<Store>().db_path().to_path_buf();
    assert!(!kinige_store::needs_recovery(&db_path));
    assert_eq!(
        call(
            &main,
            "restore_backup",
            json!({"name": "daily-2026-10-21.db"})
        )
        .unwrap_err(),
        json!({"code": "internal"}),
        "журнал открыт — восстанавливать нельзя"
    );

    // Журнал не открыт: окно получает копии, новые первыми.
    let store = fixture.app.state::<Store>();
    store
        .send_message(INBOX, "из копии", &kinige_store::Now::system())
        .unwrap();
    let report = store.run_maintenance(&kinige_store::Now::system()).unwrap();
    assert!(report.backup_made);
    // Отдельное приложение без открытого хранилища, как при повреждённой базе: только состояние восстановления.
    let again = fixture_without_store(store.db_path());
    let main = window(&again, "main");
    let state = call(&main, "startup_state", json!({})).unwrap();
    assert_eq!(state["needsRecovery"], true);
    let backups = state["backups"].as_array().unwrap();
    assert_eq!(backups.len(), 1);
    assert_eq!(backups[0]["kind"], "daily");
    assert!(backups[0]["name"].as_str().unwrap().starts_with("daily-"));
    // Неизвестное имя копии — отказ, база не тронута.
    assert_eq!(
        call(&main, "restore_backup", json!({"name": "../kinige.db"})).unwrap_err(),
        json!({"code": "backup_not_found"})
    );
}
