//! Attachments (slice 7, PRD §4.2, §4.3; ADR-0014, ADR-0001 appendix §C.6): kinds, file names, the extension rules, the
//! `core` group of an attachment and the types the interface receives. No IO: reading files, hashing and decoding images are in
//! `kinige-store`.

use crate::groups::{GroupError, SCHEMA_VERSION, assemble};
use crate::hlc::is_uuid_v4;
use serde::Serialize;
use serde_json::json;
use ts_rs::TS;

mod names;
pub use names::{
    classify, is_openable_document, mirror_file_names, sanitize_filename, stored_ext, summary_of,
};

/// The largest attachment: 2 GiB (the free Telegram limit; the user's decision of 03.11.2026).
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
/// The most attachments in one entry.
pub const MAX_PER_ENTRY: usize = 10;
/// The largest pasted image or file: it crosses IPC as one raw body.
pub const MAX_PASTE_BYTES: usize = 64 * 1024 * 1024;
/// The largest file `kinige-media://…/full/…` serves: the responder holds the whole body in memory (ADR-0014).
pub const MAX_FULL_BYTES: u64 = 64 * 1024 * 1024;
/// The longest `filename` in bytes (ADR-0001 appendix §C.6).
pub const MAX_FILENAME_BYTES: usize = 255;

/// What an attachment is. `link` comes with the P1 previews.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "AttachmentKind.ts")]
pub enum AttachmentKind {
    /// JPEG, PNG, WebP or GIF whose header decodes: a thumbnail in the feed.
    Image,
    /// Anything else: a file card.
    File,
    Audio,
    Video,
}

impl AttachmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            AttachmentKind::Image => "image",
            AttachmentKind::File => "file",
            AttachmentKind::Audio => "audio",
            AttachmentKind::Video => "video",
        }
    }

    pub fn parse(text: &str) -> Option<AttachmentKind> {
        match text {
            "image" => Some(AttachmentKind::Image),
            "file" => Some(AttachmentKind::File),
            "audio" => Some(AttachmentKind::Audio),
            "video" => Some(AttachmentKind::Video),
            _ => None,
        }
    }
}

/// The formats the `image` crate decodes here (features `jpeg`, `png`, `gif`, `webp`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    WebP,
}

impl ImageFormat {
    pub fn ext(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::Gif => "gif",
            ImageFormat::WebP => "webp",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Gif => "image/gif",
            ImageFormat::WebP => "image/webp",
        }
    }

    /// From a stored `mime`; `None` for anything that is not one of the four.
    pub fn from_mime(mime: &str) -> Option<ImageFormat> {
        match mime {
            "image/jpeg" => Some(ImageFormat::Jpeg),
            "image/png" => Some(ImageFormat::Png),
            "image/gif" => Some(ImageFormat::Gif),
            "image/webp" => Some(ImageFormat::WebP),
            _ => None,
        }
    }
}

/// The state of the file on this device (until slice 11: `local` or `missing`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "AttachmentState.ts")]
pub enum AttachmentState {
    Local,
    Missing,
}

/// An attachment as the interface sees it, in an entry or in a draft. It never carries `blob_key`, `remote_blob_id`, a path
/// or the stored file name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AttachmentView.ts")]
pub struct AttachmentView {
    pub id: String,
    pub kind: AttachmentKind,
    pub filename: String,
    pub mime: String,
    #[ts(type = "number")]
    pub byte_size: i64,
    /// Pixels after the EXIF orientation; only for `image` (the feed reserves the box before the file loads).
    pub width: Option<u32>,
    pub height: Option<u32>,
    #[ts(type = "number | null")]
    pub duration_ms: Option<i64>,
    pub state: AttachmentState,
}

/// What an entry's attachments amount to, for the lines that have no room for them: the reply quote, "On this day", the forest
/// card, search and the capture context. The strings ("Photo", "3 photos", "menu.pdf") are in the interface language.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "AttachmentSummary.ts")]
pub struct AttachmentSummary {
    pub photos: u32,
    /// Everything that is not a photo: files, audio, video.
    pub files: u32,
    /// The file name when the entry has exactly one attachment.
    pub name: Option<String>,
}

/// Everything the `core`, `order` and `deleted` groups of a new attachment are built from.
#[derive(Debug, Clone)]
pub struct NewAttachment<'a> {
    pub id: &'a str,
    pub message_id: &'a str,
    pub kind: AttachmentKind,
    pub sort_index: u32,
    /// 64 lowercase hex characters.
    pub sha256: &'a str,
    pub filename: &'a str,
    pub mime: &'a str,
    pub byte_size: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<i64>,
    /// A UUID v7 in lower case.
    pub remote_blob_id: &'a str,
    /// 64 lowercase hex characters (32 bytes from the OS CSPRNG).
    pub blob_key: &'a str,
}

