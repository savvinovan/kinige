//! Файл настроек `settings.json` (слайс 4b, ADR-0010): создание, внешние правки, правки окна, битый и удалённый файл,
//! ошибки ключей, строка журнала обращений.

mod common;

use common::settings_file::*;
use common::*;
use kinige_domain::{KeyIssueKind, ShortcutKind, TextSize};
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

/// Есть ли в каталоге (рекурсивно) файл дня `.md`.
fn has_day_file(dir: &std::path::Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                has_day_file(&path)
            } else {
                path.extension().is_some_and(|ext| ext == "md")
            }
        })
    })
}

#[test]
fn a_mirror_folder_from_the_file_wakes_the_copy_writer_at_once() {
    // Без пробуждения писатель копии спал бы до следующей записи или перезапуска, и в новой папке дней не было бы.
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    store
        .send_message(
            kinige_domain::groups::INBOX_ID,
            "запись для копии",
            &moscow(T0),
        )
        .unwrap();
    let rig = start_in(dir, store, |_| {});
    let target = tempfile::tempdir().unwrap();
    assert!(!has_day_file(target.path()));
    rig.write(&json!({"mirror": {"folder": target.path()}}).to_string());
    wait_until("файл дня в новой папке", || {
        has_day_file(target.path())
    });
}

#[test]
fn caps_keep_the_error_and_unknown_key_lists_short_whatever_the_file_holds() {
    let rig = rig();
    let junk: Vec<serde_json::Value> = (0..5000)
        .map(|i| json!({"id": format!("ghost-{i}")}))
        .collect();
    let mut doc = serde_json::Map::new();
    for i in 0..5000 {
        doc.insert(format!("unknown_key_{i}"), json!(i));
    }
    doc.insert(
        "agent_access".into(),
        json!({"enabled": true, "chats": junk}),
    );
    rig.write(&serde_json::Value::Object(doc).to_string());
    let state = rig.file.state();
    assert!(state.issues.len() <= 100, "{}", state.issues.len());
    assert_eq!(state.unknown_keys.len(), 100);
    assert_eq!(state.unknown_more, 4900);
}
