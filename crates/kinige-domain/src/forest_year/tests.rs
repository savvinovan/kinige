use super::*;
use crate::forest::{CellPos, date_at_cell};

const MON: FirstWeekday = FirstWeekday::Monday;
const SUN: FirstWeekday = FirstWeekday::Sunday;
const INBOX: &str = "inbox";

fn day(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn note(len: usize) -> NoteStat {
    NoteStat {
        len,
        attachments: 0,
    }
}

fn stat(date: NaiveDate, notes: &[usize], body: Option<&str>) -> DayStats {
    DayStats {
        date,
        notes: notes.iter().map(|len| note(*len)).collect(),
        first_body: body.map(str::to_string),
    }
}

fn find<'a>(table: &'a YearTable, date: &str) -> &'a ForestDay {
    table.days.iter().find(|d| d.date == date).unwrap()
}

#[test]
fn every_day_is_present_once_in_calendar_order() {
    for (year, want) in [(2026, 365), (2024, 366), (1900, 365), (2000, 366)] {
        for first in [MON, SUN] {
            let table = year_table(year, first, &[]).unwrap();
            assert_eq!(table.days.len(), want, "{year}");
            let dates: Vec<_> = table.days.iter().map(|d| d.date.clone()).collect();
            let mut sorted = dates.clone();
            sorted.sort();
            sorted.dedup();
            assert_eq!(dates, sorted, "{year}: sorted and unique");
            assert_eq!(
                dates.first().map(String::as_str),
                Some(&*format!("{year}-01-01"))
            );
            assert_eq!(
                dates.last().map(String::as_str),
                Some(&*format!("{year}-12-31"))
            );
        }
    }
}

#[test]
fn cells_agree_with_the_position_rules_and_are_unique() {
    for (year, first) in [
        (2026, MON),
        (2026, SUN),
        (2012, MON),
        (2028, SUN),
        (2024, MON),
    ] {
        let table = year_table(year, first, &[]).unwrap();
        let mut seen = std::collections::HashSet::new();
        for d in &table.days {
            assert!(seen.insert((d.row, d.col)), "{} collides", d.date);
            assert!(d.row < 7 && d.col < table.cols);
            let back = date_at_cell(
                year,
                first,
                CellPos {
                    row: d.row,
                    col: d.col,
                },
            );
            assert_eq!(back.map(|x| x.to_string()), Some(d.date.clone()));
        }
    }
}

#[test]
fn leap_years_that_start_on_the_last_weekday_have_54_columns() {
    assert_eq!(year_table(2026, MON, &[]).unwrap().cols, 53);
    assert_eq!(year_table(2012, MON, &[]).unwrap().cols, 54);
    assert_eq!(year_table(2028, SUN, &[]).unwrap().cols, 54);
    assert_eq!(year_table(2028, MON, &[]).unwrap().cols, 53);
}

#[test]
fn seasons_follow_the_month_of_the_date() {
    let table = year_table(2026, MON, &[]).unwrap();
    for (date, season) in [
        ("2026-01-01", Season::Winter),
        ("2026-02-28", Season::Winter),
        ("2026-03-01", Season::Spring),
        ("2026-05-31", Season::Spring),
        ("2026-06-01", Season::Summer),
        ("2026-08-31", Season::Summer),
        ("2026-09-01", Season::Autumn),
        ("2026-11-30", Season::Autumn),
        ("2026-12-01", Season::Winter),
        ("2026-12-31", Season::Winter),
    ] {
        assert_eq!(find(&table, date).season, season, "{date}");
    }
}

#[test]
fn short_notes_count_as_notes_but_not_in_the_weight() {
    let stats = [
        stat(day(2026, 3, 3), &[1, 2, 0], Some("a")),
        stat(day(2026, 3, 4), &[1, 2, 200], Some("long enough")),
        stat(day(2026, 3, 5), &[200; 30], Some("many")),
    ];
    let table = year_table(2026, MON, &stats).unwrap();
    let short = find(&table, "2026-03-03");
    assert_eq!(
        (short.notes, short.stage),
        (3, 0),
        "only short notes: three notes, bare ground"
    );
    let one = find(&table, "2026-03-04");
    assert_eq!(one.notes, 3);
    assert_eq!(
        one.stage, 1,
        "the 200-character note carries weight 1, the short ones none"
    );
    assert_eq!(find(&table, "2026-03-05").stage, 6);
    assert_eq!(find(&table, "2026-03-06").notes, 0);
    assert_eq!(find(&table, "2026-03-06").stage, 0);
}

