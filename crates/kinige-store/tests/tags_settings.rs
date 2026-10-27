//! Теги и фильтр ленты, настройки слайса 3 (свёрнутый сайдбар, недавние эмодзи).

mod common;

use common::ops::*;
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_store::Now;

fn at(minutes: i64) -> Now {
    moscow(T0 + minutes * MINUTE)
}

fn ids(page: &kinige_domain::MessagePage) -> Vec<String> {
    page.messages.iter().map(|m| m.body_md.clone()).collect()
}

#[test]
fn the_tag_filter_keeps_only_entries_of_this_chat_with_this_tag_ignoring_case() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store.create_chat("Книги", None, &at(0)).unwrap();
    store.send_message(INBOX_ID, "раз #идея", &at(1)).unwrap();
    store
        .send_message(INBOX_ID, "два #Идея и #работа", &at(2))
        .unwrap();
    store
        .send_message(INBOX_ID, "три без тега", &at(3))
        .unwrap();
    store
        .send_message(INBOX_ID, "`#идея` в коде, `a#идея` не тег", &at(4))
        .unwrap();
    store
        .send_message(&books.id, "в другом чате #идея", &at(5))
        .unwrap();
    let in_trash = store
        .send_message(INBOX_ID, "в корзине #идея", &at(6))
        .unwrap();
    store.delete_message(&in_trash.id, &at(7)).unwrap();

    let page = store
        .list_messages_tagged(INBOX_ID, None, 50, Some("идея"), &at(8))
        .unwrap();
    assert_eq!(ids(&page), ["раз #идея", "два #Идея и #работа"]);
    assert_eq!(page.messages[1].tags, ["идея", "работа"]);
    for spelling in ["ИДЕЯ", "Идея"] {
        assert_eq!(
            ids(&store
                .list_messages_tagged(INBOX_ID, None, 50, Some(spelling), &at(8))
                .unwrap())
            .len(),
            2,
            "{spelling}"
        );
    }
    assert!(
        store
            .list_messages_tagged(INBOX_ID, None, 50, Some("нет"), &at(8))
            .unwrap()
            .messages
            .is_empty()
    );
    assert_eq!(
        ids(&store
            .list_messages_tagged(&books.id, None, 50, Some("идея"), &at(8))
            .unwrap()),
        ["в другом чате #идея"]
    );
    // Без фильтра лента целая; счётчик чата тот же.
    assert_eq!(
        store
            .list_messages(INBOX_ID, None, 50, &at(8))
            .unwrap()
            .messages
            .len(),
        4
    );

    // Пагинация по курсору внутри фильтра.
    for i in 0..5 {
        store
            .send_message(INBOX_ID, &format!("страница {i} #стр"), &at(10 + i))
            .unwrap();
    }
    let first = store
        .list_messages_tagged(INBOX_ID, None, 2, Some("стр"), &at(20))
        .unwrap();
    assert_eq!(ids(&first), ["страница 3 #стр", "страница 4 #стр"]);
    let second = store
        .list_messages_tagged(INBOX_ID, first.older.as_ref(), 2, Some("стр"), &at(20))
        .unwrap();
    assert_eq!(ids(&second), ["страница 1 #стр", "страница 2 #стр"]);
}

#[test]
fn tags_follow_edits_task_toggles_appends_and_the_trash() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = store
        .send_message(INBOX_ID, "- [ ] дело #старый", &at(1))
        .unwrap();
    let tags = |tag: &str| {
        store
            .list_messages_tagged(INBOX_ID, None, 50, Some(tag), &at(9))
            .unwrap()
            .messages
            .len()
    };
    assert_eq!((tags("старый"), tags("новый")), (1, 0));
    store
        .edit_message(&sent.id, "- [ ] дело #новый", &at(2))
        .unwrap();
    assert_eq!(
        (tags("старый"), tags("новый")),
        (0, 1),
        "правка пересчитала теги"
    );
    store.toggle_task_item(&sent.id, 0, true, &at(3)).unwrap();
    assert_eq!(tags("новый"), 1);
    // Дописывание из окна захвата тоже обновляет теги.
    let appended = store.capture_append("и ещё #хвост", &at(4)).unwrap();
    assert_eq!(appended.message.id, sent.id);
    assert_eq!(appended.message.tags, ["новый", "хвост"]);
    assert_eq!(tags("хвост"), 1);
    // Таблица совпадает с тем, что получится из текстов (она производная).
    let rows: Vec<(String, String)> = raw(dir.path())
        .prepare("SELECT message_id, tag FROM message_tag ORDER BY tag")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        [
            (sent.id.clone(), "новый".to_string()),
            (sent.id, "хвост".to_string())
        ]
    );
}

#[test]
fn the_sidebar_state_and_recent_emoji_persist() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        let settings = store.get_settings().unwrap();
        assert!(!settings.sidebar_collapsed);
        assert!(settings.recent_emoji.is_empty());
        store.set_sidebar_collapsed(true).unwrap();
        assert_eq!(store.add_recent_emoji("📚").unwrap(), ["📚"]);
        assert_eq!(store.add_recent_emoji("🎬").unwrap(), ["🎬", "📚"]);
        assert_eq!(
            store.add_recent_emoji("📚").unwrap(),
            ["📚", "🎬"],
            "повтор переезжает вперёд"
        );
        assert!(store.add_recent_emoji("").is_err());
        assert!(store.add_recent_emoji("два слова").is_err());
    }
    wait_closed(dir.path());
    let store = open_without_mirror(dir.path());
    let settings = store.get_settings().unwrap();
    assert!(
        settings.sidebar_collapsed,
        "свёрнутость пережила перезапуск"
    );
    assert_eq!(settings.recent_emoji, ["📚", "🎬"]);
    store.set_sidebar_collapsed(false).unwrap();
    assert!(!store.get_settings().unwrap().sidebar_collapsed);
}

#[test]
fn recent_emoji_are_limited_and_survive_a_broken_stored_value() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let pool: Vec<String> = ('😀'..='😿').take(40).map(|c| c.to_string()).collect();
    for emoji in &pool {
        store.add_recent_emoji(emoji).unwrap();
    }
    let recent = store.get_settings().unwrap().recent_emoji;
    assert_eq!(recent.len(), 24);
    assert_eq!(recent[0], pool[39]);
    raw(dir.path())
        .execute(
            "UPDATE app_setting SET value = '{не json' WHERE key = 'recent_emoji'",
            [],
        )
        .unwrap();
    assert!(
        store.get_settings().unwrap().recent_emoji.is_empty(),
        "недавние не стоят ошибки"
    );
    assert_eq!(store.add_recent_emoji("📚").unwrap(), ["📚"]);
}
