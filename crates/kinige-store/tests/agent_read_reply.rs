//! Ответы пользователя и агент (слайс 5, ADR-0009): `reply_to_id` и текст цитаты агенту не отдаются, «в этот день» через MCP
//! считает те же даты, что карточка (29 февраля — только 29 февраля).

mod common;

use common::agent::*;
use common::*;
use kinige_domain::AgentMode;
use kinige_domain::groups::INBOX_ID;
use serde_json::json;

const MARCH: i64 = 1_773_468_000_000;

#[test]
fn a_reply_reaches_the_agent_without_reply_to_id_or_the_quoted_text() {
    // Слайс 5: ответ пользователя читается агентом как обычная запись; ссылка и цитата ему не отдаются (ADR-0009).
    let a = agent(AgentMode::Read, true);
    let old = a
        .store
        .send_message(
            INBOX_ID,
            "СТАРЫЙ-ТЕКСТ-ЦИТАТЫ",
            &moscow(MARCH - 3 * 365 * DAY),
        )
        .unwrap();
    a.store
        .send_reply(INBOX_ID, "новый ответ", Some(&old.id), &moscow(MARCH))
        .unwrap();
    let reply = call(
        &a.store,
        "get_entries",
        json!({"date": "2026-03-14"}),
        MARCH,
    )
    .unwrap();
    let text = reply.to_string();
    assert!(text.contains("новый ответ"), "{text}");
    assert!(
        !text.contains(&old.id),
        "id старой записи не отдаётся: {text}"
    );
    assert!(!text.contains("reply"), "{text}");
    assert!(!text.contains("СТАРЫЙ-ТЕКСТ-ЦИТАТЫ"), "{text}");
}

#[test]
fn on_this_day_through_mcp_uses_the_shared_dates_and_the_leap_day_matches_only_itself() {
    let a = agent(AgentMode::Read, true);
    let at = |ms: i64| moscow(ms);
    let leap_2024 = a
        .store
        .send_message(INBOX_ID, "29 февраля 2024", &at(1_709_200_800_000))
        .unwrap()
        .id;
    a.store
        .send_message(INBOX_ID, "28 февраля 2025", &at(1_740_729_600_000))
        .unwrap();
    a.store
        .send_message(INBOX_ID, "1 марта 2025", &at(1_740_816_000_000))
        .unwrap();
    // «Сегодня» — 29 февраля 2028: совпадает только с 29 февраля 2024.
    let leap_day = call(
        &a.store,
        "on_this_day",
        json!({"date": "2028-02-29"}),
        MARCH,
    )
    .unwrap();
    let text = leap_day.to_string();
    assert!(text.contains(&leap_2024), "{text}");
    assert!(
        !text.contains("28 февраля") && !text.contains("1 марта"),
        "{text}"
    );
    // 28 февраля 2026 не совпадает с 29-м: находится только 28 февраля 2025.
    let plain = call(
        &a.store,
        "on_this_day",
        json!({"date": "2026-02-28"}),
        MARCH,
    )
    .unwrap()
    .to_string();
    assert!(
        plain.contains("28 февраля 2025") && !plain.contains("29 февраля 2024"),
        "{plain}"
    );
    // Дата без прошлых лет и мусорная дата не ломают ответ.
    let none = call(
        &a.store,
        "on_this_day",
        json!({"date": "2024-02-29"}),
        MARCH,
    )
    .unwrap()
    .to_string();
    assert!(!none.contains("29 февраля 2024"), "{none}");
    assert!(
        call(
            &a.store,
            "on_this_day",
            json!({"date": "2026-02-30"}),
            MARCH
        )
        .is_err()
    );
}