#[test]
fn the_excerpt_is_the_plain_start_of_the_first_entry() {
    let long = "x".repeat(500);
    let stats = [
        stat(
            day(2026, 10, 16),
            &[40],
            Some("**Bold** start\nsecond line"),
        ),
        stat(day(2026, 10, 17), &[500], Some(&long)),
        stat(day(2026, 10, 18), &[0], Some("")),
    ];
    let table = year_table(2026, MON, &stats).unwrap();
    assert_eq!(
        find(&table, "2026-10-16").excerpt.as_deref(),
        Some("Bold start second line")
    );
    let cut = find(&table, "2026-10-17").excerpt.clone().unwrap();
    assert!(cut.chars().count() <= crate::reply::EXCERPT_CHARS && cut.ends_with('…'));
    assert_eq!(
        find(&table, "2026-10-18").excerpt,
        None,
        "an empty body gives no excerpt"
    );
    assert_eq!(find(&table, "2026-10-19").excerpt, None);
}

#[test]
fn facts_outside_the_year_are_ignored_and_repeated_dates_are_merged() {
    let stats = [
        stat(day(2025, 12, 31), &[200], Some("last year")),
        stat(day(2027, 1, 1), &[200], Some("next year")),
        stat(day(2026, 1, 1), &[200], Some("first")),
        stat(day(2026, 1, 1), &[200], Some("second")),
    ];
    let table = year_table(2026, MON, &stats).unwrap();
    let jan1 = find(&table, "2026-01-01");
    assert_eq!(jan1.notes, 2);
    assert_eq!(jan1.excerpt.as_deref(), Some("first"));
    assert_eq!(table.days.iter().map(|d| d.notes).sum::<u32>(), 2);
}

#[test]
fn years_outside_the_range_give_no_table() {
    for year in [i32::MIN, -1, 0, 1, 999, 10_000, i32::MAX] {
        assert_eq!(year_table(year, MON, &[]), None, "{year}");
    }
    assert!(year_table(1000, SUN, &[]).is_some());
    assert!(year_table(9999, MON, &[]).is_some());
}

#[test]
fn the_view_and_mode_settings_fall_back_to_the_defaults() {
    for (stored, want) in [
        (None, ForestView::Strip),
        (Some("strip"), ForestView::Strip),
        (Some("hidden"), ForestView::Hidden),
        (Some("expanded"), ForestView::Expanded),
        (Some(""), ForestView::Strip),
        (Some("Expanded"), ForestView::Strip),
        (Some("\u{0}"), ForestView::Strip),
    ] {
        assert_eq!(ForestView::from_setting(stored), want, "{stored:?}");
    }
    for view in [ForestView::Hidden, ForestView::Strip, ForestView::Expanded] {
        assert_eq!(ForestView::from_setting(Some(view.as_setting())), view);
    }
    assert_eq!(ForestMode::from_setting(None), ForestMode::Forest);
    assert_eq!(
        ForestMode::from_setting(Some("calendar")),
        ForestMode::Calendar
    );
    assert_eq!(ForestMode::from_setting(Some("tree")), ForestMode::Forest);
    for mode in [ForestMode::Forest, ForestMode::Calendar] {
        assert_eq!(ForestMode::from_setting(Some(mode.as_setting())), mode);
    }
}

fn counts(items: &[(&str, usize)]) -> Vec<(String, usize)> {
    items
        .iter()
        .map(|(id, n)| ((*id).to_string(), *n))
        .collect()
}

#[test]
fn the_open_chat_wins_when_it_has_entries_that_day() {
    let c = counts(&[("a", 1), (INBOX, 5)]);
    assert_eq!(day_chat(&c, Some("a"), INBOX), Some("a"));
}

#[test]
fn otherwise_the_chat_with_the_most_entries() {
    let c = counts(&[("a", 1), ("b", 3), (INBOX, 2)]);
    assert_eq!(day_chat(&c, Some("zzz"), INBOX), Some("b"));
    assert_eq!(day_chat(&c, None, INBOX), Some("b"));
    assert_eq!(
        day_chat(&counts(&[("a", 1), ("b", 0)]), Some("b"), INBOX),
        Some("a")
    );
}

#[test]
fn a_tie_goes_to_the_inbox_and_otherwise_to_the_smaller_id() {
    let c = counts(&[("a", 2), (INBOX, 2), ("b", 2)]);
    assert_eq!(day_chat(&c, None, INBOX), Some(INBOX));
    let c = counts(&[("b", 2), ("a", 2), ("c", 1)]);
    assert_eq!(day_chat(&c, None, INBOX), Some("a"));
    let reversed = counts(&[("c", 1), ("a", 2), ("b", 2)]);
    assert_eq!(day_chat(&reversed, None, INBOX), Some("a"));
}

#[test]
fn an_empty_day_has_no_chat() {
    assert_eq!(day_chat(&[], Some("a"), INBOX), None);
    assert_eq!(day_chat(&counts(&[("a", 0)]), Some("a"), INBOX), None);
}
