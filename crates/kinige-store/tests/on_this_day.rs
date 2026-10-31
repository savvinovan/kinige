//! «В этот день» (слайс 5): отбор записей Inbox прошлых лет, состав карточки, скрытие до следующего дня журнала.

mod common;

use common::days::{at, ms};
use common::*;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::on_this_day::{DETAILED_YEARS, ENTRIES_PER_YEAR};
use kinige_domain::{OnThisDay, OnThisDayYear};
use kinige_store::{Now, Store, StoreError};

fn put(store: &Store, chat: &str, body: &str, when: i64) -> String {
    store.send_message(chat, body, &moscow(when)).unwrap().id
}

fn card(store: &Store, now: &Now) -> Option<OnThisDay> {
    store.get_on_this_day(now).unwrap()
}

fn year_dates(card: &OnThisDay) -> Vec<&str> {
    card.years.iter().map(|y| y.journal_date.as_str()).collect()
}

#[test]
fn only_inbox_entries_of_past_years_with_the_same_month_and_day_are_selected() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let books = store
        .create_chat("Книги", None, &at(2020, 1, 1, 9, 0))
        .unwrap();
    put(
        &store,
        INBOX_ID,
        "в тот же день, 2023",
        ms(2023, 10, 21, 10, 0),
    );
    put(&store, INBOX_ID, "другой день", ms(2023, 10, 22, 10, 0));
    put(&store, INBOX_ID, "другой месяц", ms(2023, 11, 21, 10, 0));
    put(&store, INBOX_ID, "сегодня", ms(2026, 10, 21, 8, 0));
    put(&store, INBOX_ID, "будущее", ms(2027, 10, 21, 8, 0));
    put(&store, &books.id, "чужой чат", ms(2024, 10, 21, 10, 0));
    let doomed = put(&store, INBOX_ID, "в корзине", ms(2022, 10, 21, 10, 0));
    store
        .delete_message(&doomed, &at(2026, 10, 21, 7, 0))
        .unwrap();

    let found = card(&store, &at(2026, 10, 21, 12, 0)).expect("карточка");
    assert_eq!(found.today, "2026-10-21");
    assert_eq!(year_dates(&found), ["2023-10-21"]);
    assert_eq!(found.years[0].years_ago, 3);
    assert_eq!(found.years[0].total, 1);
    assert_eq!(found.years[0].messages[0].body_md, "в тот же день, 2023");
}

#[test]
fn no_card_without_entries_in_past_years_or_without_any_entries() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    assert_eq!(card(&store, &at(2026, 10, 21, 12, 0)), None, "пустая база");
    put(&store, INBOX_ID, "сегодня", ms(2026, 10, 21, 8, 0));
    put(
        &store,
        INBOX_ID,
        "вчера в прошлом году",
        ms(2025, 10, 20, 8, 0),
    );
    assert_eq!(card(&store, &at(2026, 10, 21, 12, 0)), None);
    // Все записи этого числа ушли в корзину.
    let id = put(&store, INBOX_ID, "год назад", ms(2025, 10, 21, 8, 0));
    assert!(card(&store, &at(2026, 10, 21, 12, 0)).is_some());
    store.delete_message(&id, &at(2026, 10, 21, 12, 5)).unwrap();
    assert_eq!(card(&store, &at(2026, 10, 21, 12, 10)), None);
}

#[test]
fn years_go_from_new_to_old_three_are_detailed_with_two_entries_and_the_rest_are_compact() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let mut first_ids = Vec::new();
    for year in [2016, 2019, 2021, 2023, 2024, 2025] {
        for (n, hour) in [9, 11, 13, 15].iter().enumerate() {
            let id = put(
                &store,
                INBOX_ID,
                &format!("{year} запись {n}"),
                ms(year, 10, 21, *hour, 0),
            );
            if n == 0 {
                first_ids.push((year, id));
            }
        }
    }
    let found = card(&store, &at(2026, 10, 21, 12, 0)).unwrap();
    assert_eq!(
        year_dates(&found),
        [
            "2025-10-21",
            "2024-10-21",
            "2023-10-21",
            "2021-10-21",
            "2019-10-21",
            "2016-10-21"
        ]
    );
    let ago: Vec<i64> = found.years.iter().map(|y| y.years_ago).collect();
    assert_eq!(ago, [1, 2, 3, 5, 7, 10]);
    for (index, year) in found.years.iter().enumerate() {
        assert_eq!(year.total, 4);
        let id = &first_ids
            .iter()
            .find(|(y, _)| year.journal_date.starts_with(&y.to_string()))
            .unwrap()
            .1;
        assert_eq!(&year.first_id, id, "первая запись дня");
        if index < DETAILED_YEARS {
            assert_eq!(year.messages.len(), ENTRIES_PER_YEAR);
            let texts: Vec<&str> = year.messages.iter().map(|m| m.body_md.as_str()).collect();
            assert_eq!(texts[0], format!("{} запись 0", &year.journal_date[..4]));
            assert_eq!(texts[1], format!("{} запись 1", &year.journal_date[..4]));
        } else {
            assert!(year.messages.is_empty(), "старые годы — строкой");
        }
    }
}

#[test]
fn the_card_counts_only_years_with_entries_when_choosing_the_detailed_ones() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for year in [2017, 2018, 2019, 2020] {
        put(&store, INBOX_ID, "запись", ms(year, 10, 21, 9, 0));
    }
    let found = card(&store, &at(2026, 10, 21, 12, 0)).unwrap();
    let detailed: Vec<&OnThisDayYear> = found
        .years
        .iter()
        .filter(|y| !y.messages.is_empty())
        .collect();
    assert_eq!(
        detailed.len(),
        DETAILED_YEARS,
        "годы без записей не занимают места"
    );
    assert_eq!(found.years.len(), 4);
}

