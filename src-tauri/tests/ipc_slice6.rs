//! Slice 6 over IPC on the mock runtime with the real capabilities: the forest commands belong to the main window only
//! (the capture window keeps exactly seven commands, the settings window is denied), and garbage arguments come back as
//! errors without a panic and without writing anything.

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances
// (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::{CAPTURE_COMMANDS, MAIN_COMMANDS, SETTINGS_COMMANDS};
use serde_json::{Value, json};

const INBOX: &str = kinige_domain::groups::INBOX_ID;

/// The commands of slice 6. The capture window and the settings window are denied each of them.
const SLICE_SIX: &[&str] = &[
    "get_forest_year",
    "get_forest_day_target",
    "set_forest_view",
];

fn year_of(main: &tauri::WebviewWindow<tauri::test::MockRuntime>, year: i64) -> Value {
    call(
        main,
        "get_forest_year",
        json!({"year": year, "firstWeekday": "monday"}),
    )
    .unwrap()
}

#[test]
fn slice_six_commands_are_main_only_and_the_capture_set_is_unchanged() {
    assert_eq!(
        CAPTURE_COMMANDS,
        [
            "capture_state",
            "capture_send",
            "capture_append",
            "capture_save_draft",
            "capture_hide",
            "capture_resize",
            "capture_add_recent_emoji"
        ],
        "the capture window still has seven commands"
    );
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let settings = window(&fixture, "settings");
    for command in SLICE_SIX {
        assert!(MAIN_COMMANDS.contains(command), "{command}");
        assert!(!CAPTURE_COMMANDS.contains(command), "{command}");
        assert!(!SETTINGS_COMMANDS.contains(command), "{command}");
        for denied in [&capture, &settings] {
            let error = call(denied, command, args_for(command))
                .expect_err(&format!("{command} must be denied to this window"));
            assert!(is_acl_denial(&error), "{command}: {error}");
        }
        let result = call(&main, command, args_for(command));
        if let Err(error) = result {
            assert!(!is_acl_denial(&error), "{command}: {error}");
        }
    }
}

#[test]
fn a_note_written_today_shows_up_in_the_year_table_and_opens_from_its_day() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let sent = call(
        &main,
        "send_message",
        json!({"chatId": INBOX, "body": "synthetic note for the forest"}),
    )
    .unwrap();
    let today = sent["journalDate"].as_str().unwrap().to_string();
    let year: i64 = today.split('-').next().unwrap().parse().unwrap();

    let table = year_of(&main, year);
    assert_eq!(table["today"], json!(today));
    assert_eq!(table["year"], json!(year));
    assert_eq!(table["firstWeekday"], "monday");
    let days = table["days"].as_array().unwrap();
    assert!(days.len() == 365 || days.len() == 366);
    let day = days.iter().find(|d| d["date"] == json!(today)).unwrap();
    assert_eq!(day["notes"], 1);
    assert_eq!(day["excerpt"], "synthetic note for the forest");
    assert_eq!(
        days.iter()
            .map(|d| d["notes"].as_u64().unwrap())
            .sum::<u64>(),
        1
    );
    assert_eq!(table["minYear"], json!(year));

    let target = call(
        &main,
        "get_forest_day_target",
        json!({"date": today, "currentChatId": null}),
    )
    .unwrap();
    assert_eq!(target, json!({"chatId": INBOX, "messageId": sent["id"]}));
    // A chat id that is not a chat id (garbage from the window) counts as "no open chat": the day still opens.
    for bad in [
        "",
        "no-such-chat",
        "'; DROP TABLE chat; --",
        &"я".repeat(1 << 20),
    ] {
        let again = call(
            &main,
            "get_forest_day_target",
            json!({"date": today, "currentChatId": bad}),
        )
        .unwrap();
        assert_eq!(
            again,
            target,
            "{:?}",
            bad.chars().take(12).collect::<String>()
        );
    }
    let empty = call(
        &main,
        "get_forest_day_target",
        json!({"date": "2001-01-01", "currentChatId": INBOX}),
    )
    .unwrap();
    assert_eq!(empty, json!(null));
}

