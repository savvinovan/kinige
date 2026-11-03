//! The settings file thread (ADR-0010): polls once a second, applies external edits, writes after edits from the window.
//! All work with the file happens here, in one thread; an edit from the window runs in it as a job.

use super::apply::KEY_FILE_HASH;
use super::io::{self, Sig};
use crate::Store;
use crate::settings::get_setting;
use kinige_domain::{SettingsFileState, ShortcutKind};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often the thread looks at the file.
pub const POLL: Duration = Duration::from_secs(1);

/// The system (or the validation) refused a shortcut from the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutRefusal {
    Invalid,
    Duplicate,
    Taken,
}

/// Accepts a shortcut from the file: validation, registration in the system, saving -- as on a change in the window. The Tauri layer.
pub trait ShortcutApplier: Send + Sync {
    fn apply(
        &self,
        store: &Store,
        kind: ShortcutKind,
        accelerator: &str,
    ) -> Result<(), ShortcutRefusal>;
}

/// Options of the file thread.
pub struct SettingsFileOptions {
    pub poll: Duration,
    /// Accepts shortcuts after startup. `None` means a plain database write (store tests).
    pub shortcuts: Option<Arc<dyn ShortcutApplier>>,
    /// Something changed (the file was applied, the file was written, a window edit): the Tauri layer tells the windows. Does not block.
    pub on_change: Arc<dyn Fn() + Send + Sync>,
    /// Tests only: a step between the app's `rename` and the hash write (the "an agent replaced the file" race).
    #[doc(hidden)]
    pub after_rename: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl SettingsFileOptions {
    pub fn new(on_change: Arc<dyn Fn() + Send + Sync>) -> Self {
        SettingsFileOptions {
            poll: POLL,
            shortcuts: None,
            on_change,
            after_rename: None,
        }
    }
}

type Job = Box<dyn FnOnce(&mut Engine) + Send>;

/// The handle of the file thread. The thread lives as long as the handle; when the handle is dropped, the thread is
/// stopped and joined so that it cannot make one more poll of an already removed directory.
pub struct SettingsFile {
    jobs: Option<Sender<Job>>,
    thread: Option<JoinHandle<()>>,
    status: Arc<Mutex<SettingsFileState>>,
    /// Finished polls of the file thread (idle ones too). Only for tests, see [`SettingsFile::polls`].
    polls: Arc<AtomicU64>,
}

impl SettingsFile {
    /// Removes abandoned temporary files, reconciles the file and the database once (synchronously: the file is applied
    /// before the shortcuts are registered and before the MCP listener) and starts the polling thread.
    pub fn start(store: Store, data_dir: PathBuf, options: SettingsFileOptions) -> SettingsFile {
        io::remove_stale_temp(&data_dir);
        let path = io::file_path(&data_dir);
        let status = Arc::new(Mutex::new(SettingsFileState {
            path: path.to_string_lossy().into_owned(),
            ..SettingsFileState::default()
        }));
        let stored_hash = store
            .read(|conn| get_setting(conn, KEY_FILE_HASH).map_err(crate::error::read_err))
            .ok()
            .flatten();
        let mut engine = Engine {
            store,
            dir: data_dir,
            path,
            options,
            status: status.clone(),
            stored_hash,
            last_sig: None,
            pending_write: false,
            writable: false,
            base: None,
            initial: true,
            logged: HashSet::new(),
            write_failures: 0,
            retry_at: None,
        };
        guarded(&mut engine, Engine::reconcile);
        engine.initial = false;
        let (jobs, inbox) = mpsc::channel::<Job>();
        let polls = Arc::new(AtomicU64::new(0));
        let counter = polls.clone();
        let thread = std::thread::Builder::new()
            .name("kinige-settings-file".into())
            .spawn(move || run(engine, inbox, counter))
            .ok();
        SettingsFile {
            jobs: Some(jobs),
            thread,
            status,
            polls,
        }
    }

