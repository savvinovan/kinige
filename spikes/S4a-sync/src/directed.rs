//! Generators of schedules that have a shape: the first half of the story is fixed, the choices inside it are random. The mixes in
//! `schedules` reach most of the protocol; these reach the corners that only a particular order of events does (a device away across
//! a compaction, a chain of edits, a chat purge with a late move, a restore after activity).

use crate::envelope::DAY_MS;
use crate::schedules::{Mix, schedule, skew, step};
use crate::world::{Op, PullFault, PushFault, Schedule, Step};
use proptest::prelude::*;

/// One device stays away while the others delete, purge and let the server compact, then it comes back (property 4).
pub fn away_schedule(clients: usize) -> impl Strategy<Value = Schedule> {
    let away = clients - 1;
    let others = Mix {
        create: 6,
        edit: 4,
        delete: 6,
        restore: 1,
        chat_ops: 1,
        purge: 0,
        long_gap: 0,
        compact: 0,
        bind: 0,
        ..Mix::GENERAL
    };
    (
        prop::collection::vec(skew(), clients),
        any::<bool>(),
        prop::collection::vec(step(clients, others), 6..24),
        prop::collection::vec(away_op(away), 0..6),
        prop::collection::vec(step(away, others), 0..16),
        (0usize..6, 181u64..420),
        prop::collection::vec(
            prop_oneof![
                Just(Step::Pull(away, PullFault::Ok)),
                Just(Step::Push(away, PushFault::Ok)),
                Just(Step::Pull(away, PullFault::Drop))
            ],
            1..6,
        ),
    )
        .prop_map(
            move |(skews, compact_outbox, prefix, away_ops, mid, (victim, gap), tail)| {
                let mut steps = prefix;
                steps.push(Step::SyncAll { skip: usize::MAX });
                steps.extend(away_ops);
                steps.extend(mid);
                // A message is deleted by a device that stayed, thirty days pass, it is purged, and the log is compacted much later.
                steps.push(Step::Local(0, Op::DeleteMessage(victim)));
                steps.push(Step::SyncAll { skip: away });
                steps.push(Step::Advance(31 * DAY_MS));
                steps.push(Step::Purge(0));
                steps.push(Step::SyncAll { skip: away });
                steps.push(Step::Advance(gap * DAY_MS));
                steps.push(Step::Compact);
                steps.extend(tail);
                Schedule {
                    clients,
                    skews,
                    unbound: vec![false; clients],
                    compact_outbox,
                    page: 2,
                    batch: 3,
                    steps,
                }
            },
        )
}

fn away_op(away: usize) -> impl Strategy<Value = Step> {
    (0usize..6, prop_oneof![Just(0u8), Just(1u8)]).prop_map(move |(i, kind)| {
        Step::Local(
            away,
            if kind == 0 {
                Op::Edit(i)
            } else {
                Op::Favorite(i)
            },
        )
    })
}

/// Only device 0 ever edits or moves a message after it exists (the others create, favorite, delete and restore): a linear history
/// per message, however the network behaves. There is nothing to conflict, so no `conflict` revision may appear (ADR-0001 appendix
/// §B items 5 and 36).
pub fn single_writer_schedule(clients: usize) -> impl Strategy<Value = Schedule> {
    schedule(
        clients,
        Mix {
            bind: 0,
            ..Mix::GENERAL
        },
        8..70,
    )
    .prop_map(|mut s| {
        for step in &mut s.steps {
            if let Step::Local(c, op) = step {
                if *c != 0 && matches!(op, Op::Edit(_) | Op::Move(..)) {
                    *op = Op::Favorite(0);
                }
            }
        }
        s
    })
}

/// Concurrent edits and moves, then 200 days pass and the server compacts everything it may: a device that comes back pulls only
/// the newest version of each chain (ADR-0001 §3.4, "a return after 180+ days"). S4a finding 2 lives here.
pub fn compacted_schedule(clients: usize) -> impl Strategy<Value = Schedule> {
    schedule(
        clients,
        Mix {
            long_gap: 0,
            compact: 0,
            ..Mix::CONFLICTS
        },
        8..50,
    )
    .prop_map(|mut s| {
        s.steps.push(Step::Advance(200 * DAY_MS));
        s.steps.push(Step::Compact);
        s
    })
}

