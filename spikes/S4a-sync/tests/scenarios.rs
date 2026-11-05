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

/// Review blocker (guardian): the drop epoch was not scoped to a generation. A restored server counted epochs from its backup, so a
/// later compaction brought the epoch back to a number device A already held from before the restore; A's pull was not stale, and
/// the `purged` the compaction dropped were never learned: X came back live on A, and an edit pushed from A would resurrect it.
fn a_drop_after_a_restore_reuses_an_epoch_a_device_holds() -> Schedule {
    let (a, b) = (0, 1);
    two_devices(vec![
        Local(a, CreateChat),
        Local(a, CreateMessage(0)), // X
        Local(a, CreateMessage(0)), // Y
        SyncAll { skip: usize::MAX },
        Backup,
        Local(b, DeleteMessage(1)), // B deletes Y
        SyncAll { skip: usize::MAX },
        Advance(31 * DAY_MS),
        Purge(b),
        SyncAll { skip: usize::MAX }, // A sees purged(Y)
        Advance(181 * DAY_MS),
        Compact,                      // a drop, epoch 1
        SyncAll { skip: usize::MAX }, // A's page carries epoch 1
        RestoreServer,                // generation 2, back to the backup
        SyncAll { skip: usize::MAX }, // both resync from 0
        Local(b, DeleteMessage(0)),   // B deletes X; A is away from here on
        SyncAll { skip: a },
        Advance(31 * DAY_MS),
        Purge(b),
        SyncAll { skip: a },
        Advance(181 * DAY_MS),
        Compact,                // the next drop: the old rule numbered it 1 again
        Pull(a, PullFault::Ok), // A must be told its cursor is stale
    ])
}

#[test]
fn review_epoch_after_a_restore_with_revision_8_resurrects() {
    let revision_8 = Switches {
        epoch_from_backup: true,
        epoch_survives_generation: true,
        ..Switches::default()
    };
    let reason = props::no_resurrection(
        &a_drop_after_a_restore_reuses_an_epoch_a_device_holds(),
        revision_8,
    )
    .expect_err("the purged record is live again on device 0");
    println!("{reason}");
    assert!(reason.contains("live again") || reason.contains("differ"));
}

#[test]
fn review_epoch_after_a_restore_is_stale_with_a_counter_that_only_grows() {
    let schedule = a_drop_after_a_restore_reuses_an_epoch_a_device_holds();
    assert_eq!(
        props::no_resurrection(&schedule, Switches::default()),
        Ok(())
    );
    // Either half of the fix alone is enough here; the server's counter is the rule (ADR-0001 §6.3), the client's reset a second line.
    for only_one in [
        Switches {
            epoch_from_backup: true,
            ..Switches::default()
        },
        Switches {
            epoch_survives_generation: true,
            ..Switches::default()
        },
    ] {
        assert_eq!(
            props::no_resurrection(&schedule, only_one),
            Ok(()),
            "{only_one:?}"
        );
    }
}

/// Review (guardian, "both causes at once"): device A is away for 200 days; meanwhile B deletes and purges Y, the server compacts the
/// `purged` away and is then restored from a backup taken after the compaction. The new generation has the drop; the old cursor of A
/// is below it. The server says so in its answer, and A treats Y as an old entry (quarantine), not as something to push back.
#[test]
fn review_both_causes_at_once_the_server_says_the_old_cursor_was_stale() {
    let (a, b) = (0, 1);
    let schedule = two_devices(vec![
        Local(a, CreateChat),
        Local(a, CreateMessage(0)), // X
        Local(a, CreateMessage(0)), // Y
        SyncAll { skip: usize::MAX },
        Local(b, DeleteMessage(1)), // B deletes Y
        SyncAll { skip: a },
        Advance(31 * DAY_MS),
        Purge(b),
        SyncAll { skip: a },
        Advance(181 * DAY_MS),
        Compact,
        Backup,
        RestoreServer,
        Pull(a, PullFault::Ok),
    ]);
    let world = props::run(&schedule, Switches::default()).expect("runs clean");
    let causes: Vec<_> = world.clients[a]
        .events
        .iter()
        .filter_map(|e| {
            if let s4a_sync::client::Event::Resync(c) = e {
                Some(*c)
            } else {
                None
            }
        })
        .collect();
    assert!(
        causes.contains(&s4a_sync::client::ResyncCause::StaleAndGeneration),
        "{causes:?}"
    );
    assert_eq!(
        props::no_resurrection(&schedule, Switches::default()),
        Ok(())
    );
}

