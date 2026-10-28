//! Импорт из Telegram (слайс 4, задача 4.8), чаты: личные чаты и их имена, выбор чатов, корзина, ход импорта.

mod common;

use common::import::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Store;

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
fn personal_chat_gets_its_own_chat_once_and_foreign_messages_carry_the_author() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let theirs = format!("user{PETER}");
    let mine = format!("user{OWNER}");
    let messages = vec![
        msg(1, SAVED_T, "привет", &theirs),
        msg(2, SAVED_T + 1, "здравствуй", &mine),
    ];
    let personal = single("personal_chat", PETER, Some("Пётр Тестов"), &messages);
    let export = parse(&full(&[saved_n(2), personal]));
    let report = run(&store, &export, &[]).unwrap();
    assert_eq!(report.imported, 4);
    assert_eq!(report.chats_created, 1);
    let chats = store.list_chats().unwrap();
    assert_eq!(chats.len(), 2);
    let chat = chats.iter().find(|c| !c.is_inbox).unwrap();
    assert_eq!(chat.title, "Пётр Тестов");
    assert_eq!(chat.icon.as_deref(), Some("💬"));
    assert_eq!(chat.count, 2);
    let texts: Vec<String> = feed(&store, &chat.id)
        .into_iter()
        .map(|m| m.body_md)
        .collect();
    assert_eq!(texts, ["**Имя user2000002**\nпривет", "здравствуй"]);
    // Повтор — тот же чат, ни новых записей, ни пустого дубля.
    let again = run(&store, &export, &[]).unwrap();
    assert_eq!((again.imported, again.chats_created), (0, 0));
    assert_eq!(store.list_chats().unwrap().len(), 2);
    // Новое сообщение в уже импортированном чате доносится в тот же чат.
    let more = single(
        "personal_chat",
        PETER,
        Some("Пётр Тестов"),
        &[
            messages[0].clone(),
            messages[1].clone(),
            msg(3, SAVED_T + 5, "ещё", &theirs),
        ],
    );
    let third = run(&store, &parse(&more), &[]).unwrap();
    assert_eq!((third.imported, third.chats_created), (1, 0));
    assert_eq!(
        store
            .list_chats()
            .unwrap()
            .iter()
            .find(|c| !c.is_inbox)
            .unwrap()
            .count,
        3
    );
}

#[test]
fn personal_chat_without_a_name_is_called_telegram_and_its_id() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let export = parse(&single(
        "personal_chat",
        PETER,
        None,
        &[msg(1, SAVED_T, "привет", &format!("user{PETER}"))],
    ));
    run(&store, &export, &[]).unwrap();
    let chats = store.list_chats().unwrap();
    assert_eq!(
        chats.iter().find(|c| !c.is_inbox).unwrap().title,
        format!("Telegram {PETER}")
    );
}

#[test]
fn a_personal_chat_with_nothing_to_import_does_not_create_an_empty_chat() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let service =
        r#"{"id":1,"type":"service","date_unixtime":"1552035600","text":"","text_entities":[]}"#;
    let export = parse(&single(
        "personal_chat",
        PETER,
        Some("Пустой"),
        &[service.to_string()],
    ));
    let report = run(&store, &export, &[]).unwrap();
    assert_eq!(
        (
            report.imported,
            report.chats_created,
            report.skipped_service
        ),
        (0, 0, 1)
    );
    assert_eq!(report.backup_name, None);
    assert_eq!(store.list_chats().unwrap().len(), 1);
}

#[test]
fn only_selected_chats_are_imported_and_groups_are_ignored() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let group = format!(
        r#"{{"name":"Группа","type":"private_group","id":-1001,"messages":[{}]}}"#,
        msg(1, SAVED_T, "из группы", "user5")
    );
    let personal = single(
        "personal_chat",
        PETER,
        Some("Пётр"),
        &[msg(1, SAVED_T, "личное", &format!("user{PETER}"))],
    );
    let export = parse(&full(&[saved_n(2), personal, group]));
    let preview = store.import_preview(&export).unwrap();
    assert_eq!(preview.skipped_chats, 1);
    assert_eq!(preview.chats.len(), 2);
    assert_eq!((preview.chats[0].total, preview.chats[0].existing), (2, 0));
    let report = run(&store, &export, &[OWNER, "-1001"]).unwrap();
    assert_eq!(
        report.imported, 2,
        "группа не выбрана к импорту, личный чат не отмечен"
    );
    assert_eq!(store.list_chats().unwrap().len(), 1);
    // После импорта предпросмотр знает, что «Избранное» уже есть.
    let preview = store.import_preview(&export).unwrap();
    assert_eq!((preview.chats[0].total, preview.chats[0].existing), (2, 2));
    assert_eq!((preview.chats[1].total, preview.chats[1].existing), (1, 0));
}

