use super::*;
use tauri::test::mock_app;

const CLIPBOARD: &str = "синтетический текст буфера";

#[test]
fn the_clipboard_hotkey_without_a_journal_writes_nothing_and_does_not_panic() {
    // Recovery mode: the store is not managed, while the clipboard hotkey is already registered.
    let app = mock_app();
    assert!(app.handle().try_state::<Store>().is_none());
    save_clipboard_text(app.handle(), CLIPBOARD.to_string());
}

#[test]
fn a_failed_clipboard_without_a_journal_still_hands_its_text_to_the_window() {
    let app = mock_app();
    app.manage(CaptureShell::default());
    open_with_failed_clipboard(app.handle(), CLIPBOARD.to_string(), ErrorCode::Internal);
    let shell = app.state::<CaptureShell>();
    let pending = shell.pending();
    let pending = pending
        .as_ref()
        .expect("the clipboard text was handed to the window");
    assert_eq!(pending.text, CLIPBOARD);
    assert_eq!(pending.error, ErrorCode::Internal);
}

#[test]
fn losing_focus_hides_the_window_unless_the_capture_window_holds_the_microphone() {
    let app = mock_app();
    // No claim state at all (recovery mode) and a free microphone: the window hides, as before KINIG-153.
    assert!(hides_on_blur(app.handle()));
    app.manage(MicrophoneClaim::default());
    let claim = app.state::<MicrophoneClaim>();
    assert!(hides_on_blur(app.handle()));
    // A recording of the capture window (or its wait for the system's permission prompt, which takes the key status) keeps it.
    assert!(claim.hold(CAPTURE_LABEL));
    assert!(!hides_on_blur(app.handle()));
    // The main window's recording is not this window's business.
    claim.release(CAPTURE_LABEL);
    assert!(claim.hold(MAIN_LABEL));
    assert!(hides_on_blur(app.handle()));
}

#[test]
fn any_hide_of_the_capture_window_lets_its_microphone_go() {
    // The hotkey, Esc and a page that never answered all end in `hide_capture_on_main` (the mock window counts as visible).
    let app = mock_app();
    app.manage(MicrophoneClaim::default());
    tauri::WebviewWindowBuilder::new(&app, CAPTURE_LABEL, Default::default())
        .build()
        .expect("a mock window");
    let claim = app.state::<MicrophoneClaim>();
    assert!(claim.hold(CAPTURE_LABEL));
    hide_capture_on_main(app.handle());
    assert!(!claim.is_held_by(CAPTURE_LABEL));
    assert!(claim.hold(MAIN_LABEL), "the main window may record now");
    // The other window's claim is not the capture window's to give away.
    hide_capture_on_main(app.handle());
    assert!(claim.is_held_by(MAIN_LABEL));
}
