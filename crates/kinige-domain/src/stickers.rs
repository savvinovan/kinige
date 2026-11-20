//! The sticker library of slice 5c (PRD §2.3, §4.3; ADR-0001 appendix §C.10, ADR-0021): the types the interface receives, our input rules
//! for a pack link, a bot token, a Telegram `file_id` and `file_path`, and the caps of a download. The Bot API answers are read in
//! [`bot_api`]. No IO: requests, files and rows are in `src-tauri` and `kinige-store`.

pub mod bot_api;

use serde::Serialize;
use ts_rs::TS;

/// The most stickers a pack may hold (a 201st is "the pack is too large"; nothing is added).
pub const MAX_PACK_STICKERS: usize = 200;
/// The largest sticker file, in bytes (checked against `file_size` before the request and counted while streaming).
pub const MAX_STICKER_BYTES: u64 = 2 * 1024 * 1024;
/// The most bytes one pack may add up to.
pub const MAX_PACK_BYTES: u64 = 64 * 1024 * 1024;
/// The longest pack title, in characters.
pub const MAX_TITLE_CHARS: usize = 128;
/// The title of a pack whose own title is empty or unusable.
pub const DEFAULT_TITLE: &str = "Sticker pack";
/// The title of the pack the export import fills.
pub const EXPORT_PACK_TITLE: &str = "From Telegram";
/// How many stickers the Recent strip keeps (`app_setting.recent_stickers`).
pub const MAX_RECENT: usize = 20;
/// The longest a pack name, token, `file_id` and `file_path` may be (the bounds of their rules below).
const MAX_NAME: usize = 64;

/// Where a pack came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "StickerPackSource.ts")]
pub enum PackSource {
    /// The stickers of a Telegram Desktop export: one pack, "From Telegram".
    TelegramExport,
    /// A pack added by its link.
    TelegramLink,
}

impl PackSource {
    pub fn as_str(self) -> &'static str {
        match self {
            PackSource::TelegramExport => "telegram_export",
            PackSource::TelegramLink => "telegram_link",
        }
    }

    pub fn parse(text: &str) -> Option<PackSource> {
        match text {
            "telegram_export" => Some(PackSource::TelegramExport),
            "telegram_link" => Some(PackSource::TelegramLink),
            _ => None,
        }
    }
}

/// A sticker of the library as the panel shows it. It never carries a path or a hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "StickerView.ts")]
pub struct StickerView {
    /// The library id; the picture is `kinige-media://localhost/sticker/<id>`.
    pub id: String,
    pub emoji: Option<String>,
    pub width: u32,
    pub height: u32,
}

/// A pack of the library with its stickers in their order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "StickerPackView.ts")]
pub struct StickerPackView {
    pub id: String,
    pub source: PackSource,
    pub title: String,
    pub stickers: Vec<StickerView>,
}

/// What the Stickers tab shows: the recent stickers (the most recently sent first), the packs in the order added, and whether a bot
/// token is saved (a flag only; the token is never read for this).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "StickerLibrary.ts")]
pub struct StickerLibrary {
    pub recent: Vec<StickerView>,
    pub packs: Vec<StickerPackView>,
    pub has_token: bool,
}

/// One GIF of the journal for the GIFs tab: a live, uncovered `image/gif` attachment, one per content hash, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "JournalGif.ts")]
pub struct JournalGif {
    pub attachment_id: String,
    pub width: u32,
    pub height: u32,
    #[ts(type = "number")]
    pub byte_size: i64,
}

/// The progress of a pack download: counts only (never a name, a title or a file id).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "StickerPackProgress.ts")]
pub struct StickerPackProgress {
    pub done: u32,
    pub total: u32,
}

/// What a finished pack download added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "StickerPackAdded.ts")]
pub struct StickerPackAdded {
    pub title: String,
    pub added: u32,
    /// Animated, video and unreadable files that were left out.
    pub skipped: u32,
}

/// What Settings knows about the bot token: whether one is saved, and after a "Remove" whether the system refused to delete the old item.
/// The token never comes back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "TelegramTokenState.ts")]
pub struct TelegramTokenState {
    pub saved: bool,
    /// Only the answer of "Remove": the system refused to delete the item (it asked for access and was denied), so the old token is still in
    /// the keychain although Kinige no longer uses it. Settings then says so, in one line.
    pub item_left: bool,
}

/// The pack name of a link: `https://t.me/addstickers/<name>`, `t.me/addstickers/<name>` or the bare name. Our rule, not Telegram's
/// (`^[A-Za-z0-9_]{1,64}$`; Telegram's own are not verified). Anything else, a query or a second segment included, is `None`.
pub fn pack_name(input: &str) -> Option<&str> {
    let text = input.trim();
    let name = ["https://t.me/addstickers/", "t.me/addstickers/"]
        .iter()
        .find_map(|prefix| text.strip_prefix(prefix))
        .unwrap_or(text);
    is_pack_name(name).then_some(name)
}

/// `^[A-Za-z0-9_]{1,64}$`.
pub fn is_pack_name(name: &str) -> bool {
    (1..=MAX_NAME).contains(&name.len())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The bot token: `^[0-9]{1,20}:[A-Za-z0-9_-]{1,100}$` (our rule; Telegram's format is not verified).
pub fn is_token(token: &str) -> bool {
    let Some((id, secret)) = token.split_once(':') else {
        return false;
    };
    (1..=20).contains(&id.len())
        && id.bytes().all(|b| b.is_ascii_digit())
        && (1..=100).contains(&secret.len())
        && secret
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `^[A-Za-z0-9_-]{1,256}$`: a `file_id` goes into a query value, and nothing else may.
pub fn is_file_id(id: &str) -> bool {
    (1..=256).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// `^[A-Za-z0-9_./-]{1,256}$` with no `..` segment, no empty segment and no leading `/`: a `file_path` goes into the path of a URL.
pub fn is_file_path(path: &str) -> bool {
    (1..=256).contains(&path.len())
        && path
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'-'))
        && !path.starts_with('/')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "..")
}

/// A pack title as the library keeps it: the file-name rules of slice 7 (no path, control or bidi characters), at most
/// [`MAX_TITLE_CHARS`] characters, and [`DEFAULT_TITLE`] when nothing is left.
pub fn clean_title(raw: &str) -> String {
    let name = crate::attachment::sanitize_filename(raw);
    let name = name.trim();
    if name.is_empty() || name == "file" && raw.trim() != "file" {
        return DEFAULT_TITLE.to_string();
    }
    name.chars().take(MAX_TITLE_CHARS).collect()
}

#[cfg(test)]
mod tests;
