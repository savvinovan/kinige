//! Global keyboard shortcuts (PRD §5.1): registration in the system, the "assigned / taken" status, change and reset from "Settings".
//! Registered by the `tauri-plugin-global-shortcut` plugin, from Rust only (the windows have no JS permissions). Work with the plugin is hidden
//! behind [`Registrar`], so that the rules (check before saving, rollback on refusal, recording mode) can be tested without a window and the system.

use kinige_domain::{ErrorCode, Settings, ShortcutKind, ShortcutState};
use kinige_store::{Store, StoreError};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// The system did not accept the shortcut (taken by another app or by the system itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused;

/// Registration of shortcuts in the system.
pub trait Registrar: Send + Sync {
    fn register(&self, accelerator: &str) -> Result<(), Refused>;
    fn unregister(&self, accelerator: &str);
}

#[derive(Default)]
struct Inner {
    current: HashMap<ShortcutKind, String>,
    state: HashMap<ShortcutKind, ShortcutState>,
}

/// The state of the app shortcuts.
pub struct Hotkeys {
    registrar: Box<dyn Registrar>,
    inner: Mutex<Inner>,
    /// A new shortcut is being recorded in "Settings": presses must not open the capture window (otherwise an already assigned
    /// shortcut could not be pressed in the recording field).
    recording: AtomicBool,
}

fn code(error: StoreError) -> ErrorCode {
    // Only the category goes to the log, without the text of the internal error.
    log::error!("store error: {}", error.category());
    error.code()
}

impl Hotkeys {
    pub fn new(registrar: Box<dyn Registrar>) -> Hotkeys {
        Hotkeys {
            registrar,
            inner: Mutex::new(Inner::default()),
            recording: AtomicBool::new(false),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A poisoned lock means a panic in the middle of a shortcut change; the data in it stays consistent field by field.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Start: registers both shortcuts from the settings. What the system did not accept stays "unassigned", the rest works.
    pub fn register_saved(&self, store: &Store) {
        for kind in ShortcutKind::ALL {
            let accelerator = store
                .shortcut_accelerator(kind)
                .unwrap_or_else(|_| kind.default_accelerator().to_string());
            let state = match self.registrar.register(&accelerator) {
                Ok(()) => ShortcutState::Assigned,
                Err(Refused) => {
                    log::warn!("shortcut {kind:?} not assigned: taken");
                    ShortcutState::Taken
                }
            };
            let mut inner = self.lock();
            inner.current.insert(kind, accelerator);
            inner.state.insert(kind, state);
        }
    }

    /// Overlays the "taken" state on the settings from the store (there all shortcuts are "assigned").
    pub fn overlay(&self, settings: &mut Settings) {
        let inner = self.lock();
        for status in &mut settings.shortcuts {
            if let Some(state) = inner.state.get(&status.kind) {
                status.state = *state;
            }
        }
    }

    /// Which action the press triggered. Compared by the canonical form: the plugin sends a parsed shortcut, while the
    /// settings hold a string, so the string is parsed by the same parser.
    pub fn kind_for(&self, matches: impl Fn(&str) -> bool) -> Option<ShortcutKind> {
        let inner = self.lock();
        ShortcutKind::ALL.into_iter().find(|kind| {
            inner.state.get(kind) == Some(&ShortcutState::Assigned)
                && inner.current.get(kind).is_some_and(|accel| matches(accel))
        })
    }

    pub fn set_recording(&self, active: bool) {
        self.recording.store(active, Ordering::SeqCst);
    }

    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    /// Changing one's own shortcut. Order: check the form and duplicates (nothing changes) -> register the new one in the system
    /// (refusal gives `ShortcutTaken`, the previous one keeps working) -> save (on failure the new one is released) -> release the previous one.
    pub fn change(
        &self,
        store: &Store,
        kind: ShortcutKind,
        accelerator: &str,
    ) -> Result<(), ErrorCode> {
        let canonical = store.check_shortcut(kind, accelerator).map_err(code)?;
        self.apply(
            kind,
            canonical,
            |store, canonical| store.save_shortcut(kind, canonical),
            store,
        )
    }

    /// "Reset": the default shortcut, with the same checks.
    pub fn reset(&self, store: &Store, kind: ShortcutKind) -> Result<(), ErrorCode> {
        let canonical = store
            .check_shortcut(kind, kind.default_accelerator())
            .map_err(code)?;
        self.apply(
            kind,
            canonical,
            |store, _| store.reset_shortcut(kind),
            store,
        )
    }

    fn apply(
        &self,
        kind: ShortcutKind,
        canonical: String,
        persist: impl FnOnce(&Store, &str) -> Result<(), StoreError>,
        store: &Store,
    ) -> Result<(), ErrorCode> {
        let (old, old_state) = {
            let inner = self.lock();
            (
                inner.current.get(&kind).cloned(),
                inner.state.get(&kind).copied(),
            )
        };
        let unchanged = old.as_deref() == Some(canonical.as_str());
        if unchanged && old_state == Some(ShortcutState::Assigned) {
            persist(store, &canonical).map_err(code)?;
            return Ok(());
        }
        if self.registrar.register(&canonical).is_err() {
            return Err(ErrorCode::ShortcutTaken);
        }
        if let Err(error) = persist(store, &canonical) {
            self.registrar.unregister(&canonical);
            return Err(code(error));
        }
        if let (Some(old), Some(ShortcutState::Assigned), false) = (&old, old_state, unchanged) {
            self.registrar.unregister(old);
        }
        let mut inner = self.lock();
        inner.current.insert(kind, canonical);
        inner.state.insert(kind, ShortcutState::Assigned);
        Ok(())
    }
}

/// A registry on the `tauri-plugin-global-shortcut` plugin.
pub struct PluginRegistrar<R: tauri::Runtime> {
    pub app: tauri::AppHandle<R>,
}

impl<R: tauri::Runtime> Registrar for PluginRegistrar<R> {
    fn register(&self, accelerator: &str) -> Result<(), Refused> {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        self.app
            .global_shortcut()
            .register(accelerator)
            .map_err(|_| Refused)
    }

    fn unregister(&self, accelerator: &str) {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let _ = self.app.global_shortcut().unregister(accelerator);
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;
    use std::collections::HashSet;

    /// A registry without the system: the "taken" shortcuts are set by the test, the registered ones are remembered.
    #[derive(Default)]
    pub struct FakeRegistrar {
        pub taken: Mutex<HashSet<String>>,
        pub registered: Mutex<HashSet<String>>,
    }

    impl Registrar for std::sync::Arc<FakeRegistrar> {
        fn register(&self, accelerator: &str) -> Result<(), Refused> {
            if self.taken.lock().unwrap().contains(accelerator) {
                return Err(Refused);
            }
            self.registered.lock().unwrap().insert(accelerator.into());
            Ok(())
        }

        fn unregister(&self, accelerator: &str) {
            self.registered.lock().unwrap().remove(accelerator);
        }
    }
}

#[cfg(test)]
mod tests;
