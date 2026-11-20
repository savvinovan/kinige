//! Stickers and GIFs (slice 5c, PRD §2.3, §12.3, ADR-0021). Commands of the main window: the library, adding a pack by its link, sending a
//! sticker, the GIFs of the journal. Commands of the settings window, and only there: the bot token. There are no business rules here (they
//! are in `kinige-domain` and `kinige-store`); the token lives in the OS secret store (`secret`) and the one outbound request of the app is
//! `client`. Until a token is saved Kinige makes no network request, and the token is read only when a pack is downloaded (`job`).

mod client;
mod job;
pub mod pack_command;
pub mod secret;

pub use client::{BotClient, BotError};
pub use job::{JobContext, run as run_job};

use crate::capture::notify_entries_changed;
use kinige_domain::stickers::{JournalGif, StickerLibrary, TelegramTokenState, is_token};
use kinige_domain::{ApiError, ErrorCode, MessageView};
use kinige_store::{Now, Store, StoreError};
use secret::{SecretError, SecretStore};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, Runtime, State};

type ApiResult<T> = Result<T, ApiError>;

/// An event for the main window: the progress of a pack download (`StickerPackProgress`, counts only). Only `main` listens.
pub const STICKER_PACK_PROGRESS: &str = "sticker-pack-progress";

/// Builds the client when a job first needs it, so that opening the app costs nothing.
type ClientFactory = Box<dyn Fn() -> Result<BotClient, BotError> + Send + Sync>;

/// The state of the feature: the token store, the client (made on first use) and the one pack job that may run.
pub struct StickerShell {
    secrets: Arc<dyn SecretStore>,
    factory: ClientFactory,
    client: Mutex<Option<Arc<BotClient>>>,
    job: Mutex<Option<Arc<AtomicBool>>>,
}

impl StickerShell {
    /// The real store and the real API (or the overrides of the `test-hooks` feature).
    pub fn production() -> StickerShell {
        StickerShell::new(secret::production(), Box::new(BotClient::production))
    }

    pub fn new(secrets: Arc<dyn SecretStore>, factory: ClientFactory) -> StickerShell {
        StickerShell {
            secrets,
            factory,
            client: Mutex::new(None),
            job: Mutex::new(None),
        }
    }

    fn client(&self) -> Result<Arc<BotClient>, BotError> {
        let mut slot = self.client.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(client) = &*slot {
            return Ok(client.clone());
        }
        let client = Arc::new((self.factory)()?);
        *slot = Some(client.clone());
        Ok(client)
    }

    /// The slot of the one job; `None` when a job is running.
    fn begin(&self) -> Option<JobSlot<'_>> {
        let mut slot = self.job.lock().unwrap_or_else(PoisonError::into_inner);
        if slot.is_some() {
            return None;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        *slot = Some(cancel.clone());
        Some(JobSlot {
            shell: self,
            cancel,
        })
    }

    /// Cancel is idempotent: with no job it does nothing.
    pub fn cancel(&self) {
        if let Some(cancel) = &*self.job.lock().unwrap_or_else(PoisonError::into_inner) {
            cancel.store(true, Ordering::SeqCst);
        }
    }
}

/// Holds the job slot while a job runs.
struct JobSlot<'a> {
    shell: &'a StickerShell,
    cancel: Arc<AtomicBool>,
}

impl Drop for JobSlot<'_> {
    fn drop(&mut self) {
        *self
            .shell
            .job
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = None;
    }
}

fn api(error: StoreError) -> ApiError {
    // Only the category goes to the process log: never a pack name, a title or a path.
    log::error!("store error: {}", error.category());
    error.code().into()
}

/// The Stickers tab: the recent stickers, the packs, and whether a token is saved (a flag; the secret store is not touched).
#[tauri::command(async)]
pub fn get_sticker_library(store: State<'_, Store>) -> ApiResult<StickerLibrary> {
    let saved = store.telegram_token_saved().unwrap_or(false);
    store.sticker_library(saved).map_err(api)
}

/// "Cancel" in the add-pack dialog. Idempotent.
#[tauri::command]
pub fn cancel_sticker_pack(shell: State<'_, StickerShell>) {
    shell.cancel();
}

