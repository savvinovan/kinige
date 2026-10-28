//! Демо-данные для проверки слайса 3 в настоящем окне (скриншоты, ручная проверка): чаты, записи, теги, задачи, корзина.
//! Только для разработки: пишет во временный каталог, который передан аргументом, и ничего не делает с настоящей базой.
//!
//! Использование: `cargo run -p kinige-store --example seed_demo -- <каталог данных> [<папка Markdown-копии>]`

// Пример — консольная утилита: печать в stdout разрешена (правило «только через log» — для кода приложения).
#![allow(clippy::print_stdout)]

use kinige_domain::MessageFlag;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store, StoreOptions};
use std::path::PathBuf;

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
    // Копия дня для строки в «Настройках».
    store.run_maintenance(&at(0)).unwrap();
    println!("готово: {}", data.display());
}
