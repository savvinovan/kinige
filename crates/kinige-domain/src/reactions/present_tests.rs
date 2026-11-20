//! What reactions look like outside the group: the years-later rule, the copy line, the quick set and the recent list.

use super::present::*;
use crate::add_group::rewrite_or_add_group;
use crate::rewrite::Groups;
use serde_json::json;

const HEART: &str = "\u{2764}\u{FE0F}";

#[test]
fn years_later_counts_full_calendar_years_and_never_goes_below_zero() {
    let cases = [
        ("2025-11-19", "2026-11-19", 1),
        ("2025-11-19", "2026-11-18", 0),
        ("2025-11-19", "2025-11-19", 0),
        ("2025-11-19", "2024-11-19", 0),
        ("2025-11-19", "2035-01-01", 9),
        ("2024-02-29", "2025-02-28", 0),
        ("2024-02-29", "2025-03-01", 1),
        ("2024-02-29", "2028-02-29", 4),
        ("2000-01-01", "2026-01-01", 26),
    ];
    for (entry, put, years) in cases {
        assert_eq!(years_later(entry, put), years, "{entry} -> {put}");
    }
    for garbage in [
        "",
        "2026-13-01",
        "2026-02-30",
        "x",
        "0999-01-01",
        "2026-1-1",
    ] {
        assert_eq!(years_later(garbage, "2026-11-19"), 0, "{garbage}");
        assert_eq!(years_later("2025-11-19", garbage), 0, "{garbage}");
    }
}

#[test]
fn the_copy_line_has_each_reaction_with_its_date_in_the_order_put() {
    let line = |pairs: &[(&str, &str)]| {
        markdown_line(
            &pairs
                .iter()
                .map(|(e, d)| (e.to_string(), d.to_string()))
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(line(&[]), None, "no reactions, no line");
    assert_eq!(
        line(&[(HEART, "2026-11-18"), ("🔥", "2026-11-18")]).as_deref(),
        Some("\u{2764}\u{FE0F} 2026-11-18 · 🔥 2026-11-18")
    );
    assert_eq!(
        line(&[("🥲", "2026-11-19")]).as_deref(),
        Some("🥲 2026-11-19")
    );
}

#[test]
fn the_quick_set_is_the_recent_reactions_then_the_fixed_ones_without_repeats_by_key() {
    let list = |recent: &[&str]| {
        quick_reactions(&recent.iter().map(|e| e.to_string()).collect::<Vec<_>>())
    };
    assert_eq!(list(&[]), [HEART, "😂", "😢"]);
    assert_eq!(list(&["🔥", HEART]), ["🔥", HEART, "😂"]);
    assert_eq!(list(&["🔥", "🎉", "🙏", "👍"]), ["🔥", "🎉", "🙏"]);
    // The bare heart and the heart with VS16 are one reaction: the fixed heart does not follow it.
    assert_eq!(list(&["\u{2764}", "🔥"]), ["\u{2764}", "🔥", "😂"]);
    assert_eq!(list(&["😢", "😂"]), ["😢", "😂", HEART]);
}

#[test]
fn a_broken_recent_list_is_empty_and_a_good_one_is_cleaned_and_capped() {
    for broken in [
        None,
        Some(""),
        Some("{"),
        Some("{}"),
        Some("[1,2]"),
        Some("\"x\""),
        Some("null"),
    ] {
        assert!(parse_recent(broken).is_empty(), "{broken:?}");
    }
    let stored = json!([
        "🔥",
        "not an emoji",
        "\u{2764}\u{FE0F}",
        "\u{2764}",
        "🔥",
        "",
        "😂😂"
    ])
    .to_string();
    assert_eq!(parse_recent(Some(&stored)), ["🔥", HEART]);
    let many = json!(
        (0..20u32)
            .map(|i| char::from_u32(0x1F600 + i).unwrap().to_string())
            .collect::<Vec<_>>()
    )
    .to_string();
    assert_eq!(parse_recent(Some(&many)).len(), MAX_RECENT);
}

#[test]
fn a_pushed_reaction_goes_first_and_a_repeat_by_key_moves_up() {
    let start = vec!["🔥".to_string(), HEART.to_string(), "😂".to_string()];
    assert_eq!(push_recent(&start, "\u{2764}"), ["\u{2764}", "🔥", "😂"]);
    assert_eq!(push_recent(&start, "🎉")[0], "🎉");
    let full: Vec<String> = (0..MAX_RECENT as u32)
        .map(|i| char::from_u32(0x1F600 + i).unwrap().to_string())
        .collect();
    let pushed = push_recent(&full, "🎉");
    assert_eq!(pushed.len(), MAX_RECENT);
    assert_eq!(
        pushed.last(),
        full.get(MAX_RECENT - 2),
        "the oldest fell off"
    );
}

/// 7b's allowlist is by entity and group: `reactions` is for a message only, and a group that is not on the list is not added.
#[test]
fn the_dispatch_adds_only_what_the_allowlist_names() {
    let (raw, hlcs) = (
        r#"{"core":{}}"#,
        format!(
            r#"{{"core":{{"h":"{}","sv":1}}}}"#,
            super::tests::hlc(1_000)
        ),
    );
    let current = Groups {
        groups_raw: raw,
        hlc_json: &hlcs,
    };
    let hlc = super::tests::hlc(2_000);
    let edit = |group: &mut serde_json::Map<String, serde_json::Value>| {
        group.insert("items".into(), json!([]));
        Ok(())
    };
    assert!(rewrite_or_add_group(&current, "message", "reactions", &hlc, None, edit).is_ok());
    for (entity, group) in [
        ("attachment", "reactions"),
        ("message", "body"),
        ("message", "name"),
        ("chat", "reactions"),
    ] {
        assert!(
            rewrite_or_add_group(&current, entity, group, &hlc, None, edit).is_err(),
            "{entity}/{group}"
        );
    }
}
