//! Telegram import and search (slice 4): main window commands. There are no business rules here (they are in `kinige-domain` and
//! `kinige-store`). These commands are not granted to the capture window (§12.3).
//!
//! The file path is chosen by a system dialog called from Rust and never reaches the frontend. The file is read whole (up to
//! 1 GiB), parsed and waits in the app state under a session number until the import starts or the import window closes.
//! Only one import runs at a time; progress goes as the `import-progress` event only to the `main` window.

use crate::capture::{MAIN_LABEL, notify_entries_changed};
use kinige_domain::telegram::{TgExport, TgParseError, parse_export};
use kinige_domain::{
    ApiError, ErrorCode, FeedCursor, ImportOpened, ImportProgress, ImportReport, SearchFilters,
    SearchPage,
};
use kinige_store::{Now, Store, StoreError};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tauri::{AppHandle, Emitter, Runtime, State};
use tauri_plugin_dialog::DialogExt;

type ApiResult<T> = Result<T, ApiError>;

/// An event for the main window: import progress (`ImportProgress`). Only `main` listens.
pub const IMPORT_PROGRESS: &str = "import-progress";

/// The size limit of the export file: 1 GiB (slice 4 specification, decision 11).
pub const MAX_IMPORT_FILE_BYTES: u64 = 1 << 30;

fn api(error: StoreError) -> ApiError {
    // Only the category goes to the process log: the text of an internal error may contain pieces of data.
    log::error!("store error: {}", error.category());
    error.code().into()
}

#[derive(Default)]
struct Inner {
    /// The number of the next session.
    next_session: u64,
    /// The parsed export, its number and the folder of `result.json` (slice 7: the files of its media lie next to it). The folder
    /// stays in Rust; the window never sees a path.
    session: Option<(u64, Arc<TgExport>, Option<PathBuf>)>,
    /// An import is running.
    running: bool,
}

/// The import state: the parsed export, the "running" flag and the stop flag.
#[derive(Default)]
pub struct ImportState {
    inner: Mutex<Inner>,
    stop: Arc<AtomicBool>,
}

/// While it lives, the import counts as running; on drop it clears the flag.
pub struct RunGuard<'a> {
    state: &'a ImportState,
    pub export: Arc<TgExport>,
    /// The folder of the chosen `result.json`; the media are read from it.
    pub folder: Option<PathBuf>,
}

impl Drop for RunGuard<'_> {
    fn drop(&mut self) {
        self.state.lock().running = false;
    }
}

impl ImportState {
    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Remembers the parsed export and the folder it was read from, the previous one is forgotten. The reply is the session number.
    pub fn open(&self, export: TgExport, folder: Option<PathBuf>) -> u64 {
        let mut inner = self.lock();
        inner.next_session += 1;
        let id = inner.next_session;
        inner.session = Some((id, Arc::new(export), folder));
        id
    }

    /// Starts the import of session `session`: a foreign or forgotten session gives `import_session_expired`, an import already running gives
    /// `import_in_progress`.
    pub fn begin(&self, session: u64) -> Result<RunGuard<'_>, ErrorCode> {
        let mut inner = self.lock();
        if inner.running {
            return Err(ErrorCode::ImportInProgress);
        }
        let (export, folder) = match &inner.session {
            Some((id, export, folder)) if *id == session => (export.clone(), folder.clone()),
            _ => return Err(ErrorCode::ImportSessionExpired),
        };
        inner.running = true;
        self.stop.store(false, Ordering::SeqCst);
        Ok(RunGuard {
            state: self,
            export,
            folder,
        })
    }

    /// The import window is closed: the parsed export is forgotten (a running import holds its own reference and is not interrupted).
    pub fn close(&self) {
        self.lock().session = None;
    }

    /// "Stop": the import finishes the current batch and stops.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.lock().running
    }
}

