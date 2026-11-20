//! One pack download (slice 5c, ADR-0021): read the token once, `getStickerSet`, then `getFile` and one download per static sticker, one
//! after another, into a staging folder, then one store call writes the pack all or nothing. Only the job reads the token; a dismissed
//! prompt ends it with its own error and writes nothing. Cancel and every error remove the staging folder (it is dropped).

use super::client::{BotClient, BotError};
use super::secret::{SecretError, SecretStore};
use kinige_domain::stickers::bot_api::{ReadError, read_file_path, read_sticker_set};
use kinige_domain::stickers::{
    MAX_PACK_BYTES, MAX_STICKER_BYTES, PackSource, StickerPackAdded, StickerPackProgress, is_token,
    pack_name,
};
use kinige_domain::{ApiError, ErrorCode};
use kinige_store::{NewPack, PackItem, Store};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The whole job: ten minutes.
const JOB_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Everything a job needs, so that a test can run one without a window.
pub struct JobContext {
    pub store: Store,
    pub client: Arc<BotClient>,
    pub secrets: Arc<dyn SecretStore>,
    pub cancel: Arc<AtomicBool>,
    /// Counts only: `done` of `total` files.
    pub progress: Arc<dyn Fn(StickerPackProgress) + Send + Sync>,
}

/// Runs the job for `link`. Nothing is added on any error, and the error is a category, never a text.
pub async fn run(ctx: &JobContext, link: &str) -> Result<StickerPackAdded, ApiError> {
    let name = pack_name(link)
        .ok_or(ErrorCode::StickerPackLinkInvalid)?
        .to_string();
    let token = read_token(ctx).await?;
    // The read may have waited for a system prompt, and "Cancel" may have been pressed meanwhile: then not one request is made.
    if ctx.cancel.load(Ordering::SeqCst) {
        return Err(ErrorCode::StickerPackCancelled.into());
    }
    match tokio::time::timeout(JOB_TIMEOUT, download_and_write(ctx, &token, &name)).await {
        Ok(result) => result,
        Err(_) => Err(ErrorCode::StickerPackUnreachable.into()),
    }
}

/// The one read of the token, off the main thread (it may wait for a system prompt). **The flag of Settings comes first**: with it off (never saved, or
/// "Remove" was pressed, whatever the keychain still holds) the job reads nothing and ends as "no token", so "Remove" is final. A missing token also
/// corrects the flag.
async fn read_token(ctx: &JobContext) -> Result<String, ApiError> {
    let saved = ctx.store.telegram_token_saved().map_err(store_error)?;
    if !saved {
        return Err(ErrorCode::StickerPackNoToken.into());
    }
    let secrets = ctx.secrets.clone();
    let read = tauri::async_runtime::spawn_blocking(move || secrets.get())
        .await
        .map_err(|_| ErrorCode::Internal)?;
    let flag = |saved: bool| {
        let _ = ctx.store.set_telegram_token_saved(saved);
    };
    match read {
        Ok(Some(token)) if is_token(&token) => {
            flag(true);
            Ok(token)
        }
        Ok(_) => {
            flag(false);
            Err(ErrorCode::StickerPackNoToken.into())
        }
        Err(SecretError::Denied) => Err(ErrorCode::SecretStoreDenied.into()),
        Err(SecretError::Unavailable) => Err(ErrorCode::SecretStoreUnavailable.into()),
        Err(SecretError::Unsupported) => Err(ErrorCode::SecretStoreUnsupported.into()),
    }
}

async fn download_and_write(
    ctx: &JobContext,
    token: &str,
    name: &str,
) -> Result<StickerPackAdded, ApiError> {
    let body = ctx
        .client
        .get_sticker_set(token, name)
        .await
        .map_err(set_error)?;
    let set = read_sticker_set(&body).map_err(|error| match error {
        ReadError::TooLarge => ErrorCode::StickerPackTooLarge,
        ReadError::OnlyAnimated => ErrorCode::StickerPackOnlyAnimated,
        ReadError::BadResponse => ErrorCode::StickerPackBadResponse,
    })?;
    let stage = ctx.store.pack_stage().map_err(store_error)?;
    let total = u32::try_from(set.stickers.len()).unwrap_or(u32::MAX);
    (ctx.progress)(StickerPackProgress { done: 0, total });
    let (mut items, mut skipped, mut bytes) = (Vec::new(), set.skipped, 0u64);
    for (index, sticker) in set.stickers.iter().enumerate() {
        if ctx.cancel.load(Ordering::SeqCst) {
            return Err(ErrorCode::StickerPackCancelled.into());
        }
        match fetch(ctx, token, &sticker.file_id).await {
            Ok(file) => {
                bytes = bytes.saturating_add(u64::try_from(file.len()).unwrap_or(u64::MAX));
                if bytes > MAX_PACK_BYTES {
                    return Err(ErrorCode::StickerPackTooLarge.into());
                }
                if write_file(&stage, index, &file).is_ok() {
                    items.push(PackItem {
                        index,
                        emoji: sticker.emoji.clone(),
                    });
                } else {
                    skipped = skipped.saturating_add(1);
                }
            }
            // A file that is gone or odd is left out; the token, the network and the rate limit stop the whole job.
            Err(BotError::NotFound | BotError::Bad | BotError::TooLarge) => {
                skipped = skipped.saturating_add(1);
            }
            Err(other) => return Err(set_error(other)),
        }
        (ctx.progress)(StickerPackProgress {
            done: u32::try_from(index + 1).unwrap_or(u32::MAX),
            total,
        });
    }
    if ctx.cancel.load(Ordering::SeqCst) {
        return Err(ErrorCode::StickerPackCancelled.into());
    }
    let store = ctx.store.clone();
    let pack = NewPack {
        source: PackSource::TelegramLink,
        link_name: Some(name.to_string()),
        title: set.title,
        items,
    };
    let added = tauri::async_runtime::spawn_blocking(move || store.add_sticker_pack(stage, pack))
        .await
        .map_err(|_| ErrorCode::Internal)?
        .map_err(|error| match error {
            // Not one file of the pack passed the probe: what came is not what the set said.
            kinige_store::StoreError::StickerUnreadable => ErrorCode::StickerPackBadResponse,
            other => other.code(),
        })?;
    Ok(StickerPackAdded {
        skipped: added.skipped.saturating_add(skipped),
        ..added
    })
}

/// `getFile`, then the download of its path.
async fn fetch(ctx: &JobContext, token: &str, file_id: &str) -> Result<Vec<u8>, BotError> {
    let answer = ctx.client.get_file(token, file_id).await?;
    let path = read_file_path(&answer).map_err(|_| BotError::Bad)?;
    ctx.client.download(token, &path, MAX_STICKER_BYTES).await
}

fn write_file(stage: &kinige_store::PackStage, index: usize, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = stage.create(index)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The error of a call whose failure ends the job.
fn set_error(error: BotError) -> ApiError {
    match error {
        BotError::TokenRefused => ErrorCode::StickerPackTokenRefused,
        BotError::NotFound => ErrorCode::StickerPackNotFound,
        BotError::Wait => ErrorCode::StickerPackWait,
        BotError::Unreachable => ErrorCode::StickerPackUnreachable,
        BotError::Bad => ErrorCode::StickerPackBadResponse,
        BotError::TooLarge => ErrorCode::StickerPackTooLarge,
    }
    .into()
}

fn store_error(error: kinige_store::StoreError) -> ApiError {
    log::error!("store error: {}", error.category());
    error.code().into()
}
