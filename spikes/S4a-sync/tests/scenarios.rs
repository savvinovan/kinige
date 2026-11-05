//! Named counterexamples found by the properties, kept as plain scenarios so that they read as a story and stay fixed.

use s4a_sync::envelope::DAY_MS;
use s4a_sync::props;
use s4a_sync::switches::Switches;
use s4a_sync::world::{Op::*, PullFault, PushFault, Schedule, Step::*};

fn two_devices(steps: Vec<s4a_sync::world::Step>) -> Schedule {
    Schedule {
        clients: 2,
        skews: vec![0, 0],
        unbound: vec![false, false],
        compact_outbox: false,
        page: 5,
        batch: 5,
        steps,
    }
}

/// S4a finding 2. A device pushed its edit P (acked) and goes away for more than 180 days. Another device, which had not yet seen P,
/// edits the message twice (the second time from its own confirmed first edit, so `base` points at the first one) and the server
/// compacts the first one away. The away device returns, pulls only the second edit, and P is below its `base`.
fn a_confirmed_version_below_the_base_of_a_winner_whose_parent_was_compacted() -> Schedule {
    two_devices(vec![
        Local(0, CreateMessage(0)),
        SyncAll { skip: usize::MAX },
        Pull(0, PullFault::Ok),
        Local(1, Edit(0)),
        Pull(1, PullFault::Ok),
        Push(1, PushFault::Ok),
        Local(0, Edit(0)),
        Push(0, PushFault::Ok),
        Local(0, Edit(0)),
        Pull(0, PullFault::Ok),
        Push(0, PushFault::Ok),
        Advance(200 * DAY_MS),
        Compact,
    ])
}

#[test]
fn finding_2_with_base_and_supersedes_only_a_version_is_lost() {
    let old_rule = Switches {
        no_vv: true,
        ..Switches::default()
    };
    let result = props::no_version_lost(
        &a_confirmed_version_below_the_base_of_a_winner_whose_parent_was_compacted(),
        old_rule,
    );
    let reason = result.expect_err("the rule as ADR-0001 revision 7 states it loses the version");
    println!("{reason}");
    assert!(reason.contains("is gone"));
}

#[test]
fn finding_2_with_vv_the_version_is_kept() {
    let result = props::no_version_lost(
        &a_confirmed_version_below_the_base_of_a_winner_whose_parent_was_compacted(),
        Switches::default(),
    );
    assert_eq!(result, Ok(()));
}

/// S4a finding 6. Two devices purge the same message. The server accepts the first `purged` (device 0's, whose snapshot includes a later
/// flag change); the second is a no-op `ok`. Device 1's tombstone keeps its own older snapshot, and the flag change, pulled after the
/// `purged` (the page arrives in another order), is "newer than the snapshot" and lifts the purge the server already made.
fn two_devices_purge_the_same_message() -> Schedule {
    Schedule {
        clients: 2,
        skews: vec![0, 0],
        unbound: vec![false, false],
        compact_outbox: false,
        page: 2,
        batch: 3,
        steps: vec![
            Local(0, CreateChat),
            Local(0, CreateMessage(0)),
            Local(0, CreateChat),
            Local(1, CreateChat),
            Local(0, CreateChat),
            Local(0, Edit(0)),
            Local(0, Edit(0)),
            Local(1, CreateChat),
            SyncAll { skip: usize::MAX },
            Local(0, DeleteChat(0)),
            Local(0, DeleteMessage(0)),
            Push(0, PushFault::DropResponse),
            Local(0, Favorite(0)),
            Advance(31 * DAY_MS),
            Purge(0),
            Purge(1),
            Push(0, PushFault::Ok),
            Push(1, PushFault::Duplicate),
            Pull(1, PullFault::Shuffle(0)),
        ],
    }
}

#[test]
fn finding_6_a_tombstone_that_keeps_its_own_snapshot_lets_a_purged_message_come_back() {
    let old_rule = Switches {
        keep_own_snapshot: true,
        ..Switches::default()
    };
    let reason = props::no_resurrection(&two_devices_purge_the_same_message(), old_rule)
        .expect_err("the purged message comes back");
    println!("{reason}");
    assert!(reason.contains("live again") || reason.contains("differ"));
}

#[test]
fn finding_6_with_the_accepted_snapshot_it_stays_purged() {
    assert_eq!(
        props::no_resurrection(&two_devices_purge_the_same_message(), Switches::default()),
        Ok(())
    );
}