    /// The file state for the window.
    pub fn state(&self) -> SettingsFileState {
        self.status
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// An edit from the window: in the file thread the external edit is applied first (if the file changed), then
    /// `change`, then the file is written. `None` means the thread stopped or `change` panicked.
    pub fn edit<T: Send + 'static, E: Send + 'static>(
        &self,
        change: impl FnOnce(&Store) -> Result<T, E> + Send + 'static,
    ) -> Option<Result<T, E>> {
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |engine| {
            engine.reconcile();
            let result = change(&engine.store);
            if result.is_ok() {
                engine.after_edit();
            }
            let _ = reply.send(result);
        });
        self.jobs.as_ref()?.send(job).ok()?;
        answer.recv().ok()
    }

    /// How many polls the file thread has finished (a poll that found nothing to do counts too). Tests that assert "the
    /// log did not grow over N polls" wait for this number instead of sleeping.
    #[doc(hidden)]
    pub fn polls(&self) -> u64 {
        self.polls.load(Ordering::SeqCst)
    }

    /// Re-read the file now without waiting for a poll (tests).
    #[doc(hidden)]
    pub fn sync_now(&self) {
        let (reply, answer) = mpsc::channel();
        let job: Job = Box::new(move |engine| {
            engine.reconcile();
            let _ = reply.send(());
        });
        if self
            .jobs
            .as_ref()
            .is_some_and(|jobs| jobs.send(job).is_ok())
        {
            let _ = answer.recv();
        }
    }
}

/// Dropping the handle waits for the file thread to stop. So it must not be dropped on the app's main thread while the
/// file thread waits for `run_on_main_thread` (shortcut registration): the two would wait for each other. Today this is
/// unreachable: the handle is held by `AppHandle`, which lives to the end of the process (a cycle through `AppHandle`),
/// so `drop` is never called in the app.
impl Drop for SettingsFile {
    fn drop(&mut self) {
        self.jobs = None;
        if let Some(thread) = self.thread.take()
            && thread.thread().id() != std::thread::current().id()
        {
            let _ = thread.join();
        }
    }
}

fn run(mut engine: Engine, inbox: mpsc::Receiver<Job>, polls: Arc<AtomicU64>) {
    loop {
        match inbox.recv_timeout(engine.options.poll) {
            Ok(job) => {
                let _ = catch_unwind(AssertUnwindSafe(|| job(&mut engine)));
            }
            Err(RecvTimeoutError::Timeout) => {
                guarded(&mut engine, Engine::tick);
                polls.fetch_add(1, Ordering::SeqCst);
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// A panic in the file thread must not stop it: the error goes to the log (without data), the thread continues.
fn guarded(engine: &mut Engine, step: fn(&mut Engine)) {
    if catch_unwind(AssertUnwindSafe(|| step(engine))).is_err() {
        log::error!("panic in the settings file thread, step skipped");
    }
}

pub(super) struct Engine {
    pub(super) store: Store,
    pub(super) dir: PathBuf,
    pub(super) path: PathBuf,
    pub(super) options: SettingsFileOptions,
    pub(super) status: Arc<Mutex<SettingsFileState>>,
    /// Hash of the bytes the database and the file were last reconciled with (a mirror of `app_setting`).
    pub(super) stored_hash: Option<String>,
    pub(super) last_sig: Option<Sig>,
    /// The file write is deferred (the file is unsafe, unparsed, or the write failed).
    pub(super) pending_write: bool,
    /// The file may be overwritten: it does not exist, or it is safe and parsed.
    pub(super) writable: bool,
    /// The document of the last successful read: foreign keys are taken from it on write.
    pub(super) base: Option<Map<String, Value>>,
    /// App startup: there are no windows yet, shortcuts go straight to the database, no events.
    pub(super) initial: bool,
    /// Warnings already written to the process log (kind and category): a repeat is not written until the same kind has succeeded.
    pub(super) logged: HashSet<String>,
    /// Consecutive failed file writes and when to try again (after 2^n seconds, at least once a minute).
    pub(super) write_failures: u32,
    pub(super) retry_at: Option<Instant>,
}