#[test]
fn the_leap_day_matches_only_leap_days() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    put(&store, INBOX_ID, "29 февраля 2020", ms(2020, 2, 29, 9, 0));
    put(&store, INBOX_ID, "28 февраля 2023", ms(2023, 2, 28, 9, 0));
    put(&store, INBOX_ID, "1 марта 2023", ms(2023, 3, 1, 9, 0));
    put(&store, INBOX_ID, "29 февраля 2024", ms(2024, 2, 29, 9, 0));
    let leap = card(&store, &at(2028, 2, 29, 12, 0)).unwrap();
    assert_eq!(year_dates(&leap), ["2024-02-29", "2020-02-29"]);
    let common = card(&store, &at(2026, 2, 28, 12, 0)).unwrap();
    assert_eq!(
        year_dates(&common),
        ["2023-02-28"],
        "28 февраля не совпадает с 29-м"
    );
    assert_eq!(
        card(&store, &at(2026, 3, 1, 12, 0)).unwrap().years[0].journal_date,
        "2023-03-01"
    );
}

#[test]
fn the_day_boundary_is_four_in_the_morning() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    put(
        &store,
        INBOX_ID,
        "год назад, 20 октября",
        ms(2025, 10, 20, 12, 0),
    );
    // 21 октября 02:30 — ещё 20-е по журналу.
    let night = card(&store, &at(2026, 10, 21, 2, 30)).unwrap();
    assert_eq!(night.today, "2026-10-20");
    assert_eq!(year_dates(&night), ["2025-10-20"]);
    assert_eq!(
        card(&store, &at(2026, 10, 21, 4, 0)),
        None,
        "с 04:00 уже 21-е"
    );
}

#[test]
fn year_edges_are_handled() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    put(&store, INBOX_ID, "новогодняя ночь", ms(2024, 12, 31, 12, 0));
    put(&store, INBOX_ID, "первое января", ms(2025, 1, 1, 12, 0));
    assert_eq!(
        year_dates(&card(&store, &at(2026, 12, 31, 12, 0)).unwrap()),
        ["2024-12-31"]
    );
    assert_eq!(
        year_dates(&card(&store, &at(2026, 1, 1, 12, 0)).unwrap()),
        ["2025-01-01"]
    );
}

#[test]
fn dismissal_hides_the_card_until_the_next_journal_day_and_survives_a_restart() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        put(&store, INBOX_ID, "год назад", ms(2025, 10, 21, 9, 0));
        put(&store, INBOX_ID, "и в прошлый день", ms(2025, 10, 22, 9, 0));
        assert!(card(&store, &at(2026, 10, 21, 12, 0)).is_some());
        store.dismiss_on_this_day("2026-10-21").unwrap();
        assert_eq!(card(&store, &at(2026, 10, 21, 12, 1)), None);
        assert_eq!(
            card(&store, &at(2026, 10, 22, 3, 59)),
            None,
            "до 04:00 — ещё тот же день журнала"
        );
    }
    let store = open_without_mirror(dir.path());
    assert_eq!(
        card(&store, &at(2026, 10, 21, 20, 0)),
        None,
        "перезапуск карточку не возвращает"
    );
    let next = card(&store, &at(2026, 10, 22, 4, 0)).expect("следующий день журнала");
    assert_eq!(year_dates(&next), ["2025-10-22"]);
}

#[test]
fn dismissal_is_local_and_leaves_no_trace_in_synced_tables() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let before: i64 = raw(dir.path())
        .query_row("SELECT COUNT(*) FROM chat", [], |r| r.get(0))
        .unwrap();
    store.dismiss_on_this_day("2026-10-21").unwrap();
    let conn = raw(dir.path());
    let value: String = conn
        .query_row(
            "SELECT value FROM app_setting WHERE key = 'on_this_day_dismissed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(value, "2026-10-21");
    let after: i64 = conn
        .query_row("SELECT COUNT(*) FROM chat", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
}

#[test]
fn dismissing_garbage_dates_is_refused_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let huge = "9".repeat(1 << 20);
    for garbage in [
        "",
        "x",
        "2026-02-30",
        "2026-13-01",
        "../../etc/passwd",
        "2026-10-21 ",
        huge.as_str(),
    ] {
        assert!(
            matches!(
                store.dismiss_on_this_day(garbage),
                Err(StoreError::InvalidDate)
            ),
            "{}",
            &garbage[..garbage.len().min(30)]
        );
    }
    let count: i64 = raw(dir.path())
        .query_row(
            "SELECT COUNT(*) FROM app_setting WHERE key = 'on_this_day_dismissed'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn the_card_entries_carry_their_reply_quote() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let old = put(&store, INBOX_ID, "старая мысль", ms(2022, 10, 21, 9, 0));
    store
        .send_reply(
            INBOX_ID,
            "ответ тогда же",
            Some(&old),
            &moscow(ms(2025, 10, 21, 9, 0)),
        )
        .unwrap();
    let found = card(&store, &at(2026, 10, 21, 12, 0)).unwrap();
    let reply = &found.years[0].messages[0];
    let quote = reply.reply_to.as_ref().expect("цитата");
    assert_eq!(
        (quote.years_later, quote.excerpt.as_str()),
        (3, "старая мысль")
    );
}
