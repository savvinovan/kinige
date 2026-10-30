//! Файл настроек `settings.json` (слайс 4b, ADR-0010): создание, внешние правки, правки окна, битый и удалённый файл,
//! ошибки ключей, строка журнала обращений.

mod common;

use common::settings_file::*;
use common::*;
use kinige_domain::{AgentMode, KeyIssueKind, ShortcutKind, TextSize};
use serde_json::json;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

#[test]
fn a_missing_file_is_created_from_the_state_with_all_keys_and_private_mode() {
    let rig = rig();
    assert_eq!(mode_of(&rig.path()), 0o600);
    assert_eq!(
        rig.json(),
        json!({
            "text_size": "normal",
            "shortcuts": {"capture": "Ctrl+Alt+KeyN", "clipboard": "Alt+Shift+KeyV"},
            // Папка по умолчанию — null: личный путь не попадает в файл.
            "mirror": {"folder": null},
            "agent_access": {"enabled": false, "mode": "read", "chats": []},
        })
    );
    assert_eq!(rig.file.state().path, rig.path().to_string_lossy());
    assert!(rig.file.state().problem.is_none() && rig.file.state().issues.is_empty());
}

#[test]
fn an_external_edit_is_applied_by_the_poll_and_the_file_is_left_as_written() {
    let rig = rig_with(fast_poll);
    let text = "{\n  \"text_size\": \"large\"\n}\n";
    write_file(rig.dir.path(), text.as_bytes(), 0o600);
    // Без `sync_now`: за несколько интервалов опроса.
    wait_until("размер текста из файла", || {
        rig.file.state().changed.appearance.is_some()
    });
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
    assert_eq!(
        rig.bytes(),
        text.as_bytes(),
        "формат пользователя не трогается"
    );
    assert!(rig.changes.load(std::sync::atomic::Ordering::SeqCst) >= 1);
}

#[test]
fn our_own_write_is_not_applied_again() {
    let rig = rig_with(fast_poll);
    let before = rig.changes.load(std::sync::atomic::Ordering::SeqCst);
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    assert_eq!(rig.json()["text_size"], "large");
    std::thread::sleep(Duration::from_millis(200));
    // Одно уведомление за правку окна; опрос свою запись не применяет и не уведомляет.
    assert_eq!(
        rig.changes.load(std::sync::atomic::Ordering::SeqCst),
        before + 1
    );
    assert!(rig.log_rows().is_empty());
    assert!(rig.file.state().changed.appearance.is_none());
}

#[test]
fn a_window_edit_keeps_foreign_keys_and_fills_the_missing_known_ones() {
    let rig = rig();
    rig.write(r#"{"x_mine": {"a": [1, 2]}, "shortcuts": {"extra": 7}, "text_size": "normal"}"#);
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    let doc = rig.json();
    assert_eq!(doc["x_mine"], json!({"a": [1, 2]}));
    assert_eq!(doc["shortcuts"]["extra"], 7);
    assert_eq!(doc["text_size"], "large");
    assert_eq!(doc["shortcuts"]["capture"], "Ctrl+Alt+KeyN");
    assert_eq!(doc["agent_access"]["enabled"], false);
    let mut unknown = rig.file.state().unknown_keys;
    unknown.sort();
    assert_eq!(unknown, vec!["shortcuts.extra", "x_mine"]);
}

#[test]
fn a_window_edit_shortcut_change_lands_in_the_file_key() {
    let rig = rig();
    rig.file
        .edit(|store| store.save_shortcut(ShortcutKind::Capture, "Ctrl+Alt+KeyJ"))
        .unwrap()
        .unwrap();
    assert_eq!(rig.json()["shortcuts"]["capture"], "Ctrl+Alt+KeyJ");
}

#[test]
fn a_broken_file_changes_nothing_is_not_rewritten_and_the_window_edit_goes_to_the_database() {
    let rig = rig();
    rig.write(r#"{"text_size": "#);
    let state = rig.file.state();
    let problem = state.problem.expect("файл не разобран");
    assert_eq!(problem.kind, kinige_domain::FileProblemKind::Parse);
    assert_eq!((problem.line, problem.column), (Some(1), Some(14)));
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        TextSize::Normal
    );

    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    assert_eq!(
        rig.bytes(),
        br#"{"text_size": "#,
        "битый файл не перезаписывается"
    );
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);

    // Файл починили позже правки окна: побеждает последняя правка.
    rig.write(r#"{"text_size": "normal"}"#);
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        TextSize::Normal
    );
    assert!(rig.file.state().problem.is_none());
}

#[test]
fn a_deleted_file_is_recreated_from_the_state_and_settings_are_not_reset() {
    let rig = rig_with(fast_poll);
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    fs::remove_file(rig.path()).unwrap();
    wait_until("файл воссоздан", || rig.path().exists());
    assert_eq!(rig.json()["text_size"], "large");
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
}

