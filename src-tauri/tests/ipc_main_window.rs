//! Closing the main window on the mock runtime (KINIG-156, PRD §5, ADR-0015 round 5): the page of `main` is told before the window hides, so it can
//! stop a recording and keep it; no other window hears of it. The event carries nothing (no entry text, no path).

// A whole integration-test root is test code: `cfg(test)` gives helpers outside #[test] the same clippy.toml allowances as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_lib::main_window::{MAIN_WINDOW_HIDING, hide};
use std::sync::mpsc::channel;
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

/// A `CloseRequested` as the runtime sends it, and the receiver of the answer: `true` is `prevent_close`.
fn close_request() -> (tauri::WindowEvent, std::sync::mpsc::Receiver<bool>) {
    let (signal_tx, rx) = channel();
    let event = tauri_runtime::window::WindowEvent::CloseRequested { signal_tx };
    (event.into(), rx)
}

/// The close request goes through the app's own window-event handler, `kinige_lib::on_window_event` (the function `builder` registers; the
/// test below reads the registration from the source): the window is kept (`prevent_close`) and `main` hears `main-window-hiding`. A handler that
/// only prevents the close and hides the window (the code before KINIG-156) fails this test: nobody tells the page. The mock's `hide()` does
/// nothing and cannot be observed, so "hidden" is not asserted. The mock runtime never calls the handlers a builder registers, so a close sent
/// through `Window::close()` cannot reach them there: the event is built and handed to the handler directly.
#[test]
fn a_close_request_of_the_main_window_goes_through_the_apps_handler_and_main_hears_it() {
    let fixture = fixture();
    let main = window(&fixture, "main");
    let capture = window(&fixture, "capture");
    let to_main = listen_to(&main);
    let to_capture = listen_to(&capture);

    let (event, answer) = close_request();
    kinige_lib::on_window_event(&main.as_ref().window(), &event);

    assert_eq!(
        answer.try_recv(),
        Ok(true),
        "the close is prevented: the window is hidden, not destroyed"
    );
    wait_until("the event reaches main", || {
        !to_main.lock().unwrap().is_empty()
    });
    assert_eq!(
        *to_main.lock().unwrap(),
        ["null"],
        "main heard main-window-hiding once, with no payload"
    );
    assert!(to_capture.lock().unwrap().is_empty());
}

#[test]
fn the_app_registers_that_handler_in_its_builder_so_the_tests_and_the_app_share_it() {
    // `tauri::Builder::on_window_event(on_window_event)` is in `builder`, which `run` and the IPC fixtures both use; a registration that moved
    // back into `run` only, or to another function, would leave this test green and the app on something else, so the source is read.
    let lib =
        std::fs::read_to_string(format!("{}/src/lib.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let builder = lib
        .split("pub fn builder<")
        .nth(1)
        .and_then(|rest| rest.split("\n}\n").next())
        .expect("lib.rs has `pub fn builder`");
    assert!(
        builder.contains(".on_window_event(on_window_event)"),
        "`builder` must register `on_window_event`"
    );
    assert_eq!(
        lib.matches(".on_window_event(").count(),
        1,
        "and nothing else registers a window event handler"
    );
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
