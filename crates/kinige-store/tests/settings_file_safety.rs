//! Файл настроек: небезопасный файл (ADR-0010), правило «хэш совпал — побеждает база», гонки, обрывы, мусор на входе.

mod common;

use common::settings_file::*;
use common::*;
use kinige_domain::{KeyIssueKind, TextSize, UnsafeReason};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

const ACCESS_ON: &str =
    r#"{"text_size":"large","agent_access":{"enabled":true,"mode":"read_write"}}"#;

#[test]
fn a_group_writable_file_does_not_switch_agent_access_on_but_the_rest_applies() {
    let rig = rig();
    rig.write_mode(ACCESS_ON, 0o664);
    assert_eq!(
        rig.file.state().unsafe_reason,
        Some(UnsafeReason::Permissions)
    );
    assert!(!rig.store.agent_access().unwrap().enabled);
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
    let issues = rig.file.state().issues;
    assert!(
        issues
            .iter()
            .any(|i| i.key == "agent_access" && i.kind == KeyIssueKind::UnsafeIgnored)
    );
    assert!(
        rig.log_rows().is_empty(),
        "доступ не менялся — строки журнала нет"
    );
}

#[test]
fn a_world_writable_file_is_unsafe_too_and_the_mirror_folder_is_ignored() {
    let rig = rig();
    let target = tempfile::tempdir().unwrap();
    rig.write_mode(
        &json!({"mirror": {"folder": target.path()}}).to_string(),
        0o666,
    );
    assert_eq!(
        rig.file.state().unsafe_reason,
        Some(UnsafeReason::Permissions)
    );
    assert!(rig.store.get_settings().unwrap().mirror.is_default);
    assert!(
        rig.file
            .state()
            .issues
            .iter()
            .any(|i| i.key == "mirror.folder")
    );
}

#[test]
fn an_unsafe_file_is_not_overwritten_by_a_window_edit() {
    let rig = rig();
    rig.write_mode(ACCESS_ON, 0o664);
    rig.file
        .edit(|store| store.set_text_size(TextSize::Normal))
        .unwrap()
        .unwrap();
    assert_eq!(rig.bytes(), ACCESS_ON.as_bytes());
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        TextSize::Normal
    );
}

