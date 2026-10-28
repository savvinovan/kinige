//! Импорт из Telegram (слайс 4, задача 4.8): запись, повтор, копия, время, ответы, ход импорта, Markdown-копия. Личные чаты,
//! выбор чатов и корзина — `import_chats.rs`, неизменяемость `origin` и `ext` — `import_immutable.rs`.

mod common;

use common::import::*;
use common::ops::*;
use common::*;
use kinige_domain::BackupKind;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::message_ops::message_columns;
use kinige_store::{Store, backups, backups_dir};
use std::collections::BTreeSet;

fn open_plain(dir: &std::path::Path) -> Store {
    open_without_mirror(dir)
}

fn feed(store: &Store, chat: &str) -> Vec<kinige_domain::MessageView> {
    store
        .list_messages(chat, None, 500, &moscow(T0))
        .unwrap()
        .messages
}

#[test]
fn saved_messages_go_to_inbox_with_original_time_core_and_one_hlc_per_record() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let export = parse(&saved_n(3));
    let report = run(&store, &export, &[]).unwrap();
    assert_eq!(report.imported, 3);
    assert_eq!(
        (
            report.skipped_existing,
            report.skipped_service,
            report.skipped_empty
        ),
        (0, 0, 0)
    );
    assert_eq!(report.chats_created, 0);
    assert!(!report.stopped);
    let page = feed(&store, INBOX_ID);
    assert_eq!(page.len(), 3);
    let texts: Vec<_> = page.iter().map(|m| m.body_md.as_str()).collect();
    assert_eq!(texts, ["запись 1", "запись 2", "запись 3"]);
    let first = &page[0];
    // 2019-03-08 12:00:01 по Москве; пояс устройства (в тесте Москва), день по границе 04:00.
    assert_eq!(first.created_at, (SAVED_T + 1) * 1000);
    assert_eq!(first.created_tz, "Europe/Moscow");
    assert_eq!(first.journal_date, "2019-03-08");
    // Группы: `core` с `origin` и `ext` (id строками), колонки совпадают с группами.
    let row = common::capture::row(dir.path(), &first.id);
    assert_eq!(row.origin, "import_telegram");
    let groups = groups(dir.path(), "message", &first.id);
    assert_eq!(
        groups["core"],
        format!(
            r#"{{"created_at":{},"created_tz":"Europe/Moscow","ext":{{"chat":"{OWNER}","msg":"1","src":"telegram"}},"origin":"import_telegram","reply_to_id":null}}"#,
            (SAVED_T + 1) * 1000
        )
    );
    let columns = message_columns(&row.groups_raw).unwrap();
    assert_eq!(columns.origin, "import_telegram");
    assert_eq!(
        columns.ext_json.as_deref(),
        Some(format!(r#"{{"chat":"{OWNER}","msg":"1","src":"telegram"}}"#).as_str())
    );
    // Все группы под одним HLC момента импорта, `sv = 1`, `server_known = 0`; HLC записей строго возрастают.
    let mut hlcs = Vec::new();
    for message in &page {
        let entries = hlc_entries(dir.path(), "message", &message.id);
        assert_eq!(entries.len(), 5);
        let one = group_hlc(dir.path(), "message", &message.id, "core");
        for (group, entry) in &entries {
            let value: serde_json::Value = serde_json::from_str(entry).unwrap();
            assert_eq!(value["h"], one, "{group}");
            assert_eq!(value["sv"], 1, "{group}");
        }
        assert_eq!(
            common::capture::row(dir.path(), &message.id).server_known,
            0
        );
        hlcs.push(one);
    }
    assert!(hlcs.windows(2).all(|w| w[0] < w[1]), "HLC растут: {hlcs:?}");
    // HLC импорта — момент импорта, а не Telegram: свежая локальная запись позже любой импортированной.
    let local = store
        .send_message(INBOX_ID, "после", &moscow(T0 + MINUTE))
        .unwrap();
    assert!(group_hlc(dir.path(), "message", &local.id, "core") > hlcs[2]);
    // Стена HLC — момент импорта (заданное «сейчас» плюс прошедшие миллисекунды), а не 2019 год из Telegram.
    let wall: i64 = hlcs[0][..13].parse().unwrap();
    assert!((T0..T0 + 60_000).contains(&wall), "{}", hlcs[0]);
    // Счётчик в сайдбаре вырос.
    assert_eq!(store.list_chats().unwrap()[0].count, 4);
}

#[test]
fn repeat_import_adds_nothing_and_takes_no_second_copy() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let export = parse(&saved_n(5));
    let first = run(&store, &export, &[]).unwrap();
    assert_eq!(first.imported, 5);
    let name = first
        .backup_name
        .clone()
        .expect("копия перед первой записью");
    let again = run(&store, &export, &[]).unwrap();
    assert_eq!(again.imported, 0);
    assert_eq!(again.skipped_existing, 5);
    assert_eq!(again.backup_name, None, "писать нечего — копия не нужна");
    assert_eq!(count_where(dir.path(), "1 = 1"), 5);
    assert_eq!(backups(&common::db_path(dir.path())).len(), 1);
    assert_eq!(backups(&common::db_path(dir.path()))[0].name, name);
}

