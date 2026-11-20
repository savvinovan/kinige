//! The `sticker` group of an attachment (slice 5c, ADR-0001 appendix §C.10): `{"emoji":<string|null>,"sticker":<bool>}`, `sv = 1`,
//! canonical JCS, written once at creation under the attachment's HLC and never again. This module builds the bytes, reads them back
//! and decides what an entry that holds a sticker is drawn as. Reading is total: it never errors, it does no probe of the file (the
//! device that wrote `"sticker":true` did), and every failure reads as an ordinary picture, which hides nothing.

use super::AttachmentView;
use crate::attachment::AttachmentKind;
use crate::emoji::is_emoji_sequence;
use crate::groups::SCHEMA_VERSION;
use crate::jcs;
use serde_json::{Map, Value, json, value::RawValue};
use std::collections::BTreeMap;

/// The name of the group on an attachment.
pub const STICKER_GROUP: &str = "sticker";
/// The version of the rules that turn the record into the derived columns (what [`read_sticker`] and [`clean_emoji`] let through,
/// and the emoji table the check shares with reactions). The store keeps it in `app_setting.sticker_rules_version` and rebuilds the
/// columns on open when it differs; a fingerprint test fails until it is bumped.
pub const RULES_VERSION: u32 = 1;
/// A sticker's side: Telegram's static stickers have one side of exactly 512 and the other at most 512; the reader accepts 1 to 512.
pub const MAX_SIDE: u32 = 512;
/// The largest sticker file, in bytes.
pub const MAX_BYTES: i64 = 2 * 1024 * 1024;
/// The largest canonical group the reader accepts.
pub const MAX_GROUP_BYTES: usize = 1024;
/// The longest emoji, in bytes.
pub const MAX_EMOJI_BYTES: usize = 40;
/// The file name of a sticker in an entry, without the extension: search finds it by this stem and the Markdown copy writes it.
pub const FILE_STEM: &str = "sticker";

/// The columns of the attachment that the reader checks the group against (the record's `core`, never the file).
#[derive(Debug, Clone, Copy)]
pub struct StickerCore<'a> {
    pub kind: AttachmentKind,
    pub mime: &'a str,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub byte_size: i64,
}

impl StickerCore<'_> {
    /// Whether a sticker may have this core: an image in WebP or PNG, each side 1 to 512, at most 2 MiB.
    pub fn fits(&self) -> bool {
        let side = |value: Option<u32>| value.is_some_and(|v| (1..=MAX_SIDE).contains(&v));
        self.kind == AttachmentKind::Image
            && matches!(self.mime, "image/webp" | "image/png")
            && side(self.width)
            && side(self.height)
            && (0..=MAX_BYTES).contains(&self.byte_size)
    }
}

/// What a record's `sticker` group amounts to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sticker {
    /// Drawn as a sticker (when the entry also has no text and the picture is not covered).
    pub sticker: bool,
    /// The emoji of a sticker, only when it is a valid emoji sequence of at most [`MAX_EMOJI_BYTES`] bytes.
    pub emoji: Option<String>,
}

/// The canonical value of the group. A picture holds `{"emoji":null,"sticker":false}`.
pub fn sticker_group(emoji: Option<&str>, sticker: bool) -> Value {
    json!({"emoji": emoji, "sticker": sticker})
}

/// The emoji as the columns, the view, the copy and MCP may carry it: a valid emoji sequence of at most [`MAX_EMOJI_BYTES`] bytes.
pub fn clean_emoji(text: &str) -> Option<&str> {
    (text.len() <= MAX_EMOJI_BYTES && is_emoji_sequence(text)).then_some(text)
}

/// The sticker facts of a record: a total function of its columns and its two maps (the table of §C.10). Anything that is not exactly a
/// well-formed group of `sv = 1` in both maps, on a core that fits, is a picture.
pub fn read_sticker(core: &StickerCore<'_>, groups_raw: &str, hlc_json: &str) -> Sticker {
    read(core, groups_raw, hlc_json).unwrap_or_default()
}

fn read(core: &StickerCore<'_>, groups_raw: &str, hlc_json: &str) -> Option<Sticker> {
    if !core.fits() {
        return None;
    }
    let groups: BTreeMap<String, Box<RawValue>> = serde_json::from_str(groups_raw).ok()?;
    let hlcs: BTreeMap<String, Value> = serde_json::from_str(hlc_json).ok()?;
    let raw = groups.get(STICKER_GROUP)?.get();
    let supported = hlcs
        .get(STICKER_GROUP)?
        .get("sv")
        .and_then(Value::as_u64)
        .is_some_and(|sv| sv == u64::from(SCHEMA_VERSION));
    if !supported || raw.len() > MAX_GROUP_BYTES {
        return None;
    }
    let value: Value = serde_json::from_str(raw).ok()?;
    if jcs::canonical_string(&value).ok()? != raw {
        return None;
    }
    let object: &Map<String, Value> = value.as_object()?;
    if object.get("sticker")? != &Value::Bool(true) {
        return None;
    }
    let emoji = object
        .get("emoji")
        .and_then(Value::as_str)
        .and_then(clean_emoji)
        .map(str::to_owned);
    Some(Sticker {
        sticker: true,
        emoji,
    })
}

/// The attachment an entry is drawn as, when it is drawn as a sticker: the body is empty, the only live attachment reads as a sticker,
/// and it is not covered (a covered one is drawn as the 7b cover, and can still be uncovered).
pub fn sticker_of<'a>(
    body_md: &str,
    attachments: &'a [AttachmentView],
) -> Option<&'a AttachmentView> {
    match attachments {
        [only] if body_md.is_empty() && only.sticker && !only.spoiler => Some(only),
        _ => None,
    }
}
