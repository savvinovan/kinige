//! Purge, tombstone, resync and feed rules (ADR-0001 §3.5, §3.6, §6.3).

use super::*;
use crate::hlc::Clock;

const A: &str = "5f0c3e2a-9b1d-4c7e-8a64-0d2f1b3c4e5f";
const B: &str = "7a1b2c3d-4e5f-4a6b-8c7d-9e0f1a2b3c4d";

fn hlc(l: u64, c: u32, device: &str) -> String {
    Clock { l, c }.format(device)
}

fn snap(entries: &[(&str, &str)]) -> Snapshot {
    entries
        .iter()
        .map(|(g, h)| (g.to_string(), h.to_string()))
        .collect()
}

#[test]
fn a_group_missing_from_the_snapshot_is_newer_than_it() {
    let h = hlc(10, 0, A);
    let later = hlc(11, 0, A);
    let snapshot = snap(&[("body", &h), ("deleted", &h)]);
    assert!(!newer_than_snapshot(
        &snap(&[("body", &h), ("deleted", &h)]),
        &snapshot
    ));
    assert!(
        !newer_than_snapshot(&snap(&[("body", &h)]), &snapshot),
        "a group only in the snapshot is not newer"
    );
    assert!(newer_than_snapshot(&snap(&[("body", &later)]), &snapshot));
    assert!(
        newer_than_snapshot(&snap(&[("flags", &h)]), &snapshot),
        "an unknown group counts as newer"
    );
    assert!(!newer_than_snapshot(&Snapshot::new(), &snapshot));
}

#[test]
fn purge_verdicts_follow_the_snapshot() {
    let h = hlc(10, 0, A);
    let later = hlc(11, 0, B);
    let purged_of = snap(&[("body", &h), ("deleted", &h)]);
    assert_eq!(
        on_purge(&snap(&[("body", &h), ("deleted", &h)]), &purged_of),
        PurgeVerdict::DeleteSilently
    );
    assert_eq!(
        on_purge(&snap(&[("body", &h), ("deleted", &later)]), &purged_of),
        PurgeVerdict::Quarantine
    );
    assert_eq!(
        envelope_vs_tombstone(&snap(&[("body", &h)]), &purged_of),
        TombstoneVerdict::Drop
    );
    assert_eq!(
        envelope_vs_tombstone(&snap(&[("deleted", &later)]), &purged_of),
        TombstoneVerdict::Lift
    );
}

#[test]
fn chat_children_are_the_messages_placed_before_the_deletion() {
    let deleted = hlc(100, 0, A);
    let purged_of = snap(&[("deleted", &deleted), ("meta", &hlc(1, 0, A))]);
    assert!(is_chat_child(&hlc(99, 5, B), &purged_of));
    assert!(!is_chat_child(&deleted, &purged_of), "equal is not less");
    assert!(
        !is_chat_child(&hlc(101, 0, B), &purged_of),
        "moved in after the deletion: live"
    );
    assert!(
        !is_chat_child(&hlc(1, 0, A), &snap(&[("meta", &hlc(1, 0, A))])),
        "no deletion, no children"
    );
}

#[test]
fn the_resync_table_matches_adr_6_3() {
    use ResyncAction::*;
    use ResyncCause::*;
    let facts = |server_known, on_server, newer_than_server| Facts {
        server_known,
        on_server,
        newer_than_server,
    };
    // Not on the server, never left the device: push at any age, whatever the cause.
    for cause in [Binding, StaleCursor, GenerationChanged, StaleAndGeneration] {
        assert_eq!(
            resync_action(cause, facts(false, false, true)),
            Push,
            "{cause:?}"
        );
    }
    // Not on the server, the server knew it.
    assert_eq!(
        resync_action(StaleCursor, facts(true, false, true)),
        Quarantine
    );
    assert_eq!(
        resync_action(GenerationChanged, facts(true, false, true)),
        Push
    );
    assert_eq!(
        resync_action(StaleAndGeneration, facts(true, false, true)),
        Quarantine,
        "both: stale wins"
    );
    assert_eq!(
        resync_action(Binding, facts(true, false, true)),
        Quarantine,
        "impossible state: the safe side"
    );
    // On the server: only a record that is ahead is pushed.
    assert_eq!(resync_action(StaleCursor, facts(true, true, true)), Push);
    assert_eq!(
        resync_action(StaleCursor, facts(true, true, false)),
        Nothing
    );
    assert_eq!(resync_action(Binding, facts(false, true, false)), Nothing);
}

#[test]
fn the_feed_hides_children_until_the_parent_arrives() {
    use ChatState::*;
    assert_eq!(message_slot(Live, false), Slot::InChat);
    assert_eq!(message_slot(Trashed, true), Slot::Hidden);
    assert_eq!(
        message_slot(Unknown, false),
        Slot::Hidden,
        "the chat may still be on its way"
    );
    assert_eq!(
        message_slot(Unknown, true),
        Slot::Inbox(InboxMark::ChatUnavailable)
    );
    let child = |recent| Purged {
        message_is_child: true,
        recent,
    };
    assert_eq!(
        message_slot(child(true), true),
        Slot::Hidden,
        "its purged follows"
    );
    assert_eq!(
        message_slot(child(false), false),
        Slot::Hidden,
        "pull has not reached the head: more may be coming"
    );
    assert_eq!(
        message_slot(child(false), true),
        Slot::Inbox(InboxMark::ChatDeleted),
        "S4a: a child whose purged never came is not hidden until the tombstone expires"
    );
    assert_eq!(
        message_slot(
            Purged {
                message_is_child: false,
                recent: true
            },
            false
        ),
        Slot::Inbox(InboxMark::ChatDeleted)
    );
}
