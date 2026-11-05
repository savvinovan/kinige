//! Negative controls: for each property, a switch that breaks one rule must make the property fail. A property that passes on broken
//! code proves nothing. Each test prints the property's failure report (the reason, the seed and the shrunk schedule), which goes
//! into the PR description. Cases: `S4A_CONTROL_CASES` (default 20 000, a failure normally comes within a few hundred).

use proptest::strategy::Strategy;
use s4a_sync::props::{self, check};
use s4a_sync::schedules::{Mix, away_schedule, schedule};
use s4a_sync::switches::Switches;
use s4a_sync::world::Schedule;

fn cases() -> u32 {
    std::env::var("S4A_CONTROL_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(20_000)
}

fn seed() -> u64 {
    std::env::var("S4A_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1)
}

type Property = fn(&Schedule, Switches) -> Result<(), String>;

/// The property must fail with `switches` on, and the reason must contain one of `expect`.
fn must_fail(
    name: &str,
    strategy: impl Strategy<Value = Schedule>,
    property: Property,
    switches: Switches,
    expect: &[&str],
) {
    match check(name, seed(), cases(), strategy, |s| property(s, switches)) {
        Ok(ran) => panic!(
            "negative control `{name}`: the property still passed on {ran} cases, so it does not catch this breakage"
        ),
        Err(report) => {
            println!("{report}");
            assert!(
                expect.iter().any(|e| report.contains(e)),
                "negative control `{name}` failed for another reason than {expect:?}:\n{report}"
            );
        }
    }
}

#[test]
fn control_1_no_device_tiebreak_breaks_convergence() {
    let on = Switches {
        no_device_tiebreak: true,
        ..Switches::default()
    };
    must_fail(
        "1 convergence without the device_id tiebreak",
        schedule(2, Mix::GENERAL, 8..70),
        props::convergence,
        on,
        &["differ"],
    );
}

#[test]
fn control_1_seq_without_the_lock_breaks_convergence() {
    let on = Switches {
        no_seq_lock: true,
        ..Switches::default()
    };
    must_fail(
        "1 convergence with seq handed out without the lock",
        schedule(2, Mix::GENERAL, 8..70),
        props::convergence,
        on,
        &["differ"],
    );
}

#[test]
fn control_2_no_conflict_revision_loses_a_version() {
    let on = Switches {
        no_conflict_revision: true,
        ..Switches::default()
    };
    must_fail(
        "2 no version lost without conflict revisions",
        schedule(2, Mix::CONFLICTS, 8..70),
        props::no_version_lost,
        on,
        &["is gone"],
    );
}

#[test]
fn control_3_delete_always_wins_beats_the_restore() {
    let on = Switches {
        delete_always_wins: true,
        ..Switches::default()
    };
    must_fail(
        "3 restore beats delete, when delete always wins",
        schedule(2, Mix::TRASH, 8..70),
        props::restore_beats_delete,
        on,
        &["newest write", "differ"],
    );
}

#[test]
fn control_3_clock_that_does_not_receive_gives_a_restore_a_smaller_stamp() {
    let on = Switches {
        no_clock_receive: true,
        ..Switches::default()
    };
    must_fail(
        "3 restore beats delete, clock does not receive",
        schedule(2, Mix::TRASH, 8..70),
        props::restore_beats_delete,
        on,
        &["HLC", "parent"],
    );
}

#[test]
fn control_4_no_stale_check_resurrects() {
    let on = Switches {
        no_stale_check: true,
        ..Switches::default()
    };
    must_fail(
        "4 no resurrection without the stale check",
        away_schedule(2),
        props::no_resurrection,
        on,
        &["live again"],
    );
}

#[test]
fn control_5_no_dedup_gives_a_repeat_a_new_seq() {
    let on = Switches {
        no_dedup: true,
        ..Switches::default()
    };
    must_fail(
        "5 idempotent push without op_id dedup",
        schedule(
            2,
            Mix {
                faults: 8,
                ..Mix::GENERAL
            },
            8..70,
        ),
        props::push_idempotent,
        on,
        &["not a duplicate"],
    );
}

#[test]
fn control_6_whole_record_lww_loses_the_other_group() {
    let on = Switches {
        whole_record_lww: true,
        ..Switches::default()
    };
    must_fail(
        "6 groups both kept, whole-record LWW",
        schedule(2, Mix::GENERAL, 8..70),
        props::groups_both_kept,
        on,
        &["newest write", "differ"],
    );
}

#[test]
fn control_7_orphans_shown_at_once() {
    let on = Switches {
        show_orphans: true,
        ..Switches::default()
    };
    must_fail(
        "7 no orphan shown, orphans shown at once",
        schedule(2, Mix::CHATS, 8..70),
        props::no_orphans,
        on,
        &["does not have"],
    );
}

#[test]
fn control_8_ignoring_a_new_generation_loses_what_the_server_lost() {
    let on = Switches {
        ignore_generation: true,
        ..Switches::default()
    };
    must_fail(
        "8 server restore, generation ignored",
        schedule(2, Mix::RESTORE, 8..70),
        props::server_restore,
        on,
        &["fresh device", "newest write", "differ"],
    );
}

#[test]
fn control_2_without_vv_a_compacted_chain_loses_a_version() {
    let on = Switches {
        no_vv: true,
        ..Switches::default()
    };
    must_fail(
        "2 no version lost, compaction, without vv",
        s4a_sync::schedules::chain_schedule(2),
        props::no_version_lost,
        on,
        &["is gone"],
    );
}

#[test]
fn control_7_without_the_grace_children_of_a_purged_chat_stay_hidden() {
    let on = Switches {
        no_child_grace: true,
        ..Switches::default()
    };
    must_fail(
        "7 nothing hidden for good, children hidden for as long as the tombstone lives",
        s4a_sync::schedules::chat_purge_schedule(2),
        props::no_orphans,
        on,
        &["is hidden after a complete pull"],
    );
}

#[test]
fn control_1_cascade_at_the_parent_ack_loses_what_only_the_initiator_had() {
    let on = Switches {
        cascade_at_parent_ack: true,
        ..Switches::default()
    };
    must_fail(
        "1 convergence, chat purge deleting the messages at the chat's ack (ADR-0001 revision 7)",
        s4a_sync::schedules::chat_purge_schedule(2),
        props::convergence,
        on,
        &["differ"],
    );
}

// The control for the tombstone that keeps its own snapshot (S4a finding 6) is a named scenario, tests/scenarios.rs: the random
// schedules reach it only after about thirty thousand cases.
