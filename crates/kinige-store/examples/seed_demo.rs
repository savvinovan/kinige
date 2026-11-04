//! Demo data for checking slices 3-5 in the real window (screenshots, manual checks): chats, entries, tags, tasks, trash;
//! slice 5 - Inbox entries on today's journal date 1, 3, 4, 6 and 9 years ago ("On this day") and a synthetic personal
//! Telegram chat with replies. Development only: it writes to a temporary directory passed as an argument and does nothing
//! to the real database. All texts are synthetic. Slice 6 - a synthetic current year and previous year of entries for the
//! forest: every growth stage, a week without entries, days with several chats, an entry in the trash; today's tree is at
//! the third stage right at the border of the fourth (one entry of a hundred characters raises it). The forest is seeded
//! only with `KINIGE_SEED_FOREST=1`, so that the scenarios of earlier slices do not change their data. Slice 7 - drawn
//! pictures and documents attached to entries, pending tiles, a missing file, a trashed photo and a Telegram import with
//! photos, only with `KINIGE_SEED_ATTACH=1`.
//!
//! Usage: `cargo run -p kinige-store --example seed_demo -- <data directory> [<Markdown copy folder>]`

// The example is a console utility: printing to stdout and panicking on any error are allowed (the rules "only through
// log" and "never panic" are for app code).
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::string_slice
)]

