//! `year_stats` against the domain rules read independently from the same rows: the weight is exactly
//! `forest::day_weight` of the entries one by one, the first entry the one with the smallest `(created_at, id)`, the
//! excerpt is cut by characters, damaged rows are skipped, and the query walks an index.

use super::*;
use crate::StoreOptions;
use kinige_domain::forest::{NoteStat, day_weight, stage_of};
use std::collections::BTreeMap;

const YEAR: i32 = 2026;
const START: i64 = 1_767_225_600_000; // 2026-01-01 00:00 UTC
const DAY_MS: i64 = 86_400_000;

/// Pieces of text: ASCII, Cyrillic (2 bytes a character), emoji (4 bytes), a combining mark (2 code points), a heading, NUL.
/// A NUL character is there too: the domain counts it as a character, and so does the store.
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
fn weights_equal_the_domain_weight_over_random_days_with_nul_pieces() {
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

    // The same days by the domain's own rules, from the rows read again on a separate connection.
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
            "{}: scan {} domain {}",
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
                .prepare(&format!("EXPLAIN QUERY PLAN {}", year_sql()))
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

/// A day with a blob row next to good entries, a body over the excerpt limit made of multibyte characters, and entries whose
/// creation times decide the first one (negative, zero and equal times included).
#[test]
fn a_blob_row_is_skipped_the_excerpt_is_cut_by_characters_and_creation_time_picks_the_first_entry()
{
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path());
    let at = |day: i64| Now::at(START + day * DAY_MS + 3_600_000, "UTC");
    let raw = Connection::open(dir.path().join("kinige.db")).unwrap();
    let set_created = |id: &str, created: i64| {
        raw.execute(
            "UPDATE message SET created_at = ?2 WHERE id = ?1",
            (id, created),
        )
        .unwrap();
    };

    // Day 1: a blob, then two good entries; the blob is the earliest and must not count or become the excerpt.
    let blob = store
        .send_message(INBOX_ID, "blob blob blob", &at(1))
        .unwrap();
    let late = store
        .send_message(INBOX_ID, "late entry text", &at(1))
        .unwrap();
    let early = store
        .send_message(INBOX_ID, "early entry text", &at(1))
        .unwrap();
    raw.execute(
        "UPDATE message SET body_md = CAST(body_md AS BLOB) WHERE id = ?1",
        [&blob.id],
    )
    .unwrap();
    set_created(&blob.id, -100);
    set_created(&late.id, 5);
    set_created(&early.id, -5);

    // Day 2: 4200 characters of 4-byte emoji and Cyrillic: the cut must land on a character, not in the middle of one.
    let long: String = "😀ж".repeat(2100);
    assert!(long.chars().count() > 4096 && long.len() > 4096 * 2);
    let long_entry = store.send_message(INBOX_ID, &long, &at(2)).unwrap();

    // Day 3: a zero and a positive time; day 4: equal times, the smaller id wins.
    let positive = store
        .send_message(INBOX_ID, "positive time", &at(3))
        .unwrap();
    let zero = store.send_message(INBOX_ID, "zero time", &at(3)).unwrap();
    set_created(&positive.id, 1);
    set_created(&zero.id, 0);
    let tie_a = store.send_message(INBOX_ID, "tie a", &at(4)).unwrap();
    let tie_b = store.send_message(INBOX_ID, "tie b", &at(4)).unwrap();
    set_created(&tie_a.id, 7);
    set_created(&tie_b.id, 7);

    let stats = store.read(|conn| year_stats(conn, YEAR)).unwrap();
    let stat = |date: &str| stats.iter().find(|s| s.date.to_string() == date).unwrap();

    let one = stat(&early.journal_date);
    assert_eq!(one.notes, 2, "the blob row is not an entry");
    assert_eq!(one.first_body.as_deref(), Some("early entry text"));
    let expected = day_weight(&[
        NoteStat {
            len: 16,
            attachments: 0,
        },
        NoteStat {
            len: 15,
            attachments: 0,
        },
    ]);
    assert!((one.weight - expected).abs() < 1e-12);

    let two = stat(&long_entry.journal_date);
    let excerpt = two.first_body.as_deref().unwrap();
    assert_eq!(excerpt.chars().count(), EXCERPT_CHARS);
    assert!(long.starts_with(excerpt));
    assert_eq!(two.notes, 1);
    assert!(
        (two.weight - 1.0).abs() < 1e-12,
        "the weight uses the whole entry"
    );

    assert_eq!(
        stat(&zero.journal_date).first_body.as_deref(),
        Some("zero time")
    );
    let tie_first = if tie_a.id < tie_b.id {
        "tie a"
    } else {
        "tie b"
    };
    assert_eq!(
        stat(&tie_a.journal_date).first_body.as_deref(),
        Some(tie_first)
    );

    // The entry a click opens is the same one.
    for (date, id) in [
        (&early.journal_date, &early.id),
        (&zero.journal_date, &zero.id),
    ] {
        let target = store.forest_day_target(date, None).unwrap().unwrap();
        assert_eq!(&target.message_id, id);
    }
}
