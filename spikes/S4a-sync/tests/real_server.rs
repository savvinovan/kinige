//! The S4a properties with every device talking to the real `kinige-server` on Postgres through its router (S4c-1, KINIG-98, feature
//! `real-server`): the protocol, the bytes (`real-envelope`) and the server together. Needs `KINIGE_TEST_DATABASE_URL`.
//!
//! Counts are small and fixed (each case is hundreds of HTTP requests, each a database transaction): `S4A_REAL_CASES` (default 30, 2
//! devices), `S4A_REAL_CASES3` (default 8, 3 devices), seed `S4A_SEED` (default 1). Property 8 (a restore of the server) and the restore
//! generators are not run: restore is S4c-2, and the real server has no backup yet. The default build and the other test files keep the
//! in-memory server.
#![cfg(feature = "real-server")]

use s4a_sync::backend::with_backend;
use s4a_sync::directed::{
    away_schedule, chain_schedule, chat_purge_schedule, compacted_schedule, single_writer_schedule,
};
use s4a_sync::http_server::RealServerHost;
use s4a_sync::props::{self, check};
use s4a_sync::schedules::{Mix, schedule};
use s4a_sync::switches::Switches;
use s4a_sync::world::Schedule;

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn cases(clients: usize) -> u32 {
    if clients == 2 {
        u32::try_from(env_u64("S4A_REAL_CASES", 30)).unwrap_or(30)
    } else {
        u32::try_from(env_u64("S4A_REAL_CASES3", 8)).unwrap_or(8)
    }
}

fn seed() -> u64 {
    env_u64("S4A_SEED", 1)
}

type Property = fn(&Schedule, Switches) -> Result<(), String>;

/// Runs `property` on schedules from `strategy` against the real server and fails with the report if one breaks it.
fn on_real_server<S>(
    name: &str,
    clients: usize,
    strategy: S,
    property: Property,
    switches: Switches,
) where
    S: proptest::strategy::Strategy<Value = Schedule>,
{
    let host = RealServerHost::connect().expect("a database on KINIGE_TEST_DATABASE_URL");
    let factory = {
        let host = host.clone();
        move || host.server()
    };
    let label = format!("{name}, {clients} clients, real server");
    let result = with_backend(factory, || {
        check(&label, seed(), cases(clients), strategy, |s| {
            property(s, switches)
        })
    });
    match result {
        Ok(ran) => println!("{label}: {ran} cases passed (seed {})", seed()),
        Err(report) => panic!("{report}"),
    }
}

fn run_both<S, F>(name: &str, make: F, property: Property)
where
    S: proptest::strategy::Strategy<Value = Schedule>,
    F: Fn(usize) -> S,
{
    for clients in [2usize, 3] {
        on_real_server(name, clients, make(clients), property, Switches::default());
    }
}

#[test]
fn p1_convergence_and_a_fresh_device_rebuilds_the_state_from_the_real_log() {
    run_both(
        "1 convergence",
        |n| schedule(n, Mix::GENERAL, 8..40),
        props::convergence,
    );
}

#[test]
fn p2_concurrent_edits_keep_both_versions_also_after_compaction_in_sql() {
    run_both(
        "2 no version lost",
        |n| schedule(n, Mix::CONFLICTS, 8..40),
        props::no_version_lost,
    );
    run_both("2 compacted chain", chain_schedule, props::no_version_lost);
    run_both(
        "2 random then compacted",
        compacted_schedule,
        props::no_version_lost,
    );
}

#[test]
fn p3_a_restore_beats_the_older_delete() {
    run_both(
        "3 restore beats delete",
        |n| schedule(n, Mix::TRASH, 8..40),
        props::restore_beats_delete,
    );
}

#[test]
fn p4_a_device_back_after_200_days_does_not_resurrect() {
    run_both("4 no resurrection", away_schedule, props::no_resurrection);
}

#[test]
fn p5_a_repeated_push_is_idempotent_in_postgres() {
    run_both(
        "5 idempotent push",
        |n| {
            schedule(
                n,
                Mix {
                    faults: 8,
                    ..Mix::GENERAL
                },
                8..40,
            )
        },
        props::push_idempotent,
    );
}

#[test]
fn p6_edits_of_different_groups_are_both_kept() {
    run_both(
        "6 groups both kept",
        |n| schedule(n, Mix::GENERAL, 8..40),
        props::groups_both_kept,
    );
}

#[test]
fn p7_no_orphan_is_shown_also_through_chat_purges() {
    run_both(
        "7 no orphan",
        |n| schedule(n, Mix::CHATS, 8..40),
        props::no_orphans,
    );
    run_both(
        "7 no orphan, chat purges",
        chat_purge_schedule,
        props::no_orphans,
    );
    run_both(
        "1 convergence, chat purges",
        chat_purge_schedule,
        props::convergence,
    );
}

#[test]
fn p9_no_false_conflicts_when_one_device_writes() {
    run_both(
        "9 no false conflicts",
        single_writer_schedule,
        props::no_false_conflicts,
    );
}

/// The suite has teeth against the real server too: a breakage in the client's merge makes a property fail here as it does in memory.
#[test]
fn control_a_client_that_lets_the_whole_record_win_fails_against_the_real_server() {
    let host = RealServerHost::connect().expect("a database on KINIGE_TEST_DATABASE_URL");
    let factory = {
        let host = host.clone();
        move || host.server()
    };
    let breakage = Switches {
        whole_record_lww: true,
        ..Switches::default()
    };
    let result = with_backend(factory, || {
        check(
            "6 groups both kept, whole-record last-writer-wins, real server",
            seed(),
            200,
            schedule(2, Mix::GENERAL, 8..40),
            |s| props::groups_both_kept(s, breakage),
        )
    });
    match result {
        Ok(ran) => panic!(
            "the control passed on {ran} cases against the real server: the suite would not catch the breakage"
        ),
        Err(report) => println!("{report}"),
    }
}
