//! The forest part of `seed_demo` (slice 6): a lived-in synthetic year and the year before it, every growth stage, a week without
//! entries, days with entries in several chats, an entry in the trash, today's tree at the edge of the next stage. Taken out of
//! `seed_demo.rs` to keep that file under 400 lines; the same example rules apply (a utility, not app code).

#![allow(
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
use kinige_store::{Now, Store};

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
pub fn seed_forest(
    store: &Store,
    now: &Now,
    history: &[String],
    old: &str,
    data: &std::path::Path,
) {
    let today = journal_date(now.ms, now.utc_offset_secs, DAY_BOUNDARY_MINUTES);
    let today = NaiveDate::parse_from_str(&today, "%Y-%m-%d").expect("journal day");
    let tz: Tz = now.tz.parse().unwrap_or(chrono_tz::UTC);
    let at = |date: NaiveDate, minutes: u32| {
        let local = date
            .and_hms_opt(6 + minutes / 60, minutes % 60, 0)
            .expect("time");
        let ms = tz
            .from_local_datetime(&local)
            .earliest()
            .expect("local time exists")
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
            NaiveDate::from_ymd_opt(year, 12, 31).expect("end of the year")
        };
        let gap = if year == today.year() {
            today - Duration::days(45)
        } else {
            NaiveDate::from_ymd_opt(year, 8, 3).expect("start of August")
        };
        let mut date = NaiveDate::from_ymd_opt(year, 1, 1).expect("start of the year");
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
                    .expect("forest entry");
            }
            date += Duration::days(1);
        }
    }

    // One day with an entry in the trash: the forest counts only the live ones.
    let day = today - Duration::days(20);
    store
        .send_message(INBOX_ID, "Первая запись дня, остаётся", &at(day, 0))
        .expect("entry");
    let doomed = store
        .send_message(INBOX_ID, "Эта запись уйдёт в корзину", &at(day, 20))
        .expect("entry")
        .id;
    store.delete_message(&doomed, now).expect("to the trash");

    // Today's tree: stage 3, weight 6.2.
    let db = rusqlite::Connection::open(data.join("kinige.db")).expect("database");
    let mut statement = db
        .prepare("SELECT body_md FROM message WHERE journal_date = ?1 AND deleted_at IS NULL")
        .expect("query");
    let have: Vec<NoteStat> = statement
        .query_map([today.format("%Y-%m-%d").to_string()], |row| {
            row.get::<_, String>(0)
        })
        .expect("today's entries")
        .map(|body| NoteStat {
            len: body.expect("text").chars().count(),
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
            .expect("entry today");
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
