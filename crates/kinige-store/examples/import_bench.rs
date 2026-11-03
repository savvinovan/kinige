//! Benchmark of import and search (slice 4): parse, import, search and cold start times, database size before and after.
//! Prints only numbers and times - texts, names and paths from the export file are not printed.
//!
//! Usage (the data directory is temporary, not the real database!):
//! `cargo run --release -p kinige-store --example import_bench -- <data directory> synthetic <N>`
//! `cargo run --release -p kinige-store --example import_bench -- <data directory> file <result.json>`

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

use kinige_domain::Language;
use kinige_domain::SearchFilters;
use kinige_domain::forest::FirstWeekday;
use kinige_domain::telegram::{TgExport, parse_export};
use kinige_store::{Now, Store, StoreOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

fn main() {
    let mut args = std::env::args().skip(1);
    let data = PathBuf::from(args.next().expect("data directory"));
    let mode = args.next().expect("synthetic | file");
    let arg = args.next().expect("N or the path to result.json");
    let json = match mode.as_str() {
        "synthetic" => synthetic(arg.parse().expect("number of messages")),
        "file" => std::fs::read_to_string(&arg).expect("the file is readable"),
        other => panic!("unknown mode {other}"),
    };
    std::fs::create_dir_all(&data).unwrap();
    let options = || {
        let mut options = StoreOptions::new(data.join("kinige.db"), Some(data.join("mirror")));
        options.start_maintenance = false;
        options
    };
    let t = Instant::now();
    let store = Store::open(options()).unwrap();
    println!("opening an empty database: {:.0} ms", ms(t));
    let size_before = db_size(&data);

    let t = Instant::now();
    let export: Arc<TgExport> = Arc::new(parse_export(&json).expect("the export parses"));
    let messages: usize = export.chats.iter().map(|c| c.messages.len()).sum();
    println!(
        "parse: {:.0} ms, {} MB of JSON, chats {}, messages {messages}",
        ms(t),
        json.len() / 1_000_000,
        export.chats.len()
    );
    drop(json);
    let t = Instant::now();
    let preview = store.import_preview(&export).unwrap();
    println!(
        "preview: {:.0} ms, chats to import {}, entries {}",
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
        "import: {:.0} ms ({:.1} us per message); imported {}, skipped: already present {}, service {}, empty {}; \
         attachment placeholders {}; chats created {}; backup: {}",
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
        "repeated import: {:.0} ms, new {}, already present {}",
        ms(t),
        again.imported,
        again.skipped_existing
    );
    drop(store);
    let size_after = db_size(&data);
    println!(
        "database size: before {:.1} MB, after {:.1} MB",
        size_before as f64 / 1e6,
        size_after as f64 / 1e6
    );

    // Cold start after the import: reopening the database and the first feed page.
    let t = Instant::now();
    let store = Store::open(options()).unwrap();
    let open = ms(t);
    let t = Instant::now();
    let page = store
        .list_messages(kinige_domain::groups::INBOX_ID, None, 200, &Now::system())
        .unwrap();
    println!(
        "cold start: open {open:.0} ms, first page ({} entries) {:.0} ms",
        page.messages.len(),
        ms(t)
    );

    // "On this day" (slice 5): the card for 200 different "todays" of 2023, with five years of entries behind (2018-2022).
    // Opening Inbox = the first feed page + the card; the slice target is p95 and opening under 300 ms.
    let mut card_times = Vec::new();
    let mut card_entries = 0usize;
    for day in 0..200i64 {
        let when = Now::at(
            1_672_567_200_000 + day * 365 * 86_400_000 / 200,
            "Europe/Moscow",
        );
        let t = Instant::now();
        let card = store.get_on_this_day(&when).unwrap();
        card_times.push(ms(t));
        card_entries += card.map_or(0, |c| {
            c.years.iter().map(|y| y.entries.len()).sum::<usize>()
        });
    }
    card_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "\"On this day\" over 200 days: p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms; entries in the card on average {:.1}",
        card_times[card_times.len() / 2],
        card_times[(card_times.len() * 95).div_ceil(100) - 1],
        card_times[card_times.len() - 1],
        card_entries as f64 / card_times.len() as f64
    );
    let t = Instant::now();
    let page = store
        .list_messages(kinige_domain::groups::INBOX_ID, None, 200, &Now::system())
        .unwrap();
    let card = store.get_on_this_day(&Now::system()).unwrap();
    println!(
        "opening Inbox (a page of {} entries + the card, {} years): {:.0} ms",
        page.messages.len(),
        card.map_or(0, |c| c.years.len()),
        ms(t)
    );

    // The forest (slice 6): the table of one year, every year of the data, 20 rounds each. The target is p95 < 100 ms.
    let mut forest_times = Vec::new();
    let mut forest_notes = 0u64;
    for round in 0..20 {
        for year in 2018..=2023 {
            let first = if round % 2 == 0 {
                FirstWeekday::Monday
            } else {
                FirstWeekday::Sunday
            };
            let t = Instant::now();
            let table = store.get_forest_year(year, first, &Now::system()).unwrap();
            forest_times.push(ms(t));
            forest_notes += table.days.iter().map(|d| u64::from(d.notes)).sum::<u64>();
        }
    }
    forest_times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "forest year table (get_forest_year, {} calls over 2018..2023): p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms; {} notes counted per call on average",
        forest_times.len(),
        forest_times[forest_times.len() / 2],
        forest_times[(forest_times.len() * 95).div_ceil(100) - 1],
        forest_times[forest_times.len() - 1],
        forest_notes / forest_times.len() as u64
    );
    let t = Instant::now();
    let target = store
        .forest_day_target("2021-06-15", Some(kinige_domain::groups::INBOX_ID))
        .unwrap();
    println!(
        "forest day target: {:.1} ms (found: {})",
        ms(t),
        target.is_some()
    );

    // Search: the words are taken from the entries themselves and are not printed.
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
            "search over {} words from the entries (prefix query, page of 50): p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms; found on average {:.1}",
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
            "search of a frequent one-letter word: {:.1} ms, found on the page {}",
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

/// Words of at least four characters from the database entries, one for each `n`-th entry.
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
            // An unfinished word is the path search takes while typing.
            words.push(word.chars().take(word.chars().count().max(4) - 1).collect());
        }
        if words.len() >= n {
            break;
        }
    }
    words
}

/// An export of "Saved Messages" with `n` messages over five years: Zipf-distributed words from a pseudo-vocabulary, some messages formatted.
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
