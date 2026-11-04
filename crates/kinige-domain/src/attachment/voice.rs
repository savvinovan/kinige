//! Voice notes (slice 7a, ADR-0015): the limits the recorder and `send_voice` share, the container decided by magic bytes, the
//! generated file name, the search word and the `m:ss` form of a duration. No IO.

use chrono::{DateTime, TimeDelta};

/// A shorter recording is not saved ("Too short to save").
pub const MIN_VOICE_MS: i64 = 1_000;
/// The recorder stops and sends at 30 minutes; 5 seconds of slack for the recorder's own clock.
pub const MAX_VOICE_MS: i64 = 1_805_000;
/// What a recorder file is found by in search: the single word `voice`, not the dated stem (`unicode61` would split the stem at
/// the dashes and "11" would find every note of the month).
pub const VOICE_SEARCH_WORD: &str = "voice";

const NAME_PREFIX: &str = "voice-";

/// The container of a recording, decided by the first bytes and never by what the recorder says about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceContainer {
    /// AAC in MP4 (WKWebView, and WebView2 when it chooses it).
    Mp4,
    /// Opus in WebM (Chromium).
    WebM,
}

impl VoiceContainer {
    /// `ftyp` at offset 4 is MP4; the EBML header `1A 45 DF A3` is WebM; anything else is not a voice note.
    pub fn detect(head: &[u8]) -> Option<VoiceContainer> {
        if head.get(4..8) == Some(b"ftyp".as_slice()) {
            Some(VoiceContainer::Mp4)
        } else if head.get(..4) == Some([0x1A, 0x45, 0xDF, 0xA3].as_slice()) {
            Some(VoiceContainer::WebM)
        } else {
            None
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            VoiceContainer::Mp4 => "m4a",
            VoiceContainer::WebM => "webm",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            VoiceContainer::Mp4 => "audio/mp4",
            VoiceContainer::WebM => "audio/webm",
        }
    }
}

/// `voice-YYYY-MM-DD-HHMMSS.<ext>` in the local time of the zone with the offset `utc_offset_secs`. `None` only for a moment chrono
/// cannot represent.
pub fn voice_file_name(
    now_ms: i64,
    utc_offset_secs: i32,
    container: VoiceContainer,
) -> Option<String> {
    let utc = DateTime::from_timestamp_millis(now_ms)?.naive_utc();
    let local = utc.checked_add_signed(TimeDelta::try_seconds(i64::from(utc_offset_secs))?)?;
    Some(format!(
        "{NAME_PREFIX}{}.{}",
        local.format("%Y-%m-%d-%H%M%S"),
        container.ext()
    ))
}

/// Whether `filename` is a recorder's name: `voice-YYYY-MM-DD-HHMMSS.m4a` or `.webm`.
pub fn is_voice_file_name(filename: &str) -> bool {
    let Some(rest) = filename.strip_prefix(NAME_PREFIX) else {
        return false;
    };
    let Some((stamp, ext)) = rest.rsplit_once('.') else {
        return false;
    };
    matches!(ext, "m4a" | "webm")
        && stamp.len() == 17
        && stamp.bytes().enumerate().all(|(index, byte)| match index {
            4 | 7 | 10 => byte == b'-',
            _ => byte.is_ascii_digit(),
        })
}

/// `m:ss`, or `h:mm:ss` from one hour. A negative duration is zero.
pub fn format_duration(ms: i64) -> String {
    let total = ms.max(0) / 1000;
    let (hours, minutes, seconds) = (total / 3600, total % 3600 / 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// The `Content-Type` the audio route may send for a stored MIME type: an allowlist, so a row cannot make the route answer with
/// anything else (`text/html`, `image/svg+xml`, a script). Any other type is `None`, which is a 404 and "Can't play here".
pub fn playable_audio_mime(mime: &str) -> Option<&'static str> {
    [
        "audio/mp4",
        "audio/aac",
        "audio/mpeg",
        "audio/ogg",
        "audio/opus",
        "audio/wav",
        "audio/flac",
        "audio/webm",
    ]
    .into_iter()
    .find(|allowed| *allowed == mime)
}

/// Whether the first bytes of a file fit the container its MIME type claims. The audio route reads 12 bytes of the file it opens and
/// answers only when they match, so a row cannot get an HTML page or a script served as "audio".
pub fn audio_head_matches(content_type: &str, head: &[u8]) -> bool {
    let starts = |prefix: &[u8]| head.get(..prefix.len()) == Some(prefix);
    let mp4 = head.get(4..8) == Some(b"ftyp".as_slice());
    // An MPEG frame starts with 11 set bits; an ADTS (AAC) frame is the same sync pattern.
    let frame_sync =
        matches!(head.first(), Some(0xFF)) && head.get(1).is_some_and(|b| b & 0xE0 == 0xE0);
    match content_type {
        "audio/mp4" => mp4,
        "audio/aac" => mp4 || frame_sync,
        "audio/mpeg" => starts(b"ID3") || frame_sync,
        "audio/ogg" | "audio/opus" => starts(b"OggS"),
        "audio/wav" => starts(b"RIFF") && head.get(8..12) == Some(b"WAVE".as_slice()),
        "audio/flac" => starts(b"fLaC"),
        "audio/webm" => starts(&[0x1A, 0x45, 0xDF, 0xA3]),
        _ => false,
    }
}
