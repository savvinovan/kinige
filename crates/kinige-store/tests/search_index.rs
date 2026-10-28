//! Индекс поиска и `message_ext` (слайс 4, задача 4.6): миграция 3 → 4 с копией, индекс совпадает с текстом после каждой
//! операции, перестройка при смене версии, откат вместе с записью, `abort` сразу после фиксации.

mod common;

use common::*;
use kinige_domain::MessageFlag;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::DAY_MS;
use kinige_domain::search::{SEARCH_INDEX_VERSION, fts_query};
use kinige_store::{LATEST_VERSION, Store};
use std::path::Path;
use std::process::Command;

/// Id записей, найденных запросом в индексе (без корзины и без разбора порядка).
fn found(dir: &Path, input: &str) -> Vec<String> {
    let query = fts_query(input).expect("в запросе есть слова");
    let conn = raw(dir);
    let mut statement = conn
        .prepare(
            "SELECT mp.message_id FROM search_fts f JOIN search_map mp ON mp.rowid = f.rowid \
             WHERE search_fts MATCH ?1 ORDER BY mp.message_id",
        )
        .unwrap();
    statement
        .query_map([query], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn count(dir: &Path, table: &str) -> i64 {
    raw(dir)
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

fn user_version(dir: &Path) -> i64 {
    raw(dir)
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap()
}

/// Возвращает базу в состояние слайса 3: без таблиц миграции 4, `user_version = 3`.
fn downgrade_to_three(dir: &Path) {
    let conn = raw(dir);
    conn.execute_batch(
        "DROP TABLE message_ext; DROP TABLE search_fts; DROP TABLE search_map; \
         DELETE FROM app_setting WHERE key = 'search_index_version';",
    )
    .unwrap();
    conn.pragma_update(None, "user_version", 3).unwrap();
}

#[test]
fn migration_four_takes_a_copy_and_builds_the_index_and_ext_from_old_records() {
    let dir = tempfile::tempdir().unwrap();
    let (first, second, third, imported) = {
        let store = open_without_mirror(dir.path());
        let a = store
            .send_message(INBOX_ID, "Купить молоко", &moscow(T0))
            .unwrap()
            .id;
        let b = store
            .send_message(INBOX_ID, "Книгу про ёлку", &moscow(T0 + MINUTE))
            .unwrap()
            .id;
        let c = store
            .send_message(INBOX_ID, "Удалённая вещь", &moscow(T0 + 2 * MINUTE))
            .unwrap()
            .id;
        store.delete_message(&c, &moscow(T0 + 3 * MINUTE)).unwrap();
        let d = store
            .send_message(INBOX_ID, "Из телеграма", &moscow(T0 + 4 * MINUTE))
            .unwrap()
            .id;
        (a, b, c, d)
    };
    // Запись с внешним id, как её оставит импорт (поле `ext_json`), и повреждённая строка `ext_json` другой записи.
    raw(dir.path())
        .execute(
            "UPDATE message SET ext_json = '{\"chat\":\"9223372036854775807\",\"msg\":\"-5\",\"src\":\"telegram\"}' \
             WHERE id = ?1",
            [&imported],
        )
        .unwrap();
    raw(dir.path())
        .execute(
            "UPDATE message SET ext_json = '{не json' WHERE id = ?1",
            [&first],
        )
        .unwrap();
    downgrade_to_three(dir.path());

    let later = moscow(T0 + 10 * MINUTE);
    let _store = Store::open_at(options(dir.path()), &later).unwrap();
    assert_eq!(user_version(dir.path()), LATEST_VERSION);
    // Копия снята до миграции, в ней схема 3.
    let copies: Vec<String> =
        std::fs::read_dir(db_path(dir.path()).parent().unwrap().join("backups"))
            .unwrap()
            .flatten()
            .map(|e| e.file_name().into_string().unwrap())
            .collect();
    assert_eq!(
        copies,
        [format!(
            "pre-migration-v3-{}.db",
            kinige_store::utc_stamp(later.ms)
        )]
    );
    let copy = db_path(dir.path())
        .parent()
        .unwrap()
        .join("backups")
        .join(&copies[0]);
    let in_copy: i64 = rusqlite::Connection::open(&copy)
        .unwrap()
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(in_copy, 3);

    // Индекс полон: живые и удалённая запись (корзину отсекает запрос, а не индекс).
    assert_eq!(count(dir.path(), "search_map"), 4);
    assert_eq!(found(dir.path(), "молоко "), [first.clone()]);
    assert_eq!(found(dir.path(), "книги "), [second.clone()]);
    assert_eq!(found(dir.path(), "елка "), [second]);
    assert_eq!(found(dir.path(), "удалённая"), [third]);
    assert_eq!(found(dir.path(), "моло"), [first]);
    assert_eq!(
        raw(dir.path())
            .query_row(
                "SELECT value FROM app_setting WHERE key = 'search_index_version'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
        SEARCH_INDEX_VERSION.to_string()
    );
    // `message_ext` собран из `ext_json`; повреждённый `ext_json` строк не даёт.
    let ext: Vec<(String, String, String, String)> = raw(dir.path())
        .prepare("SELECT src, ext_chat, ext_msg, message_id FROM message_ext")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        ext,
        [(
            "telegram".to_string(),
            "9223372036854775807".to_string(),
            "-5".to_string(),
            imported
        )]
    );
}

#[test]
fn index_follows_send_append_edit_toggle_and_purge() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // Отправка.
    let sent = store
        .send_message(
            INBOX_ID,
            "Сегодня аксолотль\n- [ ] позвонить маме",
            &moscow(T0),
        )
        .unwrap();
    assert_eq!(found(dir.path(), "аксолотль "), [sent.id.clone()]);
    assert_eq!(found(dir.path(), "позвонить мама "), [sent.id.clone()]);
    // Правка: старое слово ушло, новое нашлось.
    store
        .edit_message(
            &sent.id,
            "Теперь бегемотик\n- [ ] позвонить маме",
            &moscow(T0 + MINUTE),
        )
        .unwrap();
    assert!(found(dir.path(), "аксолотль ").is_empty());
    assert_eq!(found(dir.path(), "бегемотик "), [sent.id.clone()]);
    // Галочка `- [ ]`: слова те же, индекс не ломается и не двоится.
    store
        .toggle_task_item(&sent.id, 0, true, &moscow(T0 + 2 * MINUTE))
        .unwrap();
    assert_eq!(found(dir.path(), "бегемотик "), [sent.id.clone()]);
    assert_eq!(count(dir.path(), "search_map"), 1);
    // Дописывание из окна захвата (запись создана пользователем, не старше десяти минут).
    let outcome = store
        .capture_append("и ещё фламинго", &moscow(T0 + 3 * MINUTE))
        .unwrap();
    assert_eq!(outcome.message.id, sent.id, "дописано к той же записи");
    assert_eq!(found(dir.path(), "фламинго "), [sent.id.clone()]);
    assert_eq!(found(dir.path(), "бегемотик "), [sent.id.clone()]);
    // Корзина: индекс на месте (запись можно вернуть), удаление навсегда убирает.
    store
        .delete_message(&sent.id, &moscow(T0 + 4 * MINUTE))
        .unwrap();
    assert_eq!(found(dir.path(), "бегемотик "), [sent.id.clone()]);
    store
        .purge_message(&sent.id, &moscow(T0 + 5 * MINUTE))
        .unwrap();
    assert!(found(dir.path(), "бегемотик ").is_empty());
    assert!(found(dir.path(), "фламинго ").is_empty());
    assert_eq!(count(dir.path(), "search_map"), 0);
    // В индексе нет ни одной строки: удаляются и документы, и их строки соответствия.
    let rows: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM search_fts_docsize", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn flags_date_and_move_do_not_touch_the_index_but_it_stays_consistent() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_message(INBOX_ID, "Пеликан идёт", &moscow(T0))
        .unwrap();
    let chat = store.create_chat("Птицы", None, &moscow(T0)).unwrap();
    store
        .set_message_flag(&sent.id, MessageFlag::Favorite, true, &moscow(T0 + MINUTE))
        .unwrap();
    store
        .set_message_date(&sent.id, "2026-10-01", &moscow(T0 + 2 * MINUTE))
        .unwrap();
    store
        .move_message(&sent.id, &chat.id, &moscow(T0 + DAY_MS))
        .unwrap();
    assert_eq!(found(dir.path(), "пеликан "), [sent.id]);
    assert_eq!(count(dir.path(), "search_map"), 1);
}

#[test]
fn changing_the_version_rebuilds_the_index_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let id = {
        let store = open_without_mirror(dir.path());
        store
            .send_message(INBOX_ID, "Вермишель и мармелад", &moscow(T0))
            .unwrap()
            .id
    };
    // Индекс испорчен «старыми правилами»: пуст и с другой версией.
    {
        let conn = raw(dir.path());
        conn.execute(
            "INSERT INTO search_fts (search_fts) VALUES ('delete-all')",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM search_map", []).unwrap();
        conn.execute(
            "UPDATE app_setting SET value = '0' WHERE key = 'search_index_version'",
            [],
        )
        .unwrap();
    }
    assert!(found(dir.path(), "вермишель ").is_empty());
    let _store = open_without_mirror(dir.path());
    assert_eq!(found(dir.path(), "вермишель "), [id.clone()]);
    assert_eq!(found(dir.path(), "мармелад "), [id]);
    // Версия совпала — повторное открытие индекс не трогает (метка id строки соответствия не меняется).
    let before: i64 = raw(dir.path())
        .query_row("SELECT rowid FROM search_map", [], |r| r.get(0))
        .unwrap();
    let _again = open_without_mirror(dir.path());
    let after: i64 = raw(dir.path())
        .query_row("SELECT rowid FROM search_map", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn a_failing_index_write_rolls_back_the_record_too() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let kept = store
        .send_message(INBOX_ID, "Остаётся", &moscow(T0))
        .unwrap();
    // Отказ записи индекса (как `ops_abort.rs`: триггер): новая запись не должна остаться без индекса.
    raw(dir.path())
        .execute_batch(
            "CREATE TRIGGER fail_index BEFORE INSERT ON search_map BEGIN SELECT RAISE(ABORT, 'index down'); END;",
        )
        .unwrap();
    let error = store
        .send_message(INBOX_ID, "Не запишется", &moscow(T0 + MINUTE))
        .unwrap_err();
    assert_eq!(error.code(), kinige_domain::ErrorCode::StoreWriteFailed);
    assert_eq!(count(dir.path(), "message"), 1);
    assert_eq!(count(dir.path(), "search_map"), 1);
    assert!(found(dir.path(), "запишется ").is_empty());
    // Правка: сбой уже после замены строки индекса (отметка копии идёт в той же транзакции следом) откатывает и индекс.
    raw(dir.path())
        .execute_batch(
            "DROP TRIGGER fail_index; \
             CREATE TRIGGER fail_after_index BEFORE INSERT ON mirror_dirty \
             BEGIN SELECT RAISE(ABORT, 'mirror down'); END;",
        )
        .unwrap();
    assert!(
        store
            .edit_message(&kept.id, "Изменённый текст", &moscow(T0 + 2 * MINUTE))
            .is_err()
    );
    assert_eq!(found(dir.path(), "остаётся "), [kept.id]);
    assert!(found(dir.path(), "изменённый ").is_empty());
}

const CHILD_ENV: &str = "KINIGE_SEARCH_ABORT_DIR";

#[test]
fn abort_right_after_commit_keeps_the_index() {
    if let Ok(dir) = std::env::var(CHILD_ENV) {
        let store = open_without_mirror(dir.as_ref());
        store
            .send_message(INBOX_ID, "Переживёт саламандра", &moscow(T0))
            .expect("отправка");
        std::process::abort();
    }
    let dir = tempfile::tempdir().unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .args([
            "abort_right_after_commit_keeps_the_index",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD_ENV, dir.path())
        .status()
        .unwrap();
    assert!(!status.success(), "дочерний процесс должен был упасть");
    let ids = found(dir.path(), "саламандры ");
    assert_eq!(ids.len(), 1, "индекс пережил abort");
    let quick: String = raw(dir.path())
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(quick, "ok");
    let store = open_without_mirror(dir.path());
    assert_eq!(
        store
            .list_messages(INBOX_ID, None, 10, &moscow(T0 + MINUTE))
            .unwrap()
            .messages[0]
            .id,
        ids[0]
    );
}
