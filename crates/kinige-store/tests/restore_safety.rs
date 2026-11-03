//! Repeated guardian review: the Markdown copy folder is moved aside before any database move, a failure before the
//! first `rename` puts everything back, the integrity check runs once a day and after a finding nothing is written to
//! the database, `verify_database` only reads, auto-delete is not blocked by skipped entries, restoring from a schema 2
//! copy migrates the database.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::ops::*;
use common::*;
use kinige_domain::ErrorCode;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, StoreError, backups, backups_dir};
use std::fs;
use std::path::Path;

fn at(ms: i64) -> Now {
    moscow(ms)
}

/// A database with one entry and a day copy; returns the copy name. The store is closed.
fn database_with_copy(dir: &Path) -> String {
    let store = open_without_mirror(dir);
    store.send_message(INBOX_ID, "из копии", &at(T0)).unwrap();
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir);
    backups(&db_path(dir))[0].name.clone()
}

/// Everything next to the database (names), without the copy folder and the database itself: traces of an unfinished restore.
fn leftovers(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.starts_with(".restore-") || n.starts_with("corrupt-"))
        .collect();
    names.sort();
    names
}

fn marker(dir: &Path) -> Option<Vec<u8>> {
    fs::read(dir.join("needs-recovery")).ok()
}

// ---------- the Markdown copy folder: first, before the database move ----------

#[test]
fn a_failure_before_the_first_rename_puts_everything_back() {
    for prior in [None, Some(b"".to_vec())] {
        let dir = tempfile::tempdir().unwrap();
        let name = database_with_copy(dir.path());
        let mirror = mirror_dir(dir.path());
        fs::create_dir_all(mirror.join("Inbox")).unwrap();
        fs::write(mirror.join("Inbox/keep.md"), "моё").unwrap();
        if prior.is_some() {
            kinige_store::mark_needs_recovery(&db_path(dir.path())).unwrap();
        }
        let before = fs::read(db_path(dir.path())).unwrap();

        let error = kinige_store::restore_backup_with(
            &db_path(dir.path()),
            &name,
            T0,
            Some(&mirror),
            &|| Err(StoreError::Write("disk full".into())),
            &|| Ok(()),
        )
        .unwrap_err();

        assert!(matches!(error, StoreError::Write(_)), "{error}");
        assert_eq!(
            fs::read(db_path(dir.path())).unwrap(),
            before,
            "the database is intact"
        );
        assert_eq!(
            marker(dir.path()),
            prior,
            "the mark is as it was (it was set: {})",
            prior.is_some()
        );
        assert!(
            mirror.join("Inbox/keep.md").exists(),
            "the copy folder returned to its place"
        );
        assert!(
            !fs::read_dir(dir.path())
                .unwrap()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().contains("before-restore")),
            "no renamed folder is left"
        );
        assert!(leftovers(dir.path()).is_empty(), "no temporary files");
        // And a repeated attempt succeeds.
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror)).unwrap();
    }
}

#[test]
fn a_failure_after_the_first_rename_keeps_the_mark_and_the_retry_reports_the_folder_moved_before() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    fs::write(mirror.join("Inbox/keep.md"), "моё").unwrap();

    // A failure right after the database file went to `corrupt-...`: the folder already moved, the database is not in place.
    let error = kinige_store::restore_backup_with(
        &db_path(dir.path()),
        &name,
        T0,
        Some(&mirror),
        &|| Ok(()),
        &|| {
            Err(StoreError::Write(
                "failure in the middle of a rename".into(),
            ))
        },
    )
    .unwrap_err();
    assert!(matches!(error, StoreError::Write(_)), "{error}");
    assert_eq!(
        marker(dir.path()),
        Some(b"restoring".to_vec()),
        "the mark stayed"
    );
    assert!(
        !db_path(dir.path()).exists(),
        "the database is not in place"
    );
    assert!(!mirror.exists(), "the folder already moved");
    let moved: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.contains("before-restore"))
        .collect();
    assert_eq!(moved.len(), 1, "{moved:?}");
    // In this state startup offers copies instead of creating an empty database.
    assert_eq!(
        kinige_store::startup_check(&db_path(dir.path())),
        kinige_store::StartupCheck::Recovery
    );

    // A retry: it succeeds, does not move the folder a second time, but the window learns about the one moved in the previous attempt.
    let report =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0 + MINUTE, Some(&mirror))
            .unwrap();
    assert_eq!(
        report.previous_mirror.as_deref(),
        Some(dir.path().join(&moved[0]).as_path())
    );
    let after: Vec<String> = fs::read_dir(dir.path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .filter(|n| n.contains("before-restore"))
        .collect();
    assert_eq!(after, moved, "no second folder appeared");
    assert!(dir.path().join(&moved[0]).join("Inbox/keep.md").exists());
    assert_eq!(marker(dir.path()), None, "the mark is cleared");
    assert_eq!(
        count_rows(dir.path(), "message"),
        1,
        "the database is restored"
    );
}

#[test]
fn a_restore_that_was_not_interrupted_does_not_borrow_an_old_before_restore_folder() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    // A folder from an old restore sits nearby, and the copy folder itself is absent (the copy is off or empty).
    fs::create_dir_all(dir.path().join("mirror-before-restore-20200101T000000Z")).unwrap();

    let report =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror)).unwrap();

    assert_eq!(report.previous_mirror, None);
}

