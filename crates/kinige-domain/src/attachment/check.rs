//! The input rules of a new attachment (appendix §C.6, §C.8, §C.10): the bytes that sync will send hold only what the formats accept.

use super::{
    AttachmentKind, MAX_DURATION_MS, MAX_FILE_BYTES, MAX_FILENAME_BYTES, MAX_PER_ENTRY,
    NewAttachment, StickerCore, is_hex64, is_uuid_v7, normalize_description, sanitize_filename,
    sticker, valid_display_name,
};
use crate::groups::GroupError;
use crate::hlc::is_uuid_v4;

pub(super) fn check(input: &NewAttachment<'_>) -> Result<(), GroupError> {
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
    // Details (slice 7b): the bytes that sync sends hold only what the rules accept, and a spoiler is an image's.
    if input
        .display_name
        .is_some_and(|name| !valid_display_name(name, input.filename))
        || input.display_name == Some(input.filename)
    {
        return invalid("display_name");
    }
    if input
        .description
        .is_some_and(|text| normalize_description(text) != Ok(Some(text.to_string())))
    {
        return invalid("description");
    }
    if input.spoiler && input.kind != AttachmentKind::Image {
        return invalid("a spoiler belongs to images");
    }
    // A sticker (slice 5c): the bytes that sync sends say "sticker" only for a core the readers of every device accept, and the emoji is
    // a valid sequence; a picture has no emoji.
    let core = StickerCore {
        kind: input.kind,
        mime: input.mime,
        width: input.width,
        height: input.height,
        byte_size: input.byte_size,
    };
    if input.sticker && (!core.fits() || input.spoiler) {
        return invalid(
            "a sticker is an uncovered WebP or PNG of at most 512 by 512 pixels and 2 MiB",
        );
    }
    if input
        .sticker_emoji
        .is_some_and(|emoji| !input.sticker || sticker::clean_emoji(emoji).is_none())
    {
        return invalid("sticker emoji");
    }
    // Every writer must satisfy this: the Telegram import writes seconds as a `u32` times 1000 (slice 7), the recorder 1 s to 30 min
    // (slice 7a). The recorder's own limits live in `send_voice`, not here.
    if let Some(duration_ms) = input.duration_ms {
        let timed = matches!(input.kind, AttachmentKind::Audio | AttachmentKind::Video);
        if !timed || !(0..=MAX_DURATION_MS).contains(&duration_ms) {
            return invalid("duration_ms belongs to audio and video, 0 to 4294967295000");
        }
    }
    Ok(())
}
