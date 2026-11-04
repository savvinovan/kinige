//! The `Range` arithmetic and the answers of `kinige-media://localhost/audio/<id>` (slice 7a, ADR-0015). The store hands over an
//! open file that passed its checks (`Store::audio_file`); everything here is about how much of it to send.
//!
//! The Tauri responder takes a whole body (no streaming), so an answer is built in memory: the file is at most 64 MiB (the store
//! refuses a bigger one), and a range answer is at most 1 MiB. All arithmetic is `u64` with `checked_*` or `saturating_*`.
//! A request without `Range`, or with a unit other than `bytes`, gets the whole file with `Content-Length` and `Accept-Ranges`.
//! A single satisfiable range gets 206 with `Content-Range` and an explicit `Content-Length`. An out-of-bounds, empty-suffix or
//! multi-range request gets 416, and so does a file that changed under the handler (never a body shorter than its length).

use kinige_store::AudioFile;
use std::io::{Read, Seek, SeekFrom};
use tauri::http::{Response, StatusCode, header};

/// The most one range answer carries (the Tauri asset protocol does the same with 1000 KiB, verified). A player asks again.
pub const MAX_ANSWER_BYTES: u64 = 1024 * 1024;
/// How long the webview may keep an answer: an id is never reused, and its file never changes.
const CACHE_CONTROL: &str = "private, max-age=300";

/// What a request asks for, given the file length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plan {
    /// The whole file: no `Range`, a unit other than `bytes`, or a header that is not a range at all (ignored, as RFC 9110 says).
    Whole,
    /// Bytes `start..=end`, already clipped to the file and to [`MAX_ANSWER_BYTES`].
    Part { start: u64, end: u64 },
    /// 416.
    Unsatisfiable,
}

/// Reads a `Range` header for a file of `len` bytes.
pub fn plan(header: Option<&str>, len: u64) -> Plan {
    let Some(text) = header.map(str::trim) else {
        return Plan::Whole;
    };
    let Some((unit, spec)) = text.split_once('=') else {
        return Plan::Whole;
    };
    if !unit.trim().eq_ignore_ascii_case("bytes") {
        return Plan::Whole;
    }
    // Several ranges: 416 (a 200 with the whole file would also be allowed; the player never asks for them).
    if spec.contains(',') {
        return Plan::Unsatisfiable;
    }
    let Some((first, last)) = spec.split_once('-') else {
        return Plan::Whole;
    };
    let (first, last) = (first.trim(), last.trim());
    let Some(end_of_file) = len.checked_sub(1) else {
        // An empty file has no byte to ask for.
        return Plan::Unsatisfiable;
    };
    let (start, end) = if first.is_empty() {
        // A suffix: the last N bytes; N larger than the file means the whole file, N of zero is nothing.
        if !is_digits(last) {
            return Plan::Whole;
        }
        match last.parse::<u64>() {
            Ok(0) => return Plan::Unsatisfiable,
            Ok(count) => (len.saturating_sub(count), end_of_file),
            Err(_) => (0, end_of_file),
        }
    } else {
        if !is_digits(first) || !(last.is_empty() || is_digits(last)) {
            return Plan::Whole;
        }
        // A start too large for a u64 is certainly past the end of the file.
        let Ok(start) = first.parse::<u64>() else {
            return Plan::Unsatisfiable;
        };
        let end = if last.is_empty() {
            end_of_file
        } else {
            // An end too large for a u64 means "to the end".
            last.parse::<u64>().unwrap_or(end_of_file)
        };
        if start >= len || end < start {
            return Plan::Unsatisfiable;
        }
        (start, end.min(end_of_file))
    };
    let capped = start
        .checked_add(MAX_ANSWER_BYTES - 1)
        .map_or(end, |limit| end.min(limit));
    Plan::Part { start, end: capped }
}

/// The answer for a request with this `Range` header (or none).
pub fn respond(audio: AudioFile, range: Option<&str>) -> Response<Vec<u8>> {
    let response = answer(audio, range);
    // Only the status and the size: no id and no file name (the screens run reads this line to prove the player reached the route).
    log::debug!(
        "audio route: {} {} bytes",
        response.status().as_u16(),
        response.body().len()
    );
    response
}

fn answer(audio: AudioFile, range: Option<&str>) -> Response<Vec<u8>> {
    let AudioFile {
        mut file,
        len,
        content_type,
    } = audio;
    match plan(range, len) {
        Plan::Whole => match read_at(&mut file, 0, len) {
            Some(bytes) => build(StatusCode::OK, content_type, bytes, len, None),
            None => unsatisfiable(len),
        },
        Plan::Part { start, end } => {
            let count = end.saturating_sub(start).saturating_add(1);
            match read_at(&mut file, start, count) {
                Some(bytes) => build(
                    StatusCode::PARTIAL_CONTENT,
                    content_type,
                    bytes,
                    count,
                    Some(format!("bytes {start}-{end}/{len}")),
                ),
                None => unsatisfiable(len),
            }
        }
        Plan::Unsatisfiable => unsatisfiable(len),
    }
}

/// `count` bytes from `start`; `None` if the file gave fewer (it changed under the handler).
fn read_at(file: &mut std::fs::File, start: u64, count: u64) -> Option<Vec<u8>> {
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::with_capacity(usize::try_from(count).ok()?);
    file.take(count).read_to_end(&mut bytes).ok()?;
    (u64::try_from(bytes.len()).ok()? == count).then_some(bytes)
}