use chrono::{NaiveDate, TimeZone};
use chrono_tz::Tz;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::telegram::parse_export;
use kinige_domain::{ChatColor, Language, MessageFlag};
use kinige_store::{Now, Store, StoreOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[path = "seed_demo/attach.rs"]
mod attach;
#[path = "seed_demo/forest.rs"]
mod forest;

use attach::seed_attachments;
use forest::seed_forest;

const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;

fn main() {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().expect("data directory"));
    let mirror = args.next().map(PathBuf::from);
    let mut options = StoreOptions::new(data.join("kinige.db"), mirror);
    options.start_maintenance = false;
    let store = Store::open(options).expect("the store opens");

    let now = Now::system();
    let at = |ms: i64| Now::at(now.ms + ms, &now.tz);
    let send = |chat: &str, body: &str, ago: i64| {
        store.send_message(chat, body, &at(-ago)).expect("entry").id
    };

    let books = store
        .create_chat("Книги", Some("📚"), &at(-30 * DAY))
        .unwrap();
    let ideas = store
        .create_chat("Идеи", Some("💡"), &at(-29 * DAY))
        .unwrap();
    let work = store
        .create_chat("Работа", Some("💼"), &at(-28 * DAY))
        .unwrap();
    let old = store
        .create_chat("Старые заметки", Some("🗄️"), &at(-27 * DAY))
        .unwrap();
    let long_name = store
        .create_chat(
            "Очень длинное название чата, которое не помещается в строку сайдбара",
            Some("🌿"),
            &at(-26 * DAY),
        )
        .unwrap();
    let mut history = vec![books.id.clone(), work.id.clone(), long_name.id.clone()];
    // Chats without an emoji (stored with "💬" by default): the sidebar shows a tile with the first letter of the name for them (slice D2).
    for (days_ago, title) in [(25, "Проекты"), (24, "Linux notes"), (23, "Мама")] {
        let chat = store
            .create_chat(title, None, &at(-days_ago * DAY))
            .unwrap();
        // One chat has an explicit colour (KINIG-59); the others keep the automatic one.
        if title == "Linux notes" {
            store
                .set_chat_color(&chat.id, ChatColor::Grey, &at(-days_ago * DAY))
                .unwrap();
        }
        history.push(chat.id);
    }

    send(
        INBOX_ID,
        "Проснулся рано и до завтрака записал мысль: _пишешь как в мессенджере, перечитываешь как книгу_.",
        2 * DAY + 9 * 60 * MINUTE,
    );
    let task = send(
        INBOX_ID,
        "### Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [ ] Почитать книгу",
        3 * 60 * MINUTE,
    );
    send(
        INBOX_ID,
        "Идея для приложения #идея: показывать год как лес.",
        2 * 60 * MINUTE,
    );
    let fav = send(
        INBOX_ID,
        "Купить билеты на субботу и позвонить маме",
        90 * MINUTE,
    );
    let doomed = send(INBOX_ID, "Эта запись уйдёт в корзину #идея", 60 * MINUTE);
    let in_trash = send(books.id.as_str(), "Дюна — перечитать к зиме", 5 * DAY);
    send(
        books.id.as_str(),
        "Прочитал «Конец вечности». Сильная концовка. #книги",
        45 * MINUTE,
    );
    send(
        ideas.id.as_str(),
        "Лента как журнал: серии записей ближе друг к другу. #идея",
        30 * MINUTE,
    );
    send(
        work.id.as_str(),
        "Созвон в 15:00, подготовить заметки #работа",
        20 * MINUTE,
    );
    send(old.id.as_str(), "Старая заметка из архива", 40 * DAY);

    store
        .set_message_flag(&task, MessageFlag::Task, true, &at(-2 * MINUTE))
        .unwrap();
    store
        .set_message_flag(&fav, MessageFlag::Favorite, true, &at(-2 * MINUTE))
        .unwrap();
    store
        .edit_message(&task, "### Планы на выходные\n- [x] Поход в горы\n- [ ] Разобрать фотографии\n- [ ] Почитать книгу\n\nПогода обещает быть хорошей.", &at(-MINUTE))
        .unwrap();
    store.delete_message(&doomed, &at(-30 * MINUTE)).unwrap();
    store.delete_message(&in_trash, &at(-3 * DAY)).unwrap();
    store.set_chat_pinned(&work.id, true, &at(-DAY)).unwrap();
    store.set_chat_archived(&old.id, true, &at(-DAY)).unwrap();
    seed_on_this_day(&store, &now);
    // The forest year (slice 6) only when asked: the scenarios of the earlier slices keep their data.
    if std::env::var_os("KINIGE_SEED_FOREST").is_some() {
        for (days_ago, title, icon) in [
            (22, "Фильмы", "🎬"),
            (21, "Путешествия", "✈️"),
            (20, "Здоровье", "💪"),
            (19, "Финансы", "💰"),
            (18, "Кухня", "🍳"),
        ] {
            history.push(
                store
                    .create_chat(title, Some(icon), &at(-days_ago * DAY))
                    .unwrap()
                    .id,
            );
        }
        seed_forest(&store, &now, &history, &old.id, &data);
    }
    seed_telegram_chat(&store, &now);
    // Photos and files (slice 7) only when asked, like the forest.
    if std::env::var_os("KINIGE_SEED_ATTACH").is_some() {
        seed_attachments(&store, &now, &data, &ideas.id);
    }
    // The day's copy for the line in "Settings".
    store.run_maintenance(&at(0)).unwrap();
    println!("done: {}", data.display());
}

/// How many years ago the "On this day" entries lie (slice 5): the three newest years are shown in detail, the rest as a line.
const PAST_YEARS: [i32; 7] = [1, 3, 4, 6, 9, 12, 16];

