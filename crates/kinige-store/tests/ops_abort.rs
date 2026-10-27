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

const MOVE_CHILD_ENV: &str = "KINIGE_OPS_ABORT_MOVE_DIR";

/// Дочерний процесс: перенос записи застревает внутри транзакции (триггер на `mirror_dirty` нового чата считает долго,
/// когда запись и день старого чата уже изменены), а главный поток убивает процесс `abort` до `COMMIT`.
fn child_in_the_middle_of_a_move(dir: &std::path::Path) -> ! {
    let store = open_without_mirror(dir);
    let sent = store
        .send_message(INBOX_ID, "переезд", &moscow(T0))
        .unwrap();
    let books = store.create_chat("Книги", Some("📚"), &moscow(T0)).unwrap();
    raw(dir)
        .execute_batch(&format!(
            "CREATE TRIGGER stall_move BEFORE INSERT ON mirror_dirty WHEN NEW.chat_id = '{}' BEGIN \
             SELECT count(*) FROM (WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c WHERE x < 4000000000) \
             SELECT x FROM c); END;",
            books.id
        ))
        .unwrap();
    let (id, chat) = (sent.id, books.id);
    let when = moscow(T0 + DAY_MS);
    std::thread::spawn(move || {
        let _ = store.move_message(&id, &chat, &when);
    });
    // Писатель входит в транзакцию за миллисекунды и до конца «считает»: через секунду он гарантированно посередине.
    std::thread::sleep(std::time::Duration::from_millis(1000));
    std::process::abort();
}

#[test]
fn abort_in_the_middle_of_a_move_rolls_the_whole_move_back() {
    if let Ok(dir) = std::env::var(MOVE_CHILD_ENV) {
        child_in_the_middle_of_a_move(dir.as_ref());
    }
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "abort_in_the_middle_of_a_move_rolls_the_whole_move_back",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(MOVE_CHILD_ENV, dir.path())
        .status()
        .unwrap();
    assert!(!status.success(), "дочерний процесс должен был упасть");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(6), "SIGABRT");
    }

    // После перезапуска (журнал WAL разбирается при открытии) записи как до переноса: ни чата, ни групп, ни отметок копии
    // нового чата, ни ревизий; база цела.
    let id: String = raw(dir.path())
        .query_row("SELECT id FROM message", [], |r| r.get(0))
        .unwrap();
    let before_hlc = group_hlc(dir.path(), "message", &id, "placement");
    let store = open_without_mirror(dir.path());
    let cols = columns(dir.path(), &id);
    assert_eq!(cols["chat_id"], serde_json::json!(INBOX_ID));
    assert_eq!(cols["placement_supersedes"], serde_json::json!("[]"));
    assert_eq!(
        group_hlc(dir.path(), "message", &id, "placement"),
        before_hlc
    );
    assert_eq!(revisions(dir.path(), &id), 0);
    let dirty_chats: Vec<String> = raw(dir.path())
        .prepare("SELECT chat_id FROM mirror_dirty")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        dirty_chats,
        [INBOX_ID],
        "отметка только у дня Inbox, от отправки"
    );
    let integrity: String = raw(dir.path())
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_columns_match_groups(dir.path(), &id);
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.iter().map(|c| c.count).sum::<i64>(), 1);

    // Сняв «застревание», перенос выполняется как обычно.
    raw(dir.path())
        .execute_batch("DROP TRIGGER stall_move;")
        .unwrap();
    let books = chats.iter().find(|c| c.title == "Книги").unwrap();
    let moved = store
        .move_message(&id, &books.id, &moscow(T0 + 2 * DAY_MS))
        .unwrap();
    assert_eq!(moved.chat_id, books.id);
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