fn build(
    status: StatusCode,
    content_type: &str,
    bytes: Vec<u8>,
    length: u64,
    content_range: Option<String>,
) -> Response<Vec<u8>> {
    let mut builder = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, CACHE_CONTROL)
        // Not a script, not a frame: the answer is audio for an <audio> element.
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; sandbox",
        );
    if let Some(range) = content_range {
        builder = builder.header(header::CONTENT_RANGE, range);
    }
    builder.body(bytes).unwrap_or_else(|_| {
        let mut response = Response::new(Vec::new());
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        response
    })
}

/// 416 with `Content-Range: bytes */len` and no body. No `Access-Control-Allow-Origin`, ever.
fn unsatisfiable(len: u64) -> Response<Vec<u8>> {
    Response::builder()
        .status(StatusCode::RANGE_NOT_SATISFIABLE)
        .header(header::CONTENT_RANGE, format!("bytes */{len}"))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Vec::new())
        .unwrap_or_else(|_| Response::new(Vec::new()))
}

fn is_digits(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEN: u64 = 10_000_000;

    fn part(start: u64, end: u64) -> Plan {
        Plan::Part { start, end }
    }

    #[test]
    fn no_range_or_not_a_bytes_range_is_the_whole_file() {
        for header in [
            None,
            Some(""),
            Some("items=0-5"),
            Some("Bytes"),
            Some("bytes"),
            Some("bytes=abc"),
            Some("bytes=a-b"),
            Some("bytes=5-x"),
            Some("bytes=--5"),
            Some("bytes=-x"),
            Some("0-5"),
        ] {
            assert_eq!(plan(header, LEN), Plan::Whole, "{header:?}");
        }
    }

    #[test]
    fn an_open_range_is_clipped_to_one_mebibyte_and_to_the_file() {
        assert_eq!(plan(Some("bytes=0-"), LEN), part(0, MAX_ANSWER_BYTES - 1));
        assert_eq!(plan(Some("bytes=0-1"), LEN), part(0, 1));
        assert_eq!(plan(Some("bytes=0-0"), LEN), part(0, 0));
        assert_eq!(plan(Some(" BYTES = 5 - 9 "), LEN), part(5, 9));
        assert_eq!(
            plan(Some("bytes=100-99999999"), LEN),
            part(100, 100 + MAX_ANSWER_BYTES - 1)
        );
        // The end is clipped to the last byte of a small file.
        assert_eq!(plan(Some("bytes=10-999"), 100), part(10, 99));
        assert_eq!(plan(Some("bytes=99-"), 100), part(99, 99));
        // Numbers past a u64: the end means "to the end", the start is past the file.
        assert_eq!(
            plan(Some("bytes=0-99999999999999999999999"), 100),
            part(0, 99)
        );
        assert_eq!(
            plan(Some("bytes=99999999999999999999999-"), 100),
            Plan::Unsatisfiable
        );
    }

    #[test]
    fn a_suffix_is_the_tail_and_a_suffix_larger_than_the_file_is_the_whole_file() {
        assert_eq!(plan(Some("bytes=-500"), 1000), part(500, 999));
        assert_eq!(plan(Some("bytes=-1"), 1000), part(999, 999));
        assert_eq!(plan(Some("bytes=-1000"), 1000), part(0, 999));
        assert_eq!(plan(Some("bytes=-5000"), 1000), part(0, 999));
        assert_eq!(
            plan(Some("bytes=-99999999999999999999999"), 1000),
            part(0, 999)
        );
        // Still at most one mebibyte per answer.
        assert_eq!(
            plan(Some("bytes=-5000000"), LEN),
            part(LEN - 5_000_000, LEN - 5_000_000 + MAX_ANSWER_BYTES - 1)
        );
        assert_eq!(plan(Some("bytes=-0"), 1000), Plan::Unsatisfiable);
    }

    #[test]
    fn out_of_bounds_reversed_multiple_and_empty_file_ranges_are_416() {
        for header in [
            "bytes=1000-",
            "bytes=1000-1001",
            "bytes=5000-6000",
            "bytes=9-3",
            "bytes=0-0,5-9",
            "bytes=0-1, 4-5",
            "bytes=-0",
        ] {
            assert_eq!(plan(Some(header), 1000), Plan::Unsatisfiable, "{header}");
        }
        for header in ["bytes=0-", "bytes=0-0", "bytes=-1", "bytes=5-9"] {
            assert_eq!(
                plan(Some(header), 0),
                Plan::Unsatisfiable,
                "empty: {header}"
            );
        }
        assert_eq!(plan(None, 0), Plan::Whole);
    }

    #[test]
    fn the_arithmetic_survives_the_largest_numbers() {
        let big = u64::MAX;
        for header in [
            format!("bytes={big}-"),
            format!("bytes={big}-{big}"),
            format!("bytes=0-{big}"),
            format!("bytes=-{big}"),
            format!("bytes={}-", big - 1),
        ] {
            // No panic and a sane result for a 100 byte file.
            let _ = plan(Some(&header), 100);
        }
        assert_eq!(
            plan(Some(&format!("bytes=0-{}", u64::MAX)), 100),
            part(0, 99)
        );
        assert_eq!(
            plan(Some(&format!("bytes={}-", u64::MAX)), 100),
            Plan::Unsatisfiable
        );
        assert_eq!(
            plan(Some("bytes=0-"), u64::MAX),
            part(0, MAX_ANSWER_BYTES - 1)
        );
    }
}
