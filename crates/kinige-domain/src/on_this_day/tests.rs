use super::*;

fn strings(dates: Vec<NaiveDate>) -> Vec<String> {
    dates
        .iter()
        .map(|d| d.format("%Y-%m-%d").to_string())
        .collect()
}

#[test]
fn past_dates_go_from_newest_to_oldest_year() {
    assert_eq!(
        strings(past_dates("2026-10-31", 2023)),
        ["2025-10-31", "2024-10-31", "2023-10-31"]
    );
}

#[test]
fn past_dates_start_at_the_year_of_the_oldest_entry_and_exclude_this_year() {
    assert_eq!(
        strings(past_dates("2026-10-31", 2026)),
        Vec::<String>::new()
    );
    assert_eq!(
        strings(past_dates("2026-10-31", 2030)),
        Vec::<String>::new()
    );
    assert_eq!(strings(past_dates("2026-01-01", 2025)), ["2025-01-01"]);
}

#[test]
fn the_leap_day_matches_only_leap_days() {
    assert_eq!(
        strings(past_dates("2028-02-29", 2019)),
        ["2024-02-29", "2020-02-29"]
    );
    assert_eq!(
        strings(past_dates("2026-02-28", 2023)),
        ["2025-02-28", "2024-02-28", "2023-02-28"],
        "28 февраля не совпадает с 29-м"
    );
    assert_eq!(
        strings(past_dates("2026-03-01", 2024)),
        ["2025-03-01", "2024-03-01"]
    );
}

#[test]
fn past_dates_cover_year_edges() {
    assert_eq!(strings(past_dates("2026-12-31", 2025)), ["2025-12-31"]);
    assert_eq!(
        strings(past_dates("2026-01-01", 2024)),
        ["2025-01-01", "2024-01-01"]
    );
}

#[test]
fn past_dates_reject_garbage() {
    for garbage in ["", "x", "2026-02-30", "2026-13-01", "../../x", "2026-1-1"] {
        assert!(past_dates(garbage, 2000).is_empty(), "{garbage}");
    }
}

#[test]
fn years_ago_is_the_year_difference() {
    let date = NaiveDate::from_ymd_opt(2023, 10, 31).unwrap();
    assert_eq!(years_ago("2026-10-31", &date), 3);
    assert_eq!(year_of("2026-10-31"), Some(2026));
    assert_eq!(year_of("nope"), None);
}
