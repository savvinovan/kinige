//! ВРЕМЕННО (KINIG-40): запись с длинной строкой кода и абзацем после неё. Удаляется вместе с tmp-canvas-check.yml.
#![allow(clippy::print_stdout)]

use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store, StoreOptions};
use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().expect("каталог данных"));
    let mirror = args.next().map(PathBuf::from);
    let mut options = StoreOptions::new(data.join("kinige.db"), mirror);
    options.start_maintenance = false;
    let store = Store::open(options).expect("хранилище открывается");
    let now = Now::system();
    let body = "Вот команда сборки:\n\n```\ncargo build --release --workspace --all-targets --features sync,encryption,import-telegram,search-morphology,markdown-copy --target-dir /var/tmp/kinige-build-output/release-candidate\n```\n\nПосле неё запускаю проверки и жду зелёного результата.";
    // Лента длиннее окна: появляется вертикальная полоса прокрутки ленты (тёмная ли она).
    for index in 0..8 {
        let filler = format!(
            "Заполнитель ленты номер {index}: нужен, чтобы лента не помещалась в окно и прокручивалась."
        );
        store
            .send_message(INBOX_ID, &filler, &Now::at(now.ms + index, &now.tz))
            .expect("запись");
    }
    store
        .send_message(INBOX_ID, body, &Now::at(now.ms + 100, &now.tz))
        .expect("запись");
    println!("ok");
}
