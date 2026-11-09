//! The pure rules of the contract: each test feeds the cases of one fixture file to the real function.

use super::{fixture, int, list, opt_text, spec, text};
use chrono::DateTime;
use kinige_domain::append::{APPEND_WINDOW_MS, AppendCandidate, can_append, joined_body};
use kinige_domain::attachment::{
    AttachmentSummary, ImageFormat, MAX_PASTE_BYTES, MAX_PER_ENTRY, MAX_VOICE_MS, MIN_VOICE_MS,
    WAVEFORM_PEAKS, classify, stored_ext,
};
use kinige_domain::chat::{
    ChatInputError, MAX_ICON_BYTES, MAX_TITLE_CHARS, normalize_icon, normalize_title,
};
use kinige_domain::chat_color::resolve_color;
use kinige_domain::forest::{DEFAULT_W_MAX, NoteStat, day_weight, stage_of};
use kinige_domain::forest_year::day_chat;
use kinige_domain::groups::INBOX_ID;
use kinige_domain::history::{TRASH_DAYS, days_left};
use kinige_domain::journal::{DAY_BOUNDARY_MINUTES, is_valid_journal_date, journal_date};
use kinige_domain::on_this_day::{DETAILED_YEARS, ENTRIES_PER_YEAR, PREVIEW_CHARS};
use kinige_domain::reply::{EXCERPT_CHARS, ReplyTarget, excerpt, reply_to, years_later};
use kinige_domain::tags::{MAX_TAG_CHARS, parse_tags};
use kinige_domain::tasks::set_task_item;
use kinige_store::{DEFAULT_PAGE, LOG_PAGE, MAX_PAGE, StoreError};
use serde_json::{Value, json};

#[test]
fn the_chat_colour() {
    assert_eq!(INBOX_ID, "00000000-0000-4000-8000-000000000001");
    for case in list(&fixture("chat-color"), "cases") {
        let (id, picked) = (text(case, "id"), opt_text(case, "picked"));
        let colour = resolve_color(id, picked).as_str();
        assert_eq!(colour, text(case, "expected"), "{case}");
    }
}

#[test]
fn a_real_journal_date() {
    for case in list(&fixture("date-valid"), "cases") {
        let valid = case["valid"].as_bool().unwrap();
        assert_eq!(is_valid_journal_date(text(case, "text")), valid, "{case}");
    }
}

#[test]
fn the_entry_day_in_utc() {
    for case in list(&fixture("journal-day-utc"), "cases") {
        let ms = at_ms(text(case, "atUtc"));
        let day = journal_date(ms, 0, DAY_BOUNDARY_MINUTES);
        assert_eq!(day, text(case, "day"), "{case}");
    }
}

#[test]
fn chat_name_and_emoji() {
    let file = fixture("chat-input");
    for case in list(&file, "titles") {
        let raw = repeated(case, "text");
        let got = normalize_title(&raw).map_err(|e| code_of(&StoreError::ChatInput(e)));
        assert_eq!(got, expectation(case), "{case}");
    }
    for case in list(&file, "icons") {
        let raw = case["text"].as_str().map(|_| repeated(case, "text"));
        let got = normalize_icon(raw.as_deref()).map_err(|e| code_of(&StoreError::ChatInput(e)));
        assert_eq!(got, expectation(case), "{case}");
    }
    // The error type is the one the store turns into these codes.
    assert!(matches!(
        normalize_title(""),
        Err(ChatInputError::TitleEmpty)
    ));
}

#[test]
fn the_entry_that_capture_appends_to() {
    for case in list(&fixture("append-target"), "cases") {
        let now = at_ms(text(case, "now"));
        let today = text(case, "today");
        assert_eq!(journal_date(now, 0, DAY_BOUNDARY_MINUTES), today, "{case}");
        let candidate = AppendCandidate {
            origin: text(case, "origin"),
            created_at: now - int(case, "ageMs"),
            journal_date: text(case, "journalDate"),
            hlc_json: r#"{"body":{"sv":1}}"#,
        };
        let appends = can_append(&candidate, now, today).unwrap();
        assert_eq!(appends, case["expected"].as_bool().unwrap(), "{case}");
    }
}

#[test]
fn the_text_after_appending() {
    for case in list(&fixture("append-join"), "cases") {
        let joined = joined_body(text(case, "old"), text(case, "addition"));
        assert_eq!(joined, text(case, "expected"), "{case}");
    }
}

#[test]
fn the_quote_of_an_old_entry() {
    let file = fixture("reply-quote");
    for case in list(&file, "yearsLater") {
        let years = years_later(text(case, "old"), text(case, "reply"));
        assert_eq!(years, int(case, "expected"), "{case}");
    }
    for case in list(&file, "excerpts") {
        assert_eq!(
            excerpt(&spec(&case["body"])),
            spec(&case["expected"]),
            "{case}"
        );
    }
    for case in list(&file, "quotes") {
        let target = (!case["target"].is_null()).then(|| ReplyTarget {
            chat_id: text(&case["target"], "chatId").to_string(),
            journal_date: text(&case["target"], "journalDate").to_string(),
            body_md: text(&case["target"], "bodyMd").to_string(),
            in_trash: case["target"]["inTrash"].as_bool().unwrap(),
            attachments: AttachmentSummary::default(),
        });
        let quote = reply_to(text(case, "id"), text(case, "replyDate"), target);
        let got = serde_json::to_value(&quote).unwrap();
        let expected = &case["expected"];
        for key in ["chatId", "journalDate", "excerpt", "state", "yearsLater"] {
            assert_eq!(got[key], expected[key], "{key} of {case}");
        }
        assert_eq!(got["id"], case["id"], "{case}");
    }
}

