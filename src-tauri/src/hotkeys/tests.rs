use super::fake::FakeRegistrar;
use super::*;
use kinige_store::StoreOptions;
use std::sync::Arc;

struct Fixture {
    hotkeys: Hotkeys,
    registrar: Arc<FakeRegistrar>,
    store: Store,
    _dir: tempfile::TempDir,
}

fn fixture(taken: &[&str]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let mut options = StoreOptions::new(dir.path().join("kinige.db"), None);
    options.start_mirror = false;
    options.start_maintenance = false;
    let store = Store::open(options).unwrap();
    let registrar = Arc::new(FakeRegistrar::default());
    registrar
        .taken
        .lock()
        .unwrap()
        .extend(taken.iter().map(|s| s.to_string()));
    let hotkeys = Hotkeys::new(Box::new(registrar.clone()));
    hotkeys.register_saved(&store);
    Fixture {
        hotkeys,
        registrar,
        store,
        _dir: dir,
    }
}

fn states(f: &Fixture) -> Vec<(ShortcutKind, String, ShortcutState)> {
    let mut settings = f.store.get_settings().unwrap();
    f.hotkeys.overlay(&mut settings);
    settings
        .shortcuts
        .into_iter()
        .map(|s| (s.kind, s.accelerator, s.state))
        .collect()
}

fn registered(f: &Fixture) -> Vec<String> {
    let mut all: Vec<String> = f
        .registrar
        .registered
        .lock()
        .unwrap()
        .iter()
        .cloned()
        .collect();
    all.sort();
    all
}

#[test]
fn both_defaults_are_registered_and_assigned() {
    let f = fixture(&[]);
    assert_eq!(registered(&f), ["Alt+Shift+KeyV", "Ctrl+Alt+KeyN"]);
    assert_eq!(
        states(&f),
        [
            (
                ShortcutKind::Capture,
                "Ctrl+Alt+KeyN".into(),
                ShortcutState::Assigned
            ),
            (
                ShortcutKind::Clipboard,
                "Alt+Shift+KeyV".into(),
                ShortcutState::Assigned
            ),
        ]
    );
}

#[test]
fn a_taken_shortcut_is_reported_and_the_other_one_still_works() {
    let f = fixture(&["Ctrl+Alt+KeyN"]);
    assert_eq!(registered(&f), ["Alt+Shift+KeyV"]);
    let s = states(&f);
    assert_eq!(s[0].2, ShortcutState::Taken);
    assert_eq!(s[1].2, ShortcutState::Assigned);
    // Нажатие занятого сочетания ничего не вызывает, нажатие рабочего — вызывает своё действие.
    assert_eq!(f.hotkeys.kind_for(|a| a == "Ctrl+Alt+KeyN"), None);
    assert_eq!(
        f.hotkeys.kind_for(|a| a == "Alt+Shift+KeyV"),
        Some(ShortcutKind::Clipboard)
    );
}

#[test]
fn changing_registers_the_new_one_saves_it_and_releases_the_old_one() {
    let f = fixture(&[]);
    f.hotkeys
        .change(&f.store, ShortcutKind::Capture, "Cmd+Alt+KeyN")
        .unwrap();
    assert_eq!(registered(&f), ["Alt+Cmd+KeyN", "Alt+Shift+KeyV"]);
    assert_eq!(states(&f)[0].1, "Alt+Cmd+KeyN");
    assert_eq!(
        f.store.shortcut_accelerator(ShortcutKind::Capture).unwrap(),
        "Alt+Cmd+KeyN"
    );
    assert_eq!(f.hotkeys.kind_for(|a| a == "Ctrl+Alt+KeyN"), None);
    assert_eq!(
        f.hotkeys.kind_for(|a| a == "Alt+Cmd+KeyN"),
        Some(ShortcutKind::Capture)
    );
}

#[test]
fn a_taken_replacement_is_refused_before_saving_and_the_old_one_keeps_working() {
    let f = fixture(&["Alt+Cmd+KeyN"]);
    let error = f
        .hotkeys
        .change(&f.store, ShortcutKind::Capture, "Cmd+Alt+KeyN")
        .unwrap_err();
    assert_eq!(error, ErrorCode::ShortcutTaken);
    assert_eq!(registered(&f), ["Alt+Shift+KeyV", "Ctrl+Alt+KeyN"]);
    assert_eq!(
        f.store.shortcut_accelerator(ShortcutKind::Capture).unwrap(),
        "Ctrl+Alt+KeyN",
        "не сохранено"
    );
    assert_eq!(states(&f)[0].2, ShortcutState::Assigned);
}

#[test]
fn invalid_and_duplicate_shortcuts_never_reach_the_system() {
    let f = fixture(&[]);
    for (accelerator, expected) in [
        ("Space", ErrorCode::ShortcutInvalid),
        ("Cmd+KeyQ", ErrorCode::ShortcutInvalid),
        ("garbage", ErrorCode::ShortcutInvalid),
        ("Alt+Shift+KeyV", ErrorCode::ShortcutDuplicate),
    ] {
        assert_eq!(
            f.hotkeys
                .change(&f.store, ShortcutKind::Capture, accelerator),
            Err(expected),
            "{accelerator}"
        );
    }
    assert_eq!(registered(&f), ["Alt+Shift+KeyV", "Ctrl+Alt+KeyN"]);
}

#[test]
fn reset_returns_the_default_and_a_taken_default_is_reported() {
    let f = fixture(&[]);
    f.hotkeys
        .change(&f.store, ShortcutKind::Clipboard, "Ctrl+Alt+KeyB")
        .unwrap();
    f.hotkeys.reset(&f.store, ShortcutKind::Clipboard).unwrap();
    assert_eq!(registered(&f), ["Alt+Shift+KeyV", "Ctrl+Alt+KeyN"]);
    assert_eq!(states(&f)[1].1, "Alt+Shift+KeyV");

    // Умолчание занято другой программой: сброс отказывает, действующее сочетание остаётся.
    f.hotkeys
        .change(&f.store, ShortcutKind::Clipboard, "Ctrl+Alt+KeyB")
        .unwrap();
    f.registrar
        .taken
        .lock()
        .unwrap()
        .insert("Alt+Shift+KeyV".into());
    f.registrar.unregister("Alt+Shift+KeyV");
    assert_eq!(
        f.hotkeys.reset(&f.store, ShortcutKind::Clipboard),
        Err(ErrorCode::ShortcutTaken)
    );
    assert_eq!(states(&f)[1].1, "Ctrl+Alt+KeyB");
}

#[test]
fn a_taken_shortcut_becomes_assigned_when_it_is_freed_and_set_again() {
    let f = fixture(&["Ctrl+Alt+KeyN"]);
    assert_eq!(states(&f)[0].2, ShortcutState::Taken);
    f.registrar.taken.lock().unwrap().clear();
    f.hotkeys
        .change(&f.store, ShortcutKind::Capture, "Ctrl+Alt+KeyN")
        .unwrap();
    assert_eq!(states(&f)[0].2, ShortcutState::Assigned);
    assert_eq!(registered(&f), ["Alt+Shift+KeyV", "Ctrl+Alt+KeyN"]);
}

#[test]
fn recording_flag_is_toggled() {
    let f = fixture(&[]);
    assert!(!f.hotkeys.is_recording());
    f.hotkeys.set_recording(true);
    assert!(f.hotkeys.is_recording());
    f.hotkeys.set_recording(false);
    assert!(!f.hotkeys.is_recording());
}
