//! Shared by the voice note tests (slice 7a): synthetic recordings (no real audio, only the magic bytes the store looks at) and the
//! "nothing of a refused note is left" check.

use super::attach::names;
use super::*;

pub const CLIP_MS: i64 = 40_000;

pub fn now() -> Now {
    super::days::at(2026, 11, 3, 21, 30)
}

/// AAC in MP4 as far as the store looks: `ftyp` at offset 4, then bytes.
pub fn m4a(len: usize) -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0x20];
    bytes.extend_from_slice(b"ftypM4A ");
    bytes.resize(len.max(12), 0x5a);
    bytes
}

/// Opus in WebM as far as the store looks: the EBML header.
pub fn webm(len: usize) -> Vec<u8> {
    let mut bytes = vec![0x1A, 0x45, 0xDF, 0xA3];
    bytes.resize(len.max(4), 0x33);
    bytes
}

/// Deterministic noise with no magic bytes at the start.
pub fn noise(len: usize) -> Vec<u8> {
    let mut state = 0x1234_5678_u32;
    (0..len)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

pub fn stored(dir: &Path) -> Vec<String> {
    names(&dir.join("attachments"))
        .into_iter()
        .filter(|name| !name.starts_with('.'))
        .collect()
}

pub fn count(dir: &Path, sql: &str) -> i64 {
    raw(dir).query_row(sql, [], |row| row.get(0)).unwrap()
}

/// Nothing of a refused note is left: no entry, no row, no file, no copy of the bytes in `.tmp`.
pub fn assert_nothing_written(dir: &Path) {
    assert_eq!(count(dir, "SELECT COUNT(*) FROM message"), 0, "no entry");
    assert_eq!(count(dir, "SELECT COUNT(*) FROM attachment"), 0, "no row");
    assert!(stored(dir).is_empty(), "no stored file");
    let tmp = dir.join("attachments/.tmp");
    let left: usize = names(&tmp)
        .iter()
        .map(|process| names(&tmp.join(process)).len())
        .sum();
    assert_eq!(left, 0, "no staged copy");
}