#[test]
fn each_key_with_a_wrong_type_or_value_is_an_error_of_that_key_and_the_rest_is_applied() {
    let rig = rig();
    rig.write(
        &json!({
            "text_size": 5,
            "shortcuts": {"capture": "bogus", "clipboard": "Ctrl+Alt+KeyJ", "zzz": 1},
            "mirror": {"folder": 12},
            "agent_access": {"enabled": "yes", "mode": "write", "chats": [5, {}, {"id": "ghost"}]},
            "surprise": null
        })
        .to_string(),
    );
    let state = rig.file.state();
    let mut found: Vec<(String, KeyIssueKind)> = state
        .issues
        .iter()
        .map(|i| (i.key.clone(), i.kind))
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    use KeyIssueKind::*;
    assert_eq!(
        found,
        vec![
            ("agent_access.chats[0]".to_string(), Type),
            ("agent_access.chats[1]".to_string(), Value),
            ("agent_access.chats[2]".to_string(), ChatNotFound),
            ("agent_access.enabled".to_string(), Type),
            ("agent_access.mode".to_string(), Value),
            ("mirror.folder".to_string(), Type),
            ("shortcuts.capture".to_string(), ShortcutInvalid),
            ("text_size".to_string(), Type),
        ]
    );
    assert!(state.unknown_keys.contains(&"surprise".to_string()));
    assert!(state.unknown_keys.contains(&"shortcuts.zzz".to_string()));
    // Применилось годное: сочетание буфера. Остальное прежнее.
    assert_eq!(
        rig.store
            .shortcut_accelerator(ShortcutKind::Clipboard)
            .unwrap(),
        "Ctrl+Alt+KeyJ"
    );
    assert_eq!(
        rig.store
            .shortcut_accelerator(ShortcutKind::Capture)
            .unwrap(),
        "Ctrl+Alt+KeyN"
    );
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        TextSize::Normal
    );
}

#[test]
fn null_means_the_default_and_a_missing_key_means_no_change() {
    let rig = rig();
    rig.write(r#"{"text_size":"large","shortcuts":{"capture":"Ctrl+Alt+KeyJ"}}"#);
    rig.write(r#"{"shortcuts":{"capture":null}}"#);
    // text_size не упомянут — остался large; capture сброшен к умолчанию.
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
    assert_eq!(
        rig.store
            .shortcut_accelerator(ShortcutKind::Capture)
            .unwrap(),
        "Ctrl+Alt+KeyN"
    );
    rig.write(r#"{"text_size":null}"#);
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        TextSize::Normal
    );
}

#[test]
fn a_shortcut_the_system_refuses_is_an_error_of_that_key_and_the_old_one_stays() {
    let rig = rig_with(|options| {
        options.shortcuts = Some(Arc::new(Refusing(vec!["Ctrl+Alt+KeyZ"])));
    });
    rig.write(r#"{"shortcuts":{"capture":"Ctrl+Alt+KeyZ","clipboard":"Ctrl+Alt+KeyJ"}}"#);
    let issues = rig.file.state().issues;
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].key, "shortcuts.capture");
    assert_eq!(issues[0].kind, KeyIssueKind::ShortcutTaken);
    assert_eq!(
        rig.store
            .shortcut_accelerator(ShortcutKind::Capture)
            .unwrap(),
        "Ctrl+Alt+KeyN"
    );
    assert_eq!(
        rig.store
            .shortcut_accelerator(ShortcutKind::Clipboard)
            .unwrap(),
        "Ctrl+Alt+KeyJ"
    );
    // Правка пользователя в файле не стирается.
    assert!(
        String::from_utf8(rig.bytes())
            .unwrap()
            .contains("Ctrl+Alt+KeyZ")
    );
}

#[test]
fn a_shortcut_equal_to_the_other_action_is_a_duplicate_error() {
    let rig = rig();
    rig.write(r#"{"shortcuts":{"capture":"Alt+Shift+KeyV"}}"#);
    let issues = rig.file.state().issues;
    assert_eq!(issues[0].kind, KeyIssueKind::ShortcutDuplicate);
    assert_eq!(
        rig.store
            .shortcut_accelerator(ShortcutKind::Capture)
            .unwrap(),
        "Ctrl+Alt+KeyN"
    );
}

#[test]
fn the_mirror_folder_must_be_absolute_existing_and_outside_the_data_directory() {
    let rig = rig();
    let target = tempfile::tempdir().unwrap();
    let inside = rig.dir.path().join("inner");
    fs::create_dir(&inside).unwrap();
    for (folder, kind) in [
        ("relative/path".to_string(), KeyIssueKind::FolderNotAbsolute),
        ("".to_string(), KeyIssueKind::FolderNotAbsolute),
        (
            "/definitely/not/here-4b".to_string(),
            KeyIssueKind::FolderMissing,
        ),
        (
            inside.to_string_lossy().into_owned(),
            KeyIssueKind::FolderInsideData,
        ),
        (
            rig.dir.path().to_string_lossy().into_owned(),
            KeyIssueKind::FolderInsideData,
        ),
    ] {
        rig.write(&json!({"mirror": {"folder": folder}}).to_string());
        let issues = rig.file.state().issues;
        assert_eq!(issues.len(), 1, "{folder}");
        assert_eq!(issues[0].kind, kind, "{folder}");
        assert!(
            rig.store.get_settings().unwrap().mirror.is_default,
            "{folder}"
        );
    }
    rig.write(&json!({"mirror": {"folder": target.path()}}).to_string());
    let mirror = rig.store.get_settings().unwrap().mirror;
    assert_eq!(mirror.folder.as_deref(), target.path().to_str());
    assert!(!mirror.is_default);
    assert!(rig.file.state().changed.mirror.is_some());
    // Окно меняет что-то другое: путь в файле остаётся (он не по умолчанию).
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    assert_eq!(rig.json()["mirror"]["folder"], json!(target.path()));
    // null возвращает папку по умолчанию.
    rig.write(r#"{"mirror":{"folder":null}}"#);
    assert!(rig.store.get_settings().unwrap().mirror.is_default);
}

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
fn a_duplicate_key_takes_the_last_value() {
    let rig = rig();
    rig.write(r#"{"text_size":"large","text_size":"normal"}"#);
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        TextSize::Normal
    );
    rig.write(r#"{"text_size":"normal","text_size":"large"}"#);
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
}