/// A whole row of the `attachment` table: the parsed columns and the groups' bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentRow {
    pub id: String,
    pub message_id: String,
    pub kind: AttachmentKind,
    pub sort_index: u32,
    pub sha256: String,
    pub filename: String,
    pub mime: String,
    pub byte_size: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub duration_ms: Option<i64>,
    pub remote_blob_id: String,
    pub blob_key: String,
    pub hlc_json: String,
    pub schema_version: u32,
    pub groups_raw: String,
}

/// A new attachment with the groups `core`, `order` and `deleted` under one HLC and `sv = 1` (appendix §C.6). The values
/// are checked here, because they reach the bytes that sync will send: nothing out of the format gets through.
pub fn new_attachment(input: &NewAttachment<'_>, hlc: &str) -> Result<AttachmentRow, GroupError> {
    check(input)?;
    let core = json!({
        "blob_key": input.blob_key,
        "byte_size": input.byte_size,
        "duration_ms": input.duration_ms,
        "filename": input.filename,
        "height": input.height,
        "kind": input.kind.as_str(),
        "link_site": null,
        "link_title": null,
        "message_id": input.message_id,
        "mime": input.mime,
        "remote_blob_id": input.remote_blob_id,
        "sha256": input.sha256,
        "url": null,
        "width": input.width,
    });
    let groups = [
        ("core", core),
        ("order", json!({"sort_index": input.sort_index})),
        ("deleted", json!({"deleted_at": null})),
    ];
    let (groups_raw, hlc_json) = assemble(&groups, hlc)?;
    Ok(AttachmentRow {
        id: input.id.to_string(),
        message_id: input.message_id.to_string(),
        kind: input.kind,
        sort_index: input.sort_index,
        sha256: input.sha256.to_string(),
        filename: input.filename.to_string(),
        mime: input.mime.to_string(),
        byte_size: input.byte_size,
        width: input.width,
        height: input.height,
        duration_ms: input.duration_ms,
        remote_blob_id: input.remote_blob_id.to_string(),
        blob_key: input.blob_key.to_string(),
        hlc_json,
        schema_version: SCHEMA_VERSION,
        groups_raw,
    })
}

fn check(input: &NewAttachment<'_>) -> Result<(), GroupError> {
    let invalid = |why: &'static str| Err(GroupError::Invalid(why));
    if !is_uuid_v4(input.id) || !is_uuid_v4(input.message_id) {
        return invalid("attachment or message id is not a UUID v4");
    }
    if !is_uuid_v7(input.remote_blob_id) {
        return invalid("remote_blob_id is not a UUID v7");
    }
    if !is_hex64(input.sha256) || !is_hex64(input.blob_key) {
        return invalid("sha256 or blob_key is not 64 lowercase hex characters");
    }
    if input.filename.is_empty()
        || input.filename.len() > MAX_FILENAME_BYTES
        || sanitize_filename(input.filename) != input.filename
    {
        return invalid("filename");
    }
    if input.mime.is_empty() || input.mime.len() > 255 || input.mime.chars().any(char::is_control) {
        return invalid("mime");
    }
    if input.byte_size < 0 || input.byte_size as u64 > MAX_FILE_BYTES {
        return invalid("byte_size");
    }
    if input.sort_index as usize >= MAX_PER_ENTRY {
        return invalid("sort_index");
    }
    if input.kind != AttachmentKind::Image && (input.width.is_some() || input.height.is_some()) {
        return invalid("width and height belong to images");
    }
    Ok(())
}

/// A UUID v7 in lower case: `7` at the start of the third group, variant `8`, `9`, `a` or `b` at the start of the fourth.
pub fn is_uuid_v7(text: &str) -> bool {
    let bytes = text.as_bytes();
    crate::hlc::is_uuid(text)
        && bytes.get(14) == Some(&b'7')
        && matches!(bytes.get(19), Some(b'8' | b'9' | b'a' | b'b'))
}

/// 64 lowercase hexadecimal characters.
pub fn is_hex64(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Lowercase hex of bytes (`blob_key`, `sha256`).
pub fn hex_lower(bytes: &[u8]) -> String {
    let digit = |nibble: u8| char::from_digit(u32::from(nibble), 16).unwrap_or('0');
    let mut out = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        out.push(digit(byte >> 4));
        out.push(digit(byte & 15));
    }
    out
}

#[cfg(test)]
mod tests;
