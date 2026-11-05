//! The eight properties of PRD §10.2 (criteria S4a) as functions of a schedule, and the runner that applies them to generated
//! schedules with a fixed seed. A failure prints the seed and the shrunk schedule; running again with the same seed and case count
//! gives the same schedule.

use crate::client::Event;
use crate::client::purge::deleted_at;
use crate::envelope::{DAY_MS, Entity};
use crate::oracle;
use crate::switches::Switches;
use crate::world::{Schedule, Step, World};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngAlgorithm, RngSeed, TestError, TestRunner};
use std::sync::atomic::{AtomicU32, Ordering};

/// Runs a schedule to the end and lets the network quiesce. A clock that went back, a device that showed an orphan, an `op_conflict`
/// or an envelope that failed to open is already a failure.
pub fn run(schedule: &Schedule, switches: Switches) -> Result<World, String> {
    let mut world = World::new(schedule, switches);
    world.run(&schedule.steps);
    world.quiesce()?;
    if let Some(first) = world.violations.first() {
        return Err(first.clone());
    }
    for client in &world.clients {
        if let Some(event) = client
            .events
            .iter()
            .find(|e| matches!(e, Event::OpConflict | Event::BadEnvelope))
        {
            return Err(format!("device {}: {event:?}", client.idx));
        }
    }
    oracle::quarantine_is_exclusive(&world)?;
    Ok(world)
}

/// 1. Two and three clients converge to the same record state after quiescence, under any schedule.
pub fn convergence(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let mut world = run(schedule, switches)?;
    oracle::converged(&world)?;
    oracle::server_log_rebuilds_state(&mut world)
}

/// 2. A concurrent text edit (or move) keeps both versions: one wins, the other is a `conflict` revision; none is lost.
pub fn no_version_lost(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let world = run(schedule, switches)?;
    oracle::converged(&world)?;
    oracle::no_version_lost(&world)
}

/// 3. A restore from the trash with a newer HLC reaches the other device and beats the older delete: the newest write of every
/// group is what the devices hold, and every write is above the version it was made from, even on a clock that is behind.
pub fn restore_beats_delete(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let world = run(schedule, switches)?;
    oracle::converged(&world)?;
    oracle::newest_write_wins(&world)
}

/// 4. A device back after 200 days does not resurrect what was deleted and purged: it is told its cursor is stale, resyncs, and the
/// record goes to quarantine, never back to the journal.
pub fn no_resurrection(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let world = run(schedule, switches)?;
    oracle::purged_stays_gone(&world)?;
    oracle::converged(&world)
}

/// 5. A repeated push is idempotent: the same state, no new `seq`.
pub fn push_idempotent(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let mut world = run(schedule, switches)?;
    oracle::converged(&world)?;
    let before = world.clients.first().map(|c| c.state());
    oracle::push_is_idempotent(&mut world)?;
    world.quiesce()?;
    oracle::converged(&world)?;
    if world.clients.first().map(|c| c.state()) != before {
        return Err("replaying the server log changed the state".to_string());
    }
    Ok(())
}

/// 6. Concurrent edits of different groups are both kept.
pub fn groups_both_kept(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let world = run(schedule, switches)?;
    oracle::converged(&world)?;
    oracle::newest_write_wins(&world)
}

/// 7. No orphan is shown, and once pull reaches the head nothing is hidden for want of a parent.
pub fn no_orphans(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let mut world = run(schedule, switches)?;
    oracle::converged(&world)?;
    // A child of a purged chat may be hidden for a day while the initiator's cascade arrives (S4a); after that nothing is.
    world.step(Step::Advance(2 * DAY_MS));
    oracle::no_orphan_hidden(&world)
}

/// 8. After a server restore (a new generation) nothing acked after the backup is lost once clients resync. The honest limitation
/// of ADR-0001 §6.3 is asserted and no more: a record purged after the backup may come back, in the trash.
pub fn server_restore(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let mut world = run(schedule, switches)?;
    oracle::converged(&world)?;
    oracle::server_log_rebuilds_state(&mut world)?;
    oracle::newest_write_wins(&world)?;
    if !world.restored {
        return oracle::purged_stays_gone(&world);
    }
    let Some(first) = world.clients.first() else {
        return Ok(());
    };
    for key in &world.server.purged_ever {
        if let Some(local) = first.records.get(key) {
            // In the trash is the honest limitation; out of it is fine only if a person restored it (a write after the purge).
            let restored_by_a_person = local
                .v
                .get("deleted")
                .is_some_and(|g| world.oracle.is_restore(key, &g.hlc));
            if deleted_at(&local.v).is_none() && !restored_by_a_person {
                return Err(format!(
                    "{key:?} was purged, came back after the restore, and is not in the trash"
                ));
            }
        }
    }
    Ok(())
}

/// 9. (Beyond the eight.) No false conflicts: when one device writes a message's `body` and `placement`, however many times offline,
/// with or without outbox compaction, and however the network duplicates and reorders, no `conflict` revision is ever made.
pub fn no_false_conflicts(schedule: &Schedule, switches: Switches) -> Result<(), String> {
    let world = run(schedule, switches)?;
    oracle::converged(&world)?;
    for client in &world.clients {
        if let Some(key) = client.records.keys().find(|k| k.0 == Entity::Revision) {
            return Err(format!(
                "device {}: a conflict revision {key:?} appeared although only one device ever edited",
                client.idx
            ));
        }
    }
    if let Some(env) = world
        .server
        .all_envelopes()
        .iter()
        .find(|e| e.header.entity == Entity::Revision)
    {
        return Err(format!(
            "the server holds a revision envelope {} although only one device ever edited",
            env.header.id
        ));
    }
    Ok(())
}

/// Applies `property` to schedules from `strategy` with a fixed seed. Returns the number of cases that passed, or the report of the
/// failure: the seed, the shrunk schedule and the reason.
pub fn check<S>(
    name: &str,
    seed: u64,
    cases: u32,
    strategy: S,
    property: impl Fn(&Schedule) -> Result<(), String>,
) -> Result<u32, String>
where
    S: Strategy<Value = Schedule>,
{
    let config = Config {
        cases,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        max_shrink_iters: 20_000,
        max_shrink_time: 120_000,
        ..Config::default()
    };
    let mut runner = TestRunner::new(config);
    let ran = AtomicU32::new(0);
    let outcome = runner.run(&strategy, |schedule| {
        ran.fetch_add(1, Ordering::Relaxed);
        property(&schedule).map_err(TestCaseError::fail)
    });
    match outcome {
        Ok(()) => Ok(ran.load(Ordering::Relaxed)),
        Err(TestError::Fail(reason, schedule)) => Err(format!(
            "property `{name}` failed\n  seed: {seed}, cases: {cases} (rerun with the same values to get the same schedule)\n  reason: {reason}\n  shrunk schedule: {schedule:?}"
        )),
        Err(TestError::Abort(reason)) => Err(format!("property `{name}` aborted: {reason}")),
    }
}