/// "Remove pack": the stickers already sent stay in their entries.
#[tauri::command(async)]
pub fn remove_sticker_pack(store: State<'_, Store>, pack_id: String) -> ApiResult<()> {
    store.remove_sticker_pack(&pack_id).map_err(api)
}

/// A click on a sticker: one new entry at once. The text draft and the tiles stay; a pending reply goes with it.
#[tauri::command(async)]
pub fn send_sticker<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    chat_id: String,
    sticker_id: String,
    reply_to_id: Option<String>,
) -> ApiResult<MessageView> {
    let message = store
        .send_sticker(
            &chat_id,
            &sticker_id,
            reply_to_id.as_deref(),
            &Now::system(),
        )
        .map_err(api)?;
    notify_entries_changed(&app);
    Ok(message)
}

/// The GIFs tab: the GIFs of the journal, newest first, one per content.
#[tauri::command(async)]
pub fn list_journal_gifs(store: State<'_, Store>) -> ApiResult<Vec<JournalGif>> {
    store.list_journal_gifs().map_err(api)
}

/// "Send again" from the GIFs tab.
#[tauri::command(async)]
pub fn send_gif_again<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    chat_id: String,
    attachment_id: String,
    reply_to_id: Option<String>,
) -> ApiResult<MessageView> {
    let message = store
        .send_gif_again(
            &chat_id,
            &attachment_id,
            reply_to_id.as_deref(),
            &Now::system(),
        )
        .map_err(api)?;
    notify_entries_changed(&app);
    Ok(message)
}

/// Settings: whether a bot token is saved. A flag in `app_setting`; the secret store is not touched (a read may prompt).
#[tauri::command(async)]
pub fn get_telegram_token_state(store: State<'_, Store>) -> ApiResult<TelegramTokenState> {
    Ok(TelegramTokenState {
        saved: store.telegram_token_saved().map_err(api)?,
        item_left: false,
    })
}

/// Settings: saves the token in the OS secret store. What the system does when another build made the item is not verified in full
/// (ADR-0021, "not verified"). The token is never logged, echoed or written anywhere else; a wrong shape is `telegram_token_invalid`.
#[tauri::command(async)]
pub fn set_telegram_bot_token(
    store: State<'_, Store>,
    shell: State<'_, StickerShell>,
    token: String,
) -> ApiResult<TelegramTokenState> {
    let token = token.trim();
    if !is_token(token) {
        return Err(ErrorCode::TelegramTokenInvalid.into());
    }
    // `(async)` runs this on a pool thread, never the main thread: a system prompt may block here.
    shell.secrets.set(token).map_err(secret_error)?;
    store.set_telegram_token_saved(true).map_err(api)?;
    Ok(TelegramTokenState {
        saved: true,
        item_left: false,
    })
}

/// Settings: "Remove". The flag goes first and the item is always deleted, whatever the flag said. When the system refuses the delete (it
/// asked for access and was denied, or the item belongs to another build), the command still succeeds, because Kinige no longer uses the
/// token, and the answer says `item_left`: the old token is still in the keychain, and Settings tells the user where to delete it.
#[tauri::command(async)]
pub fn clear_telegram_bot_token(
    store: State<'_, Store>,
    shell: State<'_, StickerShell>,
) -> ApiResult<TelegramTokenState> {
    store.set_telegram_token_saved(false).map_err(api)?;
    match shell.secrets.delete() {
        Ok(()) => Ok(TelegramTokenState {
            saved: false,
            item_left: false,
        }),
        Err(SecretError::Denied) => Ok(TelegramTokenState {
            saved: false,
            item_left: true,
        }),
        // No store on this system: there is no item to delete, and nothing was ever saved.
        Err(SecretError::Unsupported) => Ok(TelegramTokenState {
            saved: false,
            item_left: false,
        }),
        Err(other) => Err(secret_error(other).into()),
    }
}

fn secret_error(error: SecretError) -> ErrorCode {
    match error {
        SecretError::Denied => ErrorCode::SecretStoreDenied,
        SecretError::Unavailable => ErrorCode::SecretStoreUnavailable,
        SecretError::Unsupported => ErrorCode::SecretStoreUnsupported,
    }
}