/// The same restore with no drop in the backup: a device that was only idle is not "stale", its acked records the server lost are
/// pushed back, Inbox included (the first heuristic, the device's own clock, quarantined them).
#[test]
fn review_an_idle_device_is_not_stale_after_a_restore() {
    let a = 0;
    let schedule = two_devices(vec![
        Backup,
        Pull(a, PullFault::Ok),
        Push(a, PushFault::Ok),
        Advance(181 * DAY_MS),
        RestoreServer,
        Local(a, CreateChat),
    ]);
    assert_eq!(props::convergence(&schedule, Switches::default()), Ok(()));
}

/// Review (guardian): the initiator of a chat purge shows the chat's messages in Inbox, "chat deleted", once the day is over, while the
/// `purged` it is about to send for them are still in its own outbox.
#[test]
fn review_the_initiator_does_not_show_what_it_is_about_to_purge() {
    use s4a_sync::client::FeedSlot;
    use s4a_sync::envelope::Kind;
    let schedule = Schedule {
        clients: 2,
        skews: vec![0, 0],
        unbound: vec![false, false],
        compact_outbox: false,
        page: 5,
        batch: 1,
        steps: vec![
            Local(0, CreateChat),
            Local(0, CreateMessage(1)),
            Local(0, CreateMessage(1)),
            SyncAll { skip: usize::MAX },
            Local(0, DeleteChat(1)),
            SyncAll { skip: usize::MAX },
            Advance(31 * DAY_MS),
            Purge(0),               // pull to head, enqueue purged(chat)
            Push(0, PushFault::Ok), // a batch of one: purged(chat) acked, the cascade is enqueued
            Advance(2 * DAY_MS),    // offline for two days
        ],
    };
    for (switches, shown) in [
        (Switches::default(), false),
        (
            Switches {
                show_children_with_purge_pending: true,
                ..Switches::default()
            },
            true,
        ),
    ] {
        let mut world = s4a_sync::world::World::new(&schedule, switches);
        world.run(&schedule.steps);
        let a = &world.clients[0];
        let pending = a
            .outbox
            .iter()
            .filter(|r| r.acked_seq.is_none() && r.env.header.kind == Kind::Purged)
            .count();
        assert!(pending > 0, "the cascade is in the outbox");
        let in_inbox = a
            .feed()
            .iter()
            .any(|item| matches!(item.slot, FeedSlot::Inbox(_)));
        assert_eq!(in_inbox, shown, "{switches:?}");
    }
}

/// Review (guardian): a record this device holds in quarantine comes back from the server (a device that did not know it edits it).
/// It is merged into the quarantined copy; it is not applied live, so it is never in the feed and in quarantine at once.
#[test]
fn review_a_quarantined_record_that_the_server_sends_again_stays_in_quarantine() {
    let (a, b) = (0, 1);
    let schedule = two_devices(vec![
        Local(a, CreateChat), // C
        SyncAll { skip: usize::MAX },
        Local(a, CreateMessage(1)), // M, written before B deletes C
        Advance(1000),
        Local(b, DeleteChat(1)),
        SyncAll { skip: a }, // B pushes the delete without having seen M
        Advance(31 * DAY_MS),
        Purge(b), // B pulls, has no M, and enqueues purged(C)
        Pull(a, PullFault::Ok),
        Push(a, PushFault::Ok),  // A pushes M
        Push(b, PushFault::Ok),  // the server takes purged(C)
        Local(a, RenameChat(1)), // A edits C: newer than the purge's snapshot
        Pull(a, PullFault::Ok),  // purged(C): C and its child M go to A's quarantine
        Pull(b, PullFault::Ok),  // B now has M
        Local(b, Edit(0)),       // and edits it
        Push(b, PushFault::Ok),
        Pull(a, PullFault::Ok), // M comes back to A
    ]);
    for (switches, live) in [
        (Switches::default(), false),
        (
            Switches {
                quarantine_applies_live: true,
                ..Switches::default()
            },
            true,
        ),
    ] {
        let mut world = s4a_sync::world::World::new(&schedule, switches);
        world.run(&schedule.steps);
        let a = &world.clients[a];
        let message_live = a
            .records
            .keys()
            .any(|k| k.0 == s4a_sync::envelope::Entity::Message);
        let message_held = a
            .quarantine
            .iter()
            .any(|q| q.key.0 == s4a_sync::envelope::Entity::Message);
        assert!(message_held, "M is in quarantine ({switches:?})");
        assert_eq!(message_live, live, "{switches:?}");
    }
}
