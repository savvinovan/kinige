//! Names and kinds of attachments: the stored file name, the extension rules, the kind by content or extension, the allowlist
//! for opening in a system app and the file names of the Markdown copy (ADR-0014).

use super::{
    AttachmentKind, AttachmentSummary, AttachmentView, ImageFormat, MAX_FILENAME_BYTES,
};
use std::collections::HashSet;

const VIDEO_EXTS: &[&str] = &[
    "mp4", "m4v", "mov", "webm", "mkv", "avi", "mpg", "mpeg", "3gp", "wmv", "flv",
];
const AUDIO_EXTS: &[&str] = &[
    "mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "opus", "aiff", "aif", "wma", "amr",
];
/// Types a system app may open straight from a clone (ADR-0014). Everything else is revealed in the file manager. No executables,
/// scripts, archives, SVG (a browser runs its script) or macro-enabled Office files.
const OPENABLE_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "avif", "tif", "tiff", "bmp", "pdf",
    "txt", "md", "rtf", "csv", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "odt", "ods", "odp",
    "pages", "numbers", "key", "mp3", "m4a", "aac", "wav", "flac", "ogg", "oga", "opus", "aiff",
    "mp4", "m4v", "mov", "webm", "mkv", "avi",
];
/// MIME types by extension for what is not an image the crate reads; a small table, not a database.
const MIME_BY_EXT: &[(&str, &str)] = &[
    ("pdf", "application/pdf"),
    ("txt", "text/plain"),
    ("md", "text/markdown"),
    ("csv", "text/csv"),
    ("rtf", "application/rtf"),
    ("json", "application/json"),
    ("zip", "application/zip"),
    ("doc", "application/msword"),
    (
        "docx",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    ),
    ("xls", "application/vnd.ms-excel"),
    (
        "xlsx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    ),
    ("ppt", "application/vnd.ms-powerpoint"),
    (
        "pptx",
        "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    ),
    ("svg", "image/svg+xml"),
    ("heic", "image/heic"),
    ("heif", "image/heif"),
    ("avif", "image/avif"),
    ("tif", "image/tiff"),
    ("tiff", "image/tiff"),
    ("bmp", "image/bmp"),
    ("mp3", "audio/mpeg"),
    ("m4a", "audio/mp4"),
    ("aac", "audio/aac"),
    ("wav", "audio/wav"),
    ("flac", "audio/flac"),
    ("ogg", "audio/ogg"),
    ("oga", "audio/ogg"),
    ("opus", "audio/opus"),
    ("mp4", "video/mp4"),
    ("m4v", "video/x-m4v"),
    ("mov", "video/quicktime"),
    ("webm", "video/webm"),
    ("mkv", "video/x-matroska"),
    ("avi", "video/x-msvideo"),
];
/// Names Windows refuses as a file name whatever the extension.
const WINDOWS_RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];
/// The longest file name in the Markdown copy, in bytes: room for the ` (2)` suffix inside the 255 of the file systems.
const MIRROR_NAME_BYTES: usize = 200;

/// The name of the file as the user sees it (the `filename` of the `core` group): the last path component, no control or
/// bidi-override characters, at most 255 bytes (the extension is kept), never empty (`file`).
pub fn sanitize_filename(raw: &str) -> String {
    let last = raw
        .rsplit(['/', '\\'])
        .find(|part| !part.trim().is_empty())
        .unwrap_or_default();
    let cleaned: String = last
        .chars()
        .filter(|c| !c.is_control() && !is_bidi_override(*c))
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        return "file".to_string();
    }
    let cut = truncate_keeping_ext(trimmed, MAX_FILENAME_BYTES);
    // Cutting may leave a space at the end; the result must be a fixed point so that a stored name passes the check again.
    match cut.trim() {
        "" => "file".to_string(),
        text => text.to_string(),
    }
}