/// Inbox entries on today's journal date in past years. "Today" is counted from the run's `journal_date` (the 04:00 boundary),
/// not from the UTC calendar day; 29 February matches only 29 February, so the missing years are taken from the spare ones.
fn seed_on_this_day(store: &Store, now: &Now) {
    let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
    let today = NaiveDate::parse_from_str(&today, "%Y-%m-%d").expect("journal day");
    let tz: Tz = now.tz.parse().unwrap_or(chrono_tz::UTC);
    let years: Vec<NaiveDate> = PAST_YEARS
        .iter()
        .filter_map(|ago| {
            use chrono::Datelike;
            NaiveDate::from_ymd_opt(today.year() - ago, today.month(), today.day())
        })
        .take(5)
        .collect();
    let long = "Нужно начать готовиться к системному дизайну. Посмотреть материалы по Raft и распределённым транзакциям, \
        разобрать, чем консенсус отличается от двухфазной фиксации, и записать вопросы, которые остались после чтения. \
        Потом вернуться к этой записи через неделю и проверить, на сколько удалось ответить. Ещё раз пройтись по конспекту \
        и выписать то, что осталось непонятным.";
    let texts: [&[&str]; 5] = [
        &[
            "Проснулся рано, до завтрака записал мысль про журнал.",
            "Купил билеты на субботу.",
            "Позвонил маме, договорились на воскресенье.",
        ],
        &[
            "Так будет лучше: перестать откладывать и написать первую страницу.",
            "Эта запись уйдёт в корзину",
        ],
        &[
            "Первый день на новой работе. Всё незнакомое, но люди приятные.",
            long,
        ],
        &["Дочитал «Конец вечности». Сильная концовка."],
        &[
            "Записи за этот день из старого архива.",
            "Вечером разбирал фотографии.",
            "Составил список дел на неделю #планы",
            "Забыл ключи, ждал соседку.",
            "Сходил в кино, фильм средний.",
        ],
    ];
    for (year, (date, bodies)) in years.iter().zip(texts).enumerate() {
        for (n, body) in bodies.iter().enumerate() {
            let local = date.and_hms_opt(9 + 2 * n as u32, 12, 0).expect("time");
            let ms = tz
                .from_local_datetime(&local)
                .earliest()
                .expect("local time exists")
                .timestamp_millis();
            let when = Now::at(ms, &now.tz);
            let id = store
                .send_message(INBOX_ID, body, &when)
                .expect("entry of a past year")
                .id;
            // The second entry of the third year (3 years ago) goes to the trash: it is not in the card.
            if (year, n) == (1, 1) {
                store.delete_message(&id, now).expect("to the trash");
            }
        }
    }
}

/// A synthetic personal Telegram chat "Анна" with replies: to one's own and to the other's message, the other's reply to
/// mine, and a reply to a message that is not in the file. The JSON is built here and goes through `parse_export` and the
/// store import, like a real file.
fn seed_telegram_chat(store: &Store, now: &Now) {
    const ME: &str = "user1000001";
    const ANNA: &str = "user2000002";
    let base = 1_678_787_400i64; // 2023-03-14 10:30 UTC
    let message = |id: u64, from: &str, name: &str, text: &str, reply: Option<u64>, at: i64| {
        let text = serde_json::to_string(text).expect("text");
        let reply = reply
            .map(|r| format!(r#""reply_to_message_id":{r},"#))
            .unwrap_or_default();
        format!(
            r#"{{"id":{id},"type":"message","date":"x","date_unixtime":"{at}","from":"{name}","from_id":"{from}",{reply}"text":{text},"text_entities":[{{"type":"plain","text":{text}}}]}}"#
        )
    };
    let messages = [
        message(
            1,
            ANNA,
            "Анна К.",
            "Привет! Ты вчера забыл у меня книгу",
            None,
            base,
        ),
        message(2, ME, "Я", "Ой, заберу в субботу", None, base + 60),
        message(
            3,
            ANNA,
            "Анна К.",
            "Ну что, едем в субботу?",
            None,
            base + 3_600,
        ),
        message(4, ME, "Я", "Едем! Бери зонт", Some(3), base + 3_700),
        message(5, ME, "Я", "Спасибо, что сохранила", Some(1), base + 3_800),
        message(
            6,
            ANNA,
            "Анна К.",
            "Не за что, я рада",
            Some(5),
            base + 3_900,
        ),
        message(
            7,
            ME,
            "Я",
            "Это ответ на сообщение, которого нет в файле",
            Some(999),
            base + 4_000,
        ),
    ];
    let json = format!(
        r#"{{"personal_information":{{"user_id":1000001}},"chats":{{"list":[{{"name":"Анна","type":"personal_chat","id":2000002,"messages":[{}]}}]}}}}"#,
        messages.join(",")
    );
    let export = Arc::new(parse_export(&json).expect("the synthetic export parses"));
    let ids = export.chats.iter().map(|c| c.id.clone()).collect();
    store
        .import_telegram(
            export,
            ids,
            Language::Ru,
            now,
            |_| {},
            Arc::new(AtomicBool::new(false)),
        )
        .expect("import of the synthetic chat");
}
