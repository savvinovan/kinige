//! Slice 5a: the two counts the caps use reach the view the right way round, and one damaged derived row costs its entry its keys and nothing else.

#![cfg(test)]

mod common;

use common::reactions::*;
use common::*;
use kinige_domain::groups::INBOX_ID;

#[test]
fn the_key_count_and_the_live_count_are_not_swapped() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let sent = send(&store, "counts");
    // Three different emoji put, two removed: the group holds three items, one of them live.
    for (i, e) in [HEART, "🔥", "😂"].iter().enumerate() {
        set(&store, &sent.id, e, true, &later(i as u32 + 1));
    }
    set(&store, &sent.id, HEART, false, &later(4));
    let view = set(&store, &sent.id, "🔥", false, &later(5));
    assert_eq!((view.reaction_key_count, view.reaction_live_count), (3, 1));
    assert_eq!(view.reactions.len(), 1);
    // The feed's view says the same.
    let page = store.list_messages(INBOX_ID, None, 10, &later(6)).unwrap();
    let listed = page.messages.iter().find(|m| m.id == sent.id).unwrap();
    assert_eq!(
        (listed.reaction_key_count, listed.reaction_live_count),
        (3, 1)
    );
}

#[test]
fn a_damaged_keys_value_still_gives_the_page_and_search_with_the_chips_and_no_keys() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    let hurt = send(&store, "damaged exam row");
    let fine = send(&store, "fine exam row");
    for entry in [&hurt, &fine] {
        set(&store, &entry.id, HEART, true, &later(1));
        set(&store, &entry.id, "🔥", true, &later(2));
    }
    for broken in ["{broken", r#"{"a":1}"#, "", "[1,2", "null"] {
        raw(dir.path())
            .execute(
                "UPDATE message_reaction_keys SET keys = ?1 WHERE message_id = ?2",
                (broken, &hurt.id),
            )
            .unwrap();
        let page = store
            .list_messages(INBOX_ID, None, 10, &later(3))
            .unwrap_or_else(|e| panic!("page failed on {broken:?}: {e:?}"));
        let bad = page.messages.iter().find(|m| m.id == hurt.id).unwrap();
        assert_eq!(
            bad.reactions.len(),
            2,
            "the chips come from the other table ({broken:?})"
        );
        assert!(
            bad.reaction_keys_had.is_empty(),
            "no keys from a damaged value ({broken:?})"
        );
        assert_eq!((bad.reaction_key_count, bad.reaction_live_count), (2, 2));
        let good = page.messages.iter().find(|m| m.id == fine.id).unwrap();
        assert_eq!(
            good.reaction_keys_had.len(),
            2,
            "the neighbour is untouched"
        );
        let found = store
            .search("exam", &Default::default(), None, 10)
            .unwrap_or_else(|e| panic!("search failed on {broken:?}: {e:?}"));
        assert_eq!(found.hits.len(), 2, "{broken:?}");
        let hit = found.hits.iter().find(|h| h.message.id == hurt.id).unwrap();
        assert_eq!(hit.message.reactions.len(), 2);
        assert!(hit.message.reaction_keys_had.is_empty());
    }
    // A damaged count reads as 0, and a column that holds text where a number should be does not fail the page either.
    raw(dir.path())
        .execute(
            "UPDATE message_reaction_keys SET item_count = 'x', live_count = 'y' WHERE message_id = ?1",
            [&hurt.id],
        )
        .unwrap();
    let page = store.list_messages(INBOX_ID, None, 10, &later(3)).unwrap();
    let bad = page.messages.iter().find(|m| m.id == hurt.id).unwrap();
    assert_eq!((bad.reaction_key_count, bad.reaction_live_count), (0, 0));
}
