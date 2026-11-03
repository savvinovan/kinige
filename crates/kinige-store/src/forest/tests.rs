//! The SQL aggregate of `year_stats` against the domain rules: the weight must be exactly `forest::day_weight` of the
//! entries one by one, the first entry the one with the smallest `(created_at, id)`, and the query must walk an index.

use super::*;
use crate::StoreOptions;
use kinige_domain::forest::{NoteStat, day_weight, stage_of};
use std::collections::BTreeMap;

const YEAR: i32 = 2026;
const START: i64 = 1_767_225_600_000; // 2026-01-01 00:00 UTC
const DAY_MS: i64 = 86_400_000;

/// Pieces of text: ASCII, Cyrillic (2 bytes a character), emoji (4 bytes), a combining mark (2 code points), a heading, NUL.
/// A NUL character is there too: SQLite's `length()` stops at it, the domain does not, and the store has to give the domain's number.
const PIECES: [&str; 7] = ["word ", "слово ", "😀", "e\u{301}", "# ", "ж", "\0"];

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0 % n
    }
}

fn open(dir: &std::path::Path) -> Store {
    let mut options = StoreOptions::new(dir.join("kinige.db"), None);
    options.start_mirror = false;
    options.start_maintenance = false;
    Store::open_at(options, &Now::at(START, "UTC")).unwrap()
}

/// A text of exactly `chars` characters made of mixed pieces (cut on a character boundary).
fn text(rng: &mut Rng, chars: usize) -> String {
    let mut out = String::new();
    while out.chars().count() < chars {
        out.push_str(PIECES[rng.below(PIECES.len() as u64) as usize]);
    }
    out.chars().take(chars).collect()
}

#[test]
fn sql_weight_equals_the_domain_weight() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let mut chats = vec![INBOX_ID.to_string()];
    for title in ["Other", "Gone"] {
        let chat = store
            .create_chat(title, None, &Now::at(START, "UTC"))
            .unwrap();
        chats.push(chat.id);
    }
    // Lengths around every edge of the rule: 1 and 2 (do not count), 3 (counts), 199, 200, 201, long.
    let edges = [1, 2, 3, 4, 100, 199, 200, 201, 450];
    let mut sent = 0;
    for day in 0..40 {
        for _ in 0..rng.below(14) {
            let len = if rng.below(2) == 0 {
                edges[rng.below(edges.len() as u64) as usize]
            } else {
                1 + rng.below(400) as usize
            };
            // Some entries share a millisecond, so the id has to break the tie.
            let at = START + day * DAY_MS + 3_600_000 + (rng.below(5) as i64) * 1000;
            let chat = &chats[rng.below(3) as usize];
            store
                .send_message(chat, &text(&mut rng, len), &Now::at(at, "UTC"))
                .unwrap();
            sent += 1;
        }
    }
    assert!(sent > 100);
    // One entry in the trash and one in a trashed chat do not count in either computation.
    let raw = Connection::open(dir.path().join("kinige.db")).unwrap();
    raw.execute(
        "UPDATE message SET deleted_at = 1 WHERE id = (SELECT id FROM message LIMIT 1)",
        [],
    )
    .unwrap();
    raw.execute("UPDATE chat SET deleted_at = 1 WHERE id = ?1", [&chats[2]])
        .unwrap();

    let got = store.read(|conn| year_stats(conn, YEAR)).unwrap();

    // The same days by the domain's own rules, from every row the way the first version of the query read them.
    let mut statement = raw
        .prepare(&format!(
            "SELECT m.journal_date, m.created_at, m.id, m.body_md FROM message m JOIN chat c ON c.id = m.chat_id \
             WHERE m.journal_date LIKE '2026-%' AND {LIVE}"
        ))
        .unwrap();
    type Day = (Vec<NoteStat>, (i64, String, String));
    let mut want: BTreeMap<String, Day> = BTreeMap::new();
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .unwrap();
    for row in rows {
        let (date, created, id, body) = row.unwrap();
        let note = NoteStat {
            len: body.chars().count(),
            attachments: 0,
        };
        let day = want
            .entry(date)
            .or_insert_with(|| (Vec::new(), (created, id.clone(), body.clone())));
        day.0.push(note);
        if (created, id.as_str()) < (day.1.0, day.1.1.as_str()) {
            day.1 = (created, id, body);
        }
    }

    assert_eq!(got.len(), want.len());
    for stat in &got {
        let (notes, first) = &want[&stat.date.format("%Y-%m-%d").to_string()];
        assert_eq!(stat.notes as usize, notes.len(), "{}", stat.date);
        let weight = day_weight(notes);
        assert!(
            (stat.weight - weight).abs() < 1e-9,
            "{}: sql {} domain {}",
            stat.date,
            stat.weight,
            weight
        );
        for w_max in [3.0, 8.0, 12.0] {
            assert_eq!(stage_of(stat.weight, w_max), stage_of(weight, w_max));
        }
        assert_eq!(
            stat.first_body.as_deref(),
            Some(first.2.as_str()),
            "{}",
            stat.date
        );
    }
}

