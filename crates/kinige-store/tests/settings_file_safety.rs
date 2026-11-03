//! The settings file: an unsafe file (ADR-0010), the "hash matched -- the database wins" rule, races, interruptions, garbage on input.

// The integration test root is test code as a whole: `cfg(test)` gives helpers outside #[test] the same clippy.toml
// relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::settings_file::*;
use common::*;
use kinige_domain::TextSize;
// The rest is needed only by the file-mode tests (Unix).
#[cfg(unix)]
use kinige_domain::{KeyIssueKind, UnsafeReason};
#[cfg(unix)]
use serde_json::json;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

#[cfg(unix)]
const ACCESS_ON: &str =
    r#"{"text_size":"large","agent_access":{"enabled":true,"mode":"read_write"}}"#;

// The six tests below are about the file mode (0664, 0600): Windows has no POSIX modes (ADR-0010, Windows note).
#[cfg(unix)]
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
        "access did not change, so there is no log row"
    );
}

#[cfg(unix)]
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

#[cfg(unix)]
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

#[cfg(unix)]
#[test]
fn a_stale_enabled_in_an_unsafe_period_never_comes_back_when_permissions_are_fixed() {
    // A sequence from the guardian review: the file is synchronized with enabled=true, chmod 664, access is switched off in the window
    // (the file is not rewritten), chmod 600, the agent edits only text_size: access stays off.
    let rig = rig();
    rig.write(r#"{"agent_access":{"enabled":true}}"#);
    assert!(rig.store.agent_access().unwrap().enabled);
    // The window wrote the whole file, now it is "synchronized".
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
        "the unsafe file is not rewritten"
    );
    fs::set_permissions(rig.path(), fs::Permissions::from_mode(0o600)).unwrap();
    rig.file.sync_now();
    // The hash matched: the database won, the file is rewritten from it.
    assert!(!rig.store.agent_access().unwrap().enabled);
    assert_eq!(rig.json()["agent_access"]["enabled"], false);
    // The agent edits only text_size in the file it sees.
    let mut doc = rig.json();
    doc["text_size"] = json!("large");
    rig.write(&doc.to_string());
    assert!(!rig.store.agent_access().unwrap().enabled);
    assert_eq!(rig.store.get_settings().unwrap().text_size, TextSize::Large);
}

#[cfg(unix)]
#[test]
fn content_written_while_the_file_was_unsafe_is_not_applied_after_chmod() {
    let rig = rig();
    rig.write_mode(ACCESS_ON, 0o666);
    fs::set_permissions(rig.path(), fs::Permissions::from_mode(0o600)).unwrap();
    rig.file.sync_now();
    assert!(
        !rig.store.agent_access().unwrap().enabled,
        "a foreign write does not become trusted"
    );
    assert_eq!(
        rig.json()["agent_access"]["enabled"],
        false,
        "the file is rewritten from the database"
    );
}

#[cfg(unix)]
#[test]
fn after_chmod_a_fresh_save_applies_the_keys_that_were_ignored() {
    // The hint in the window: "make the file writable only by you (chmod 600), then save the file again".
    let rig = rig();
    rig.write_mode(ACCESS_ON, 0o666);
    fs::set_permissions(rig.path(), fs::Permissions::from_mode(0o600)).unwrap();
    rig.file.sync_now();
    assert!(!rig.store.agent_access().unwrap().enabled);
    // Saving again (the file differs in bytes) with safe permissions applies in full.
    rig.write(&format!("{ACCESS_ON}\n"));
    assert!(rig.store.agent_access().unwrap().enabled);
    assert!(rig.file.state().unsafe_reason.is_none());
}

#[test]
fn a_window_edit_between_a_crash_and_a_restart_survives_because_the_hash_matches() {
    // The window wrote to the database, the file stayed as it was (an interruption before the file write): at startup the file matches the stored
    // hash, so this is not an external edit: the database wins, the file is rewritten.
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
    // Applied synchronously in `start`: there is no need to wait for a poll.
    assert_eq!(
        again.store.get_settings().unwrap().text_size,
        TextSize::Large
    );
    // AI-agent access from the file is applied on Unix only; on Windows it is reported and not stored (ADR-0010).
    let agent_applied = cfg!(unix);
    assert_eq!(again.store.agent_access().unwrap().enabled, agent_applied);
    assert_eq!(again.log_rows().len(), usize::from(agent_applied));
}

#[test]
fn a_file_replaced_between_our_rename_and_the_hash_step_is_still_applied() {
    // A race from the guardian review: the hash is computed from the written bytes, not from the re-read file.
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
    wait_until("the agent's edit is applied", || {
        rig.store.get_settings().unwrap().text_size == TextSize::Large
    });
}
