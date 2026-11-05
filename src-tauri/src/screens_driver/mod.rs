//! The screens driver (KINIG-92, ADR-0016): the real-window scenarios' hands inside the app, behind the cargo feature `screens-driver`.
//! The module is compiled only with the feature, `build.rs` refuses the feature without `KINIGE_ALLOW_SCREENS_DRIVER=1`, and the
//! command and its capability exist only in that build. Without `KINIGE_SCREENS_DRIVER_DIR` the driver stays idle.
//!
//! The channel is files, no ports: a script writes `requests/<id>.json` (a temporary file, then a rename); a thread polls the directory
//! every 100 ms and sends the step to that window as an app event; the page runs it (`scripts/screens/driver.ts`) and answers through
//! `screens_driver_reply`; the answer becomes `replies/<id>.json`. Only `front` is done here, by the app itself.
//! Garbage in a request gets an error reply (or is dropped with a log line of its category) and never writes outside the directory.

mod request;

use crate::capture::{MAIN_LABEL, show_main};
use request::{
    MAX_REPLY_TEXT_BYTES, Message, REQUESTS_DIR, Reject, Reply, Request, Step, id_of_file_name,
    parse_request, read_request_file, valid_id, write_reply,
};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, Webview, Window, WindowEvent};

/// The marker `scripts/check-test-hooks.sh` searches the executable for (the same literal as in `scripts/screens/driver.ts`).
pub const MARKER: &str = "kinige-screens-driver-92";
/// The environment variable with the driver directory (`requests/` and `replies/` are made inside it).
pub const ENV_DIR: &str = "KINIGE_SCREENS_DRIVER_DIR";
/// The app event a window gets a step in; the payload is `{ "id", "step" }`.
pub const STEP_EVENT: &str = "screens-driver-step";

const POLL: Duration = Duration::from_millis(100);
const MAX_PENDING: usize = 256;
const PENDING_TTL: Duration = Duration::from_secs(600);
const MAX_ERROR_CHARS: usize = 1024;

/// The driver's state: the directory (none when idle), the windows whose page listens, the steps sent and not yet answered.
pub struct DriverState {
    dir: Option<PathBuf>,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    ready: HashSet<String>,
    pending: HashMap<String, Pending>,
}

struct Pending {
    window: String,
    since: Instant,
}

impl DriverState {
    pub fn new(dir: Option<PathBuf>) -> Self {
        DriverState {
            dir,
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn mark_ready(&self, label: &str) {
        self.lock().ready.insert(label.to_string());
    }

    fn mark_gone(&self, label: &str) {
        self.lock().ready.remove(label);
    }

    fn is_ready(&self, label: &str) -> bool {
        self.lock().ready.contains(label)
    }

    /// Records a step sent to `label`; false when the id is in use or too many steps wait for an answer.
    fn add_pending(&self, id: &str, label: &str) -> bool {
        let mut inner = self.lock();
        inner
            .pending
            .retain(|_, pending| pending.since.elapsed() < PENDING_TTL);
        if inner.pending.len() >= MAX_PENDING || inner.pending.contains_key(id) {
            return false;
        }
        inner.pending.insert(
            id.to_string(),
            Pending {
                window: label.to_string(),
                since: Instant::now(),
            },
        );
        true
    }

    /// Takes the pending step of `id` if it was sent to `label`: another window cannot answer for it.
    fn take_pending(&self, id: &str, label: &str) -> bool {
        let mut inner = self.lock();
        match inner.pending.get(id) {
            Some(pending) if pending.window == label => {
                inner.pending.remove(id);
                true
            }
            _ => false,
        }
    }

    fn forget_pending(&self, id: &str) {
        self.lock().pending.remove(id);
    }
}

/// Starts the driver: the state is always managed (the command needs it), the thread only when `KINIGE_SCREENS_DRIVER_DIR` is set.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let dir = std::env::var_os(ENV_DIR)
        .map(PathBuf::from)
        .filter(|dir| fs::create_dir_all(dir.join(REQUESTS_DIR)).is_ok());
    let active = dir.is_some();
    app.manage(DriverState::new(dir));
    if !active {
        log::info!("screens driver compiled in, idle ({MARKER})");
        return;
    }
    let handle = app.clone();
    let spawned = thread::Builder::new()
        .name("kinige-screens-driver".into())
        .spawn(move || {
            loop {
                thread::sleep(POLL);
                if let Some(state) = handle.try_state::<DriverState>() {
                    tick(&handle, &state);
                }
            }
        });
    match spawned {
        Ok(_) => log::info!("screens driver on ({MARKER})"),
        Err(_) => log::error!("screens driver: could not start the thread"),
    }
}

/// A window that is gone must send `ready` again when its page is back.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if let WindowEvent::Destroyed = event
        && let Some(state) = window.app_handle().try_state::<DriverState>()
    {
        state.mark_gone(window.label());
    }
}