#[test]
fn the_year_query_walks_the_day_index_not_the_table() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let plan: Vec<String> = store
        .read(|conn| {
            let mut statement = conn
                .prepare(&format!("EXPLAIN QUERY PLAN {YEAR_QUERY}"))
                .map_err(read_err)?;
            let rows = statement
                .query_map(["2026-01-01", "2026-12-31"], |row| row.get::<_, String>(3))
                .map_err(read_err)?;
            Ok(rows.map(Result::unwrap).collect())
        })
        .unwrap();
    println!("{plan:#?}");
    let table_step = plan.iter().find(|step| step.contains("message")).unwrap();
    assert!(table_step.contains("USING INDEX message_day"), "{plan:?}");
    assert!(
        !plan.iter().any(|step| step.contains("TEMP B-TREE")),
        "the grouping reads the index in order: {plan:?}"
    );
}

#[test]
fn damaged_rows_are_skipped_the_same_way_everywhere() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let at = |day: i64| Now::at(START + day * DAY_MS + 3_600_000, "UTC");
    let kept = store
        .send_message(INBOX_ID, "a good entry of the second day", &at(1))
        .unwrap();
    let damaged = store
        .send_message(INBOX_ID, "an entry that will lose its type", &at(-200))
        .unwrap();
    // Some days into the year, so that today is not still in the year before.
    let now = Now::at(START + 5 * DAY_MS, "UTC");
    let raw = Connection::open(dir.path().join("kinige.db")).unwrap();
    // A text that is a blob and a creation time before 1970: the first is skipped, the second must not break the first-entry order.
    raw.execute(
        "UPDATE message SET body_md = CAST(body_md AS BLOB) WHERE id = ?1",
        [&damaged.id],
    )
    .unwrap();
    raw.execute(
        "UPDATE message SET created_at = -5 WHERE id = ?1",
        [&kept.id],
    )
    .unwrap();

    let table = store
        .get_forest_year(YEAR, FirstWeekday::Monday, &now)
        .unwrap();
    let day = |date: &str| table.days.iter().find(|d| d.date == date).unwrap();
    assert_eq!(
        table.days.iter().map(|d| d.notes).sum::<u32>(),
        1,
        "the damaged entry is in another year anyway"
    );
    assert_eq!(day(&kept.journal_date).notes, 1);
    assert_eq!(
        day(&kept.journal_date).excerpt.as_deref(),
        Some("a good entry of the second day")
    );
    assert_eq!(
        store
            .forest_day_target(&damaged.journal_date, None)
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .forest_day_target(&kept.journal_date, None)
            .unwrap()
            .map(|t| t.message_id),
        Some(kept.id)
    );
    // The damaged entry is the oldest one and it is from the year before: it must not set the lower end of the year switch.
    assert_eq!(table.min_year, YEAR);
    let earlier = store
        .get_forest_year(YEAR - 1, FirstWeekday::Monday, &now)
        .unwrap();
    assert!(earlier.days.iter().all(|d| d.notes == 0));
}
