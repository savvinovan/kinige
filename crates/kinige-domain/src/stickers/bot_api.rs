//! The two Bot API answers slice 5c reads (ADR-0021), as pure functions of the body bytes: `getStickerSet` and `getFile`. The body is
//! untrusted input: it is read into a validated structure, unknown fields are ignored, and nothing from it reaches an error text.
//! The HTTP status decides errors (`src-tauri`); these functions see only a body that came with a success status.

use super::{
    MAX_PACK_BYTES, MAX_PACK_STICKERS, MAX_STICKER_BYTES, clean_title, is_file_id, is_file_path,
};
use crate::attachment::sticker::{MAX_SIDE, clean_emoji};
use serde_json::Value;
use std::collections::HashSet;

/// Why a body was not accepted. A category only: the reason never carries a value of the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadError {
    /// Not JSON, not an object, `ok` is not `true`, or a required field is missing or mistyped.
    BadResponse,
    /// More than [`MAX_PACK_STICKERS`] stickers, or the files add up to more than [`MAX_PACK_BYTES`].
    TooLarge,
    /// Every sticker of the pack is animated or video.
    OnlyAnimated,
}

/// A pack as Telegram describes it, reduced to what the download needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickerSet {
    /// Already cleaned ([`clean_title`]).
    pub title: String,
    /// The static stickers worth a request, in the pack's order, one per `file_id`.
    pub stickers: Vec<SetSticker>,
    /// Animated, video and unusable stickers that were left out (they are counted, never downloaded).
    pub skipped: u32,
}

/// A static sticker of a pack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SetSticker {
    pub file_id: String,
    /// Only a valid emoji sequence.
    pub emoji: Option<String>,
    /// What Telegram says; the bytes are counted again while streaming.
    pub file_size: Option<u64>,
}

/// Reads the body of `getStickerSet`.
pub fn read_sticker_set(body: &[u8]) -> Result<StickerSet, ReadError> {
    let result = result_of(body)?;
    let title = clean_title(
        result
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    );
    let items = result
        .get("stickers")
        .and_then(Value::as_array)
        .ok_or(ReadError::BadResponse)?;
    if items.len() > MAX_PACK_STICKERS {
        return Err(ReadError::TooLarge);
    }
    let (mut stickers, mut skipped, mut animated) = (Vec::new(), 0u32, 0u32);
    let mut seen = HashSet::new();
    let mut total = 0u64;
    for item in items {
        match classify(item) {
            Kind::Static(sticker) => {
                if !seen.insert(sticker.file_id.clone()) {
                    continue;
                }
                total = total.saturating_add(sticker.file_size.unwrap_or(0));
                stickers.push(sticker);
            }
            Kind::Animated => {
                animated = animated.saturating_add(1);
                skipped = skipped.saturating_add(1);
            }
            Kind::Unusable => skipped = skipped.saturating_add(1),
        }
    }
    if total > MAX_PACK_BYTES {
        return Err(ReadError::TooLarge);
    }
    if stickers.is_empty() {
        return Err(if animated > 0 {
            ReadError::OnlyAnimated
        } else {
            ReadError::BadResponse
        });
    }
    Ok(StickerSet {
        title,
        stickers,
        skipped,
    })
}

/// Reads the body of `getFile`: the `file_path` that goes after `/file/bot<token>/`, only when it is safe in a URL path.
pub fn read_file_path(body: &[u8]) -> Result<String, ReadError> {
    let result = result_of(body)?;
    result
        .get("file_path")
        .and_then(Value::as_str)
        .filter(|path| is_file_path(path))
        .map(str::to_owned)
        .ok_or(ReadError::BadResponse)
}

enum Kind {
    Static(SetSticker),
    Animated,
    Unusable,
}

fn classify(item: &Value) -> Kind {
    let flag = |key: &str| match item.get(key) {
        None => Some(false),
        Some(Value::Bool(value)) => Some(*value),
        Some(_) => None,
    };
    let (Some(animated), Some(video)) = (flag("is_animated"), flag("is_video")) else {
        return Kind::Unusable;
    };
    if animated || video {
        return Kind::Animated;
    }
    let side = |key: &str| {
        item.get(key)
            .and_then(Value::as_u64)
            .is_some_and(|v| (1..=u64::from(MAX_SIDE)).contains(&v))
    };
    let file_id = item
        .get("file_id")
        .and_then(Value::as_str)
        .filter(|id| is_file_id(id));
    let file_size = match item.get("file_size") {
        None | Some(Value::Null) => Some(None),
        Some(value) => value.as_u64().map(Some),
    };
    match (file_id, file_size) {
        (Some(file_id), Some(file_size))
            if side("width")
                && side("height")
                && file_size.is_none_or(|n| n <= MAX_STICKER_BYTES) =>
        {
            Kind::Static(SetSticker {
                file_id: file_id.to_string(),
                emoji: item
                    .get("emoji")
                    .and_then(Value::as_str)
                    .and_then(clean_emoji)
                    .map(str::to_owned),
                file_size,
            })
        }
        _ => Kind::Unusable,
    }
}

/// The `result` object of an answer whose `ok` is `true`.
fn result_of(body: &[u8]) -> Result<serde_json::Map<String, Value>, ReadError> {
    let Ok(Value::Object(mut answer)) = serde_json::from_slice::<Value>(body) else {
        return Err(ReadError::BadResponse);
    };
    if answer.get("ok") != Some(&Value::Bool(true)) {
        return Err(ReadError::BadResponse);
    }
    match answer.remove("result") {
        Some(Value::Object(result)) => Ok(result),
        _ => Err(ReadError::BadResponse),
    }
}

#[cfg(test)]
mod tests;
