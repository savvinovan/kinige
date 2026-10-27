//! Убийство процесса сразу после фиксации каждой операции слайса 3 (ТЗ §12.2): данные на месте, группы и колонки согласованы.

mod common;

use common::ops::*;
use common::*;
use kinige_domain::MessageFlag;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use std::process::Command;

const CHILD_ENV: &str = "KINIGE_OPS_ABORT_DIR";
const MODE_ENV: &str = "KINIGE_OPS_ABORT_MODE";
const MODES: &[&str] = &[
    "delete",
    "restore",
    "edit",
    "task",
    "date",
    "toggle",
    "create_chat",
    "rename_chat",
    "pin",
    "archive",
    "purge",
];

/// Дочерний процесс: подготовка, одна операция и `abort` сразу после её ответа (то есть после `COMMIT`).
fn child(dir: &std::path::Path, mode: &str) -> ! {
    let store = open_without_mirror(dir);
    let sent = store
        .send_message(INBOX_ID, "аборт\n- [ ] дело", &moscow(T0))
        .unwrap();
    let chat = store.create_chat("Подготовка", None, &moscow(T0)).unwrap();
    let later = moscow(T0 + DAY_MS);
    match mode {
        "delete" => store.delete_message(&sent.id, &later).unwrap(),
        "restore" => {
            store.delete_message(&sent.id, &moscow(T0 + 1)).unwrap();
            store.restore_message(&sent.id, &later).unwrap();
        }
        "edit" => drop(store.edit_message(&sent.id, "правка", &later).unwrap()),
        "task" => drop(
            store
                .set_message_flag(&sent.id, MessageFlag::Task, true, &later)
                .unwrap(),
        ),
        "date" => drop(
            store
                .set_message_date(&sent.id, "2026-10-01", &later)
                .unwrap(),
        ),
        "toggle" => drop(store.toggle_task_item(&sent.id, 0, true, &later).unwrap()),
        "create_chat" => drop(store.create_chat("Созданный", Some("📚"), &later).unwrap()),
        "rename_chat" => drop(
            store
                .rename_chat(&chat.id, "Переименованный", &later)
                .unwrap(),
        ),
        "pin" => drop(store.set_chat_pinned(&chat.id, true, &later).unwrap()),
        "archive" => drop(store.set_chat_archived(&chat.id, true, &later).unwrap()),
        "purge" => {
            store.delete_message(&sent.id, &moscow(T0 + 1)).unwrap();
            store.purge_message(&sent.id, &later).unwrap();
        }
        other => panic!("неизвестный режим {other}"),
    }
    std::process::abort();
}

#[test]
fn abort_right_after_commit_keeps_every_operation() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        child(
            dir.as_ref(),
            &std::env::var(MODE_ENV).expect("режим дочернего процесса"),
        );
    }
    for mode in MODES {
        let dir = tempfile::tempdir().unwrap();
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "abort_right_after_commit_keeps_every_operation",
                "--exact",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(CHILD_ENV, dir.path())
            .env(MODE_ENV, mode)
            .status()
            .unwrap();
        assert!(
            !status.success(),
            "{mode}: дочерний процесс должен был упасть"
        );
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            assert_eq!(status.signal(), Some(6), "{mode}: SIGABRT");
        }

        let store = open_without_mirror(dir.path());
        let later = moscow(T0 + 2 * DAY_MS);
        let id: Option<String> = raw(dir.path())
            .query_row("SELECT id FROM message LIMIT 1", [], |r| r.get(0))
            .ok();
        let cols = id.as_deref().map(|id| columns(dir.path(), id));
        let chats = store.list_chats().unwrap();
        let chat = chats
            .iter()
            .find(|c| c.title.starts_with("Подгот") || c.title == "Переименованный");
        match *mode {
            "delete" => {
                assert_eq!(
                    cols.unwrap()["deleted_at"],
                    serde_json::json!(T0 + DAY_MS),
                    "{mode}"
                );
                assert_eq!(chats[0].count, 0);
                assert_eq!(store.list_trash(&later).unwrap().len(), 1);
            }
            "restore" => {
                assert_eq!(
                    cols.unwrap()["deleted_at"],
                    serde_json::json!(null),
                    "{mode}"
                );
                assert_eq!(chats[0].count, 1);
            }
            "edit" => {
                assert_eq!(cols.unwrap()["body_md"], "правка");
                assert_eq!(
                    revisions(dir.path(), id.as_deref().unwrap()),
                    1,
                    "ревизия пережила"
                );
            }
            "toggle" => {
                assert_eq!(cols.unwrap()["body_md"], "аборт\n- [x] дело");
                assert_eq!(revisions(dir.path(), id.as_deref().unwrap()), 1);
            }
            "task" => assert_eq!(cols.unwrap()["is_task"], true),
            "date" => assert_eq!(cols.unwrap()["journal_date"], "2026-10-01"),
            "create_chat" => assert!(
                chats
                    .iter()
                    .any(|c| c.title == "Созданный" && c.icon.as_deref() == Some("📚"))
            ),
            "rename_chat" => assert_eq!(chat.unwrap().title, "Переименованный"),
            "pin" => assert!(chat.unwrap().pinned_at.is_some()),
            "archive" => assert!(chat.unwrap().archived_at.is_some()),
            "purge" => {
                assert!(id.is_none(), "запись удалена навсегда");
                assert_eq!(count_rows(dir.path(), "tombstone"), 1);
            }
            other => panic!("{other}"),
        }
        if let Some(id) = id {
            assert_columns_match_groups(dir.path(), &id);
        }
    }
}
