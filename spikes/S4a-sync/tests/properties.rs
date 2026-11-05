//! The eight properties of PRD §10.2 (criteria S4a) over 2 and 3 clients. Case counts and seeds:
//! `S4A_CASES` (default 10 000, 2 clients), `S4A_CASES3` (default 2 000, 3 clients), `S4A_SEED` (default 1).
//! CI runs a smaller fixed count with the same fixed seeds.

use s4a_sync::props::{self, check};
use s4a_sync::schedules::{Mix, away_schedule, schedule};
use s4a_sync::switches::Switches;
use s4a_sync::world::Schedule;

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn cases2() -> u32 {
    u32::try_from(env_u64("S4A_CASES", 10_000)).unwrap_or(10_000)
}

fn cases3() -> u32 {
    u32::try_from(env_u64("S4A_CASES3", 2_000)).unwrap_or(2_000)
}

fn seed() -> u64 {
    env_u64("S4A_SEED", 1)
}

type Property = fn(&Schedule, Switches) -> Result<(), String>;

fn run_both(name: &str, mix: Mix, property: Property) {
    let on = Switches::default();
    for (clients, cases, steps) in [(2usize, cases2(), 8..70usize), (3, cases3(), 8..90)] {
        let result = check(
            &format!("{name}, {clients} clients"),
            seed(),
            cases,
            schedule(clients, mix, steps),
            |s| property(s, on),
        );
        match result {
            Ok(ran) => println!(
                "{name}, {clients} clients: {ran} cases passed (seed {})",
                seed()
            ),
            Err(report) => panic!("{report}"),
        }
    }
}

#[test]
fn p1_convergence() {
    run_both("1 convergence", Mix::GENERAL, props::convergence);
}

#[test]
fn p2_concurrent_edit_keeps_both_versions() {
    run_both("2 no version lost", Mix::CONFLICTS, props::no_version_lost);
}

#[test]
fn p3_restore_beats_older_delete() {
    run_both(
        "3 restore beats delete",
        Mix::TRASH,
        props::restore_beats_delete,
    );
}

#[test]
fn p4_device_back_after_200_days_does_not_resurrect() {
    let on = Switches::default();
    for (clients, cases) in [(2usize, cases2()), (3, cases3())] {
        match check(
            &format!("4 no resurrection, {clients} clients"),
            seed(),
            cases,
            away_schedule(clients),
            |s| props::no_resurrection(s, on),
        ) {
            Ok(ran) => println!(
                "4 no resurrection, {clients} clients: {ran} cases passed (seed {})",
                seed()
            ),
            Err(report) => panic!("{report}"),
        }
    }
}

#[test]
fn p5_repeated_push_is_idempotent() {
    run_both(
        "5 idempotent push",
        Mix {
            faults: 8,
            ..Mix::GENERAL
        },
        props::push_idempotent,
    );
}

#[test]
fn p6_edits_of_different_groups_are_both_kept() {
    run_both("6 groups both kept", Mix::GENERAL, props::groups_both_kept);
}

#[test]
fn p7_no_orphan_is_shown() {
    run_both("7 no orphan", Mix::CHATS, props::no_orphans);
}

#[test]
fn p8_server_restore_loses_nothing_acked() {
    run_both("8 server restore", Mix::RESTORE, props::server_restore);
}

#[test]
fn p9_no_false_conflicts_when_one_device_writes() {
    let on = Switches::default();
    for (clients, cases) in [(2usize, cases2()), (3, cases3())] {
        match check(
            &format!("9 no false conflicts, {clients} clients"),
            seed(),
            cases,
            s4a_sync::schedules::single_writer_schedule(clients),
            |s| props::no_false_conflicts(s, on),
        ) {
            Ok(ran) => println!(
                "9 no false conflicts, {clients} clients: {ran} cases passed (seed {})",
                seed()
            ),
            Err(report) => panic!("{report}"),
        }
    }
}

#[test]
fn p2_concurrent_edit_keeps_both_versions_after_compaction() {
    // The shape of S4a finding 2 (see tests/scenarios.rs), and random schedules followed by 200 days and a compaction.
    let on = Switches::default();
    for (clients, cases) in [(2usize, cases2()), (3, cases3())] {
        let chain = check(
            &format!("2 no version lost, compacted chain, {clients} clients"),
            seed(),
            cases,
            s4a_sync::schedules::chain_schedule(clients),
            |s| props::no_version_lost(s, on),
        );
        let random = check(
            &format!("2 no version lost, random then compacted, {clients} clients"),
            seed(),
            cases,
            s4a_sync::schedules::compacted_schedule(clients),
            |s| props::no_version_lost(s, on),
        );
        for result in [chain, random] {
            match result {
                Ok(ran) => println!(
                    "2 (compaction), {clients} clients: {ran} cases passed (seed {})",
                    seed()
                ),
                Err(report) => panic!("{report}"),
            }
        }
    }
}

#[test]
fn p7_and_p1_chat_purge_cascade() {
    let on = Switches::default();
    for (clients, cases) in [(2usize, cases2()), (3, cases3())] {
        let orphans = check(
            &format!("7 no orphan, chat purges, {clients} clients"),
            seed(),
            cases,
            s4a_sync::schedules::chat_purge_schedule(clients),
            |s| props::no_orphans(s, on),
        );
        let converge = check(
            &format!("1 convergence, chat purges, {clients} clients"),
            seed(),
            cases,
            s4a_sync::schedules::chat_purge_schedule(clients),
            |s| props::convergence(s, on),
        );
        for result in [orphans, converge] {
            match result {
                Ok(ran) => println!(
                    "1 and 7 (chat purges), {clients} clients: {ran} cases passed (seed {})",
                    seed()
                ),
                Err(report) => panic!("{report}"),
            }
        }
    }
}