#[test]
fn copy_is_taken_before_the_first_write_and_holds_the_old_data_only() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    store.send_message(INBOX_ID, "своя", &moscow(T0)).unwrap();
    let report = run(&store, &parse(&saved_n(4)), &[]).unwrap();
    let name = report.backup_name.expect("копия");
    assert!(name.starts_with("pre-import-") && name.ends_with(".db"));
    let listed = backups(&common::db_path(dir.path()));
    assert_eq!(listed[0].kind, BackupKind::PreImport);
    let copy =
        rusqlite::Connection::open(backups_dir(&common::db_path(dir.path())).join(&name)).unwrap();
    let in_copy: i64 = copy
        .query_row("SELECT COUNT(*) FROM message", [], |r| r.get(0))
        .unwrap();
    assert_eq!(in_copy, 1, "в копии только то, что было до импорта");
    assert_eq!(count_where(dir.path(), "1 = 1"), 5);
}

#[test]
fn service_and_empty_messages_are_skipped_and_media_is_counted() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let own = format!("user{OWNER}");
    let messages = vec![
        msg(1, SAVED_T, "текст", &own),
        r#"{"id":2,"type":"service","date_unixtime":"1552035601","actor":"Я","action":"pin_message","text":"","text_entities":[]}"#.to_string(),
        r#"{"id":3,"type":"message","date_unixtime":"1552035602","from":"Я","from_id":"user1000001","text":"","text_entities":[]}"#.to_string(),
        r#"{"id":4,"type":"message","date_unixtime":"1552035603","from":"Я","from_id":"user1000001","photo":"(File not included.)","text":"","text_entities":[]}"#.to_string(),
        r#"{"id":5,"type":"message","date_unixtime":"1552035604","from":"Я","from_id":"user1000001","poll":{"question":"Q","answers":[{"text":"A"}]},"text":"","text_entities":[]}"#.to_string(),
    ];
    let export = parse(&single("saved_messages", OWNER, None, &messages));
    let report = run(&store, &export, &[]).unwrap();
    assert_eq!(report.imported, 3);
    assert_eq!(report.skipped_service, 1);
    assert_eq!(report.skipped_empty, 1);
    assert_eq!(report.media_placeholders, 1, "фото — заглушка, опрос — нет");
    let texts: Vec<String> = feed(&store, INBOX_ID)
        .into_iter()
        .map(|m| m.body_md)
        .collect();
    assert_eq!(texts[1], "_[Фото не импортировано]_");
    assert_eq!(texts[2], "**Опрос:** Q\n- A");
}

#[test]
fn messages_of_one_second_keep_the_file_order() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let own = format!("user{OWNER}");
    let messages: Vec<String> = (1..=9)
        .map(|i| msg(i, SAVED_T, &format!("одна секунда {i}"), &own))
        .collect();
    let report = run(
        &store,
        &parse(&single("saved_messages", OWNER, None, &messages)),
        &[],
    )
    .unwrap();
    assert_eq!(report.imported, 9);
    let texts: Vec<String> = feed(&store, INBOX_ID)
        .into_iter()
        .map(|m| m.body_md)
        .collect();
    let expected: Vec<String> = (1..=9).map(|i| format!("одна секунда {i}")).collect();
    assert_eq!(texts, expected);
    let created: Vec<i64> = feed(&store, INBOX_ID)
        .into_iter()
        .map(|m| m.created_at)
        .collect();
    assert_eq!(created[0], SAVED_T * 1000);
    assert_eq!(created[8], SAVED_T * 1000 + 8);
}