#[test]
fn a_mirror_folder_that_cannot_be_renamed_stops_the_restore_before_anything_is_touched() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    // All valid "...-before-restore-..." names are taken: there is nowhere to rename to (the same as an OS refusal).
    let stamp = kinige_store::utc_stamp(T0);
    fs::create_dir(dir.path().join(format!("mirror-before-restore-{stamp}"))).unwrap();
    for n in 2..=999 {
        fs::create_dir(
            dir.path()
                .join(format!("mirror-before-restore-{stamp}-{n}")),
        )
        .unwrap();
    }
    let before = fs::read(db_path(dir.path())).unwrap();

    let error =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror)).unwrap_err();

    assert!(matches!(error, StoreError::MirrorFolderBusy), "{error}");
    assert_eq!(error.code(), ErrorCode::MirrorFolderBusy);
    assert_eq!(
        fs::read(db_path(dir.path())).unwrap(),
        before,
        "the database is intact"
    );
    assert_eq!(marker(dir.path()), None, "the mark was not set");
    assert!(mirror.join("Inbox").is_dir(), "the folder is in place");
    assert!(leftovers(dir.path()).is_empty());
}

#[cfg(unix)]
#[test]
fn an_os_refusal_to_rename_the_mirror_folder_is_the_same_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let parent = dir.path().join("mirrors");
    let mirror = parent.join("Kinige");
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o555)).unwrap();
    // Permissions do not apply under root (a CI container): then there is nothing to check.
    let enforced = fs::write(parent.join("probe"), b"").is_err();
    let before = fs::read(db_path(dir.path())).unwrap();

    let result = kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror));

    fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
    if !enforced {
        return;
    }
    assert!(matches!(result, Err(StoreError::MirrorFolderBusy)));
    assert_eq!(fs::read(db_path(dir.path())).unwrap(), before);
    assert_eq!(marker(dir.path()), None);
    assert!(mirror.join("Inbox").is_dir());
}

/// Windows (slice 10a): a folder held open without `FILE_SHARE_DELETE` (an editor or an indexer has it open) cannot be
/// renamed; the restore stops before it touches anything, with the same `MirrorFolderBusy` as on Unix.
#[cfg(windows)]
#[test]
fn a_mirror_folder_held_open_on_windows_is_the_same_error() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ_WRITE: u32 = 0x1 | 0x2;
    const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = dir.path().join("mirrors").join("Kinige");
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    let before = fs::read(db_path(dir.path())).unwrap();
    let held = fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
        .open(&mirror)
        .unwrap();

    let result = kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror));

    drop(held);
    assert!(matches!(result, Err(StoreError::MirrorFolderBusy)));
    assert_eq!(fs::read(db_path(dir.path())).unwrap(), before);
    assert_eq!(marker(dir.path()), None);
    assert!(mirror.join("Inbox").is_dir());
}

#[test]
fn two_restores_in_one_second_get_different_folder_names() {
    let dir = tempfile::tempdir().unwrap();
    let name = database_with_copy(dir.path());
    let mirror = mirror_dir(dir.path());
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    let first = kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror))
        .unwrap()
        .previous_mirror
        .unwrap();
    fs::create_dir_all(mirror.join("Inbox")).unwrap();
    let second = kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&mirror))
        .unwrap()
        .previous_mirror
        .unwrap();
    assert_ne!(first, second);
    assert!(first.is_dir() && second.is_dir(), "both folders are kept");
    assert!(
        second
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("-2"),
        "{second:?}"
    );
}

#[test]
fn the_folder_to_move_is_the_one_the_backup_itself_names() {
    let dir = tempfile::tempdir().unwrap();
    let chosen = dir.path().join("my-notes");
    let default = mirror_dir(dir.path());
    fs::create_dir_all(chosen.join("Inbox")).unwrap();
    fs::create_dir_all(default.join("Inbox")).unwrap();
    let store = open_without_mirror(dir.path());
    store.run_maintenance(&at(T0)).unwrap();
    drop(store);
    wait_closed(dir.path());
    // The user chose their own folder: it is recorded in settings, the copy is taken with it already.
    raw(dir.path())
        .execute(
            "INSERT INTO app_setting(key, value) VALUES ('mirror_dir', ?1)",
            [chosen.to_string_lossy().into_owned()],
        )
        .unwrap();
    let store = open_without_mirror(dir.path());
    fs::remove_file(backups_dir(&db_path(dir.path())).join(&backups(&db_path(dir.path()))[0].name))
        .unwrap();
    store.run_maintenance(&at(T0 + MINUTE)).unwrap();
    drop(store);
    wait_closed(dir.path());
    let name = backups(&db_path(dir.path()))[0].name.clone();

    let report =
        kinige_store::restore_backup(&db_path(dir.path()), &name, T0, Some(&default)).unwrap();

    let moved = report.previous_mirror.unwrap();
    assert!(
        moved
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("my-notes-before-restore-"),
        "{moved:?}"
    );
    assert!(
        !chosen.exists() && default.join("Inbox").is_dir(),
        "the foreign default folder is untouched"
    );
}
