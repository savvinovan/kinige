//! The change lock of `Hotkeys` (slice 5d, ADR-0022): two changes never interleave, and a change that waits in `register` does not
//! hold up key presses or the settings window's reads.

use super::fake::FakeRegistrar;
use super::*;
use kinige_store::StoreOptions;
use std::sync::mpsc;
use std::sync::{Arc, Barrier, Condvar};
use std::time::Duration;

/// A registrar whose `register` takes time (and, for one accelerator, waits until the test opens a gate), as the plugin does when
/// it hops to the main thread.
struct SlowRegistrar {
    inner: Arc<FakeRegistrar>,
    delay: Duration,
    gate: Option<Gate>,
}

struct Gate {
    accelerator: &'static str,
    open: Arc<(Mutex<bool>, Condvar)>,
    entered: mpsc::Sender<()>,
}

impl Registrar for SlowRegistrar {
    fn register(&self, accelerator: &str) -> Result<(), Refused> {
        std::thread::sleep(self.delay);
        if let Some(gate) = &self.gate
            && gate.accelerator == accelerator
        {
            let _ = gate.entered.send(());
            let (lock, wake) = &*gate.open;
            let mut open = lock.lock().unwrap();
            while !*open {
                open = wake.wait(open).unwrap();
            }
        }
        self.inner.register(accelerator)
    }

    fn unregister(&self, accelerator: &str) {
        self.inner.unregister(accelerator);
    }
}

fn store() -> (Store, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut options = StoreOptions::new(dir.path().join("kinige.db"), None);
    options.start_mirror = false;
    options.start_maintenance = false;
    (Store::open(options).unwrap(), dir)
}

#[test]
fn two_threads_changing_one_shortcut_leave_one_registration_and_it_is_the_stored_one() {
    let (store, _dir) = store();
    let fake = Arc::new(FakeRegistrar::default());
    let hotkeys = Hotkeys::new(Box::new(SlowRegistrar {
        inner: fake.clone(),
        delay: Duration::from_millis(30),
        gate: None,
    }));
    hotkeys.register_saved(&store);
    let barrier = Barrier::new(2);
    std::thread::scope(|scope| {
        for accelerator in ["Cmd+Alt+KeyN", "Cmd+Alt+KeyB"] {
            let (hotkeys, store, barrier) = (&hotkeys, &store, &barrier);
            scope.spawn(move || {
                barrier.wait();
                hotkeys
                    .change(store, ShortcutKind::Capture, accelerator)
                    .unwrap();
            });
        }
    });
    let stored = store.shortcut_accelerator(ShortcutKind::Capture).unwrap();
    let mut registered: Vec<String> = fake.registered.lock().unwrap().iter().cloned().collect();
    registered.sort();
    // Capture has exactly one registration and it is the saved one; the clipboard shortcut is untouched.
    assert_eq!(registered.len(), 2, "{registered:?}");
    assert!(registered.contains(&stored), "{registered:?} vs {stored}");
    assert!(registered.contains(&"Alt+Shift+KeyV".to_string()));
    let mut settings = store.get_settings().unwrap();
    hotkeys.overlay(&mut settings);
    assert_eq!(settings.shortcuts[0].accelerator, stored);
    assert_eq!(settings.shortcuts[0].state, ShortcutState::Assigned);
}

#[test]
fn a_change_blocked_in_register_blocks_neither_key_presses_nor_the_overlay() {
    let (store, _dir) = store();
    let fake = Arc::new(FakeRegistrar::default());
    let open = Arc::new((Mutex::new(false), Condvar::new()));
    let (entered, entered_rx) = mpsc::channel();
    let hotkeys = Hotkeys::new(Box::new(SlowRegistrar {
        inner: fake,
        delay: Duration::ZERO,
        gate: Some(Gate {
            accelerator: "Alt+Cmd+KeyN",
            open: open.clone(),
            entered,
        }),
    }));
    hotkeys.register_saved(&store);
    std::thread::scope(|scope| {
        let change = scope.spawn(|| {
            hotkeys
                .change(&store, ShortcutKind::Capture, "Cmd+Alt+KeyN")
                .unwrap();
        });
        entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
        // The change sits in `register` with the change lock held. A key press and the window's read still answer at once.
        let (done, answered) = mpsc::channel();
        let (hotkeys, store) = (&hotkeys, &store);
        scope.spawn(move || {
            let kind = hotkeys.kind_for(|accelerator| accelerator == "Alt+Shift+KeyV");
            let mut settings = store.get_settings().unwrap();
            hotkeys.overlay(&mut settings);
            let _ = done.send((kind, settings.shortcuts.len()));
        });
        let (kind, shortcuts) = answered
            .recv_timeout(Duration::from_secs(5))
            .expect("a press and the overlay must not wait for a change");
        assert_eq!(kind, Some(ShortcutKind::Clipboard));
        assert_eq!(shortcuts, 2);
        let (lock, wake) = &*open;
        *lock.lock().unwrap() = true;
        wake.notify_all();
        change.join().unwrap();
    });
}
