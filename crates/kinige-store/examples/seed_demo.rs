//! Демо-данные для проверки слайсов 3–5 в настоящем окне (скриншоты, ручная проверка): чаты, записи, теги, задачи, корзина;
//! слайс 5 — записи Inbox на сегодняшний день журнала 1, 3, 4, 6 и 9 лет назад («В этот день») и синтетический личный чат
//! Telegram с ответами. Только для разработки: пишет во временный каталог, который передан аргументом, и ничего не делает с
//! настоящей базой. Все тексты синтетические. Слайс 6 — синтетические год и прошлый год записей для леса: все стадии роста,
//! неделя без записей, дни с несколькими чатами, запись в корзине; сегодняшнее дерево стоит на третьей стадии у самой границы
//! четвёртой (одна запись в сто знаков поднимает его). Лес засевается только при `KINIGE_SEED_FOREST=1`, чтобы сценарии прежних
//! слайсов не меняли свои данные.
//!
//! Использование: `cargo run -p kinige-store --example seed_demo -- <каталог данных> [<папка Markdown-копии>]`

// Пример — консольная утилита: печать в stdout и падение на любой ошибке разрешены (правила «только через log» и
// «никогда не падать» — для кода приложения).
#![allow(
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::string_slice
)]

use chrono::{Datelike, Duration, NaiveDate, TimeZone};
use chrono_tz::Tz;
use kinige_domain::forest::{NoteStat, day_weight};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, journal_date};
use kinige_domain::telegram::parse_export;
use kinige_domain::{Language, MessageFlag};
use kinige_store::{Now, Store, StoreOptions};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;

fn main() {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().expect("каталог данных"));
    let mirror = args.next().map(PathBuf::from);
    let mut options = StoreOptions::new(data.join("kinige.db"), mirror);
    options.start_maintenance = false;
    let store = Store::open(options).expect("хранилище открывается");

    let now = Now::system();
    let at = |ms: i64| Now::at(now.ms + ms, &now.tz);
    let send = |chat: &str, body: &str, ago: i64| {
        store
            .send_message(chat, body, &at(-ago))
            .expect("запись")
            .id
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
    // Чаты без эмодзи (хранятся с «💬» по умолчанию): сайдбар показывает для них плитку с первой буквой названия (слайс D2).
    for (days_ago, title) in [(25, "Проекты"), (24, "Linux notes"), (23, "Мама")] {
        history.push(
            store
                .create_chat(title, None, &at(-days_ago * DAY))
                .unwrap()
                .id,
        );
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
    // Копия дня для строки в «Настройках».
    store.run_maintenance(&at(0)).unwrap();
    println!("готово: {}", data.display());
}

/// Сколько лет назад лежат записи «В этот день» (слайс 5): три новых года показываются подробно, остальные — строкой.
const PAST_YEARS: [i32; 7] = [1, 3, 4, 6, 9, 12, 16];

/// Записи Inbox на сегодняшний день журнала в прошлые годы. «Сегодня» считается от `journal_date` запуска (граница 04:00),
/// а не от календарного дня UTC; 29 февраля совпадает только с 29 февраля, поэтому недостающие годы берутся из запасных.
fn seed_on_this_day(store: &Store, now: &Now) {
    let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
    let today = NaiveDate::parse_from_str(&today, "%Y-%m-%d").expect("день журнала");
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
            let local = date.and_hms_opt(9 + 2 * n as u32, 12, 0).expect("время");
            let ms = tz
                .from_local_datetime(&local)
                .earliest()
                .expect("местное время существует")
                .timestamp_millis();
            let when = Now::at(ms, &now.tz);
            let id = store
                .send_message(INBOX_ID, body, &when)
                .expect("запись прошлого года")
                .id;
            // Вторая запись третьего года (3 года назад) уходит в корзину: в карточке её нет.
            if (year, n) == (1, 1) {
                store.delete_message(&id, now).expect("в корзину");
            }
        }
    }
}

