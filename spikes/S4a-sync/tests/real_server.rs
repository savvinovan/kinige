//! The S4a properties with every device talking to the real `kinige-server` on Postgres through its router (S4c-1, KINIG-98, feature
//! `real-server`): the protocol, the bytes (`real-envelope`) and the server together. Needs `KINIGE_TEST_DATABASE_URL`.
//!
//! Counts are small and fixed (each case is hundreds of HTTP requests, each a database transaction): `S4A_REAL_CASES` (default 30, 2
//! devices), `S4A_REAL_CASES3` (default 8, 3 devices), seed `S4A_SEED` (default 1). Since S4c-2 property 8 runs here too, with the restore
//! mixes and the directed restore scenarios: every backup is a real dump and every restore loads it into a clean database. The default
//! build and the other test files keep the in-memory server.
#![cfg(feature = "real-server")]

use s4a_sync::backend::with_backend;
use s4a_sync::client::ResyncCause;
use s4a_sync::directed::{
    away_schedule, branch_of_a_branch_schedule, chain_schedule, chat_purge_schedule,
    compacted_schedule, compaction_while_a_restore_resync_pulls, older_then_newer_backup_schedule,
    own_writes_after_a_lift, restore_epoch_schedule, single_writer_schedule,
};
use s4a_sync::envelope::Entity;
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

/// Property 8 and the restore mixes (S4c-2): every restore is a real dump of the server, taken in a snapshot with the blob store and the
/// manifest, loaded into a clean database by the operator's own code path, and every device talks to the restored installation.
#[test]
fn p8_a_restored_server_loses_nothing_acked_and_resurrects_nothing() {
    run_both(
        "8 server restore (Mix::RESTORE)",
        |n| schedule(n, Mix::RESTORE, 8..40),
        props::server_restore,
    );
}

#[test]
fn p8_drop_epochs_survive_a_real_restore() {
    on_real_server(
        "8 server restore, drop epochs",
        2,
        restore_epoch_schedule(),
        props::server_restore,
        Switches::default(),
    );
}

/// The directed restore scenarios of the S4a review, against the real server.
#[test]
fn p8_directed_restore_scenarios_pass_on_a_real_dump() {
    for (name, schedule) in [
        (
            "a compaction while the resync of a restore pulls",
            compaction_while_a_restore_resync_pulls(),
        ),
        (
            "own writes after a lifted tombstone",
            own_writes_after_a_lift(),
        ),
    ] {
        let host = RealServerHost::connect().expect("a database on KINIGE_TEST_DATABASE_URL");
        let result = with_backend(
            {
                let host = host.clone();
                move || host.server()
            },
            || props::server_restore(&schedule, Switches::default()),
        );
        assert_eq!(result, Ok(()), "{name}");
    }
}

/// The branch-cut rule on a real dump (ADR-0017, the `review_round_3_*` scenario of `scenarios.rs`): the server is restored from an older
/// backup, device 0 enters that branch while the purging device is gone, then the server is restored from a newer backup that already
/// holds the drop. Device 0 is told it missed the drop, and the message goes to its quarantine instead of back to the server.
#[test]
fn p8_a_branch_cut_from_an_older_backup_than_the_one_restored_now_on_a_real_dump() {
    let host = RealServerHost::connect().expect("a database on KINIGE_TEST_DATABASE_URL");
    let schedule = older_then_newer_backup_schedule();
    let world = with_backend(
        {
            let host = host.clone();
            move || host.server()
        },
        || {
            let mut world = s4a_sync::world::World::new(&schedule, Switches::default());
            world.run(&schedule.steps);
            world
        },
    );
    let a = 0;
    let x = world.clients[1]
        .tombstones
        .keys()
        .find(|k| k.0 == Entity::Message)
        .cloned()
        .expect("device 1 holds the tombstone of the purged message");
    let causes: Vec<ResyncCause> = world.clients[a]
        .events
        .iter()
        .filter_map(|e| match e {
            s4a_sync::client::Event::Resync(cause) => Some(*cause),
            _ => None,
        })
        .collect();
    assert_eq!(
        causes.last(),
        Some(&ResyncCause::StaleAndGeneration),
        "{causes:?}"
    );
    assert!(!world.clients[a].records.contains_key(&x));
    assert!(world.clients[a].quarantine.iter().any(|q| q.key == x));
    assert!(
        !world
            .server
            .all_envelopes()
            .iter()
            .any(|e| e.header.key() == x),
        "nothing of it went back to the real server"
    );
    assert_eq!(world.server.drops_count(), 1);
}

/// The fork rule on real dumps (round 1 of the review of S4c-2, the `review_s4c2_round_1_*` scenario of `scenarios.rs`): a branch that was
/// itself restored from a backup of a branch. Device 0 holds the lineage `[G0, G1, G2]`; the server is restored from a backup that holds
/// the drop and its lineage is `[G0, G3]`. The device's own generation's cut time is after the drop, the cut time of its fork is before
/// it: the real server must say "stale" and the message goes to quarantine.
#[test]
fn p8_a_branch_of_a_branch_is_placed_by_its_fork_on_a_real_dump() {
    let host = RealServerHost::connect().expect("a database on KINIGE_TEST_DATABASE_URL");
    let schedule = branch_of_a_branch_schedule();
    let world = with_backend(
        {
            let host = host.clone();
            move || host.server()
        },
        || {
            let mut world = s4a_sync::world::World::new(&schedule, Switches::default());
            world.run(&schedule.steps);
            world
        },
    );
    let a = 0;
    let x = world.clients[1]
        .tombstones
        .keys()
        .find(|k| k.0 == Entity::Message)
        .cloned()
        .expect("device 1 holds the tombstone of the purged message");
    let causes: Vec<ResyncCause> = world.clients[a]
        .events
        .iter()
        .filter_map(|e| match e {
            s4a_sync::client::Event::Resync(cause) => Some(*cause),
            _ => None,
        })
        .collect();
    assert_eq!(
        world.clients[a].lineage.len(),
        2,
        "the device ends holding the lineage of the last answer, [G0, G3]"
    );
    assert_eq!(
        causes.last(),
        Some(&ResyncCause::StaleAndGeneration),
        "{causes:?}"
    );
    assert!(!world.clients[a].records.contains_key(&x));
    assert!(world.clients[a].quarantine.iter().any(|q| q.key == x));
    assert!(
        !world
            .server
            .all_envelopes()
            .iter()
            .any(|e| e.header.key() == x),
        "nothing of it went back to the real server"
    );
    assert_eq!(world.server.drops_count(), 1);
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

/// The restore properties have teeth against the real server too: a client that keeps its drop epoch across a generation change (the
/// round 1 blocker of the S4a review) resurrects a purged message after a real restore, and property 8 says so.
#[test]
fn control_8_a_client_that_keeps_its_epoch_across_a_real_restore_fails() {
    let host = RealServerHost::connect().expect("a database on KINIGE_TEST_DATABASE_URL");
    let breakage = Switches {
        epoch_survives_generation: true,
        ..Switches::default()
    };
    let result = with_backend(
        {
            let host = host.clone();
            move || host.server()
        },
        || {
            check(
                "8 server restore, epoch kept across generations, real server",
                seed(),
                600,
                restore_epoch_schedule(),
                |s| props::server_restore(s, breakage),
            )
        },
    );
    match result {
        Ok(ran) => panic!(
            "the control passed on {ran} cases against the real server: the restore property would not catch the breakage"
        ),
        Err(report) => println!("{report}"),
    }
}
