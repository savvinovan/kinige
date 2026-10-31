//! Файл настроек: небезопасный файл (ADR-0010), правило «хэш совпал — побеждает база», гонки, обрывы, мусор на входе.

mod common;

use common::settings_file::*;
use common::*;
use kinige_domain::{FileProblemKind, KeyIssueKind, TextSize, UnsafeReason};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

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
fn a_symlink_is_unsafe_it_is_read_through_but_never_overwritten() {
    let rig = rig();
    let elsewhere = tempfile::tempdir().unwrap();
    let real = elsewhere.path().join("real.json");
    fs::write(&real, ACCESS_ON).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(rig.path()).unwrap();
    std::os::unix::fs::symlink(&real, rig.path()).unwrap();
    rig.file.sync_now();
    assert_eq!(rig.file.state().unsafe_reason, Some(UnsafeReason::Symlink));
    assert!(!rig.store.agent_access().unwrap().enabled);
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
    // Правка окна идёт в базу; ссылка и её цель остаются как были.
    rig.file
        .edit(|store| store.set_text_size(TextSize::Normal))
        .unwrap()
        .unwrap();
    assert!(
        fs::symlink_metadata(rig.path())
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_to_string(&real).unwrap(), ACCESS_ON);
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
fn an_edit_of_the_symlink_target_is_seen_by_the_poll() {
    let rig = rig_with(fast_poll);
    let elsewhere = tempfile::tempdir().unwrap();
    let real = elsewhere.path().join("real.json");
    fs::write(&real, r#"{"text_size":"normal"}"#).unwrap();
    fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
    fs::remove_file(rig.path()).unwrap();
    std::os::unix::fs::symlink(&real, rig.path()).unwrap();
    wait_until("ссылка замечена", || {
        rig.file.state().unsafe_reason == Some(UnsafeReason::Symlink)
    });
    fs::write(&real, r#"{"text_size":"large","pad":"x"}"#).unwrap();
    wait_until(
        "правка цели ссылки применена",
        || rig.store.get_settings().unwrap().text_size == TextSize::Large,
    );
}

#[test]
fn a_missing_file_in_a_loose_directory_is_created_and_the_directory_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let loose = root.path().join("loose");
    fs::create_dir(&loose).unwrap();
    fs::set_permissions(&loose, fs::Permissions::from_mode(0o775)).unwrap();
    let store = kinige_store::Store::open_at(
        kinige_store::StoreOptions::new(loose.join("kinige.db"), None),
        &moscow(T0),
    )
    .unwrap();
    let changes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let file = kinige_store::SettingsFile::start(store, loose.clone(), options_for(&changes));
    assert!(loose.join("settings.json").exists());
    assert_eq!(file.state().unsafe_reason, Some(UnsafeReason::Directory));
}

#[test]
fn a_fifo_in_place_of_the_file_is_unsafe_and_never_blocks_or_reads() {
    let rig = rig();
    fs::remove_file(rig.path()).unwrap();
    let made = Command::new("mkfifo").arg(rig.path()).status().unwrap();
    assert!(made.success());
    let (done, wait) = std::sync::mpsc::channel();
    let handle = std::thread::spawn({
        let state = rig.file.state();
        move || {
            let _ = done.send(state);
        }
    });
    rig.file.sync_now();
    wait.recv_timeout(Duration::from_secs(5)).unwrap();
    handle.join().unwrap();
    assert_eq!(
        rig.file.state().unsafe_reason,
        Some(UnsafeReason::NotRegular)
    );
    // Окно по-прежнему работает, канал остался каналом.
    rig.file
        .edit(|store| store.set_text_size(TextSize::Large))
        .unwrap()
        .unwrap();
    assert!(!fs::metadata(rig.path()).unwrap().is_file());
}

#[test]
fn a_file_over_one_mebibyte_non_utf8_or_with_a_bom_is_a_problem_and_changes_nothing() {
    let rig = rig();
    let big = format!(
        "{{\"text_size\":\"large\",\"pad\":\"{}\"}}",
        "x".repeat(1024 * 1024)
    );
    let cases: [(Vec<u8>, FileProblemKind); 5] = [
        (big.into_bytes(), FileProblemKind::TooLarge),
        (vec![b'{', 0xFF, 0xFE, b'}'], FileProblemKind::NotUtf8),
        (
            b"\xEF\xBB\xBF{\"text_size\":\"large\"}".to_vec(),
            FileProblemKind::Bom,
        ),
        (b"[1,2,3]".to_vec(), FileProblemKind::NotObject),
        (b"".to_vec(), FileProblemKind::Parse),
    ];
    for (bytes, kind) in cases {
        write_file(rig.dir.path(), &bytes, 0o600);
        rig.file.sync_now();
        assert_eq!(
            rig.file.state().problem.map(|p| p.kind),
            Some(kind),
            "{kind:?}"
        );
        assert_eq!(
            rig.store.get_settings().unwrap().text_size,
            TextSize::Normal
        );
    }
    // Ровно 1 МиБ — ещё в пределах.
    let pad = "x".repeat(1024 * 1024 - 40);
    let text = format!("{{\"text_size\":\"large\",\"p\":\"{pad}\"}}");
    assert!(text.len() <= 1024 * 1024);
    rig.write(&text);
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
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

#[test]
fn abandoned_temp_files_are_removed_at_start() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let stale = dir.path().join(".settings.json.deadbeef.tmp");
    fs::write(&stale, b"{").unwrap();
    let unrelated = dir.path().join(".other.tmp");
    fs::write(&unrelated, b"{").unwrap();
    let rig = start_in(dir, store, |_| {});
    assert!(!stale.exists());
    assert!(unrelated.exists());
    drop(rig);
}

#[test]
fn a_new_data_directory_is_private_and_an_existing_one_is_left_alone() {
    let root = tempfile::tempdir().unwrap();
    let fresh = root.path().join("a").join("b");
    let store = kinige_store::Store::open_at(
        kinige_store::StoreOptions::new(fresh.join("kinige.db"), None),
        &moscow(T0),
    )
    .unwrap();
    assert_eq!(mode_of(&fresh), 0o700);
    drop(store);
    let loose = root.path().join("loose");
    fs::create_dir(&loose).unwrap();
    fs::set_permissions(&loose, fs::Permissions::from_mode(0o775)).unwrap();
    let store = kinige_store::Store::open_at(
        kinige_store::StoreOptions::new(loose.join("kinige.db"), None),
        &moscow(T0),
    )
    .unwrap();
    assert_eq!(mode_of(&loose), 0o775);
    // Каталог, открытый группе: доступ агента и папка копии из файла не применяются.
    let changes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    write_file(&loose, ACCESS_ON.as_bytes(), 0o600);
    let file =
        kinige_store::SettingsFile::start(store.clone(), loose.clone(), options_for(&changes));
    assert_eq!(file.state().unsafe_reason, Some(UnsafeReason::Directory));
    assert!(!store.agent_access().unwrap().enabled);
    assert_eq!(store.get_settings().unwrap().text_size, TextSize::Large);
}

#[test]
fn garbage_files_only_produce_errors_and_never_touch_the_database() {
    let rig = rig();
    let snapshot = |rig: &Rig| -> Vec<(String, String)> {
        let conn = raw(rig.dir.path());
        let mut statement = conn
            .prepare(
                "SELECT key, value FROM app_setting WHERE key <> 'settings_file_hash' ORDER BY key",
            )
            .unwrap();
        statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let before = snapshot(&rig);
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for round in 0..150 {
        let len = (next() % 600) as usize;
        let bytes: Vec<u8> = (0..len).map(|_| next() as u8).collect();
        write_file(rig.dir.path(), &bytes, 0o600);
        rig.file.sync_now();
        let state = rig.file.state();
        assert!(
            state.problem.is_some(),
            "раунд {round}: случайные байты не JSON-объект"
        );
    }
    assert_eq!(snapshot(&rig), before);
    assert!(rig.log_rows().is_empty());
}
