//! Убийство процесса сразу после фиксации (ТЗ §12.2): данные на месте, отметка копии пережила, файл появляется при старте.

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use std::process::Command;

const CHILD_ENV: &str = "KINIGE_ABORT_CHILD_DIR";

/// Дочерний процесс: открывает хранилище без фоновой копии, отправляет запись и падает через `abort` сразу после ответа
/// (то есть после `COMMIT`). Родитель запускает ту же тестовую программу с переменной окружения.
#[test]
fn abort_right_after_commit_keeps_the_entry_and_the_mirror_mark() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        store
            .send_message(INBOX_ID, "Переживёт abort\n- [ ] проверить", &moscow(T0))
            .expect("отправка");
        std::process::abort();
    }

    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "abort_right_after_commit_keeps_the_entry_and_the_mirror_mark",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, dir.path())
        .status()
        .unwrap();
    assert!(!status.success(), "дочерний процесс должен был упасть");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(6), "SIGABRT");
    }

    // В базе осталась отметка копии, файла копии ещё нет.
    let conn = raw(dir.path());
    let dirty: i64 = conn
        .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| r.get(0))
        .unwrap();
    assert_eq!(dirty, 1);
    let file = mirror_dir(dir.path())
        .join("Inbox")
        .join("2026")
        .join("2026-10-21.md");
    assert!(!file.exists());
    drop(conn);

    // Следующий запуск: запись на месте, порядок и время те же, копия дописана при старте.
    let store = open(dir.path());
    let page = store
        .list_messages(INBOX_ID, None, 50, &moscow(T0 + MINUTE))
        .unwrap();
    assert_eq!(page.messages.len(), 1);
    assert_eq!(page.messages[0].body_md, "Переживёт abort\n- [ ] проверить");
    assert_eq!(page.messages[0].created_at, T0);
    assert_eq!(page.messages[0].journal_date, "2026-10-21");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);

    wait_until("файл копии после перезапуска", || file.exists());
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        text,
        "# 2026-10-21\n\n## 09:12\n\nПереживёт abort\n- [ ] проверить\n"
    );
    wait_until("отметка копии снята", || {
        raw(dir.path())
            .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
            == 0
    });
}
