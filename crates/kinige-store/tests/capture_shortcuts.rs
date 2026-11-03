//! Keyboard shortcuts in settings: defaults, change, reset, refusal before saving.

// The root of the integration tests is entirely test code: `cfg(test)` gives helpers outside #[test] the same
// clippy.toml relaxations (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

mod common;

use common::*;
use kinige_domain::ShortcutKind;
use kinige_store::StoreError;

#[test]
fn shortcuts_default_to_the_decided_combinations_and_can_be_changed_and_reset() {
    let dir = tempfile::tempdir().unwrap();
    {
        let store = open_without_mirror(dir.path());
        let settings = store.get_settings().unwrap();
        let pairs: Vec<(ShortcutKind, &str, bool)> = settings
            .shortcuts
            .iter()
            .map(|s| (s.kind, s.accelerator.as_str(), s.is_default))
            .collect();
        assert_eq!(
            pairs,
            [
                (ShortcutKind::Capture, "Ctrl+Alt+KeyN", true),
                (ShortcutKind::Clipboard, "Alt+Shift+KeyV", true)
            ]
        );

        assert_eq!(
            store
                .check_shortcut(ShortcutKind::Capture, "Cmd+Alt+KeyN")
                .unwrap(),
            "Alt+Cmd+KeyN"
        );
        store
            .save_shortcut(ShortcutKind::Capture, "Cmd+Alt+KeyN")
            .unwrap();
        assert_eq!(
            store.shortcut_accelerator(ShortcutKind::Capture).unwrap(),
            "Alt+Cmd+KeyN"
        );
    }
    let store = open_without_mirror(dir.path());
    let settings = store.get_settings().unwrap();
    assert_eq!(settings.shortcuts[0].accelerator, "Alt+Cmd+KeyN");
    assert!(!settings.shortcuts[0].is_default);
    assert!(settings.shortcuts[1].is_default);

    store.reset_shortcut(ShortcutKind::Capture).unwrap();
    let settings = store.get_settings().unwrap();
    assert_eq!(settings.shortcuts[0].accelerator, "Ctrl+Alt+KeyN");
    assert!(settings.shortcuts[0].is_default);
}

#[test]
fn invalid_reserved_and_duplicate_shortcuts_are_refused_before_anything_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    for bad in [
        "",
        "Space",
        "Shift+KeyV",
        "Alt+Escape",
        "Cmd+KeyQ",
        "Alt+Alt+KeyV",
        "nonsense",
    ] {
        assert!(
            matches!(
                store.check_shortcut(ShortcutKind::Capture, bad),
                Err(StoreError::ShortcutInvalid)
            ),
            "{bad:?}"
        );
        assert!(store.save_shortcut(ShortcutKind::Capture, bad).is_err());
    }
    // A match with the shortcut of another action, in any notation.
    for same in ["Alt+Shift+KeyV", "Shift+Alt+KeyV", "Option+Shift+KeyV"] {
        assert!(
            matches!(
                store.check_shortcut(ShortcutKind::Capture, same),
                Err(StoreError::ShortcutDuplicate)
            ),
            "{same}"
        );
    }
    // One's own current shortcut is allowed.
    assert_eq!(
        store
            .check_shortcut(ShortcutKind::Capture, "Ctrl+Alt+KeyN")
            .unwrap(),
        "Ctrl+Alt+KeyN"
    );
    let stored: i64 = raw(dir.path())
        .query_row(
            "SELECT COUNT(*) FROM app_setting WHERE key LIKE 'shortcut_%'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, 0);
    // A corrupted value in settings gives the default.
    raw(dir.path())
        .execute(
            "INSERT INTO app_setting(key, value) VALUES ('shortcut_capture', 'мусор')",
            [],
        )
        .unwrap();
    assert_eq!(
        store.shortcut_accelerator(ShortcutKind::Capture).unwrap(),
        "Ctrl+Alt+KeyN"
    );
}
