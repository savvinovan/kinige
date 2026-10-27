//! Глобальные сочетания клавиш (ТЗ §5.1): регистрация в системе, статус «назначено / занято», смена и сброс из «Настроек».
//! Регистрирует плагин `tauri-plugin-global-shortcut`, только из Rust (JS-разрешений у окон нет). Работа с плагином спрятана
//! за [`Registrar`], чтобы правила (проверка до сохранения, откат при отказе, режим записи) проверялись без окна и системы.

use kinige_domain::{ErrorCode, Settings, ShortcutKind, ShortcutState};
use kinige_store::{Store, StoreError};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Система не приняла сочетание (занято другой программой или самой системой).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused;

/// Регистрация сочетаний в системе.
pub trait Registrar: Send + Sync {
    fn register(&self, accelerator: &str) -> Result<(), Refused>;
    fn unregister(&self, accelerator: &str);
}

#[derive(Default)]
struct Inner {
    current: HashMap<ShortcutKind, String>,
    state: HashMap<ShortcutKind, ShortcutState>,
}

/// Состояние сочетаний приложения.
pub struct Hotkeys {
    registrar: Box<dyn Registrar>,
    inner: Mutex<Inner>,
    /// В «Настройках» идёт запись нового сочетания: нажатия не должны открывать окно захвата (иначе нажать уже назначенное
    /// сочетание в поле записи нельзя).
    recording: AtomicBool,
}

fn code(error: StoreError) -> ErrorCode {
    // В журнал — категория без текста внутренней ошибки.
    eprintln!("kinige: ошибка хранилища: {}", error.category());
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
        // Отравленный замок означает панику посреди смены сочетания; данные в нём остаются согласованными по полям.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Старт: регистрирует оба сочетания из настроек. То, что система не приняла, остаётся «не назначено», остальное работает.
    pub fn register_saved(&self, store: &Store) {
        for kind in ShortcutKind::ALL {
            let accelerator = store
                .shortcut_accelerator(kind)
                .unwrap_or_else(|_| kind.default_accelerator().to_string());
            let state = match self.registrar.register(&accelerator) {
                Ok(()) => ShortcutState::Assigned,
                Err(Refused) => {
                    eprintln!("kinige: сочетание {kind:?} не назначено: занято");
                    ShortcutState::Taken
                }
            };
            let mut inner = self.lock();
            inner.current.insert(kind, accelerator);
            inner.state.insert(kind, state);
        }
    }

    /// Накладывает состояние «занято» на настройки из хранилища (там все сочетания «назначены»).
    pub fn overlay(&self, settings: &mut Settings) {
        let inner = self.lock();
        for status in &mut settings.shortcuts {
            if let Some(state) = inner.state.get(&status.kind) {
                status.state = *state;
            }
        }
    }

    /// Какое действие вызвало нажатие. Сравнение по каноничной записи: плагин присылает разобранное сочетание, а в
    /// настройках лежит строка, поэтому строка разбирается тем же разбором.
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

    /// Смена своего сочетания. Порядок: проверка записи и дубликата (ничего не меняется) → регистрация нового в системе
    /// (отказ — `ShortcutTaken`, прежнее продолжает работать) → сохранение (сбой — новое снимается) → снятие прежнего.
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

    /// «Сбросить»: сочетание по умолчанию, с теми же проверками.
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

/// Реестр на плагине `tauri-plugin-global-shortcut`.
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

    /// Реестр без системы: «занятые» сочетания заданы тестом, зарегистрированные запоминаются.
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
mod tests {
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
}
