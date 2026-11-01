//! Демо-данные для проверки слайсов 3–5 в настоящем окне (скриншоты, ручная проверка): чаты, записи, теги, задачи, корзина;
//! слайс 5 — записи Inbox на сегодняшний день журнала 1, 3, 4, 6 и 9 лет назад («В этот день») и синтетический личный чат
//! Telegram с ответами. Только для разработки: пишет во временный каталог, который передан аргументом, и ничего не делает с
//! настоящей базой. Все тексты синтетические.
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

use chrono::{NaiveDate, TimeZone};
use chrono_tz::Tz;
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
    store
        .create_chat(
            "Очень длинное название чата, которое не помещается в строку сайдбара",
            Some("🌿"),
            &at(-26 * DAY),
        )
        .unwrap();

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
