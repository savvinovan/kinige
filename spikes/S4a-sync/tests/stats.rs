//! Coverage numbers: how much of the protocol the generated schedules really exercise. Run with `--nocapture`; the output goes into the
//! README. Counts are over the quiesced worlds of `S4A_STATS_N` (default 2000) schedules per mix.

use proptest::strategy::{Strategy, ValueTree};
use proptest::test_runner::{Config, RngAlgorithm, RngSeed, TestRunner};
use s4a_sync::client::{Event, ResyncCause};
use s4a_sync::directed::{
    away_schedule, chain_schedule, chat_purge_schedule, single_writer_schedule,
};
use s4a_sync::envelope::Entity;
use s4a_sync::props::run;
use s4a_sync::schedules::{Mix, schedule};
use s4a_sync::switches::Switches;
use s4a_sync::world::Schedule;
use std::collections::BTreeMap;

fn sample(strategy: impl Strategy<Value = Schedule>, n: usize) -> Vec<Schedule> {
    let mut runner = TestRunner::new(Config {
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(7),
        failure_persistence: None,
        ..Config::default()
    });
    (0..n)
        .filter_map(|_| strategy.new_tree(&mut runner).ok().map(|t| t.current()))
        .collect()
}

fn report(name: &str, schedules: Vec<Schedule>) {
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();
    let mut add = |what: &'static str, n: usize| *totals.entry(what).or_default() += n;
    let mut steps = 0;
    for s in &schedules {
        steps += s.steps.len();
        let Ok(w) = run(s, Switches::default()) else {
            add("schedules that did not run clean", 1);
            continue;
        };
        add("records", w.clients[0].records.len());
        add(
            "messages",
            w.clients[0]
                .records
                .keys()
                .filter(|k| k.0 == Entity::Message)
                .count(),
        );
        add(
            "conflict revisions",
            w.clients[0]
                .records
                .keys()
                .filter(|k| k.0 == Entity::Revision)
                .count(),
        );
        add(
            "tombstones (all devices)",
            w.clients.iter().map(|c| c.tombstones.len()).sum(),
        );
        add(
            "cascade-child tombstones",
            w.clients
                .iter()
                .map(|c| {
                    c.tombstones
                        .values()
                        .filter(|t| t.cascade_of.is_some())
                        .count()
                })
                .sum(),
        );
        add(
            "quarantined records",
            w.clients.iter().map(|c| c.quarantine.len()).sum(),
        );
        add(
            "purges accepted by the server",
            w.server.purged_ever().len(),
        );
        add(
            "server compactions that dropped a purge",
            usize::from(w.server.drops_count() > 0),
        );
        for c in &w.clients {
            for e in &c.events {
                match e {
                    Event::Resync(cause) => add(
                        match cause {
                            ResyncCause::Binding => "resyncs: first binding",
                            ResyncCause::StaleCursor => "resyncs: stale cursor",
                            ResyncCause::GenerationChanged => "resyncs: new generation",
                            ResyncCause::StaleAndGeneration => "resyncs: both",
                        },
                        1,
                    ),
                    Event::Lifted(_) => add("tombstones lifted", 1),
                    Event::ClockWarning => add("clock warnings (> 10 min ahead)", 1),
                    Event::EqualHlcDifferentBytes(_) => add("equal HLC, different bytes", 1),
                    _ => {}
                }
            }
        }
    }
    println!("{name}: {} schedules, {steps} steps", schedules.len());
    for (what, n) in totals {
        println!("    {what}: {n}");
    }
}

#[test]
fn coverage() {
    let n: usize = std::env::var("S4A_STATS_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    report(
        "general, 2 clients",
        sample(schedule(2, Mix::GENERAL, 8..70), n),
    );
    report(
        "conflicts, 2 clients",
        sample(schedule(2, Mix::CONFLICTS, 8..70), n),
    );
    report(
        "chats, 3 clients",
        sample(schedule(3, Mix::CHATS, 8..90), n),
    );
    report(
        "server restore, 2 clients",
        sample(schedule(2, Mix::RESTORE, 8..70), n),
    );
    report(
        "device away 200 days, 2 clients",
        sample(away_schedule(2), n),
    );
    report("compacted chain, 2 clients", sample(chain_schedule(2), n));
    report("chat purges, 3 clients", sample(chat_purge_schedule(3), n));
    report(
        "single writer, 2 clients",
        sample(single_writer_schedule(2), n),
    );
}