#[test]
fn days_left_in_the_trash() {
    let deleted_at = 1_000_000_000_000;
    for case in list(&fixture("trash-days"), "cases") {
        let now = deleted_at + int(case, "sinceDeletedMs");
        assert_eq!(days_left(deleted_at, now), int(case, "expected"), "{case}");
    }
}

#[test]
fn sizes_and_limits() {
    for case in list(&fixture("constants"), "cases") {
        let name = text(case, "name");
        assert_eq!(constant(name), int(case, "value"), "{name}");
    }
}

#[test]
fn the_tags_of_an_entry() {
    for case in list(&fixture("tags"), "cases") {
        let expected: Vec<&str> = list(case, "expected")
            .iter()
            .map(|t| t.as_str().unwrap())
            .collect();
        assert_eq!(parse_tags(text(case, "body")), expected, "{case}");
    }
}

#[test]
fn toggling_a_task_item() {
    for case in list(&fixture("task-items"), "cases") {
        let index = usize::try_from(int(case, "index")).unwrap();
        let checked = case["checked"].as_bool().unwrap();
        // Both refusals reach the window as `task_item_stale` (`StoreError::TaskItemStale`).
        let got = set_task_item(text(case, "body"), index, checked)
            .map_err(|_| code_of(&StoreError::TaskItemStale));
        assert_eq!(got, expectation(case), "{case}");
    }
}

#[test]
fn the_growth_stage_of_a_day() {
    for case in list(&fixture("forest-stage"), "cases") {
        let mut notes = Vec::new();
        for note in list(case, "notes") {
            let parts: Vec<usize> = note
                .as_array()
                .unwrap()
                .iter()
                .map(|n| usize::try_from(n.as_i64().unwrap()).unwrap())
                .collect();
            let (len, attachments) = (parts[0], parts[1]);
            for _ in 0..parts.get(2).copied().unwrap_or(1) {
                notes.push(NoteStat { len, attachments });
            }
        }
        let stage = stage_of(day_weight(&notes), DEFAULT_W_MAX);
        assert_eq!(i64::from(stage), int(case, "stage"), "{case}");
    }
}

#[test]
fn the_chat_a_forest_day_opens() {
    for case in list(&fixture("forest-day-chat"), "cases") {
        let counts: Vec<(String, usize)> = list(case, "counts")
            .iter()
            .map(|pair| {
                let id = pair[0].as_str().unwrap().to_string();
                (id, usize::try_from(pair[1].as_i64().unwrap()).unwrap())
            })
            .collect();
        let chosen = day_chat(&counts, opt_text(case, "current"), "inbox");
        assert_eq!(chosen, opt_text(case, "expected"), "{case}");
    }
}

#[test]
fn the_kind_of_an_attachment() {
    for case in list(&fixture("attachment-kind"), "cases") {
        let image = opt_text(case, "imageFormat").map(|format| match format {
            "jpeg" => ImageFormat::Jpeg,
            "png" => ImageFormat::Png,
            "gif" => ImageFormat::Gif,
            "webp" => ImageFormat::WebP,
            other => panic!("unknown image format {other}"),
        });
        let ext = stored_ext(text(case, "name"), image);
        let (kind, mime) = classify(&ext, None, image);
        assert_eq!(kind.as_str(), text(case, "kind"), "{case}");
        if let Some(expected) = opt_text(case, "mime") {
            assert_eq!(mime, expected, "{case}");
        }
    }
}

/// A Rust constant by the name the fixture uses.
fn constant(name: &str) -> i64 {
    let value = match name {
        "feedDefaultPage" => DEFAULT_PAGE,
        "feedMaxPage" => MAX_PAGE,
        "agentLogPage" => LOG_PAGE,
        "chatTitleChars" => MAX_TITLE_CHARS,
        "chatIconBytes" => MAX_ICON_BYTES,
        "replyExcerptChars" | "forestExcerptChars" => EXCERPT_CHARS,
        "onThisDayDetailedYears" => DETAILED_YEARS,
        "onThisDayEntriesPerYear" => ENTRIES_PER_YEAR,
        "onThisDayPreviewChars" => PREVIEW_CHARS,
        "tagChars" => MAX_TAG_CHARS,
        "attachmentsPerEntry" => MAX_PER_ENTRY,
        "pasteMaxBytes" => MAX_PASTE_BYTES,
        "waveformPeaks" => WAVEFORM_PEAKS,
        "trashDays" => return TRASH_DAYS,
        "appendWindowMs" => return APPEND_WINDOW_MS,
        "voiceMinMs" => return MIN_VOICE_MS,
        "voiceMaxMs" => return MAX_VOICE_MS,
        other => panic!("the fixture names a constant the test does not know: {other}"),
    };
    i64::try_from(value).unwrap()
}

/// Milliseconds of an RFC 3339 time.
fn at_ms(time: &str) -> i64 {
    DateTime::parse_from_rfc3339(time)
        .unwrap()
        .timestamp_millis()
}

/// `text` or `expected`, repeated `repeat` times when the case says so.
fn repeated(case: &Value, key: &str) -> String {
    let base = text(case, key);
    match case["repeat"].as_u64() {
        Some(times) => base.repeat(usize::try_from(times).unwrap()),
        None => base.to_string(),
    }
}

/// What the case expects: `Ok(expected)` or `Err(code)`.
fn expectation(case: &Value) -> Result<String, String> {
    match opt_text(case, "error") {
        Some(code) => Err(code.to_string()),
        None => Ok(repeated(case, "expected")),
    }
}

/// The wire name of a store error.
fn code_of(error: &StoreError) -> String {
    json!(error.code()).as_str().unwrap().to_string()
}
