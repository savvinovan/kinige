//! The year's forest in the store (slice 6): what counts on a day, year boundaries, the first-entry excerpt across chats,
//! and garbage input. The click target and the view state are in `forest_target.rs`.

#![cfg(test)]

mod common;

use common::days::{at, ms};
use common::*;
use kinige_domain::ForestYear;
use kinige_domain::forest::FirstWeekday;
use kinige_domain::groups::INBOX_ID;
use kinige_store::{Now, Store, StoreError};

const MON: FirstWeekday = FirstWeekday::Monday;
const SUN: FirstWeekday = FirstWeekday::Sunday;

fn now() -> Now {
    at(2026, 10, 21, 12, 0)
}

fn put(store: &Store, chat: &str, body: &str, when: i64) -> String {
    store.send_message(chat, body, &moscow(when)).unwrap().id
}

fn year(store: &Store, y: i32) -> ForestYear {
    store.get_forest_year(y, MON, &now()).unwrap()
}

fn day<'a>(table: &'a ForestYear, date: &str) -> &'a kinige_domain::ForestDay {
    table.days.iter().find(|d| d.date == date).unwrap()
}

fn trash_chat(dir: &std::path::Path, id: &str) {
    raw(dir)
        .execute("UPDATE chat SET deleted_at = 1 WHERE id = ?1", [id])
        .unwrap();
}

#[test]
fn an_empty_journal_gives_a_full_year_of_bare_ground() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let table = year(&store, 2026);
    assert_eq!(table.days.len(), 365);
    assert!(
        table
            .days
            .iter()
            .all(|d| d.notes == 0 && d.stage == 0 && d.excerpt.is_none())
    );
    assert_eq!(
        (table.year, table.cols, table.first_weekday),
        (2026, 53, MON)
    );
    assert_eq!(table.today, "2026-10-21");
    assert_eq!(
        table.min_year, 2026,
        "no entries: the lower end is the current year"
    );
}

#[test]
fn live_entries_of_all_chats_count_trash_and_trashed_chats_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store
        .create_chat("Books", None, &at(2020, 1, 1, 9, 0))
        .unwrap();
    let old = store
        .create_chat("Old", None, &at(2020, 1, 1, 9, 1))
        .unwrap();
    let doomed = store
        .create_chat("Doomed", None, &at(2020, 1, 1, 9, 2))
        .unwrap();
    let when = ms(2026, 10, 16, 10, 0);
    put(&store, INBOX_ID, "inbox entry of that day", when);
    put(&store, &books.id, "books entry of that day", when + MINUTE);
    put(
        &store,
        &old.id,
        "archived chat entry of that day",
        when + 2 * MINUTE,
    );
    put(
        &store,
        &doomed.id,
        "entry of a chat in the trash",
        when + 3 * MINUTE,
    );
    let removed = put(&store, INBOX_ID, "deleted entry", when + 4 * MINUTE);
    store.set_chat_archived(&old.id, true, &now()).unwrap();
    store.delete_message(&removed, &now()).unwrap();
    trash_chat(dir.path(), &doomed.id);

    let table = year(&store, 2026);
    let d = day(&table, "2026-10-16");
    assert_eq!(
        d.notes, 3,
        "inbox + books + archived; not the trashed entry, not the trashed chat"
    );
    assert_eq!(day(&table, "2026-10-17").notes, 0);
}

#[test]
fn the_excerpt_is_the_first_entry_of_the_day_across_chats() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store
        .create_chat("Books", None, &at(2020, 1, 1, 9, 0))
        .unwrap();
    put(
        &store,
        INBOX_ID,
        "**later** in the inbox",
        ms(2026, 3, 3, 15, 0),
    );
    put(
        &store,
        &books.id,
        "earlier in books\nsecond line",
        ms(2026, 3, 3, 9, 0),
    );
    let table = year(&store, 2026);
    assert_eq!(
        day(&table, "2026-03-03").excerpt.as_deref(),
        Some("earlier in books second line")
    );
    // The first entry goes into the trash: the next one takes its place.
    let first = put(&store, INBOX_ID, "the very first", ms(2026, 3, 3, 8, 0));
    assert_eq!(
        year(&store, 2026)
            .days
            .iter()
            .find(|d| d.date == "2026-03-03")
            .unwrap()
            .excerpt
            .as_deref(),
        Some("the very first")
    );
    store.delete_message(&first, &now()).unwrap();
    assert_eq!(
        year(&store, 2026)
            .days
            .iter()
            .find(|d| d.date == "2026-03-03")
            .unwrap()
            .excerpt
            .as_deref(),
        Some("earlier in books second line")
    );
}

