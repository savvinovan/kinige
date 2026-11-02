//! Year tables for the frontend's tests and Storybook stories (slice 6). The frontend keeps no copy of the calendar rules, so the
//! tables it draws in stories come from the real `year_table`: this test builds them from a fixed synthetic journal and writes
//! `web/src/ui/forest/fixtures/*.json` (committed; CI fails when a regenerated file differs, like the ts-rs types). The data is
//! synthetic and deterministic: no personal entries.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

use chrono::{Datelike, NaiveDate};
use kinige_domain::ForestYear;
use kinige_domain::forest::{FirstWeekday, NoteStat};
use kinige_domain::forest_year::{DayStats, year_table};
use std::path::PathBuf;

const PHRASES: [&str; 8] = [
    "walked to the river and thought about the week",
    "**Draft** of the plan for the autumn, three points",
    "a short call, nothing new",
    "read two chapters before sleep",
    "the garden needs water, again",
    "tea, rain, and a long list of small things",
    "finished the letter I kept postponing",
    "a quiet day with the window open",
];

/// xorshift: the same numbers on every machine.
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

/// A journal for `year` up to and including `today`: a few empty days, a week-long gap in August, every growth stage.
fn journal(year: i32, today: NaiveDate, seed: u64) -> Vec<DayStats> {
    let mut rng = Rng(seed);
    let counts = [1u64, 1, 2, 3, 4, 6, 9, 14, 21, 30];
    let mut stats = Vec::new();
    let mut date = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    while date.year() == year && date <= today {
        let gap = date.month() == 8 && (3..=9).contains(&date.day());
        if !gap && rng.below(100) >= 14 {
            let n = counts[rng.below(counts.len() as u64) as usize];
            let notes = (0..n)
                .map(|_| {
                    let len = match rng.below(10) {
                        0..=1 => 1 + rng.below(2) as usize,
                        2..=4 => 40 + rng.below(80) as usize,
                        _ => 200 + rng.below(300) as usize,
                    };
                    NoteStat {
                        len,
                        attachments: 0,
                    }
                })
                .collect();
            let phrase = PHRASES[rng.below(PHRASES.len() as u64) as usize];
            stats.push(DayStats {
                date,
                notes,
                first_body: Some(format!("Synthetic entry of {date}: {phrase}")),
            });
        }
        date = date.succ_opt().unwrap();
    }
    // One day with a long first entry: its excerpt is cut at the quote length.
    if let Some(day) = stats
        .iter_mut()
        .find(|d| d.date.month() == 10 && d.date.day() == 16)
    {
        day.first_body = Some(format!(
            "{}{}",
            "The longest synthetic entry of the year, written to check how a card cuts a long first line. ",
            "More words follow, and more, until the excerpt is certainly longer than the card can show. ".repeat(3)
        ));
    }
    stats
}

fn table(year: i32, first: FirstWeekday, today: &str, min_year: i32, seed: u64) -> ForestYear {
    let today_date = NaiveDate::parse_from_str(today, "%Y-%m-%d").unwrap();
    let built = year_table(year, first, &journal(year, today_date, seed)).unwrap();
    ForestYear {
        year,
        first_weekday: first,
        cols: built.cols,
        min_year,
        today: today.to_string(),
        days: built.days,
    }
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/src/ui/forest/fixtures")
}

/// Writes the file when its content differs, so a rerun leaves the modification times alone.
fn write(name: &str, year: &ForestYear) {
    let dir = fixtures_dir();
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    let json = serde_json::to_string(year).unwrap() + "\n";
    if std::fs::read_to_string(&path).ok().as_deref() != Some(json.as_str()) {
        std::fs::write(path, json).unwrap();
    }
}

#[test]
fn the_fixtures_are_written_from_the_real_year_table_and_hold_every_stage() {
    let rich = table(2026, FirstWeekday::Monday, "2026-10-21", 2018, 0x5EED_0001);
    let previous = table(2025, FirstWeekday::Monday, "2026-10-21", 2018, 0x5EED_0002);
    let wide = table(2028, FirstWeekday::Sunday, "2028-06-15", 2024, 0x5EED_0003);
    for (name, year) in [
        ("forest-2026-monday.json", &rich),
        ("forest-2025-monday.json", &previous),
        ("forest-2028-sunday.json", &wide),
    ] {
        write(name, year);
    }

    assert_eq!(rich.cols, 53);
    assert_eq!(
        wide.cols, 54,
        "2028 starts on a Saturday: Sunday-first needs a 54th column"
    );
    for year in [&rich, &previous] {
        for stage in 0..=6u8 {
            assert!(
                year.days.iter().any(|d| d.stage == stage),
                "stage {stage} of {}",
                year.year
            );
        }
        assert!(
            year.days.iter().any(|d| d.notes > 0 && d.stage == 0),
            "a day with only short notes: bare ground with notes"
        );
    }
    let gap: Vec<_> = rich
        .days
        .iter()
        .filter(|d| ("2026-08-03"..="2026-08-09").contains(&d.date.as_str()))
        .collect();
    assert_eq!(gap.len(), 7);
    assert!(gap.iter().all(|d| d.notes == 0), "the week-long gap");
    let long = rich.days.iter().find(|d| d.date == "2026-10-16").unwrap();
    assert!(
        long.excerpt
            .as_ref()
            .is_some_and(|e| e.chars().count() <= 120 && e.ends_with('…'))
    );
    assert!(
        rich.days
            .iter()
            .filter(|d| d.date.as_str() > "2026-10-21")
            .all(|d| d.notes == 0)
    );
}