/// What a page sends back: `ready` once it listens, then one `reply` per step. Only a window that was sent the step may answer it.
#[tauri::command]
pub fn screens_driver_reply<R: Runtime>(
    message: Message,
    webview: Webview<R>,
    state: State<'_, DriverState>,
) -> Result<(), String> {
    match message {
        Message::Ready => {
            state.mark_ready(webview.label());
            Ok(())
        }
        Message::Reply {
            id,
            ok,
            value,
            error,
        } => {
            let Some(dir) = state.dir.as_deref() else {
                return Err("idle".into());
            };
            if !valid_id(&id) || !state.take_pending(&id, webview.label()) {
                log::debug!("screens driver: unexpected reply");
                return Err("unexpected reply".into());
            }
            let reply = if value
                .as_ref()
                .is_some_and(|v| v.len() > MAX_REPLY_TEXT_BYTES)
            {
                Reply::error(&id, "the reply is too large")
            } else if ok {
                Reply::ok(&id, value)
            } else {
                let text = error.unwrap_or_else(|| "the step failed".into());
                Reply::error(&id, text.chars().take(MAX_ERROR_CHARS).collect::<String>())
            };
            write_reply(dir, &reply).map(drop).map_err(|_| {
                log::warn!("screens driver: could not write a reply");
                "could not write the reply".to_string()
            })
        }
    }
}

/// One pass over `requests/`: every request file is answered, sent to its window, or (for a window whose page is not listening yet)
/// left for the next pass. Public so that the tests can drive it without the thread.
pub fn tick<R: Runtime>(app: &AppHandle<R>, state: &DriverState) {
    let Some(dir) = state.dir.as_deref() else {
        return;
    };
    let Ok(entries) = fs::read_dir(dir.join(REQUESTS_DIR)) else {
        return;
    };
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            drop_stray(&entry.path());
            continue;
        };
        // A script's temporary file (it is renamed when complete) is not ours yet.
        if name.starts_with('.') || name.ends_with(".tmp") {
            continue;
        }
        match id_of_file_name(name) {
            Some(id) => files.push((id.to_string(), entry.path())),
            None => drop_stray(&entry.path()),
        }
    }
    files.sort();
    for (id, path) in files {
        handle_request(app, state, dir, &id, &path);
    }
}

fn handle_request<R: Runtime>(
    app: &AppHandle<R>,
    state: &DriverState,
    dir: &Path,
    id: &str,
    path: &Path,
) {
    let request = match read_request_file(path).and_then(|bytes| parse_request(id, &bytes)) {
        Ok(request) => request,
        Err(reject) => {
            refuse(dir, id, path, reject);
            return;
        }
    };
    let Request { window, step, .. } = request;
    let label = window.label();
    let Some(webview) = app.get_webview_window(label) else {
        answer(dir, path, Reply::error(id, "no such window"));
        return;
    };
    if matches!(step, Step::Front {}) {
        if label == MAIN_LABEL {
            show_main(app);
        } else {
            let _ = webview.unminimize();
            let _ = webview.show();
            let _ = webview.set_focus();
        }
        answer(dir, path, Reply::ok(id, None));
        return;
    }
    // The page has not said it listens (it is still loading): the request stays where it is and the next pass tries again.
    if !state.is_ready(label) {
        return;
    }
    if !state.add_pending(id, label) {
        answer(
            dir,
            path,
            Reply::error(id, "duplicate id or too many steps"),
        );
        return;
    }
    // The file goes before the step does, so that a step is never run twice.
    if fs::remove_file(path).is_err() {
        state.forget_pending(id);
        log::warn!("screens driver: could not take a request file");
        let _ = write_reply(dir, &Reply::error(id, "could not take the request"));
        return;
    }
    if app
        // The window is in the payload: tauri hands `emit_to` events to every `Any` listener, and each page drops what is not its own.
        .emit_to(
            label,
            STEP_EVENT,
            json!({ "id": id, "window": label, "step": step }),
        )
        .is_err()
    {
        state.forget_pending(id);
        let _ = write_reply(dir, &Reply::error(id, "could not send the step"));
    }
}

/// An error reply for a request that cannot be run, and the file taken away.
fn refuse(dir: &Path, id: &str, path: &Path, reject: Reject) {
    log::debug!("screens driver: request refused: {}", reject.category());
    answer(dir, path, Reply::error(id, reject.category()));
}

/// Writes the reply and removes the request file.
fn answer(dir: &Path, path: &Path, reply: Reply) {
    if write_reply(dir, &reply).is_err() {
        log::warn!("screens driver: could not write a reply");
    }
    let _ = fs::remove_file(path);
}

/// A file in `requests/` that is not `<id>.json`: dropped with a log line. Only a file is removed (a link is removed, not followed).
fn drop_stray(path: &Path) {
    if fs::symlink_metadata(path).is_ok_and(|meta| !meta.is_dir()) {
        log::debug!(
            "screens driver: request refused: {}",
            Reject::NotAFile.category()
        );
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
