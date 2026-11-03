//! Restoring from a copy and the settings file (ADR-0010): the copy folder from the file goes to `...-before-restore-...`,
//! agent access from the copy is not carried over, the file is applied on top of the restored database.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::ops::*;
use common::settings_file::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store, backups, restore_backup};
use std::fs;
use std::path::{Path, PathBuf};

const BACKUP_MS: i64 = 1_700_000_000_000;

fn at(ms: i64) -> Now {
    moscow(ms)
}

/// A database with a day copy and an entry; `prepare` configures the store before the copy.
fn database_with_copy(dir: &Path, prepare: impl FnOnce(&Store)) -> String {
    let store = open_without_mirror(dir);
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    prepare(&store);
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir);
    backups(&db_path(dir))[0].name.clone()
}

fn restore(dir: &Path, name: &str) {
    restore_backup(&db_path(dir), name, BACKUP_MS, Some(&mirror_dir(dir))).unwrap();
}

fn aside(folder: &Path) -> Vec<PathBuf> {
    let name = folder.file_name().unwrap().to_string_lossy().into_owned();
    let prefix = format!("{name}-before-restore-");
    fs::read_dir(folder.parent().unwrap())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&prefix))
        .map(|e| e.path())
        .collect()
}

#[test]
fn the_mirror_folder_named_in_the_file_is_moved_aside_with_its_extra_day() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    // The copy says "default folder" (A), the file says another folder (B) with a day that is not in the copy.
    let elsewhere = tempfile::tempdir().unwrap();
    let folder_b = elsewhere.path().join("Kinige-B");
    let day = folder_b.join("Inbox").join("2026").join("2026-10-22.md");
    fs::create_dir_all(day.parent().unwrap()).unwrap();
    fs::write(&day, "запись после снимка").unwrap();
    write_file(
        dir.path(),
        serde_json::json!({"mirror": {"folder": folder_b}})
            .to_string()
            .as_bytes(),
        0o600,
    );
    restore(dir.path(), &name);
    assert!(
        !folder_b.exists(),
        "the folder from the file was moved aside before the file was applied"
    );
    let moved = aside(&folder_b);
    assert_eq!(moved.len(), 1);
    assert_eq!(
        fs::read_to_string(moved[0].join("Inbox/2026/2026-10-22.md")).unwrap(),
        "запись после снимка"
    );
}

// "Unsafe" here means a file with mode 0666: Windows has no modes (ADR-0010, Windows note).
#[cfg(unix)]
#[test]
fn an_unsafe_file_does_not_make_restore_move_its_mirror_folder() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    let elsewhere = tempfile::tempdir().unwrap();
    let folder_b = elsewhere.path().join("Kinige-B");
    fs::create_dir_all(&folder_b).unwrap();
    write_file(
        dir.path(),
        serde_json::json!({"mirror": {"folder": folder_b}})
            .to_string()
            .as_bytes(),
        0o666,
    );
    restore(dir.path(), &name);
    assert!(folder_b.exists());
    assert!(aside(&folder_b).is_empty());
}

#[test]
fn the_applied_file_after_restore_cannot_wipe_days_written_after_the_snapshot() {
    // The end-to-end review scenario: after a restore the file sets folder B and marks all days for rewriting; the copy writer
    // deletes day files in the folder that are not in the database. Folder B has already been moved aside by then.
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    let elsewhere = tempfile::tempdir().unwrap();
    let folder_b = elsewhere.path().join("Kinige-B");
    let day = folder_b.join("Inbox").join("2026").join("2026-10-22.md");
    fs::create_dir_all(day.parent().unwrap()).unwrap();
    fs::write(&day, "после снимка").unwrap();
    write_file(
        dir.path(),
        serde_json::json!({"mirror": {"folder": folder_b}})
            .to_string()
            .as_bytes(),
        0o600,
    );
    restore(dir.path(), &name);
    // The user recreated folder B (empty): the file sets it as the copy folder, the copy writer writes the days from the database there.
    fs::create_dir_all(&folder_b).unwrap();
    let store = open(dir.path());
    let rig = start_in(dir, store, |_| {});
    let today = folder_b.join("Inbox/2026/2026-10-21.md");
    wait_until("the day from the database in the new folder", || {
        today.exists()
    });
    let moved = aside(&folder_b);
    assert_eq!(
        fs::read_to_string(moved[0].join("Inbox/2026/2026-10-22.md")).unwrap(),
        "после снимка"
    );
    assert!(!folder_b.join("Inbox/2026/2026-10-22.md").exists());
    drop(rig);
}

