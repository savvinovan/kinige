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

/// The reply a request file gets from `handle_request`, for a step the app does itself (no page is involved).
fn reply_to(window: &str, step: &str) -> serde_json::Value {
    let dir = tempfile::tempdir().unwrap();
    let app = tauri::test::mock_app();
    for label in [MAIN_LABEL, CAPTURE_LABEL] {
        tauri::WebviewWindowBuilder::new(&app, label, Default::default())
            .build()
            .unwrap();
    }
    let requests = dir.path().join("requests");
    fs::create_dir_all(&requests).unwrap();
    let path = requests.join("r1.json");
    fs::write(
        &path,
        format!(r#"{{"id":"r1","window":"{window}","step":{step}}}"#),
    )
    .unwrap();
    handle_request(
        app.handle(),
        &DriverState::new(None),
        dir.path(),
        "r1",
        &path,
    );
    let reply = fs::read(dir.path().join("replies").join("r1.json")).unwrap();
    serde_json::from_slice(&reply).unwrap()
}

#[test]
fn close_is_for_the_main_window_only() {
    // The main window gets the close request (the same one the red button sends); the capture window is refused.
    let ok = reply_to("main", r#"{"kind":"close"}"#);
    assert_eq!(ok["ok"], true, "{ok}");
    let refused = reply_to("capture", r#"{"kind":"close"}"#);
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["error"], "close is for the main window");
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
