//! Closing the main window on the mock runtime (KINIG-156, PRD §5, ADR-0015 round 5): the page of `main` is told before the window hides, so it can
//! stop a recording and keep it; no other window hears of it. The event carries nothing (no entry text, no path).

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::main_window::{MAIN_WINDOW_HIDING, hide};
use std::sync::{Arc, Mutex};
use tauri::Listener;

fn listen_to<L: Listener<tauri::test::MockRuntime>>(target: &L) -> Arc<Mutex<Vec<String>>> {
    let heard = Arc::new(Mutex::new(Vec::<String>::new()));
    let sink = heard.clone();
    target.listen(MAIN_WINDOW_HIDING, move |event| {
        sink.lock().unwrap().push(event.payload().to_string())
    });
    heard
}

fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        if condition() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    panic!("timed out waiting for: {what}");
}

#[test]
fn closing_the_main_window_tells_only_its_page_and_carries_nothing() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let settings = window(&fixture, "settings");
    let to_main = listen_to(&main);
    let to_capture = listen_to(&capture);
    let to_settings = listen_to(&settings);

    hide(&main.as_ref().window());

    wait_until("the event reaches main", || {
        !to_main.lock().unwrap().is_empty()
    });
    assert_eq!(*to_main.lock().unwrap(), ["null"], "one event, no payload");
    assert!(to_capture.lock().unwrap().is_empty());
    assert!(to_settings.lock().unwrap().is_empty());
}

#[test]
fn the_event_name_is_the_one_the_page_listens_to() {
    // `web/src/api/tauriCall.ts` has the same literal; a rename on one side only would leave a recording running behind a hidden window.
    let page = std::fs::read_to_string(format!(
        "{}/../web/src/api/tauriCall.ts",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    assert!(
        page.contains(&format!("MAIN_WINDOW_HIDING = \"{MAIN_WINDOW_HIDING}\"")),
        "tauriCall.ts must export MAIN_WINDOW_HIDING = \"{MAIN_WINDOW_HIDING}\""
    );
}
