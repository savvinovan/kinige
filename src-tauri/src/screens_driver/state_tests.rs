//! Tests of `DriverState` in `mod.rs`: the bound on steps waiting for an answer, their expiry, and who may answer (KINIG-92).

use super::*;

fn aged(state: &DriverState, id: &str, by: Duration) {
    let mut inner = state.lock();
    let pending = inner.pending.get_mut(id).unwrap();
    pending.since = Instant::now().checked_sub(by).unwrap();
}

#[test]
fn no_more_than_max_pending_steps_wait_and_an_id_is_used_once() {
    let state = DriverState::new(None);
    for number in 0..MAX_PENDING {
        assert!(state.add_pending(&format!("r{number}"), "main"), "{number}");
    }
    assert!(!state.add_pending("one-too-many", "main"));
    assert!(
        !state.add_pending("r0", "main"),
        "an id that waits is not taken twice"
    );
    // An answer makes room again.
    assert!(state.take_pending("r0", "main"));
    assert!(state.add_pending("one-too-many", "main"));
    assert_eq!(state.lock().pending.len(), MAX_PENDING);
}

#[test]
fn a_step_nobody_answered_expires_and_makes_room() {
    let state = DriverState::new(None);
    for number in 0..MAX_PENDING {
        assert!(state.add_pending(&format!("r{number}"), "main"));
    }
    aged(&state, "r0", PENDING_TTL + Duration::from_secs(1));
    aged(&state, "r1", PENDING_TTL - Duration::from_secs(60));
    assert!(
        state.add_pending("fresh", "main"),
        "the expired step made room"
    );
    assert!(
        !state.take_pending("r0", "main"),
        "and cannot be answered any more"
    );
    assert!(
        state.take_pending("r1", "main"),
        "a step inside the time limit still can"
    );
}

#[test]
fn only_the_window_that_was_sent_a_step_may_answer_it() {
    let state = DriverState::new(None);
    assert!(state.add_pending("r1", "main"));
    assert!(!state.take_pending("r1", "settings"));
    assert!(!state.take_pending("other", "main"));
    assert!(state.take_pending("r1", "main"));
    assert!(!state.take_pending("r1", "main"), "once");
}

#[test]
fn ready_is_per_window_and_a_destroyed_window_must_say_it_again() {
    let state = DriverState::new(None);
    assert!(!state.is_ready("main"));
    state.mark_ready("main");
    assert!(state.is_ready("main") && !state.is_ready("settings"));
    state.mark_gone("main");
    assert!(!state.is_ready("main"));
}
