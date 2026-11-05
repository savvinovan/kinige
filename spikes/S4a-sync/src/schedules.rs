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
        // Months pass and the server compacts, so a restore meets drops (S4a review: with both at 0 the drop epoch was never
        // tested against a generation change).
        long_gap: 2,
        compact: 4,
        bind: 0,
        purge: 4,
        delete: 6,
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

/// The skew of a device clock: mostly none, sometimes seconds, sometimes minutes, rarely hours (never beyond `SKEW_MAX`).
pub(crate) fn skew() -> impl Strategy<Value = i64> {
    prop_oneof![
        6 => Just(0i64),
        4 => -5_000i64..5_000,
        2 => -900_000i64..900_000,
        1 => -10_800_000i64..10_800_000,
    ]
}

pub(crate) fn step(clients: usize, mix: Mix) -> impl Strategy<Value = Step> {
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
