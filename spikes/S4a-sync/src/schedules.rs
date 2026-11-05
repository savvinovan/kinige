//! Generators of schedules (proptest strategies). A schedule is plain data, so a failing one shrinks to a short list of steps and
//! prints as Rust. The mixes decide what a property stresses; the device clocks, the faults and the sizes are common.

use crate::envelope::DAY_MS;
use crate::world::{Op, PullFault, PushFault, Schedule, Step};
use proptest::prelude::*;
use proptest::strategy::Union;

/// Relative weights of the kinds of steps. Zero removes a kind.
#[derive(Debug, Clone, Copy)]
pub struct Mix {
    pub create: u32,
    pub edit: u32,
    pub flag: u32,
    pub mv: u32,
    pub delete: u32,
    pub restore: u32,
    pub chat_ops: u32,
    pub sync: u32,
    pub purge: u32,
    pub restart: u32,
    pub advance: u32,
    /// Jumps of 181 to 420 days: a device that was away comes back to a server that has compacted.
    pub long_gap: u32,
    pub compact: u32,
    pub bind: u32,
    pub backup: u32,
    pub restore_server: u32,
    /// Weight of faults against healthy delivery.
    pub faults: u32,
}

impl Mix {
    /// Everything a person and a network can do, except the server restore.
    pub const GENERAL: Mix = Mix {
        create: 8,
        edit: 8,
        flag: 3,
        mv: 3,
        delete: 4,
        restore: 3,
        chat_ops: 3,
        sync: 24,
        purge: 2,
        restart: 1,
        advance: 8,
        long_gap: 1,
        compact: 1,
        bind: 1,
        backup: 0,
        restore_server: 0,
        faults: 4,
    };
    /// Concurrent edits and moves of the same few records, nothing is ever deleted or purged. Months pass and the server compacts, so
    /// the intermediate version of a chain is sometimes gone by the time the other side's version arrives.
    pub const CONFLICTS: Mix = Mix {
        delete: 0,
        restore: 0,
        chat_ops: 1,
        purge: 0,
        long_gap: 2,
        compact: 4,
        bind: 0,
        edit: 14,
        mv: 8,
        faults: 5,
        ..Mix::GENERAL
    };
    /// Delete and restore across skewed clocks.
    pub const TRASH: Mix = Mix {
        edit: 2,
        flag: 2,
        mv: 1,
        delete: 8,
        restore: 8,
        chat_ops: 2,
        purge: 0,
        long_gap: 0,
        compact: 0,
        bind: 0,
        faults: 3,
        ..Mix::GENERAL
    };
    /// Chats come, go, are purged; messages move between them.
    pub const CHATS: Mix = Mix {
        create: 8,
        mv: 8,
        delete: 5,
        restore: 2,
        chat_ops: 10,
        purge: 5,
        advance: 10,
        long_gap: 0,
        compact: 1,
        bind: 1,
        ..Mix::GENERAL
    };
    /// Backups and a restore of the server.
    pub const RESTORE: Mix = Mix {
        backup: 3,
        restore_server: 2,
        long_gap: 0,
        compact: 0,
        bind: 0,
        purge: 2,
        ..Mix::GENERAL
    };
}

pub fn schedule(
    clients: usize,
    mix: Mix,
    steps: std::ops::Range<usize>,
) -> impl Strategy<Value = Schedule> {
    (
        prop::collection::vec(skew(), clients),
        prop::collection::vec(any::<bool>(), clients),
        any::<bool>(),
        1usize..4,
        1usize..5,
        prop::collection::vec(step(clients, mix), steps),
    )
        .prop_map(
            move |(skews, unbound, compact_outbox, page, batch, steps)| Schedule {
                clients,
                skews,
                // Starting without an account is the exception, and only for mixes that bind.
                unbound: unbound
                    .iter()
                    .enumerate()
                    .map(|(i, b)| mix.bind > 0 && *b && i % 2 == 1)
                    .collect(),
                compact_outbox,
                page,
                batch,
                steps,
            },
        )
}

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

/// The skew of a device clock: mostly none, sometimes seconds, sometimes minutes, rarely hours (never beyond `SKEW_MAX`).
fn skew() -> impl Strategy<Value = i64> {
    prop_oneof![
        6 => Just(0i64),
        4 => -5_000i64..5_000,
        2 => -900_000i64..900_000,
        1 => -10_800_000i64..10_800_000,
    ]
}

