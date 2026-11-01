use super::*;
use tauri::test::mock_app;

const CLIPBOARD: &str = "синтетический текст буфера";

#[test]
fn the_clipboard_hotkey_without_a_journal_writes_nothing_and_does_not_panic() {
    // Режим восстановления: хранилище не управляется, а хоткей буфера уже зарегистрирован.
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
    let pending = pending.as_ref().expect("текст буфера передан окну");
    assert_eq!(pending.text, CLIPBOARD);
    assert_eq!(pending.error, ErrorCode::Internal);
}