#[test]
fn a_stale_enabled_in_an_unsafe_period_never_comes_back_when_permissions_are_fixed() {
    // Последовательность из ревью guardian: файл синхронизирован с enabled=true, chmod 664, в окне выключили доступ
    // (файл не переписан), chmod 600, агент правит только text_size — доступ остаётся выключенным.
    let rig = rig();
    rig.write(r#"{"agent_access":{"enabled":true}}"#);
    assert!(rig.store.agent_access().unwrap().enabled);
    // Окно записало файл целиком, теперь он «синхронизирован».
    rig.file
        .edit(|store| store.set_agent_mode(kinige_domain::AgentMode::Read))
        .unwrap()
        .unwrap();
    assert_eq!(rig.json()["agent_access"]["enabled"], true);
    fs::set_permissions(rig.path(), fs::Permissions::from_mode(0o664)).unwrap();
    rig.file.sync_now();
    assert_eq!(
        rig.file.state().unsafe_reason,
        Some(UnsafeReason::Permissions)
    );
    rig.file
        .edit(|store| store.set_agent_enabled(false))
        .unwrap()
        .unwrap();
    assert_eq!(
        rig.json()["agent_access"]["enabled"],
        true,
        "небезопасный файл не переписан"
    );
    fs::set_permissions(rig.path(), fs::Permissions::from_mode(0o600)).unwrap();
    rig.file.sync_now();
    // Хэш совпал: победила база, файл переписан из неё.
    assert!(!rig.store.agent_access().unwrap().enabled);
    assert_eq!(rig.json()["agent_access"]["enabled"], false);
    // Агент правит только text_size в файле, который видит.
    let mut doc = rig.json();
    doc["text_size"] = json!("large");
    rig.write(&doc.to_string());
    assert!(!rig.store.agent_access().unwrap().enabled);
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
}

#[test]
fn content_written_while_the_file_was_unsafe_is_not_applied_after_chmod() {
    let rig = rig();
    rig.write_mode(ACCESS_ON, 0o666);
    fs::set_permissions(rig.path(), fs::Permissions::from_mode(0o600)).unwrap();
    rig.file.sync_now();
    assert!(
        !rig.store.agent_access().unwrap().enabled,
        "чужая запись не становится доверенной"
    );
    assert_eq!(
        rig.json()["agent_access"]["enabled"],
        false,
        "файл переписан из базы"
    );
}

#[test]
fn after_chmod_a_fresh_save_applies_the_keys_that_were_ignored() {
    // Подсказка в окне: «сделайте файл доступным на запись только вам (chmod 600), затем сохраните файл ещё раз».
    let rig = rig();
    rig.write_mode(ACCESS_ON, 0o666);
    fs::set_permissions(rig.path(), fs::Permissions::from_mode(0o600)).unwrap();
    rig.file.sync_now();
    assert!(!rig.store.agent_access().unwrap().enabled);
    // Повторное сохранение (файл другой по байтам) при безопасных правах применяется целиком.
    rig.write(&format!("{ACCESS_ON}\n"));
    assert!(rig.store.agent_access().unwrap().enabled);
    assert!(rig.file.state().unsafe_reason.is_none());
}

#[test]
fn a_window_edit_between_a_crash_and_a_restart_survives_because_the_hash_matches() {
    // Окно записало в базу, файл остался прежним (обрыв до записи файла): при старте файл совпадает с сохранённым
    // хэшем, значит это не внешняя правка — база выигрывает, файл переписывается.
    let rig = rig();
    rig.store.set_text_size(TextSize::Large).unwrap();
    assert_eq!(rig.json()["text_size"], "normal");
    let Rig {
        dir, store, file, ..
    } = rig;
    drop(file);
    let again = start_in(dir, store, |_| {});
    assert_eq!(again.json()["text_size"], "large");
    assert_eq!(
        again.store.get_settings().unwrap().text_size,
        TextSize::Large
    );
}

#[test]
fn an_external_edit_made_while_the_app_was_off_is_applied_at_start_before_anything_else() {
    let rig = rig();
    let Rig {
        dir, store, file, ..
    } = rig;
    drop(file);
    write_file(
        dir.path(),
        br#"{"text_size":"large","agent_access":{"enabled":true}}"#,
        0o600,
    );
    let again = start_in(dir, store, |_| {});
    // Применено синхронно в `start`: опроса ждать не нужно.
    assert_eq!(
        again.store.get_settings().unwrap().text_size,
        TextSize::Large
    );
    assert!(again.store.agent_access().unwrap().enabled);
    assert_eq!(again.log_rows().len(), 1);
}

#[test]
fn a_file_replaced_between_our_rename_and_the_hash_step_is_still_applied() {
    // Гонка из ревью guardian: хэш считается от записанных байтов, а не от перечитанного файла.
    let dir_cell: Arc<Mutex<Option<std::path::PathBuf>>> = Arc::new(Mutex::new(None));
    let seen = dir_cell.clone();
    let rig = rig_with(move |options| {
        fast_poll(options);
        options.after_rename = Some(Arc::new(move || {
            let Some(dir) = seen.lock().unwrap().clone() else {
                return;
            };
            write_file(&dir, br#"{"text_size":"large"}"#, 0o600);
        }));
    });
    *dir_cell.lock().unwrap() = Some(rig.dir.path().to_path_buf());
    rig.file
        .edit(|store| store.set_agent_mode(kinige_domain::AgentMode::ReadWrite))
        .unwrap()
        .unwrap();
    wait_until("правка агента применена", || {
        rig.store.get_settings().unwrap().text_size == TextSize::Large
    });
}