#[test]
fn trashed_record_is_not_imported_again_but_a_purged_one_is() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let export = parse(&saved_n(3));
    run(&store, &export, &[]).unwrap();
    let page = feed(&store, INBOX_ID);
    store
        .delete_message(&page[0].id, &moscow(T0 + MINUTE))
        .unwrap();
    let again = run(&store, &export, &[]).unwrap();
    assert_eq!(
        (again.imported, again.skipped_existing),
        (0, 3),
        "из корзины не возвращается"
    );
    store
        .purge_message(&page[0].id, &moscow(T0 + 2 * MINUTE))
        .unwrap();
    assert_eq!(count_where(dir.path(), "1 = 1"), 2);
    let ext: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM message_ext", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ext, 2, "удаление навсегда убирает строку message_ext");
    let back = run(&store, &export, &[]).unwrap();
    assert_eq!(
        (back.imported, back.skipped_existing),
        (1, 2),
        "удалённая навсегда вернулась как новая"
    );
}

#[test]
fn progress_total_counts_only_importable_messages_like_the_chat_list() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let own = format!("user{OWNER}");
    let messages = vec![
        msg(1, SAVED_T, "раз", &own),
        r#"{"id":2,"type":"service","date_unixtime":"1552035601","actor":"Я","action":"pin_message","text":"","text_entities":[]}"#.to_string(),
        msg(3, SAVED_T + 2, "два", &own),
        r#"{"id":4,"type":"message","date_unixtime":"1552035603","from":"Я","from_id":"user1000001","text":"","text_entities":[]}"#.to_string(),
        msg(5, SAVED_T + 4, "три", &own),
    ];
    let export = parse(&single("saved_messages", OWNER, None, &messages));
    let listed = store.import_preview(&export).unwrap().chats[0].total;
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let sink = seen.clone();
    store
        .import_telegram(
            export.clone(),
            all_chats(&export),
            kinige_domain::Language::Ru,
            &now(),
            move |p| sink.lock().unwrap().push((p.done, p.total)),
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
    assert_eq!(listed, 3, "в списке чатов три записи");
    assert_eq!(
        *seen.lock().unwrap(),
        [(3, 3)],
        "«N из M» считает те же три, а не пять сообщений файла"
    );
}

#[test]
fn a_reply_across_batches_goes_to_the_owner_of_a_key_taken_between_them() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    let own = format!("user{OWNER}");
    let batch = kinige_store::IMPORT_BATCH as u64;
    // Четыре пачки: ключ «1100» в третьей, ответ на него — первое сообщение четвёртой.
    let target = 2 * batch + 100;
    let reply_id = 3 * batch + 1;
    let mut messages: Vec<String> = (1..=4 * batch)
        .map(|i| msg(i, SAVED_T + i as i64, &format!("запись {i}"), &own))
        .collect();
    messages[reply_id as usize - 1] = format!(
        r#"{{"id":{reply_id},"type":"message","date_unixtime":"{}","from":"Я","from_id":"{own}","reply_to_message_id":{target},"text":"ответ","text_entities":[{{"type":"plain","text":"ответ"}}]}}"#,
        SAVED_T + reply_id as i64
    );
    let export = parse(&single("saved_messages", OWNER, None, &messages));
    // После второй пачки (до записи третьей) ключ «1100» занимает чужая запись, например с другого устройства.
    let path = dir.path().to_path_buf();
    let owner_id = "7b2c9d10-1111-4222-8333-444455556666".to_string();
    let taken = owner_id.clone();
    let progress = move |p: kinige_domain::import::ImportProgress| {
        if p.done == 2 * batch as i64 {
            let conn = raw(&path);
            conn.busy_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            conn.execute(
                "INSERT INTO message_ext (src, ext_chat, ext_msg, message_id) VALUES ('telegram', ?1, ?2, ?3)",
                (OWNER, target.to_string(), &taken),
            )
            .unwrap();
        }
    };
    let report = store
        .import_telegram(
            export.clone(),
            all_chats(&export),
            kinige_domain::Language::Ru,
            &now(),
            progress,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
    assert_eq!(report.skipped_existing, 1, "{report:?}");
    assert_eq!(report.imported, 4 * batch as i64 - 1);
    let reply_to: Option<String> = raw(dir.path())
        .query_row(
            "SELECT m.reply_to_id FROM message m JOIN message_ext e ON e.message_id = m.id \
             WHERE e.ext_msg = ?1",
            [reply_id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        reply_to.as_deref(),
        Some(owner_id.as_str()),
        "ответ из четвёртой пачки указывает на владельца ключа, а не на id пропущенной записи третьей"
    );
}
