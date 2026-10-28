//! Замер импорта и поиска (слайс 4): время разбора, импорта, поиска и холодного старта, размер базы до и после. Печатает
//! только числа и время — тексты, имена и пути из файла экспорта не выводятся.
//!
//! Использование (каталог данных — временный, не настоящая база!):
//! `cargo run --release -p kinige-store --example import_bench -- <каталог данных> synthetic <N>`
//! `cargo run --release -p kinige-store --example import_bench -- <каталог данных> file <result.json>`

use kinige_domain::Language;
use kinige_domain::SearchFilters;
use kinige_domain::telegram::{TgExport, parse_export};
use kinige_store::{Now, Store, StoreOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().expect("каталог данных"));
    let mode = args.next().expect("synthetic | file");
    let arg = args.next().expect("N или путь к result.json");
    let json = match mode.as_str() {
        "synthetic" => synthetic(arg.parse().expect("число сообщений")),
        "file" => std::fs::read_to_string(&arg).expect("файл читается"),
        other => panic!("неизвестный режим {other}"),
    };
    std::fs::create_dir_all(&data).unwrap();
    let options = || {
        let mut options = StoreOptions::new(data.join("kinige.db"), Some(data.join("mirror")));
        options.start_maintenance = false;
        options
    };
    let t = Instant::now();
    let store = Store::open(options()).unwrap();
    println!("открытие пустой базы: {:.0} мс", ms(t));
    let size_before = db_size(&data);

    let t = Instant::now();
    let export: Arc<TgExport> = Arc::new(parse_export(&json).expect("экспорт разбирается"));
    let messages: usize = export.chats.iter().map(|c| c.messages.len()).sum();
    println!(
        "разбор: {:.0} мс, {} МБ JSON, чатов {}, сообщений {messages}",
        ms(t),
        json.len() / 1_000_000,
        export.chats.len()
    );
    drop(json);
    let t = Instant::now();
    let preview = store.import_preview(&export).unwrap();
    println!(
        "предпросмотр: {:.0} мс, чатов к импорту {}, записей {}",
        ms(t),
        preview.chats.len(),
        preview.chats.iter().map(|c| c.total).sum::<i64>()
    );

    let ids = export.chats.iter().map(|c| c.id.clone()).collect();
    let t = Instant::now();
    let report = store
        .import_telegram(
            export.clone(),
            ids,
            Language::Ru,
            &Now::system(),
            |_| {},
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    let elapsed = ms(t);
    println!(
        "импорт: {:.0} мс ({:.1} мкс на сообщение); импортировано {}, пропущено: уже были {}, служебных {}, пустых {}; \
         заглушек вложений {}; чатов создано {}; копия: {}",
        elapsed,
        elapsed * 1000.0 / report.imported.max(1) as f64,
        report.imported,
        report.skipped_existing,
        report.skipped_service,
        report.skipped_empty,
        report.media_placeholders,
        report.chats_created,
        report.backup_name.is_some()
    );
    let t = Instant::now();
    let again = store
        .import_telegram(
            export.clone(),
            export.chats.iter().map(|c| c.id.clone()).collect(),
            Language::Ru,
            &Now::system(),
            |_| {},
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
    println!(
        "повторный импорт: {:.0} мс, новых {}, уже были {}",
        ms(t),
        again.imported,
        again.skipped_existing
    );
    drop(store);
    let size_after = db_size(&data);
    println!(
        "размер базы: до {:.1} МБ, после {:.1} МБ",
        size_before as f64 / 1e6,
        size_after as f64 / 1e6
    );

    // Холодный старт после импорта: новое открытие базы и первая страница ленты.
    let t = Instant::now();
    let store = Store::open(options()).unwrap();
    let open = ms(t);
    let t = Instant::now();
    let page = store
        .list_messages(kinige_domain::groups::INBOX_ID, None, 200, &Now::system())
        .unwrap();
    println!(
        "холодный старт: открытие {open:.0} мс, первая страница ({} записей) {:.0} мс",
        page.messages.len(),
        ms(t)
    );

    // Поиск: слова берутся из самих записей, не печатаются.
    let words = sample_words(&data, 200);
    let mut times = Vec::new();
    let mut found = 0usize;
    for word in &words {
        let t = Instant::now();
        let result = store
            .search(word, &SearchFilters::default(), None, 50)
            .unwrap();
        times.push(ms(t));
        found += result.hits.len();
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if !times.is_empty() {
        println!(
            "поиск по {} словам из записей (префиксный запрос, страница 50): p50 {:.1} мс, p95 {:.1} мс, max {:.1} мс; найдено в среднем {:.1}",
            times.len(),
            times[times.len() / 2],
            times[(times.len() * 95).div_ceil(100) - 1],
            times[times.len() - 1],
            found as f64 / times.len() as f64
        );
    }
    for word in ["и", "в", "не"] {
        let t = Instant::now();
        let result = store
            .search(&format!("{word} "), &SearchFilters::default(), None, 50)
            .unwrap();
        println!(
            "поиск частого слова из одной буквы: {:.1} мс, найдено на странице {}",
            ms(t),
            result.hits.len()
        );
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn db_size(dir: &Path) -> u64 {
    ["kinige.db", "kinige.db-wal"]
        .iter()
        .filter_map(|f| std::fs::metadata(dir.join(f)).ok())
        .map(|m| m.len())
        .sum()
}

/// Слова длиной не меньше четырёх знаков из записей базы, по одному на каждую `n`-ю запись.
fn sample_words(dir: &Path, n: usize) -> Vec<String> {
    let conn = rusqlite::Connection::open(dir.join("kinige.db")).unwrap();
    let mut statement = conn
        .prepare("SELECT body_md FROM message ORDER BY random() LIMIT ?1")
        .unwrap();
    let rows = statement
        .query_map([n as i64 * 3], |row| row.get::<_, String>(0))
        .unwrap();
    let mut words = Vec::new();
    for body in rows.flatten() {
        if let Some(word) = body
            .split(|c: char| !c.is_alphanumeric())
            .find(|w| w.chars().count() >= 4)
        {
            // Незаконченное слово — путь, которым ходит поиск при наборе.
            words.push(word.chars().take(word.chars().count().max(4) - 1).collect());
        }
        if words.len() >= n {
            break;
        }
    }
    words
}

/// Экспорт «Избранного» из `n` сообщений за пять лет: слова по Ципфу из псевдословаря, часть сообщений с форматированием.
fn synthetic(n: usize) -> String {
    let mut state = 0x5EED_1234_ABCD_9876u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let onset = [
        "б", "в", "г", "д", "з", "к", "л", "м", "н", "п", "р", "с", "т", "бр", "кр", "пр", "тр",
        "ст",
    ];
    let vowel = ["а", "е", "и", "о", "у", "я"];
    let coda = ["", "н", "р", "л", "т", "к", "с", "м"];
    let ending = ["", "а", "у", "ом", "е", "ы", "ой", "ая", "ть", "л", "ет"];
    let vocabulary: Vec<String> = (0..40_000)
        .map(|_| {
            let mut word = String::new();
            for _ in 0..2 + next() as usize % 3 {
                word.push_str(onset[next() as usize % onset.len()]);
                word.push_str(vowel[next() as usize % vowel.len()]);
                word.push_str(coda[next() as usize % coda.len()]);
            }
            word.push_str(ending[next() as usize % ending.len()]);
            word
        })
        .collect();
    let start = 1_514_764_800i64; // 2018-01-01T00:00:00Z
    let mut out = String::from(r#"{"type":"saved_messages","id":1000001,"messages":["#);
    for i in 0..n {
        if i > 0 {
            out.push(',');
        }
        let words = match next() % 100 {
            0..=69 => 4 + next() as usize % 12,
            70..=94 => 25 + next() as usize % 40,
            _ => 150 + next() as usize % 250,
        };
        let mut text = String::new();
        for k in 0..words {
            let rank = ((vocabulary.len() as f64).powf((next() >> 11) as f64 / (1u64 << 53) as f64)
                as usize)
                .clamp(1, vocabulary.len())
                - 1;
            if k > 0 {
                text.push(' ');
            }
            text.push_str(&vocabulary[rank]);
        }
        if next() % 100 < 30 {
            text.push_str(" сегодня");
        }
        let unix = start + (next() % (5 * 365 * 86_400)) as i64;
        let id = i as u64 + 1;
        let json_text = serde_json::to_string(&text).unwrap();
        out.push_str(&format!(
            r#"{{"id":{id},"type":"message","date_unixtime":"{unix}","from":"Я","from_id":"user1000001","text":{json_text},"text_entities":[{{"type":"plain","text":{json_text}}}]}}"#
        ));
    }
    out.push_str("]}");
    out
}
