//! Slice 5d (ADR-0022): the user's `settings.json` is never touched; the start-up clean-up removes only our own old temporary
//! files, and `app_setting` (the retired hash row included) is not read or written by it.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

const HEX: &str = "0123456789abcdef0123456789abcdef";
const SETTINGS: &[u8] =
    br#"{"appearance": {"text_size": "large"}, "agent_access": {"enabled": true}}"#;

fn app_settings(dir: &Path) -> Vec<(String, String)> {
    let conn = raw(dir);
    let mut statement = conn
        .prepare("SELECT key, value FROM app_setting ORDER BY key")
        .unwrap();
    statement
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn start_up_removes_only_the_exact_leftovers_and_leaves_the_user_file_and_the_table_alone() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    raw(dir.path())
        .execute(
            "INSERT INTO app_setting(key, value) VALUES ('settings_file_hash', 'kept-hash')",
            [],
        )
        .unwrap();
    // The user's file, with an old mtime, and what the old writer could leave.
    let file = dir.path().join("settings.json");
    fs::write(&file, SETTINGS).unwrap();
    let old = SystemTime::now() - Duration::from_secs(86_400 * 30);
    fs::File::options()
        .write(true)
        .open(&file)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let before_mtime = fs::metadata(&file).unwrap().modified().unwrap();
    let leftover = dir.path().join(format!(".settings.json.{HEX}.tmp"));
    let lookalike = dir
        .path()
        .join(".settings.json.01234567-89ab-cdef-0123-456789abcdef.tmp");
    let directory = dir
        .path()
        .join(format!(".settings.json.{}.tmp", HEX.replace('0', "1")));
    let upper = dir
        .path()
        // A different digit string: on a case-insensitive file system the upper-case twin of `HEX` would be the same file.
        .join(format!(
            ".settings.json.{}.tmp",
            HEX.replace('0', "3").to_uppercase()
        ));
    fs::write(&leftover, b"old").unwrap();
    fs::write(&lookalike, b"keep").unwrap();
    fs::write(&upper, b"keep").unwrap();
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("inside"), b"keep").unwrap();
    #[cfg(unix)]
    let linked = {
        let target = dir.path().join("target-of-the-link");
        fs::write(&target, b"keep").unwrap();
        let link = dir
            .path()
            .join(format!(".settings.json.{}.tmp", HEX.replace('0', "2")));
        std::os::unix::fs::symlink(&target, &link).unwrap();
        link
    };
    let table = app_settings(dir.path());

    assert_eq!(store.remove_settings_file_leftovers(), 1);
    assert_eq!(store.remove_settings_file_leftovers(), 0, "idempotent");

    assert!(!leftover.exists());
    assert!(lookalike.exists() && upper.exists());
    assert!(directory.join("inside").exists());
    #[cfg(unix)]
    {
        assert!(linked.symlink_metadata().unwrap().file_type().is_symlink());
        assert_eq!(
            fs::read(dir.path().join("target-of-the-link")).unwrap(),
            b"keep"
        );
    }
    assert_eq!(fs::read(&file).unwrap(), SETTINGS);
    assert_eq!(
        fs::metadata(&file).unwrap().modified().unwrap(),
        before_mtime
    );
    assert_eq!(
        app_settings(dir.path()),
        table,
        "app_setting is not touched"
    );
    assert!(names(dir.path()).contains(&"settings.json".to_string()));
}

#[test]
fn opening_the_store_neither_reads_nor_changes_a_user_settings_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("settings.json");
    // A hostile file that the old reader would have applied: access on, read and write.
    fs::write(&file, SETTINGS).unwrap();
    let before = fs::metadata(&file).unwrap();
    let store = open_without_mirror(dir.path());
    store.remove_settings_file_leftovers();
    drop(store);
    let after = fs::metadata(&file).unwrap();
    assert_eq!(fs::read(&file).unwrap(), SETTINGS);
    assert_eq!(after.modified().unwrap(), before.modified().unwrap());
    let table = app_settings(dir.path());
    assert!(
        table
            .iter()
            .all(|(key, _)| !key.starts_with("mcp_") && key != "text_size"),
        "nothing from the file reached the table: {table:?}"
    );
}
