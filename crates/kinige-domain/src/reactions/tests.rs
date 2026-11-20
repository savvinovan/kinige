//! The reader of the `reactions` group (slice 5a): the key function and its reference vectors, each well-formedness rule, hostile items.

use super::*;
use serde_json::json;

pub(super) const DEVICE: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const AT: i64 = 1_700_000_000_000;

pub(super) fn hlc(l: u64) -> String {
    format!("{l:013}-00000-{DEVICE}")
}

/// A record with the given `reactions` group bytes and group HLC; the other maps are those of an entry's `flags`.
pub(super) fn record(group: Option<&str>, group_hlc: u64) -> (String, String) {
    let mut raw = vec![r#""flags":{"favorite_at":null}"#.to_string()];
    let mut hlcs = vec![format!(r#""flags":{{"h":"{}","sv":1}}"#, hlc(1_000))];
    if let Some(bytes) = group {
        raw.push(format!(r#""reactions":{bytes}"#));
        hlcs.push(format!(
            r#""reactions":{{"h":"{}","sv":1}}"#,
            hlc(group_hlc)
        ));
    }
    (
        format!("{{{}}}", raw.join(",")),
        format!("{{{}}}", hlcs.join(",")),
    )
}

pub(super) fn item(e: &str, at: Option<i64>, tz: Option<&str>, h: u64) -> Value {
    json!({"at": at, "e": e, "h": hlc(h), "tz": tz})
}

pub(super) fn group_of(items: &[Value]) -> String {
    jcs::canonical_string(&json!({ "items": items })).unwrap()
}

fn shown_of(group: &str) -> Shown {
    let (raw, hlcs) = record(Some(group), 5_000);
    shown(&raw, &hlcs)
}

fn read(group: &str, group_hlc: u64) -> Stored {
    let (raw, hlcs) = record(Some(group), group_hlc);
    read_group(&Groups {
        groups_raw: &raw,
        hlc_json: &hlcs,
    })
    .unwrap()
}

#[test]
fn the_key_drops_variation_selector_16_and_nothing_else() {
    // The reference vectors of ADR-0001 appendix §C.9; the fixture `fixtures/api-contract/reactions.json` repeats them.
    let vectors: &[(&str, &str)] = &[
        ("\u{2764}\u{FE0F}", "\u{2764}"),
        ("\u{2764}", "\u{2764}"),
        ("😂", "😂"),
        ("👍🏽", "👍🏽"),
        ("\u{2764}\u{FE0F}\u{200D}🔥", "\u{2764}\u{200D}🔥"),
        ("1\u{FE0F}\u{20E3}", "1\u{20E3}"),
        ("🏳\u{FE0F}\u{200D}🌈", "🏳\u{200D}🌈"),
        ("🇷🇺", "🇷🇺"),
        ("", ""),
        ("\u{FE0F}", ""),
        ("\u{FE0F}\u{FE0F}x\u{FE0F}", "x"),
    ];
    for (emoji, key) in vectors {
        assert_eq!(reaction_key(emoji), *key, "{emoji:?}");
    }
}

#[test]
fn the_key_is_not_the_pickers_identity_function_by_name_or_by_promise() {
    // Both drop VS16 today; the group depends on this one, so a change of the picker's function must not reach it. The two must stay
    // separate items: here the assertion is on the behaviour, the code review on the separation.
    assert_eq!(
        reaction_key("\u{2764}\u{FE0F}"),
        crate::emoji::emoji_identity("\u{2764}\u{FE0F}")
    );
}

#[test]
fn keys_order_by_utf8_bytes() {
    let mut keys = vec![
        reaction_key("🔥"),
        reaction_key("\u{2764}\u{FE0F}"),
        reaction_key("😂"),
    ];
    keys.sort();
    // U+2764 < U+1F525 (🔥) < U+1F602 (😂); the UTF-8 byte order is the code point order.
    assert_eq!(keys, ["\u{2764}", "🔥", "😂"]);
}

#[test]
fn a_well_formed_group_shows_its_live_reactions_in_the_order_put() {
    let group = group_of(&[
        item("🔥", Some(AT + 2_000), Some("UTC"), 3_000),
        item("\u{2764}\u{FE0F}", Some(AT), Some("Europe/Moscow"), 2_000),
        item("😢", None, None, 4_000),
    ]);
    let got = shown_of(&group);
    assert_eq!(got.keys.len(), 3, "a removal marker counts as a key");
    assert_eq!(got.keys, ["\u{2764}", "🔥", "😢"], "in key order");
    let emoji: Vec<_> = got.reactions.iter().map(|r| r.emoji.as_str()).collect();
    assert_eq!(emoji, ["\u{2764}\u{FE0F}", "🔥"]);
    assert_eq!(got.reactions[0].put_tz, "Europe/Moscow");
    assert_eq!(got.reactions[0].key, "\u{2764}");
}

#[test]
fn a_tie_in_time_is_broken_by_key() {
    let group = group_of(&[
        item("🔥", Some(AT), Some("UTC"), 2_000),
        item("😂", Some(AT), Some("UTC"), 2_000),
    ]);
    let emoji: Vec<_> = shown_of(&group)
        .reactions
        .into_iter()
        .map(|r| r.emoji)
        .collect();
    assert_eq!(
        emoji,
        ["🔥", "😂"],
        "equal times: U+1F525 sorts before U+1F602"
    );
}

#[test]
fn an_absent_group_is_absent_and_shows_nothing() {
    let (raw, hlcs) = record(None, 0);
    let current = Groups {
        groups_raw: &raw,
        hlc_json: &hlcs,
    };
    assert!(matches!(read_group(&current).unwrap(), Stored::Absent));
    assert_eq!(shown(&raw, &hlcs), Shown::default());
}

#[test]
fn unknown_fields_are_allowed_and_kept() {
    let mut with_extra = item("😂", Some(AT), Some("UTC"), 2_000);
    with_extra["sticker"] = json!({"id": 7});
    let mut value = json!({"items": [with_extra], "zz_future": [1, 2]});
    let group = jcs::canonical_string(&value).unwrap();
    let Stored::Group(parsed) = read(&group, 5_000) else {
        panic!("a group with unknown fields is well-formed");
    };
    assert_eq!(parsed.shown().reactions.len(), 1);
    assert_eq!(parsed.items[0].object["sticker"], json!({"id": 7}));
    value["items"] = json!("not a list");
    assert!(matches!(
        read(&jcs::canonical_string(&value).unwrap(), 5_000),
        Stored::Unreadable
    ));
}

/// Each rule that makes a group not well-formed, by a group that breaks only that rule.
#[test]
fn every_well_formedness_rule_refuses_its_own_violation() {
    let good = || item("😂", Some(AT), Some("UTC"), 2_000);
    let cases: Vec<(&str, String)> = vec![
        ("not canonical: spaces", r#"{"items": []}"#.to_string()),
        (
            "not canonical: key order",
            format!(
                r#"{{"items":[{{"e":"😂","at":{AT},"h":"{}","tz":"UTC"}}]}}"#,
                hlc(2_000)
            ),
        ),
        ("not an object", "[]".to_string()),
        ("null", "null".to_string()),
        ("no items", "{}".to_string()),
        ("items is not an array", r#"{"items":{}}"#.to_string()),
        (
            "an item is not an object",
            r#"{"items":["😂"]}"#.to_string(),
        ),
        (
            "e is not a string",
            group_of(&[json!({"at": AT, "e": 5, "h": hlc(2_000), "tz": "UTC"})]),
        ),
        (
            "no e",
            group_of(&[json!({"at": AT, "h": hlc(2_000), "tz": "UTC"})]),
        ),
        (
            "two items of one key",
            group_of(&[good(), item("😂", None, None, 2_500)]),
        ),
        (
            "two items of one key, one with VS16",
            group_of(&[
                item("\u{2764}", None, None, 2_000),
                item("\u{2764}\u{FE0F}", None, None, 2_100),
            ]),
        ),
        (
            "h is not a string",
            group_of(&[json!({"at": AT, "e": "😂", "h": 1, "tz": "UTC"})]),
        ),
        (
            "h is not a strict HLC",
            group_of(&[json!({"at": AT, "e": "😂", "h": "0000000002000-0-x", "tz": "UTC"})]),
        ),
        (
            "h is above the group's",
            group_of(&[item("😂", Some(AT), Some("UTC"), 5_001)]),
        ),
        (
            "no h",
            group_of(&[json!({"at": AT, "e": "😂", "tz": "UTC"})]),
        ),
        (
            "at is negative",
            group_of(&[item("😂", Some(-1), Some("UTC"), 2_000)]),
        ),
        (
            "at is above the range",
            group_of(&[item("😂", Some(MAX_AT + 1), Some("UTC"), 2_000)]),
        ),
        (
            "at is a string",
            group_of(&[json!({"at": "1", "e": "😂", "h": hlc(2_000), "tz": "UTC"})]),
        ),
        (
            "at is absent",
            group_of(&[json!({"e": "😂", "h": hlc(2_000), "tz": "UTC"})]),
        ),
        (
            "at is a fraction",
            format!(
                r#"{{"items":[{{"at":1.5,"e":"😂","h":"{}","tz":"UTC"}}]}}"#,
                hlc(2_000)
            ),
        ),
        (
            "over 8 KiB",
            group_of(
                &(0..40u32)
                    .map(|i| {
                        item(
                            &format!("{}", char::from_u32(0x1F300 + i).unwrap()),
                            Some(AT),
                            Some(&"z".repeat(64)),
                            2_000,
                        )
                    })
                    .chain([
                        json!({"at": null, "e": "x".repeat(8_000), "h": hlc(2_000), "tz": null}),
                    ])
                    .collect::<Vec<_>>(),
            ),
        ),
    ];
    for (name, group) in cases {
        assert!(
            matches!(read(&group, 5_000), Stored::Unreadable),
            "{name}: {group}"
        );
        assert_eq!(shown_of(&group), Shown::default(), "{name} shows nothing");
    }
    assert!(matches!(
        read(&group_of(&[good()]), 5_000),
        Stored::Group(_)
    ));
    // The boundary: an item stamped with the group's own HLC is fine, and `at` may be 0 and the maximum.
    let edge = group_of(&[
        item("😂", Some(0), Some("UTC"), 5_000),
        item("🔥", Some(MAX_AT), Some("UTC"), 5_000),
    ]);
    assert!(matches!(read(&edge, 5_000), Stored::Group(_)));
}

#[test]
fn a_group_whose_version_is_not_the_supported_one_shows_nothing() {
    let group = group_of(&[item("😂", Some(AT), Some("UTC"), 2_000)]);
    let (raw, _) = record(Some(&group), 5_000);
    let newer = format!(
        r#"{{"flags":{{"h":"{}","sv":1}},"reactions":{{"h":"{}","sv":2}}}}"#,
        hlc(1_000),
        hlc(5_000)
    );
    assert_eq!(shown(&raw, &newer), Shown::default());
    let missing = format!(r#"{{"flags":{{"h":"{}","sv":1}}}}"#, hlc(1_000));
    assert_eq!(
        shown(&raw, &missing),
        Shown::default(),
        "a group in one map only is not shown"
    );
    let bad_h = format!(
        r#"{{"flags":{{"h":"{}","sv":1}},"reactions":{{"h":"x","sv":1}}}}"#,
        hlc(1_000)
    );
    assert_eq!(shown(&raw, &bad_h), Shown::default());
}

/// Items that never reach a view: an `e` that is not an emoji, a zone that is not valid, a live item with no zone.
#[test]
fn hostile_items_are_counted_but_never_shown() {
    let long_tz = "z".repeat(65);
    let items = [
        item("not an emoji", Some(AT), Some("UTC"), 2_000),
        item("😂\u{0}", Some(AT), Some("UTC"), 2_000),
        item("😂😂", Some(AT), Some("UTC"), 2_000),
        item("🔥", Some(AT), Some(""), 2_000),
        item("🎉", Some(AT), Some(&long_tz), 2_000),
        item("🌟", Some(AT), Some("Europe/\nMoscow"), 2_000),
        item("😢", Some(AT), None, 2_000),
        json!({"at": AT, "e": "🙏", "h": hlc(2_000), "tz": 5}),
        item("👍", Some(AT), Some("UTC"), 2_000),
    ];
    let got = shown_of(&group_of(&items));
    assert_eq!(got.keys.len(), 9);
    let emoji: Vec<_> = got.reactions.iter().map(|r| r.emoji.as_str()).collect();
    assert_eq!(
        emoji,
        ["👍"],
        "only the item that passes every check is shown"
    );
}

#[test]
fn a_zone_of_exactly_64_bytes_is_valid_and_65_is_not() {
    assert!(is_valid_tz(&"a".repeat(64)));
    assert!(!is_valid_tz(&"a".repeat(65)));
    assert!(is_valid_tz("America/Argentina/ComodRivadavia"));
    assert!(!is_valid_tz(""));
    assert!(!is_valid_tz("a\tb"));
}

#[test]
fn garbage_and_huge_records_do_not_panic_or_show_anything() {
    let huge = format!(
        r#"{{"items":[{{"at":null,"e":"{}","h":"{}","tz":null}}]}}"#,
        "x".repeat(1 << 20),
        hlc(2_000)
    );
    for group in [
        "",
        "{",
        "\u{0}",
        "\"x\"",
        "[[[[[[[[[[[[[[[[[[[[[[[[[",
        huge.as_str(),
        r#"{"items":[null]}"#,
        r#"{"items":[[]]}"#,
    ] {
        let (raw, hlcs) = record(Some(group), 5_000);
        // A group that is not JSON at all is not even a valid `groups_raw`: the record is damaged, the reader says so.
        let current = Groups {
            groups_raw: &raw,
            hlc_json: &hlcs,
        };
        match read_group(&current) {
            Ok(Stored::Group(_)) => panic!("garbage accepted: {group:.20}"),
            Ok(_) | Err(_) => {}
        }
        assert_eq!(shown(&raw, &hlcs), Shown::default());
    }
    assert_eq!(shown("not json", "{}"), Shown::default());
    assert_eq!(shown("{}", "not json"), Shown::default());
}