/// File -> export. Reading: unreadable gives `import_file_unreadable`; over 1 GiB gives `import_file_too_large`; not UTF-8,
/// not JSON or not a Telegram export gives `import_not_telegram`. Parsing is `kinige_domain::telegram`.
pub fn read_export(path: &std::path::Path) -> Result<TgExport, ErrorCode> {
    let size = std::fs::metadata(path)
        .map_err(|_| ErrorCode::ImportFileUnreadable)?
        .len();
    if size > MAX_IMPORT_FILE_BYTES {
        return Err(ErrorCode::ImportFileTooLarge);
    }
    let bytes = std::fs::read(path).map_err(|_| ErrorCode::ImportFileUnreadable)?;
    let text = String::from_utf8(bytes).map_err(|_| ErrorCode::ImportNotTelegram)?;
    parse_export(&text).map_err(|error: TgParseError| {
        // The parse detail may contain a piece of the file: only the kind of error goes to the log.
        let kind = match error {
            TgParseError::NotJson(_) => "not_json",
            TgParseError::NotTelegramExport => "not_telegram",
            TgParseError::Malformed(_) => "malformed",
        };
        log::warn!("import file rejected: {kind}");
        ErrorCode::ImportNotTelegram
    })
}

/// "Choose result.json...": the system dialog (called here, from Rust; the window has no JS permissions for dialogs), reading,
/// parsing, preview. Cancelling the dialog gives `null`, nothing changes.
#[tauri::command(async)]
pub fn import_telegram_open<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    state: State<'_, ImportState>,
) -> ApiResult<Option<ImportOpened>> {
    let picked = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file();
    let Some(picked) = picked else {
        return Ok(None);
    };
    let path = picked
        .into_path()
        .map_err(|_| ApiError::from(ErrorCode::ImportFileUnreadable))?;
    let export = read_export(&path).map_err(ApiError::from)?;
    // The media lie next to `result.json`: the session keeps that folder, in Rust only.
    let folder = path.parent().map(std::path::Path::to_path_buf);
    open_with_folder(&store, &state, export, folder).map(Some)
}

/// Preview of the parsed export and remembering the session (shared by the command and tests); no folder, so no media.
pub fn open_with(store: &Store, state: &ImportState, export: TgExport) -> ApiResult<ImportOpened> {
    open_with_folder(store, state, export, None)
}

/// The same with the folder of `result.json`, from which the media are copied.
pub fn open_with_folder(
    store: &Store,
    state: &ImportState,
    export: TgExport,
    folder: Option<PathBuf>,
) -> ApiResult<ImportOpened> {
    let preview = store.import_preview(&export).map_err(api)?;
    let session = state.open(export, folder);
    Ok(ImportOpened { session, preview })
}

/// "Import": the chosen chats of the session. Returns the outcome; progress goes as the `import-progress` event.
#[tauri::command(async)]
pub fn import_telegram_run<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    state: State<'_, ImportState>,
    session: u64,
    chat_ids: Vec<String>,
) -> ApiResult<ImportReport> {
    let guard = state.begin(session).map_err(ApiError::from)?;
    let sink = app.clone();
    let result = store.import_telegram_with_media(
        guard.export.clone(),
        chat_ids,
        crate::commands::get_language(),
        &Now::system(),
        move |progress: ImportProgress| {
            let _ = sink.emit_to(MAIN_LABEL, IMPORT_PROGRESS, progress);
        },
        state.stop.clone(),
        guard.folder.as_deref(),
    );
    drop(guard);
    // Whatever happens, the entries from earlier batches are already in the database: the window refreshes the feed and the counters.
    notify_entries_changed(&app);
    result.map_err(api)
}

/// "Stop": the current batch is finished, the next one does not start.
#[tauri::command]
pub fn import_telegram_stop(state: State<'_, ImportState>) {
    state.stop();
}

/// The import window is closed: the parsed export is forgotten.
#[tauri::command]
pub fn import_telegram_close(state: State<'_, ImportState>) {
    state.close();
}

/// Search over entries: `query` is the input as is, Rust decides the word form and prefix rules.
#[tauri::command(async)]
pub fn search_messages(
    store: State<'_, Store>,
    query: String,
    filters: Option<SearchFilters>,
    before: Option<FeedCursor>,
    limit: Option<usize>,
) -> ApiResult<SearchPage> {
    store
        .search(
            &query,
            &filters.unwrap_or_default(),
            before.as_ref(),
            limit.unwrap_or(DEFAULT_SEARCH_PAGE),
        )
        .map_err(api)
}

/// Entries per page of search results.
const DEFAULT_SEARCH_PAGE: usize = 50;