/// The shape of S4a finding 2, with the choices random: the last device pushes an edit and goes away; another device that had not
/// seen it edits (or moves) the message several times, pushing in between; 200 days pass and the server compacts; the away device
/// comes back (the quiesce that follows). Edits and moves, 2 or 3 devices, any outbox compaction setting.
pub fn chain_schedule(clients: usize) -> impl Strategy<Value = Schedule> {
    let away = clients - 1;
    let op = (0u8..2, 0usize..4).prop_map(|(kind, chat)| {
        if kind == 0 {
            Op::Edit(0)
        } else {
            Op::Move(0, chat)
        }
    });
    (
        prop::collection::vec(skew(), clients),
        any::<bool>(),
        op.clone(),
        prop::collection::vec((op, 0u8..3), 1..5),
        prop::collection::vec(away_op(1), 0..3),
    )
        .prop_map(move |(skews, compact_outbox, away_op, chain, noise)| {
            let mut steps = vec![
                Step::Local(0, Op::CreateChat),
                Step::Local(0, Op::CreateChat),
                Step::Local(0, Op::CreateMessage(0)),
                Step::SyncAll { skip: usize::MAX },
                Step::Pull(0, PullFault::Ok),
                Step::Local(away, away_op),
                Step::Pull(away, PullFault::Ok),
                Step::Push(away, PushFault::Ok),
            ];
            steps.extend(noise);
            for (op, how) in chain {
                steps.push(Step::Local(0, op));
                match how {
                    0 => steps.push(Step::Push(0, PushFault::Ok)),
                    1 => {
                        steps.push(Step::Pull(0, PullFault::Ok));
                        steps.push(Step::Push(0, PushFault::Ok));
                    }
                    _ => {}
                }
            }
            steps.extend([
                Step::Pull(0, PullFault::Ok),
                Step::Push(0, PushFault::Ok),
                Step::Advance(200 * DAY_MS),
                Step::Compact,
            ]);
            Schedule {
                clients,
                skews,
                unbound: vec![false; clients],
                compact_outbox,
                page: 5,
                batch: 5,
                steps,
            }
        })
}

/// Chats that are deleted, left in the trash for a month by one device, purged, while another device moves messages in and out, restores
/// the chat or writes into it: the cascade of §3.5 and §3.6 with its races (property 7 and, through convergence, property 1).
pub fn chat_purge_schedule(clients: usize) -> impl Strategy<Value = Schedule> {
    let busy = Mix {
        purge: 0,
        long_gap: 0,
        compact: 0,
        bind: 0,
        create: 10,
        mv: 6,
        chat_ops: 4,
        ..Mix::CHATS
    };
    (
        prop::collection::vec(skew(), clients),
        any::<bool>(),
        prop::collection::vec(step(clients, busy), 8..30),
        prop::collection::vec(0usize..6, 1..3),
        prop::collection::vec(step(clients, busy), 0..16),
        31u64..60,
        prop::collection::vec(step(clients, busy), 0..16),
    )
        .prop_map(
            move |(skews, compact_outbox, prefix, deletes, middle, days, tail)| {
                let mut steps = prefix;
                steps.push(Step::SyncAll { skip: usize::MAX });
                steps.extend(
                    deletes
                        .into_iter()
                        .map(|i| Step::Local(0, Op::DeleteChat(i))),
                );
                steps.extend(middle);
                steps.push(Step::Advance(days * DAY_MS));
                steps.extend((0..clients).map(Step::Purge));
                steps.extend(tail);
                Schedule {
                    clients,
                    skews,
                    unbound: vec![false; clients],
                    compact_outbox,
                    page: 2,
                    batch: 3,
                    steps,
                }
            },
        )
}

/// Found by the random restore schedules in round 2. Device 0 holds M in the trash and learns that the server was restored; before it
/// has pulled a page of its resync, the server compacts and drops `purged(M)` and M. A pull from 0 is never stale, so the resync would
/// take M's absence for "the restore lost it" and push it back. The pages of the resync carry the epoch: one greater than the one in
/// the `GenerationChanged` answer turns the cause into "both", and M goes to quarantine.
pub fn compaction_while_a_restore_resync_pulls() -> Schedule {
    let (a, b) = (0, 1);
    Schedule {
        clients: 2,
        skews: vec![0, 0],
        unbound: vec![false, false],
        compact_outbox: false,
        page: 5,
        batch: 5,
        steps: vec![
            Step::Local(b, Op::CreateMessage(0)),
            Step::Local(b, Op::DeleteMessage(0)),
            Step::SyncAll { skip: usize::MAX },
            Step::Pull(a, PullFault::Ok), // A holds M in the trash
            Step::Advance(31 * DAY_MS),
            Step::Purge(b),
            Step::Push(b, PushFault::Ok), // purged(M) is on the server
            Step::Backup,
            Step::Advance(181 * DAY_MS),
            Step::RestoreServer,
            Step::Pull(a, PullFault::Ok), // GenerationChanged, no page yet
            Step::Compact,                // drops purged(M) and M, epoch 1 of the new generation
            Step::Pull(a, PullFault::Ok), // the first page of the resync, from 0: M is simply absent
        ],
    }
}