#[test]
fn impossible_dates_do_not_break_the_batch_and_land_on_the_import_day() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let own = format!("user{OWNER}");
    let messages: Vec<String> = [
        (SAVED_T, "хорошая до"),
        (9_999_999_999_999_999, "за пределом двойной точности"),
        (9_007_199_254_740, "далёкое будущее"),
        (-7, "отрицательная"),
        (SAVED_T + 60, "хорошая после"),
    ]
    .iter()
    .enumerate()
    .map(|(i, (unix, text))| msg(i as u64 + 1, *unix, text, &own))
    .collect();
    let report = run(
        &store,
        &parse(&single("saved_messages", OWNER, None, &messages)),
        &[],
    )
    .unwrap();
    assert_eq!(report.imported, 5, "{report:?}");
    let all = feed(&store, INBOX_ID);
    assert_eq!(all.len(), 5);
    let today = kinige_domain::journal::journal_date(T0, 3 * 3600, 240);
    for message in &all {
        if message.body_md.starts_with("хорошая") {
            assert_eq!(message.journal_date, "2019-03-08");
        } else {
            assert!(
                (T0..T0 + 3).contains(&message.created_at),
                "{}",
                message.body_md
            );
            assert_eq!(message.journal_date, today);
        }
    }
    // Записи без настоящей даты стоят в порядке файла, а не в порядке случайных id.
    let texts: Vec<&str> = all.iter().map(|m| m.body_md.as_str()).collect();
    assert_eq!(
        texts,
        [
            "хорошая до",
            "хорошая после",
            "за пределом двойной точности",
            "далёкое будущее",
            "отрицательная",
        ]
    );
}

#[test]
fn reply_to_an_imported_message_is_stored_and_unknown_target_is_null() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let own = format!("user{OWNER}");
    let reply = |id: u64, to: u64, text: &str, unix: i64| {
        format!(
            r#"{{"id":{id},"type":"message","date_unixtime":"{unix}","from":"Я","from_id":"{own}","reply_to_message_id":{to},"text":"{text}","text_entities":[{{"type":"plain","text":"{text}"}}]}}"#
        )
    };
    let messages = vec![
        msg(1, SAVED_T, "первое", &own),
        reply(2, 1, "ответ на первое", SAVED_T + 1),
        reply(3, 99, "ответ на неизвестное", SAVED_T + 2),
        reply(4, 5, "ответ на следующее", SAVED_T + 3),
        msg(5, SAVED_T + 4, "пятое", &own),
    ];
    run(
        &store,
        &parse(&single("saved_messages", OWNER, None, &messages)),
        &[],
    )
    .unwrap();
    let page = feed(&store, INBOX_ID);
    let reply_of = |text: &str| -> Option<String> {
        let id = &page.iter().find(|m| m.body_md == text).unwrap().id;
        raw(dir.path())
            .query_row("SELECT reply_to_id FROM message WHERE id = ?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
    };
    assert_eq!(
        reply_of("ответ на первое").as_deref(),
        Some(page[0].id.as_str())
    );
    assert_eq!(reply_of("ответ на неизвестное"), None);
    assert_eq!(
        reply_of("ответ на следующее"),
        None,
        "адресат ещё не импортирован"
    );
    // И в байтах `core`.
    let core = &groups(dir.path(), "message", &page[1].id)["core"];
    assert!(
        core.contains(&format!(r#""reply_to_id":"{}""#, page[0].id)),
        "{core}"
    );
    // Повторный импорт тех же сообщений не меняет уже записанное.
    assert_eq!(
        run(
            &store,
            &parse(&single("saved_messages", OWNER, None, &messages)),
            &[]
        )
        .unwrap()
        .imported,
        0
    );
}

#[test]
fn batches_and_progress_cover_every_message() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let export = parse(&saved_n(1_203));
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = seen.clone();
    let report = store
        .import_telegram(
            export.clone(),
            all_chats(&export),
            kinige_domain::Language::Ru,
            &now(),
            move |p| sink.lock().unwrap().push((p.done, p.total)),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
    assert_eq!(report.imported, 1_203);
    assert_eq!(
        *seen.lock().unwrap(),
        [(500, 1_203), (1_000, 1_203), (1_203, 1_203)]
    );
    assert_eq!(count_where(dir.path(), "1 = 1"), 1_203);
    let ids: BTreeSet<String> = feed(&store, INBOX_ID).into_iter().map(|m| m.id).collect();
    assert_eq!(
        ids.len(),
        500,
        "страница ленты ограничена; все записи на месте по счётчику"
    );
    let integrity: String = raw(dir.path())
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
}

#[test]
fn imported_days_get_markdown_files() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let own = format!("user{OWNER}");
    // Два дня: 8 и 9 марта 2019 (по Москве).
    let messages = vec![
        msg(1, SAVED_T, "первый день", &own),
        msg(2, SAVED_T + 86_400, "второй день", &own),
    ];
    run(
        &store,
        &parse(&single("saved_messages", OWNER, None, &messages)),
        &[],
    )
    .unwrap();
    let day = |date: &str| {
        mirror_dir(dir.path())
            .join("Inbox")
            .join("2019")
            .join(format!("{date}.md"))
    };
    wait_until("файл первого дня", || {
        day("2019-03-08").exists()
    });
    wait_until("файл второго дня", || {
        day("2019-03-09").exists()
    });
    let text = std::fs::read_to_string(day("2019-03-08")).unwrap();
    assert!(text.contains("первый день"), "{text}");
}