#[test]
fn year_boundaries_stay_in_their_own_year() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    // 31.12 at 23:30 and 01.01 at 05:00 Moscow time (the journal day changes at 04:00, so 03:30 on 1 January is 31.12).
    put(&store, INBOX_ID, "new year's eve", ms(2025, 12, 31, 23, 30));
    put(&store, INBOX_ID, "still the old day", ms(2026, 1, 1, 3, 30));
    put(&store, INBOX_ID, "new year's morning", ms(2026, 1, 1, 5, 0));
    let last = year(&store, 2025);
    let first = year(&store, 2026);
    assert_eq!(day(&last, "2025-12-31").notes, 2);
    assert_eq!(last.days.iter().map(|d| d.notes).sum::<u32>(), 2);
    assert_eq!(day(&first, "2026-01-01").notes, 1);
    assert_eq!(first.days.iter().map(|d| d.notes).sum::<u32>(), 1);
    assert_eq!(first.min_year, 2025);
}

#[test]
fn short_entries_count_as_notes_but_grow_nothing_and_many_long_ones_make_a_big_tree() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for i in 0..3 {
        put(&store, INBOX_ID, "ab", ms(2026, 5, 5, 9, i));
    }
    let long = "я".repeat(220);
    for i in 0..30 {
        put(&store, INBOX_ID, &long, ms(2026, 5, 6, 9, i));
    }
    let table = year(&store, 2026);
    assert_eq!(
        (
            day(&table, "2026-05-05").notes,
            day(&table, "2026-05-05").stage
        ),
        (3, 0)
    );
    assert_eq!(
        (
            day(&table, "2026-05-06").notes,
            day(&table, "2026-05-06").stage
        ),
        (30, 6)
    );
}

#[test]
fn the_first_weekday_moves_the_cells_not_the_days() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mon = store.get_forest_year(2026, MON, &now()).unwrap();
    let sun = store.get_forest_year(2026, SUN, &now()).unwrap();
    assert_eq!(
        (day(&mon, "2026-01-01").row, day(&mon, "2026-01-01").col),
        (3, 0)
    );
    assert_eq!(
        (day(&sun, "2026-01-01").row, day(&sun, "2026-01-01").col),
        (4, 0)
    );
    let wide = store.get_forest_year(2028, SUN, &now()).unwrap();
    assert_eq!((wide.cols, wide.days.len()), (54, 366));
}

#[test]
fn min_year_is_the_oldest_live_entry_of_a_chat_not_in_the_trash() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let gone = store
        .create_chat("Gone", None, &at(2020, 1, 1, 9, 0))
        .unwrap();
    put(&store, &gone.id, "ancient", ms(2011, 6, 1, 9, 0));
    let doomed = put(&store, INBOX_ID, "old but deleted", ms(2015, 6, 1, 9, 0));
    put(&store, INBOX_ID, "oldest live", ms(2019, 6, 1, 9, 0));
    store.delete_message(&doomed, &now()).unwrap();
    assert_eq!(year(&store, 2026).min_year, 2011);
    trash_chat(dir.path(), &gone.id);
    assert_eq!(year(&store, 2026).min_year, 2019);
}

#[test]
fn garbage_years_are_invalid_dates_and_write_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for bad in [0, -1, 1, 999, 10_000, i32::MAX, i32::MIN] {
        assert!(
            matches!(
                store.get_forest_year(bad, MON, &now()),
                Err(StoreError::InvalidDate)
            ),
            "{bad}"
        );
    }
    assert!(store.get_forest_year(1000, SUN, &now()).is_ok());
    assert!(store.get_forest_year(9999, MON, &now()).is_ok());
}

#[test]
fn damaged_dates_in_the_database_are_skipped_not_fatal() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let id = put(&store, INBOX_ID, "fine", ms(2026, 2, 2, 9, 0));
    raw(dir.path())
        .execute(
            "UPDATE message SET journal_date = '2026-02-30' WHERE id = ?1",
            [&id],
        )
        .unwrap();
    put(&store, INBOX_ID, "ok", ms(2026, 2, 3, 9, 0));
    let table = year(&store, 2026);
    assert_eq!(table.days.iter().map(|d| d.notes).sum::<u32>(), 1);
}
