//! Файл настроек на диске: симлинк, канал вместо файла, размер, кодировка, брошенные временные файлы, каталог данных, мусор
//! на входе (ADR-0010). Правила небезопасных прав и «хэш совпал» — в `settings_file_safety.rs`.

mod common;

use common::settings_file::*;
use common::*;
use kinige_domain::{FileProblemKind, TextSize, UnsafeReason};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

const ACCESS_ON: &str =
    r#"{"text_size":"large","agent_access":{"enabled":true,"mode":"read_write"}}"#;

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