/// Синтетический личный чат Telegram «Анна» с ответами: на свою и чужую реплику, чужая реплика на мою и ответ на реплику,
/// которой нет в файле. JSON строится здесь и идёт через `parse_export` и импорт хранилища, как настоящий файл.
fn seed_telegram_chat(store: &Store, now: &Now) {
    const ME: &str = "user1000001";
    const ANNA: &str = "user2000002";
    let base = 1_678_787_400i64; // 2023-03-14 10:30 UTC
    let message = |id: u64, from: &str, name: &str, text: &str, reply: Option<u64>, at: i64| {
        let text = serde_json::to_string(text).expect("текст");
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
    let export = Arc::new(parse_export(&json).expect("синтетический экспорт разбирается"));
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
        .expect("импорт синтетического чата");
}

/// Seed for the forest days: the same journal on every run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Entries per active day and how likely each count is (percent): every growth stage appears (stage 6 needs a weight of 22.3,
/// about 28 entries when four in five are long).
const FOREST_COUNTS: [(u64, u64); 8] = [
    (0, 12),
    (1, 24),
    (2, 26),
    (3, 14),
    (4, 10),
    (9, 8),
    (13, 4),
    (26, 2),
];

const FOREST_PHRASES: [&str; 8] = [
    "Прошёлся к реке и подумал о неделе",
    "**Черновик** плана на осень, три пункта",
    "Короткий звонок, ничего нового",
    "Прочитал две главы перед сном",
    "Саду снова нужна вода",
    "Чай, дождь и длинный список мелких дел",
    "Дописал письмо, которое откладывал",
    "Тихий день с открытым окном",
];

/// Text of exactly `len` characters made of the forest phrases, so that long entries look like entries (the weight of a day
/// depends on the length, so the length is exact).
fn filler(len: usize, start: u64) -> String {
    let mut text = String::new();
    let mut n = start as usize;
    while text.chars().count() < len {
        text.push_str(FOREST_PHRASES[n % FOREST_PHRASES.len()]);
        text.push_str(". ");
        n += 3;
    }
    text.chars().take(len).collect()
}

/// A year of the current journal year and the year before it for the forest (slice 6): every stage, a week without entries
/// (45 days before today in the current year, the first week of August in the last), days with entries in two or three
/// chats, one entry in the trash. Today's tree is left at stage 3 with weight 6.2: one entry of a hundred characters (weight
/// 0.5) lifts it to stage 4.
fn seed_forest(store: &Store, now: &Now, history: &[String], old: &str, data: &std::path::Path) {
    let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
    let today = NaiveDate::parse_from_str(&today, "%Y-%m-%d").expect("день журнала");
    let tz: Tz = now.tz.parse().unwrap_or(chrono_tz::UTC);
    let at = |date: NaiveDate, minutes: u32| {
        let local = date
            .and_hms_opt(6 + minutes / 60, minutes % 60, 0)
            .expect("время");
        let ms = tz
            .from_local_datetime(&local)
            .earliest()
            .expect("местное время существует")
            .timestamp_millis();
        Now::at(ms, &now.tz)
    };
    let long = |n: u64| filler(200 + (n % 80) as usize, n);
    for (seed, year) in [
        (0x5EED_6001u64, today.year()),
        (0x5EED_6002, today.year() - 1),
    ] {
        let mut rng = Rng(seed);
        let last = if year == today.year() {
            today
        } else {
            NaiveDate::from_ymd_opt(year, 12, 31).expect("конец года")
        };
        let gap = if year == today.year() {
            today - Duration::days(45)
        } else {
            NaiveDate::from_ymd_opt(year, 8, 3).expect("начало августа")
        };
        let mut date = NaiveDate::from_ymd_opt(year, 1, 1).expect("начало года");
        while date <= last {
            // Today is set up separately below.
            if date == today || (date >= gap && date < gap + Duration::days(7)) {
                date += Duration::days(1);
                continue;
            }
            let roll = rng.below(100);
            let mut acc = 0;
            let count = FOREST_COUNTS
                .iter()
                .find(|(_, percent)| {
                    acc += percent;
                    roll < acc
                })
                .map_or(1, |(count, _)| *count);
            // The history is spread over several chats and the previous year sits in the archive: the feed of any chat the
            // scenario opens stays short (the window tree is read through accessibility, one element at a time). Only the
            // last ten days are in the Inbox.
            let recent = year == today.year() && date >= today - Duration::days(10);
            let home = if year != today.year() {
                old
            } else if recent {
                INBOX_ID
            } else {
                &history[(date.ordinal() as usize / 5) % history.len()]
            };
            // Busy days are split between two chats: the day target then picks the chat with more entries.
            let second = if count >= 4 && !recent && year == today.year() {
                &history[(date.ordinal() as usize / 5 + 1) % history.len()]
            } else {
                home
            };
            for n in 0..count {
                let body = if n == 0 && count == 1 && rng.below(100) < 15 {
                    "да".to_string()
                } else if n == 0 {
                    // The day is in the text: a scenario finds the first entry of a day by it.
                    // Long enough to weigh as much as any other entry (200 characters), so that one entry makes stage 1.
                    format!("{} · {date}", filler(188, rng.next()))
                } else {
                    long(rng.next())
                };
                let chat = if n % 3 == 2 { second } else { home };
                store
                    .send_message(chat, &body, &at(date, (n * 20) as u32))
                    .expect("запись леса");
            }
            date += Duration::days(1);
        }
    }

    // One day with an entry in the trash: the forest counts only the live ones.
    let day = today - Duration::days(20);
    store
        .send_message(INBOX_ID, "Первая запись дня, остаётся", &at(day, 0))
        .expect("запись");
    let doomed = store
        .send_message(INBOX_ID, "Эта запись уйдёт в корзину", &at(day, 20))
        .expect("запись")
        .id;
    store.delete_message(&doomed, now).expect("в корзину");

    // Today's tree: stage 3, weight 6.2.
    let db = rusqlite::Connection::open(data.join("kinige.db")).expect("база");
    let mut statement = db
        .prepare("SELECT body_md FROM message WHERE journal_date = ?1 AND deleted_at IS NULL")
        .expect("запрос");
    let have: Vec<NoteStat> = statement
        .query_map([today.format("%Y-%m-%d").to_string()], |row| {
            row.get::<_, String>(0)
        })
        .expect("записи сегодня")
        .map(|body| NoteStat {
            len: body.expect("текст").chars().count(),
            attachments: 0,
        })
        .collect();
    let mut weight = day_weight(&have);
    let mut seconds = 1;
    let pad = |len: usize, seconds: i64| {
        store
            .send_message(
                INBOX_ID,
                &filler(len, seconds as u64),
                &Now::at(now.ms - seconds * 1000, &now.tz),
            )
            .expect("запись сегодня");
    };
    while weight + 1.0 <= 6.2 {
        pad(220, seconds);
        weight += 1.0;
        seconds += 1;
    }
    let rest = (6.2 - weight) * 200.0;
    if rest >= 3.0 {
        pad(rest.round() as usize, seconds);
    }
}