fn step(clients: usize, mix: Mix) -> impl Strategy<Value = Step> {
    let c = 0..clients;
    let idx = 0usize..6;
    let mut kinds: Vec<(u32, BoxedStrategy<Step>)> = Vec::new();
    let mut add = |weight: u32, strategy: BoxedStrategy<Step>| {
        if weight > 0 {
            kinds.push((weight, strategy));
        }
    };
    add(
        mix.create,
        (
            c.clone(),
            prop_oneof![
                Just(Op::CreateChat),
                idx.clone().prop_map(Op::CreateMessage),
                idx.clone().prop_map(Op::CreateMessage)
            ],
        )
            .prop_map(|(c, op)| Step::Local(c, op))
            .boxed(),
    );
    add(
        mix.edit,
        (c.clone(), idx.clone())
            .prop_map(|(c, i)| Step::Local(c, Op::Edit(i)))
            .boxed(),
    );
    add(
        mix.flag,
        (c.clone(), idx.clone())
            .prop_map(|(c, i)| Step::Local(c, Op::Favorite(i)))
            .boxed(),
    );
    add(
        mix.mv,
        (c.clone(), idx.clone(), idx.clone())
            .prop_map(|(c, i, j)| Step::Local(c, Op::Move(i, j)))
            .boxed(),
    );
    add(
        mix.delete,
        (c.clone(), idx.clone())
            .prop_map(|(c, i)| Step::Local(c, Op::DeleteMessage(i)))
            .boxed(),
    );
    add(
        mix.restore,
        (c.clone(), idx.clone())
            .prop_map(|(c, i)| Step::Local(c, Op::RestoreMessage(i)))
            .boxed(),
    );
    add(
        mix.chat_ops,
        (c.clone(), idx.clone(), 0u8..3)
            .prop_map(|(c, i, kind)| {
                Step::Local(
                    c,
                    match kind {
                        0 => Op::DeleteChat(i),
                        1 => Op::RestoreChat(i),
                        _ => Op::RenameChat(i),
                    },
                )
            })
            .boxed(),
    );
    add(
        mix.sync,
        (c.clone(), push_fault(mix.faults))
            .prop_map(|(c, f)| Step::Push(c, f))
            .boxed(),
    );
    add(
        mix.sync,
        (c.clone(), pull_fault(mix.faults))
            .prop_map(|(c, f)| Step::Pull(c, f))
            .boxed(),
    );
    add(mix.purge, c.clone().prop_map(Step::Purge).boxed());
    add(mix.restart, c.clone().prop_map(Step::Restart).boxed());
    add(mix.advance, advance().prop_map(Step::Advance).boxed());
    add(
        mix.long_gap,
        (181u64..420)
            .prop_map(|d| Step::Advance(d * DAY_MS))
            .boxed(),
    );
    add(mix.compact, Just(Step::Compact).boxed());
    add(mix.bind, c.prop_map(Step::Bind).boxed());
    add(mix.backup, Just(Step::Backup).boxed());
    add(mix.restore_server, Just(Step::RestoreServer).boxed());
    Union::new_weighted(kinds)
}

fn push_fault(faults: u32) -> impl Strategy<Value = PushFault> {
    prop_oneof![
        8 => Just(PushFault::Ok),
        faults => Just(PushFault::DropRequest),
        faults => Just(PushFault::DropResponse),
        faults => Just(PushFault::Duplicate),
        faults => (0u8..3).prop_map(PushFault::CutAfter),
        faults => Just(PushFault::Reorder),
    ]
}

fn pull_fault(faults: u32) -> impl Strategy<Value = PullFault> {
    prop_oneof![
        8 => Just(PullFault::Ok),
        faults => Just(PullFault::Drop),
        faults => Just(PullFault::Duplicate),
        faults => Just(PullFault::Replay),
        faults => (0u8..5).prop_map(PullFault::Shuffle),
    ]
}

/// How much virtual time passes: often none (so stamps tie), then seconds, hours, days up to the purge age.
fn advance() -> impl Strategy<Value = u64> {
    prop_oneof![
        6 => 0u64..3,
        3 => 1_000u64..120_000,
        1 => 3_600_000u64..86_400_000,
        2 => (1u64..45).prop_map(|d| d * DAY_MS),
        // Old enough that something in the trash is due for purging.
        3 => (31u64..45).prop_map(|d| d * DAY_MS),
    ]
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