fn is_bidi_override(c: char) -> bool {
    matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Cuts a name to `max` bytes at a character boundary; a short extension survives.
fn truncate_keeping_ext(name: &str, max: usize) -> String {
    if name.len() <= max {
        return name.to_string();
    }
    let (stem, ext) = match name.rfind('.') {
        Some(dot) if dot > 0 && name.len().saturating_sub(dot) <= 11 => {
            name.split_at(dot)
        }
        _ => (name, ""),
    };
    let room = max.saturating_sub(ext.len());
    let mut end = room.min(stem.len());
    while end > 0 && !stem.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{ext}", stem.get(..end).unwrap_or_default())
}

/// The extension of the stored file (`attachments/<id>.<ext>`): the lowercase original one if it matches `^[a-z0-9]{1,10}$`,
/// otherwise the one of the detected image format, otherwise `bin`.
pub fn stored_ext(original_name: &str, detected: Option<ImageFormat>) -> String {
    let name = sanitize_filename(original_name);
    if let Some((stem, ext)) = name.rsplit_once('.') {
        let ext = ext.to_ascii_lowercase();
        if !stem.is_empty()
            && (1..=10).contains(&ext.len())
            && ext.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        {
            return ext;
        }
    }
    detected.map_or("bin", ImageFormat::ext).to_string()
}

/// The kind and MIME of a new attachment. `image` is the format the header reader recognised (decided by content, not by the
/// extension); `mime_hint` comes from a source that states it (a Telegram export) and is used only if it looks like a MIME type.
pub fn classify(
    ext: &str,
    mime_hint: Option<&str>,
    image: Option<ImageFormat>,
) -> (AttachmentKind, String) {
    if let Some(format) = image {
        return (AttachmentKind::Image, format.mime().to_string());
    }
    let hint = mime_hint.filter(|mime| plausible_mime(mime));
    let by_ext = MIME_BY_EXT
        .iter()
        .find(|(known, _)| *known == ext)
        .map(|(_, mime)| *mime);
    let mime = by_ext.or(hint).unwrap_or("application/octet-stream");
    let is_video = VIDEO_EXTS.contains(&ext) || mime.starts_with("video/");
    let is_audio = AUDIO_EXTS.contains(&ext) || mime.starts_with("audio/");
    let kind = if is_video {
        AttachmentKind::Video
    } else if is_audio {
        AttachmentKind::Audio
    } else {
        AttachmentKind::File
    };
    (kind, mime.to_string())
}

fn plausible_mime(mime: &str) -> bool {
    mime.len() <= 100
        && mime.matches('/').count() == 1
        && mime
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'/' | b'.' | b'+' | b'-'))
}

/// Whether a system app may open a clone of a file with this extension (ADR-0014); otherwise it is only revealed in the file manager.
pub fn is_openable_document(ext: &str) -> bool {
    OPENABLE_EXTS.contains(&ext.to_ascii_lowercase().as_str())
}

/// The summary of an entry's attachments for lines that have no room for them.
pub fn summary_of(attachments: &[AttachmentView]) -> AttachmentSummary {
    let photos = attachments
        .iter()
        .filter(|a| a.kind == AttachmentKind::Image)
        .count();
    let files = attachments.len().saturating_sub(photos);
    let name = match attachments {
        [only] => Some(only.filename.clone()),
        _ => None,
    };
    AttachmentSummary {
        photos: u32::try_from(photos).unwrap_or(u32::MAX),
        files: u32::try_from(files).unwrap_or(u32::MAX),
        name,
    }
}

/// File names for a day folder of the Markdown copy, in the order given: safe on macOS, Windows and Linux, and unique
/// case-insensitively (` (2)`, ` (3)` before the extension; the first one keeps its name).
pub fn mirror_file_names(filenames: &[&str]) -> Vec<String> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut out = Vec::with_capacity(filenames.len());
    for filename in filenames {
        let base = mirror_safe(filename);
        let (stem, ext) = split_ext(&base);
        let mut candidate = base.clone();
        let mut n = 2u32;
        while !taken.insert(candidate.to_lowercase()) {
            candidate = format!("{stem} ({n}){ext}");
            n = n.saturating_add(1);
        }
        out.push(candidate);
    }
    out
}

/// One file name made safe for every file system the copy may live on.
fn mirror_safe(filename: &str) -> String {
    let replaced: String = sanitize_filename(filename)
        .chars()
        .map(|c| if "/\\:*?\"<>|".contains(c) { '_' } else { c })
        .collect();
    let trimmed = replaced.trim_end_matches(['.', ' ']);
    let mut name = if trimmed.is_empty() {
        "file".to_string()
    } else {
        trimmed.to_string()
    };
    let stem_lower = name.split('.').next().unwrap_or_default().to_lowercase();
    if WINDOWS_RESERVED.contains(&stem_lower.as_str()) {
        name.insert(0, '_');
    }
    truncate_keeping_ext(&name, MIRROR_NAME_BYTES)
}

/// `("a (2)", ".jpg")` pieces of a name: the stem and the extension with its dot (empty if none).
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name.split_at(dot),
        _ => (name, ""),
    }
}
