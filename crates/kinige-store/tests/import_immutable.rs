//! Импорт из Telegram (слайс 4, задача 4.8), неизменяемость: `origin` и `ext` переживают правки, а дописывание из окна
//! захвата никогда не липнет к импортированной записи.

mod common;

use common::import::*;
use common::ops::*;
use common::*;
use kinige_domain::MessageFlag;
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
fn origin_and_ext_survive_edit_move_date_and_flags() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    run(&store, &parse(&saved_n(1)), &[]).unwrap();
    let id = feed(&store, INBOX_ID)[0].id.clone();
    let core_before = groups(dir.path(), "message", &id)["core"].clone();
    let chat = store.create_chat("Куда", None, &moscow(T0)).unwrap();
    store
        .edit_message(&id, "правка", &moscow(T0 + MINUTE))
        .unwrap();
    store
        .set_message_date(&id, "2019-01-02", &moscow(T0 + 2 * MINUTE))
        .unwrap();
    store
        .move_message(&id, &chat.id, &moscow(T0 + 3 * MINUTE))
        .unwrap();
    store
        .set_message_flag(&id, MessageFlag::Favorite, true, &moscow(T0 + 4 * MINUTE))
        .unwrap();
    assert_eq!(
        groups(dir.path(), "message", &id)["core"],
        core_before,
        "core неизменяема"
    );
    let (origin, ext): (String, String) = raw(dir.path())
        .query_row(
            "SELECT origin, ext_json FROM message WHERE id = ?1",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(origin, "import_telegram");
    assert!(ext.contains("\"telegram\""));
    let found: String = raw(dir.path())
        .query_row(
            "SELECT message_id FROM message_ext WHERE ext_msg = '1'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(found, id);
}

#[test]
fn capture_append_never_attaches_to_an_imported_record() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_plain(dir.path());
    // Импортированное сообщение «сегодняшнее» и свежее: по времени дописывание было бы допустимо.
    let at = T0 / 1000 - 120;
    let own = format!("user{OWNER}");
    let export = parse(&single(
        "saved_messages",
        OWNER,
        None,
        &[msg(1, at, "из телеграма", &own)],
    ));
    run(&store, &export, &[]).unwrap();
    let outcome = store
        .capture_append("из окна захвата", &moscow(T0))
        .unwrap();
    assert_eq!(outcome.action, kinige_domain::CaptureAction::Created);
    let page = feed(&store, INBOX_ID);
    assert_eq!(page.len(), 2);
    assert_eq!(
        page.iter()
            .find(|m| m.body_md == "из телеграма")
            .unwrap()
            .body_md,
        "из телеграма"
    );
    let view = store.capture_view(&moscow(T0)).unwrap();
    assert!(
        view.append_target.is_none()
            || view.append_target.as_deref() == Some(outcome.message.id.as_str()),
        "целью дописывания не бывает импортированная запись"
    );
}
