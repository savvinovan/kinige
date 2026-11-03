//! Killing the process right after the commit (spec §12.2): the data is in place, the copy mark survived, the file appears at startup.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_domain::groups::INBOX_ID;
use std::process::Command;

const CHILD_ENV: &str = "KINIGE_ABORT_CHILD_DIR";

/// Child process: opens the store without the background copy, sends an entry and dies via `abort` right after the reply
/// (that is, after `COMMIT`). The parent runs the same test program with an environment variable.
#[test]
fn abort_right_after_commit_keeps_the_entry_and_the_mirror_mark() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        store
            .send_message(INBOX_ID, "Переживёт abort\n- [ ] проверить", &moscow(T0))
            .expect("send");
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
    assert!(!status.success(), "the child process should have died");
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(status.signal(), Some(6), "SIGABRT");
    }

    // The database still has the copy mark, the copy file is not there yet.
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

    // Next launch: the entry is in place, order and time are the same, the copy is completed at startup.
    let store = open(dir.path());
    let page = store
        .list_messages(INBOX_ID, None, 50, &moscow(T0 + MINUTE))
        .unwrap();
    assert_eq!(page.messages.len(), 1);
    assert_eq!(page.messages[0].body_md, "Переживёт abort\n- [ ] проверить");
    assert_eq!(page.messages[0].created_at, T0);
    assert_eq!(page.messages[0].journal_date, "2026-10-21");
    assert_eq!(store.list_chats().unwrap()[0].count, 1);

    wait_until("copy file after restart", || file.exists());
    let text = std::fs::read_to_string(&file).unwrap();
    assert_eq!(
        text,
        "# 2026-10-21\n\n## 09:12\n\nПереживёт abort\n- [ ] проверить\n"
    );
    wait_until("copy mark cleared", || {
        raw(dir.path())
            .query_row("SELECT COUNT(*) FROM mirror_dirty", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
            == 0
    });
}
