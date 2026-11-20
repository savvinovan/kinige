//! The caps of a put (slice 5a, promise 1): 10 live reactions, 32 different keys ever, a group over them from elsewhere, and the size
//! limit of the group.

use super::tests::{group_of, item};
use super::write::ReactionError;
use super::write_tests::{AT, HEART, Record};
use super::*;
use serde_json::Value;

fn emoji(first: u32, count: u32) -> Vec<String> {
    (0..count)
        .map(|i| char::from_u32(first + i).unwrap().to_string())
        .collect()
}

#[test]
fn the_eleventh_live_reaction_is_refused_and_nothing_is_dropped() {
    let mut record = Record::new();
    let emoji = emoji(0x1F600, 11);
    for e in &emoji[..10] {
        record.change(e, true, AT).unwrap();
    }
    let before = record.bytes();
    assert_eq!(
        record.change(&emoji[10], true, AT),
        Err(ReactionError::Limit)
    );
    assert_eq!(record.bytes(), before, "nothing written");
    assert_eq!(record.shown().reactions.len(), 10);
    // A live one can still be put again (no-op) and removed; once one is gone an eleventh fits.
    assert!(!record.change(&emoji[0], true, AT).unwrap());
    assert!(record.change(&emoji[0], false, AT).unwrap());
    assert!(record.change(&emoji[10], true, AT).unwrap());
    // The removed one is a marker that can come back only while there is room.
    assert_eq!(
        record.change(&emoji[0], true, AT),
        Err(ReactionError::Limit)
    );
    assert_eq!(record.shown().keys.len(), 11);
}

#[test]
fn the_thirty_third_key_is_refused_even_after_most_were_removed_and_the_old_ones_still_toggle() {
    let mut record = Record::new();
    let emoji = emoji(0x1F300, 33);
    for e in &emoji[..32] {
        record.change(e, true, AT).unwrap();
        record.change(e, false, AT).unwrap();
    }
    assert_eq!(record.shown().keys.len(), 32);
    assert!(record.shown().reactions.is_empty());
    let before = record.bytes();
    assert_eq!(
        record.change(&emoji[32], true, AT),
        Err(ReactionError::Limit)
    );
    assert_eq!(record.bytes(), before);
    assert!(
        record.change(&emoji[5], true, AT + 1).unwrap(),
        "a key the entry has had can be put again"
    );
    assert!(record.change(&emoji[5], false, AT + 2).unwrap());
}

#[test]
fn a_group_over_the_caps_from_elsewhere_is_shown_in_full_refuses_a_put_and_allows_a_removal() {
    // 12 live reactions: more than a put allows, written by another device.
    let items: Vec<Value> = emoji(0x1F600, 12)
        .iter()
        .map(|e| item(e, Some(AT), Some("UTC"), 1_500))
        .collect();
    let mut record = Record::with_group(&group_of(&items), 1_500);
    assert_eq!(record.shown().reactions.len(), 12, "shown in full");
    assert_eq!(record.change("🎉", true, AT), Err(ReactionError::Limit));
    assert!(record.change(&emoji(0x1F600, 1)[0], false, AT).unwrap());
    assert_eq!(record.shown().reactions.len(), 11);
    assert_eq!(record.shown().keys.len(), 12, "nothing was trimmed");
    assert_eq!(
        record.change("🎉", true, AT),
        Err(ReactionError::Limit),
        "11 live is still over the cap"
    );
}

#[test]
fn a_put_replaces_a_live_item_whose_emoji_is_not_one() {
    let bad = item("\u{2764}\u{FE0F}\u{FE0F}", Some(AT), Some("UTC"), 1_500);
    let mut record = Record::with_group(&group_of(&[bad]), 1_500);
    assert!(record.shown().reactions.is_empty(), "nothing shows");
    assert!(record.change(HEART, true, AT + 9).unwrap());
    let got = record.shown();
    assert_eq!(
        (got.reactions.len(), got.keys.len()),
        (1, 1),
        "replaced, not added"
    );
    assert_eq!(got.reactions[0].emoji, HEART);
}

#[test]
fn the_group_stays_inside_its_size_limit_with_32_longest_items() {
    let zone = "Area/".to_string() + &"x".repeat(59);
    // 39 bytes: a ZWJ chain of six 4-byte emoji, close to the longest the picker produces, different by its first character.
    let longest = |i: u32| -> String {
        let first = char::from_u32(0x1F400 + i).unwrap();
        format!("{first}\u{200D}👩\u{200D}👩\u{200D}👧\u{200D}👦\u{200D}👦")
    };
    // Even 32 live items of that size (more than a put allows, but a peer may write them) fit the 8 KiB the reader accepts.
    let items: Vec<Value> = (0..32u32)
        .map(|i| {
            let e = longest(i);
            assert!(
                e.len() <= crate::emoji::MAX_EMOJI_BYTES && crate::emoji::is_emoji_sequence(&e),
                "{e}"
            );
            item(&e, Some(MAX_AT), Some(&zone), 1_500)
        })
        .collect();
    let group = group_of(&items);
    assert!(group.len() <= MAX_GROUP_BYTES, "{}", group.len());
    let (raw, hlcs) = super::tests::record(Some(&group), 1_500);
    assert_eq!(shown(&raw, &hlcs).reactions.len(), 32);
}
