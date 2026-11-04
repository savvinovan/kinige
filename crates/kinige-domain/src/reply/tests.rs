use super::*;

fn old(body: &str, in_trash: bool) -> ReplyTarget {
    ReplyTarget {
        chat_id: "chat".into(),
        journal_date: "2023-10-31".into(),
        body_md: body.into(),
        in_trash,
        attachments: Default::default(),
    }
}

#[test]
fn years_later_counts_full_years() {
    assert_eq!(years_later("2023-10-31", "2026-10-31"), 3);
    assert_eq!(
        years_later("2023-10-31", "2026-10-30"),
        2,
        "not an anniversary yet"
    );
    assert_eq!(
        years_later("2025-10-31", "2026-10-30"),
        0,
        "less than a year"
    );
    assert_eq!(years_later("2025-10-31", "2026-10-31"), 1);
    assert_eq!(years_later("2016-01-01", "2026-12-31"), 10);
}

#[test]
fn years_later_handles_leap_days() {
    assert_eq!(years_later("2020-02-29", "2024-02-29"), 4);
    assert_eq!(
        years_later("2020-02-29", "2023-02-28"),
        2,
        "February 28 is before the 29th"
    );
    assert_eq!(years_later("2020-02-29", "2023-03-01"), 3);
    assert_eq!(years_later("2023-02-28", "2024-02-29"), 1);
}

#[test]
fn years_later_is_zero_when_the_reply_is_earlier_or_dates_are_garbage() {
    assert_eq!(
        years_later("2026-10-31", "2023-10-31"),
        0,
        "the reply date was changed to an earlier one"
    );
    assert_eq!(years_later("2026-10-31", "2026-10-31"), 0);
    for garbage in ["", "2026-13-01", "2026-02-30", "x", "../../etc"] {
        assert_eq!(years_later(garbage, "2026-10-31"), 0, "{garbage}");
        assert_eq!(years_later("2023-10-31", garbage), 0, "{garbage}");
    }
}

#[test]
fn excerpt_is_one_plain_line() {
    assert_eq!(
        excerpt("Купить **молоко**\n\n- [ ] позвонить маме"),
        "Купить молоко позвонить маме"
    );
    assert_eq!(
        excerpt("# Заголовок\n> цитата\n`код`"),
        "Заголовок цитата код"
    );
    assert_eq!(excerpt(""), "");
    assert_eq!(excerpt("\n\n   \n"), "");
}

#[test]
fn excerpt_keeps_the_bold_author_name_of_an_imported_message() {
    assert_eq!(
        excerpt("**Анна К.**\nНу что, едем в субботу?"),
        "Анна К. Ну что, едем в субботу?"
    );
}

#[test]
fn excerpt_is_cut_at_the_limit_with_an_ellipsis() {
    let long = "слово ".repeat(100);
    let text = excerpt(&long);
    assert!(
        text.chars().count() <= EXCERPT_CHARS,
        "{}",
        text.chars().count()
    );
    assert!(text.ends_with('…'));
    let one_word = "я".repeat(500);
    let text = excerpt(&one_word);
    assert_eq!(text.chars().count(), EXCERPT_CHARS);
}

#[test]
fn excerpt_does_not_split_multibyte_characters() {
    let emoji = "😀".repeat(300);
    let text = excerpt(&emoji);
    assert!(text.chars().count() <= EXCERPT_CHARS);
    assert!(text.chars().all(|c| c == '😀' || c == '…'));
}

#[test]
fn reply_to_reports_live_trash_and_missing() {
    let live = reply_to("r", "2026-10-31", Some(old("Было **так**", false)));
    assert_eq!(live.state, ReplyState::Live);
    assert_eq!(live.excerpt, "Было так");
    assert_eq!(live.years_later, 3);
    assert_eq!(live.chat_id.as_deref(), Some("chat"));
    assert_eq!(live.journal_date.as_deref(), Some("2023-10-31"));

    let trash = reply_to("r", "2026-10-31", Some(old("Было", true)));
    assert_eq!(trash.state, ReplyState::Trash);
    assert_eq!(
        trash.excerpt, "Было",
        "the text stays, the interface will caption it \"Entry in trash\""
    );

    let missing = reply_to("r", "2026-10-31", None);
    assert_eq!(missing.state, ReplyState::Missing);
    assert_eq!(missing.years_later, 0);
    assert_eq!(missing.excerpt, "");
    assert_eq!(missing.chat_id, None);
}

#[test]
fn reply_to_serializes_in_camel_case_with_snake_case_state() {
    let json =
        serde_json::to_string(&reply_to("r", "2026-10-31", Some(old("Текст", true)))).unwrap();
    assert_eq!(
        json,
        r#"{"id":"r","chatId":"chat","journalDate":"2023-10-31","excerpt":"Текст","state":"trash","yearsLater":3,"attachments":{"photos":0,"files":0,"name":null}}"#
    );
}