/// Found by the random restore schedules in round 2 (shrunk). Device 1 deletes and edits M (one upsert, the outbox is compacted),
/// pushes it and purges it locally; device 0 favorites M meanwhile. The server is restored, and the purge, rejected by the server
/// because of the newer favorite, lifts device 1's tombstone: M is recreated from device 0's envelope, which is older than device
/// 1's own writes. Those come back in the pull as device 1's own `op_id` and were taken for an ack only: device 1 kept the old M.
pub fn own_writes_after_a_lift() -> Schedule {
    let (a, b) = (0, 1);
    Schedule {
        clients: 3,
        skews: vec![0, 0, 0],
        unbound: vec![false, false, false],
        compact_outbox: true,
        page: 1,
        batch: 3,
        steps: vec![
            Step::Local(b, Op::CreateMessage(0)), // M
            Step::Local(2, Op::CreateChat),
            Step::Local(a, Op::CreateChat),
            Step::Pull(b, PullFault::Ok),
            Step::Push(b, PushFault::Ok),
            Step::Local(a, Op::CreateChat),
            Step::Local(a, Op::CreateChat),
            Step::Purge(a), // A pulls M
            Step::Local(a, Op::CreateChat),
            Step::Local(b, Op::DeleteMessage(0)),
            Step::Advance(181 * DAY_MS),
            Step::Local(b, Op::Edit(0)),
            Step::Purge(b), // B enqueues purged(M)
            Step::Backup,
            Step::Local(a, Op::Favorite(0)),
            Step::Push(b, PushFault::Ok),
            Step::RestoreServer,
        ],
    }
}

/// What the restore generator chooses besides the delays: how many changes device 0 makes and syncs after the backup (so that its
/// cursor goes above the backup's head), whether it is away across the first restore, and how many restores there are.
#[derive(Debug, Clone, Copy)]
struct RestoreShape {
    /// Favorite toggles by device 0, each synced, between the backup and the first restore: its cursor ends far above the backup's head.
    activity: usize,
    /// Device 0 sleeps through the first restore (the guardian's second blocker) instead of resyncing at once.
    away_at_restore: bool,
    /// 0: one restore. 1: a second restore from the same backup (the new generation is abandoned, the lineage branches). 2: a second
    /// restore from a backup taken in the first new generation, after more activity (two restores in a row).
    restores: u8,
}

/// The shape of the round 1 and round 2 blockers (drop epochs, generations and a restore), with the choices random: device 0 syncs, a
/// backup is taken, device 0 keeps working and syncing, device 1 deletes and purges one message and the server compacts, the server is
/// restored (once or twice, see [`RestoreShape`]), then device 0 goes away while device 1 deletes and purges another message and the
/// server compacts again; device 0 comes back. A device 0 that holds a cursor or an epoch from before a restore must still be told
/// that its cursor is stale, whatever `seq`s and epochs the new generation has reused.
pub fn restore_epoch_schedule() -> impl Strategy<Value = Schedule> {
    (
        prop::collection::vec(skew(), 2),
        any::<bool>(),
        (0usize..4, 0usize..4),
        (1u64..40, 181u64..420, 181u64..420),
        (0usize..14, any::<bool>(), 0u8..3),
    )
        .prop_map(
            |(
                skews,
                compact_outbox,
                (first, second),
                (spare, gap_a, gap_b),
                (activity, away_at_restore, restores),
            )| {
                let shape = RestoreShape {
                    activity,
                    away_at_restore,
                    restores,
                };
                let (a, b) = (0, 1);
                let both = usize::MAX;
                let mut steps = vec![
                    Step::Local(a, Op::CreateChat),
                    Step::Local(a, Op::CreateMessage(1)),
                    Step::Local(a, Op::CreateMessage(1)),
                    Step::Local(a, Op::CreateMessage(1)),
                    Step::SyncAll { skip: both },
                    Step::Backup,
                ];
                for i in 0..shape.activity {
                    steps.push(Step::Local(a, Op::Favorite(i)));
                    steps.push(Step::SyncAll { skip: both });
                }
                steps.extend([
                    Step::Local(b, Op::DeleteMessage(first)),
                    Step::SyncAll { skip: both },
                    Step::Advance((30 + spare) * DAY_MS),
                    Step::Purge(b),
                    Step::SyncAll { skip: both },
                    Step::Advance(gap_a * DAY_MS),
                    Step::Compact,
                    Step::SyncAll { skip: both },
                    Step::RestoreServer,
                    Step::SyncAll {
                        skip: if shape.away_at_restore { a } else { both },
                    },
                ]);
                if shape.restores == 2 {
                    // A backup of the first new generation, more work, then a restore from it.
                    steps.push(Step::Backup);
                    for i in 0..shape.activity {
                        steps.push(Step::Local(b, Op::Favorite(i)));
                        steps.push(Step::SyncAll { skip: a });
                    }
                }
                if shape.restores >= 1 {
                    steps.push(Step::RestoreServer);
                    steps.push(Step::SyncAll { skip: a });
                }
                steps.extend([
                    Step::Local(b, Op::DeleteMessage(second)),
                    Step::SyncAll { skip: a },
                    Step::Advance((30 + spare) * DAY_MS),
                    Step::Purge(b),
                    Step::SyncAll { skip: a },
                    Step::Advance(gap_b * DAY_MS),
                    Step::Compact,
                    Step::Pull(a, PullFault::Ok),
                ]);
                Schedule {
                    clients: 2,
                    skews,
                    unbound: vec![false, false],
                    compact_outbox,
                    page: 5,
                    batch: 5,
                    steps,
                }
            },
        )
}
