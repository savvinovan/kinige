//! Helpers for the settings file tests (slice 4b, ADR-0010): the store, the file thread with fast polling, reading and writing the file.

#![allow(dead_code)]

use super::*;
use kinige_domain::ShortcutKind;
use kinige_store::{
    SETTINGS_FILE_NAME, SettingsFile, SettingsFileOptions, ShortcutApplier, ShortcutRefusal,
};
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::TempDir;

pub struct Rig {
    /// First in the list: the field is dropped before the directory, so the file thread has time to stop.
    pub file: SettingsFile,
    pub dir: TempDir,
    pub store: Store,
    pub changes: Arc<AtomicUsize>,
}

/// The system refuses these shortcuts.
pub struct Refusing(pub Vec<&'static str>);

impl ShortcutApplier for Refusing {
    fn apply(
        &self,
        store: &Store,
        kind: ShortcutKind,
        accelerator: &str,
    ) -> Result<(), ShortcutRefusal> {
        if self.0.contains(&accelerator) {
            return Err(ShortcutRefusal::Taken);
        }
        store
            .save_shortcut(kind, accelerator)
            .map_err(|_| ShortcutRefusal::Invalid)
    }
}

pub fn fast_poll(options: &mut SettingsFileOptions) {
    options.poll = Duration::from_millis(25);
}

pub fn options_for(changes: &Arc<AtomicUsize>) -> SettingsFileOptions {
    let counter = changes.clone();
    let mut options = SettingsFileOptions::new(Arc::new(move || {
        counter.fetch_add(1, Ordering::SeqCst);
    }));
    // Polling is rare by default: tests drive the thread themselves (`sync_now`) so that a poll does not wedge in between steps.
    options.poll = Duration::from_secs(60);
    options
}

pub fn rig() -> Rig {
    rig_with(|_| {})
}

pub fn rig_with(tune: impl FnOnce(&mut SettingsFileOptions)) -> Rig {
    let dir = tempfile::tempdir().unwrap();
    let store = open_without_mirror(dir.path());
    start_in(dir, store, tune)
}

pub fn start_in(dir: TempDir, store: Store, tune: impl FnOnce(&mut SettingsFileOptions)) -> Rig {
    let changes = Arc::new(AtomicUsize::new(0));
    let mut options = options_for(&changes);
    tune(&mut options);
    let file = SettingsFile::start(store.clone(), dir.path().to_path_buf(), options);
    Rig {
        dir,
        store,
        file,
        changes,
    }
}

pub fn file_path(dir: &Path) -> PathBuf {
    dir.join(SETTINGS_FILE_NAME)
}

impl Rig {
    pub fn path(&self) -> PathBuf {
        file_path(self.dir.path())
    }

    /// Waits until the file thread has finished `polls` full polls after this call, for "nothing happened over N polls"
    /// checks. One more is waited for, because a poll that is already running may have started before the state the caller
    /// cares about.
    pub fn after_polls(&self, polls: u64) {
        let target = self.file.polls() + polls + 1;
        wait_until("polls of the file thread", || self.file.polls() >= target);
    }

    /// How an editor edits: a temporary file alongside and `rename` (mode 0600).
    pub fn write(&self, text: &str) {
        write_file(self.dir.path(), text.as_bytes(), 0o600);
        self.file.sync_now();
    }

    pub fn write_mode(&self, text: &str, mode: u32) {
        write_file(self.dir.path(), text.as_bytes(), mode);
        self.file.sync_now();
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&fs::read(self.path()).unwrap()).unwrap()
    }

    pub fn bytes(&self) -> Vec<u8> {
        fs::read(self.path()).unwrap()
    }

    pub fn log_rows(&self) -> Vec<(String, String, String)> {
        let conn = raw(self.dir.path());
        let mut statement = conn
            .prepare("SELECT client, tool, args_json FROM mcp_log ORDER BY id")
            .unwrap();
        statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        raw(self.dir.path())
            .query_row("SELECT value FROM app_setting WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .ok()
    }
}

/// An edit of the file "from outside": a temporary file and `rename`, as editors do. The mode applies on Unix only; Windows
/// has no POSIX permissions (ADR-0010, Windows note), so the argument is ignored there.
pub fn write_file(dir: &Path, bytes: &[u8], mode: u32) {
    let tmp = dir.join(".editor.tmp");
    fs::write(&tmp, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode)).unwrap();
    }
    #[cfg(not(unix))]
    let _ = mode;
    fs::rename(&tmp, file_path(dir)).unwrap();
}

#[cfg(unix)]
pub fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// Chats for matching: "Работа" (ordinary), "Дневник" (`ai_excluded`).
pub fn chats(store: &Store, dir: &Path) -> (String, String) {
    let now = moscow(T0);
    let work = store.create_chat("Работа", None, &now).unwrap().id;
    let diary = store.create_chat("Дневник", None, &now).unwrap().id;
    raw(dir)
        .execute("UPDATE chat SET ai_excluded = 1 WHERE id = ?1", [&diary])
        .unwrap();
    (work, diary)
}