#[test]
fn garbage_years_and_weekdays_are_refused_without_a_panic() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    for bad in [
        json!(0),
        json!(-1),
        json!(1),
        json!(999),
        json!(10_000),
        json!(i32::MAX),
    ] {
        let error = call(
            &main,
            "get_forest_year",
            json!({"year": bad, "firstWeekday": "sunday"}),
        )
        .unwrap_err();
        assert_eq!(error, json!({"code": "invalid_date"}), "year {bad}");
    }
    for bad in [
        json!("wednesday"),
        json!("Monday"),
        json!(""),
        json!(1),
        json!(null),
        json!("\u{0}"),
        json!("м".repeat(100_000)),
    ] {
        let error = call(
            &main,
            "get_forest_year",
            json!({"year": 2026, "firstWeekday": bad}),
        )
        .unwrap_err();
        assert!(!is_acl_denial(&error), "{error}");
        assert!(error.is_string() || error.get("code").is_some(), "{error}");
    }
    for bad in [
        json!(2026.5),
        json!("2026"),
        json!(i64::MAX),
        json!(u64::MAX),
        json!(null),
    ] {
        let error = call(
            &main,
            "get_forest_year",
            json!({"year": bad, "firstWeekday": "monday"}),
        )
        .unwrap_err();
        assert!(!is_acl_denial(&error), "{error}");
    }
    // The table of a valid year is still served afterwards.
    assert_eq!(year_of(&main, 2026)["days"].as_array().unwrap().len(), 365);
}

#[test]
fn the_weekday_source_is_optional_and_only_three_names_are_accepted() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    for good in ["get_week_info", "week_info", "fallback"] {
        let table = call(
            &main,
            "get_forest_year",
            json!({"year": 2026, "firstWeekday": "sunday", "weekdaySource": good}),
        )
        .unwrap();
        assert_eq!(table["firstWeekday"], "sunday");
    }
    for bad in [
        json!("locale"),
        json!(""),
        json!(3),
        json!("ru-RU"),
        json!("\u{0}"),
    ] {
        let error = call(
            &main,
            "get_forest_year",
            json!({"year": 2026, "firstWeekday": "monday", "weekdaySource": bad}),
        )
        .unwrap_err();
        assert!(!is_acl_denial(&error), "{error}");
    }
}

#[test]
fn garbage_dates_and_chat_ids_of_the_day_target() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    for bad in [
        "",
        "x",
        "2026-02-30",
        "0001-01-01",
        "../../etc",
        "2026-10-16\u{0}",
    ] {
        let error = call(
            &main,
            "get_forest_day_target",
            json!({"date": bad, "currentChatId": null}),
        )
        .unwrap_err();
        assert_eq!(error, json!({"code": "invalid_date"}), "{bad:?}");
    }
    for bad in [json!(7), json!(true), json!([]), json!({})] {
        let error = call(
            &main,
            "get_forest_day_target",
            json!({"date": "2026-10-16", "currentChatId": bad}),
        )
        .unwrap_err();
        assert!(!is_acl_denial(&error), "{error}");
    }
    for chat in [
        "",
        "no-such-chat",
        "'; DROP TABLE chat; --",
        &"я".repeat(1 << 20),
    ] {
        let result = call(
            &main,
            "get_forest_day_target",
            json!({"date": "2026-10-16", "currentChatId": chat}),
        )
        .unwrap();
        assert_eq!(result, json!(null));
    }
}

#[test]
fn the_view_state_round_trips_through_settings_and_refuses_garbage() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let settings = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(settings["forestView"], "strip");
    assert_eq!(settings["forestMode"], "forest");
    call(
        &main,
        "set_forest_view",
        json!({"view": "expanded", "mode": "calendar"}),
    )
    .unwrap();
    let settings = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(settings["forestView"], "expanded");
    assert_eq!(settings["forestMode"], "calendar");
    for (view, mode) in [
        (json!("sideways"), json!("forest")),
        (json!("strip"), json!("tree")),
        (json!(null), json!("forest")),
        (json!("strip"), json!(3)),
        (json!(""), json!("")),
    ] {
        let error = call(
            &main,
            "set_forest_view",
            json!({"view": view, "mode": mode}),
        )
        .unwrap_err();
        assert!(!is_acl_denial(&error), "{error}");
    }
    let settings = call(&main, "get_settings", json!({})).unwrap();
    assert_eq!(
        (
            settings["forestView"].clone(),
            settings["forestMode"].clone()
        ),
        (json!("expanded"), json!("calendar")),
        "garbage changed nothing"
    );
}
