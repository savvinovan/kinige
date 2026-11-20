//! The reader and the writer against what another device may have written (slice 5a, guardian round 1): the text of an item that is not an emoji
//! reaches no derived value, the order of items is a writer rule that the reader does not enforce, and the rules version is tied to the rules.

use super::tests::{group_of, item, record};
use super::write::ReactionError;
use super::write_tests::{AT, HEART, Record};
use super::*;

/// An item whose emoji is not one: markup, and a text of 3,000 bytes. They are live, so they take places of the 10-live cap.
fn hostile() -> Vec<Value> {
    vec![
        item("<script>alert(1)</script>", Some(AT), Some("UTC"), 2_000),
        item(&"x".repeat(3_000), Some(AT), Some("UTC"), 2_000),
        // A removal marker whose `e` is not an emoji either.
        item("not an emoji", None, None, 2_000),
        // A valid emoji with a zone that is not valid: hidden, but its key is an emoji's.
        item("🔥", Some(AT), Some(""), 2_000),
        item(HEART, Some(AT), Some("UTC"), 2_000),
    ]
}

#[test]
fn the_text_of_an_item_that_is_not_an_emoji_reaches_no_derived_value() {
    let (raw, hlcs) = record(Some(&group_of(&hostile())), 5_000);
    let got = shown(&raw, &hlcs);
    // The keys are those of valid emoji only: no markup, no 3,000 bytes, no marker's text.
    assert_eq!(got.keys, ["\u{2764}", "🔥"]);
    assert!(got.keys.iter().all(|key| key.len() <= 40));
    assert_eq!(got.reactions.len(), 1, "only the heart is shown");
    // The caps still count every item, so the store and the interface agree: five items, four of them live (the marker is not).
    assert_eq!((got.item_count, got.live_count), (5, 4));
    // Nothing of the hostile text is in the debug form of what the reader gives out either.
    let everything = format!("{got:?}");
    assert!(
        !everything.contains("script")
            && !everything.contains("xxxx")
            && !everything.contains("not an emoji")
    );
}

#[test]
fn an_unsorted_group_from_a_peer_is_read_and_a_write_sorts_it_and_keeps_the_other_items_bytes() {
    // Items out of key order: the reader does not enforce the order (it is a writer rule), so the group is well-formed.
    let fire = item("🔥", Some(AT), Some("UTC"), 2_000);
    let heart = item(HEART, Some(AT + 1), Some("Europe/Moscow"), 3_000);
    let unsorted = group_of(&[fire.clone(), heart.clone()]);
    let sorted = group_of(&[heart.clone(), fire.clone()]);
    assert_ne!(unsorted, sorted);
    let (raw, hlcs) = record(Some(&unsorted), 5_000);
    let got = shown(&raw, &hlcs);
    assert_eq!(got.reactions.len(), 2, "an unsorted group is well-formed");
    assert_eq!(got.keys, ["\u{2764}", "🔥"]);

    let mut entry = Record::with_group(&unsorted, 5_000);
    assert!(entry.change("😢", true, AT + 2).unwrap());
    let written = entry.shown();
    assert_eq!(written.keys, ["\u{2764}", "🔥", "😢"], "sorted by key");
    // The bytes of the two old items are inside the new group exactly as they were.
    let bytes = entry.bytes().0;
    for old in [&fire, &heart] {
        let text = jcs::canonical_string(old).unwrap();
        assert!(bytes.contains(&text), "{text} is kept byte for byte");
    }
    let (heart_at, fire_at) = (bytes.find("\u{2764}").unwrap(), bytes.find("🔥").unwrap());
    assert!(
        heart_at < fire_at,
        "the heart (U+2764) is written before the fire (U+1F525)"
    );
}

/// Everything the derived rows are built from, as one number: which single code points are an emoji, which sequences and zones are accepted, how
/// keys are made, and the limits. A change to any of them changes what `shown` lets through, so `RULES_VERSION` must change with it.
fn rules_fingerprint() -> u64 {
    fn mix(hash: &mut u64, byte: u8) {
        *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    fn mix_u64(hash: &mut u64, value: u64) {
        value.to_le_bytes().iter().for_each(|byte| mix(hash, *byte));
    }
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for code in 0..=0x0010_FFFF_u32 {
        let Some(c) = char::from_u32(code) else {
            continue;
        };
        if is_emoji_sequence(&c.to_string()) {
            mix_u64(&mut hash, u64::from(code));
        }
    }
    let sequences = [
        "\u{2764}\u{FE0F}",
        "\u{2764}\u{FE0F}\u{200D}🔥",
        "👍🏽",
        "👨\u{200D}👩\u{200D}👧",
        "🇷🇺",
        "1\u{FE0F}\u{20E3}",
        "🏴\u{E0067}\u{E0062}\u{E007F}",
        "\u{FE0F}",
        "\u{200D}",
        "a",
        "",
    ];
    for text in sequences {
        mix(&mut hash, u8::from(is_emoji_sequence(text)));
        reaction_key(text)
            .bytes()
            .for_each(|byte| mix(&mut hash, byte));
    }
    let zones = [
        "UTC",
        "Europe/Moscow",
        "",
        "a\u{0}b",
        "a\nb",
        &"z".repeat(MAX_TZ_BYTES),
        &"z".repeat(MAX_TZ_BYTES + 1),
    ];
    for zone in zones {
        mix(&mut hash, u8::from(is_valid_tz(zone)));
    }
    for limit in [
        MAX_LIVE as u64,
        MAX_KEYS as u64,
        MAX_GROUP_BYTES as u64,
        MAX_TZ_BYTES as u64,
        MAX_AT as u64,
    ] {
        mix_u64(&mut hash, limit);
    }
    hash
}

#[test]
fn the_rules_version_is_tied_to_the_rules_it_stands_for() {
    // If this fails, what `shown` lets through has changed (the emoji table, the zone rule, the key, a limit). Bump `RULES_VERSION`, so that every
    // device rebuilds its derived rows on open, and then update both numbers here.
    assert_eq!(
        (RULES_VERSION, rules_fingerprint()),
        (1, 1_195_285_269_939_224_153)
    );
}

#[test]
fn a_group_of_items_that_are_all_hostile_still_counts_toward_the_caps_and_shows_nothing() {
    let items: Vec<Value> = (0..11)
        .map(|i| item(&format!("<{i}>"), Some(AT), Some("UTC"), 2_000))
        .collect();
    let (raw, hlcs) = record(Some(&group_of(&items)), 5_000);
    let got = shown(&raw, &hlcs);
    assert_eq!((got.reactions.len(), got.keys.len()), (0, 0));
    assert_eq!((got.item_count, got.live_count), (11, 11));
    // A put is refused by the live cap, as the store sees it.
    let mut entry = Record::with_group(&group_of(&items), 5_000);
    assert_eq!(entry.change(HEART, true, AT), Err(ReactionError::Limit));
}