#[test]
fn access_from_the_copy_is_switched_off_and_an_unsafe_file_cannot_bring_it_back() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |store| {
        store.set_agent_chats(vec![INBOX_ID.to_string()]).unwrap();
        store
            .set_agent_mode(kinige_domain::AgentMode::ReadWrite)
            .unwrap();
        store.set_agent_enabled(true).unwrap();
    });
    // The file is unsafe (0644 + group write) and says "off"; even without the file, access from the copy must not remain.
    write_file(dir.path(), br#"{"agent_access":{"enabled":false}}"#, 0o664);
    restore(dir.path(), &name);
    let store = open(dir.path());
    let access = store.agent_access().unwrap();
    assert!(!access.enabled, "access from the copy is not carried over");
    assert!(access.chats.iter().all(|c| !c.visible));
    let rig = start_in(dir, store, |_| {});
    assert!(!rig.store.agent_access().unwrap().enabled);
}

// The file carries AI-agent access, which is applied on Unix only (ADR-0010, Windows note).
#[cfg(unix)]
#[test]
fn after_restore_a_safe_file_is_applied_over_the_restored_database() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |store| {
        store
            .set_text_size(kinige_domain::TextSize::Normal)
            .unwrap();
    });
    write_file(
        dir.path(),
        br#"{"text_size":"large","agent_access":{"enabled":true,"chats":[{"title":"Inbox"}]}}"#,
        0o600,
    );
    // There is no hash in the copy: the file is applied unconditionally.
    restore(dir.path(), &name);
    let store = open(dir.path());
    let rig = start_in(dir, store, |_| {});
    assert_eq!(
        rig.store.get_settings().unwrap().text_size,
        kinige_domain::TextSize::Large
    );
    assert!(rig.store.agent_access().unwrap().enabled);
}

#[test]
fn a_null_folder_in_the_file_means_the_default_folder_and_it_is_moved_aside_too() {
    // The copy has its own folder C selected, the file has `mirror.folder: null`: after the file is applied the copy goes to the default folder,
    // which holds a day that appeared after the snapshot. It must be moved aside the same way as a folder from a file with a path.
    let dir = tempfile::tempdir().unwrap();
    let chosen = tempfile::tempdir().unwrap();
    let chosen_folder = chosen.path().join("Kinige-C");
    fs::create_dir(&chosen_folder).unwrap();
    let name = database_with_copy(dir.path(), |store| {
        store.set_mirror_dir(&chosen_folder).unwrap()
    });
    let default_folder = mirror_dir(dir.path());
    let day = default_folder.join("Inbox/2026/2026-10-22.md");
    fs::create_dir_all(day.parent().unwrap()).unwrap();
    fs::write(&day, "после снимка").unwrap();
    write_file(dir.path(), br#"{"mirror":{"folder":null}}"#, 0o600);
    restore(dir.path(), &name);
    assert!(
        !default_folder.exists(),
        "the default folder was moved aside"
    );
    let moved = aside(&default_folder);
    assert_eq!(moved.len(), 1);
    assert_eq!(
        fs::read_to_string(moved[0].join("Inbox/2026/2026-10-22.md")).unwrap(),
        "после снимка"
    );
    assert_eq!(
        aside(&chosen_folder).len(),
        1,
        "the folder from the copy was moved aside too"
    );
}

#[test]
fn a_null_folder_equal_to_the_copys_own_folder_is_moved_once() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path(), |_| {});
    let default_folder = mirror_dir(dir.path());
    fs::create_dir_all(default_folder.join("Inbox")).unwrap();
    write_file(dir.path(), br#"{"mirror":{"folder":null}}"#, 0o600);
    let report = restore_backup(
        &db_path(dir.path()),
        &name,
        BACKUP_MS,
        Some(&mirror_dir(dir.path())),
    )
    .unwrap();
    assert_eq!(aside(&default_folder).len(), 1);
    // The folder from the file matched the copy's folder: moved aside once and named once, there is no second row in the report.
    assert!(report.previous_mirror.is_some());
    assert_eq!(report.settings_file_mirror, None);
}
