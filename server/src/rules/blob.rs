//! The decisions about blobs as plain functions with no IO (ADR-0001 §7, appendix §A.2, ADR-0020): what a blob of a declared size may look
//! like, what an upload request does to the state it finds, which part of a blob a `Range` header asks for, and when a deleted blob may
//! finally be removed. The server sees only ciphertext: the framing it can check is the length (24 + chunks of `1 MiB + 17`).

/// The secretstream header at the start of a blob.
pub const HEADER_LEN: u64 = 24;
/// What `push` adds to every chunk.
pub const TAG_LEN: u64 = 17;
pub const CHUNK: u64 = 1 << 20;
/// `sodium_pad` pads the file to a multiple of this, always by at least one byte.
pub const PAD_BLOCK: u64 = 1024;
/// The largest file of PRD §4.2.
pub const MAX_FILE: u64 = 2 << 30;

/// The length of a blob whose padded plaintext is `padded` bytes.
pub const fn sealed_len(padded: u64) -> u64 {
    HEADER_LEN + padded + TAG_LEN * padded.div_ceil(CHUNK)
}

/// The length of the blob of the largest file: a 2 GiB file is padded by 1024 bytes, because padding always adds at least one byte.
pub const fn sealed_len_max() -> u64 {
    sealed_len(MAX_FILE + PAD_BLOCK)
}

/// Whether `len` is the length of a blob of §A.2: the header, then chunks of `1 MiB + 17` bytes and a last one that is shorter or
/// equal and never empty, holding a multiple of 1024 bytes of padded plaintext (so at least 1024).
pub fn framing_ok(len: u64) -> bool {
    let Some(body) = len.checked_sub(HEADER_LEN).filter(|body| *body > 0) else {
        return false;
    };
    let frame = CHUNK + TAG_LEN;
    match body % frame {
        0 => true,
        last => last > TAG_LEN && (last - TAG_LEN).is_multiple_of(PAD_BLOCK),
    }
}

/// What the database says about an id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Known {
    Absent,
    Published,
    /// Deleted by its device, by a purge or by the deletion of its account. The object may still be there for the backups' sake.
    Deleted,
}

/// What the staging directory holds for an id: an unfinished upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Staged {
    pub declared: u64,
    pub received: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Put {
    /// Published already: success, nothing is read (§7: the bytes are not compared, the stream header is random).
    Exists,
    /// Begin, or begin again over an unfinished upload (an unfinished upload can be overwritten).
    Start,
    /// Keep the first `offset` bytes of the unfinished upload and append.
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutRefused {
    /// The id was deleted: a deleted blob is replaced under a new id (§7).
    Gone,
    /// The declared size is not a blob length or is above the cap.
    Framing,
    TooLarge,
    /// The offset is beyond what the server holds; `received` is where to continue.
    Gap {
        received: u64,
    },
    /// The size differs from the unfinished upload's; only offset 0 may change it.
    SizeMismatch {
        declared: u64,
    },
    /// The offset and the body would run past the declared size.
    Overrun,
}

/// What an upload request for `size` bytes in total, carrying a body at `offset`, does to the state it finds.
pub fn plan_put(
    known: Known,
    staged: Option<Staged>,
    size: u64,
    offset: u64,
    max: u64,
) -> Result<Put, PutRefused> {
    match known {
        Known::Published => return Ok(Put::Exists),
        Known::Deleted => return Err(PutRefused::Gone),
        Known::Absent => {}
    }
    if size > max {
        return Err(PutRefused::TooLarge);
    }
    if !framing_ok(size) {
        return Err(PutRefused::Framing);
    }
    // An offset equal to the size with nothing to send is allowed: it publishes a complete upload whose publication failed.
    if offset > size {
        return Err(PutRefused::Overrun);
    }
    if offset == 0 {
        return Ok(Put::Start);
    }
    match staged {
        None => Err(PutRefused::Gap { received: 0 }),
        Some(held) if held.declared != size => Err(PutRefused::SizeMismatch {
            declared: held.declared,
        }),
        Some(held) if offset > held.received => Err(PutRefused::Gap {
            received: held.received,
        }),
        Some(_) => Ok(Put::Resume),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    /// Inclusive.
    pub end: u64,
}

impl ByteRange {
    /// The number of bytes in the range.
    pub fn count(&self) -> u64 {
        self.end.saturating_sub(self.start).saturating_add(1)
    }
}

/// A `Range` header that cannot be served: a 416.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsatisfiable;

/// The range a `Range` header asks for in a blob of `len` bytes: `None` for no header (the whole blob), an error for a range that
/// cannot be satisfied or that asks for several (a 416). Only the `bytes` unit with one range is understood: `a-b`, `a-`, `-n`.
pub fn parse_range(header: Option<&str>, len: u64) -> Result<Option<ByteRange>, Unsatisfiable> {
    let Some(header) = header else {
        return Ok(None);
    };
    range_of(header, len).map(Some).ok_or(Unsatisfiable)
}

fn range_of(header: &str, len: u64) -> Option<ByteRange> {
    let spec = header.trim().strip_prefix("bytes=")?;
    let last = len.checked_sub(1)?;
    if spec.contains(',') {
        return None;
    }
    let (from, to) = spec.split_once('-')?;
    let range = match (from.trim(), to.trim()) {
        ("", suffix) => {
            let n: u64 = suffix.parse().ok().filter(|n| *n > 0)?;
            ByteRange {
                start: len.saturating_sub(n),
                end: last,
            }
        }
        (start, "") => ByteRange {
            start: start.parse().ok()?,
            end: last,
        },
        (start, end) => ByteRange {
            start: start.parse().ok()?,
            end: end.parse::<u64>().ok()?.min(last),
        },
    };
    (range.start <= range.end).then_some(range)
}

/// Whether a blob deleted at `deleted_ms` may be removed from the store at `now_ms`: it is older than the backup retention plus one
/// backup interval. A backup taken just before the deletion is rotated one interval after the retention at the latest (ADR-0020).
pub fn removable(deleted_ms: u64, now_ms: u64, retention_ms: u64, interval_ms: u64) -> bool {
    now_ms
        >= deleted_ms
            .saturating_add(retention_ms)
            .saturating_add(interval_ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: u64 = 1 << 40;

    #[test]
    fn the_largest_blob_is_a_2_gib_file_padded_framed_and_tagged() {
        // 2 GiB + 1024 bytes of padded plaintext in 2049 chunks.
        assert_eq!(
            sealed_len_max(),
            24 + (2 << 30) + 1024 + 17 * 2049,
            "header, plaintext, a tag per chunk"
        );
        assert!(framing_ok(sealed_len_max()));
    }

    #[test]
    fn the_framing_accepts_exactly_the_lengths_of_appendix_a2() {
        // An empty file is 1024 padded bytes: one chunk of 1024 + 17.
        assert!(framing_ok(24 + 1024 + 17));
        // One full chunk, a full chunk and a short one, three full chunks.
        assert!(framing_ok(sealed_len(CHUNK)));
        assert!(framing_ok(sealed_len(CHUNK + 1024)));
        assert!(framing_ok(sealed_len(3 * CHUNK)));
        for bad in [
            0,
            1,
            24,
            24 + 17,
            24 + 1024 + 16,
            24 + 1024 + 18,
            24 + 1025 + 17,
            sealed_len(CHUNK) + 1,
            sealed_len(CHUNK + 1024) - 1,
            // A chunk of 1 MiB + 1024 would be cut as 1 MiB and a short one: the short one's padding is fine, but not 1 byte more.
            sealed_len(CHUNK) + 17,
            u64::MAX,
        ] {
            assert!(!framing_ok(bad), "{bad}");
        }
    }

    #[test]
    fn a_published_blob_is_success_and_a_deleted_one_is_gone_whatever_the_request_says() {
        assert_eq!(
            plan_put(Known::Published, None, 5, 999, MAX),
            Ok(Put::Exists)
        );
        assert_eq!(
            plan_put(Known::Deleted, None, sealed_len(1024), 0, MAX),
            Err(PutRefused::Gone)
        );
    }

    #[test]
    fn a_new_upload_starts_at_zero_and_can_be_overwritten_at_zero() {
        let size = sealed_len(2 * CHUNK);
        let held = Staged {
            declared: size,
            received: 500,
        };
        assert_eq!(plan_put(Known::Absent, None, size, 0, MAX), Ok(Put::Start));
        assert_eq!(
            plan_put(Known::Absent, Some(held), size, 0, MAX),
            Ok(Put::Start)
        );
        // Another size at offset 0 replaces the unfinished upload.
        assert_eq!(
            plan_put(Known::Absent, Some(held), sealed_len(CHUNK), 0, MAX),
            Ok(Put::Start)
        );
    }

    #[test]
    fn a_resumed_upload_continues_inside_what_the_server_holds_and_not_beyond() {
        let size = sealed_len(2 * CHUNK);
        let held = Staged {
            declared: size,
            received: 500,
        };
        assert_eq!(
            plan_put(Known::Absent, Some(held), size, 500, MAX),
            Ok(Put::Resume)
        );
        assert_eq!(
            plan_put(Known::Absent, Some(held), size, 100, MAX),
            Ok(Put::Resume),
            "overwriting the tail"
        );
        assert_eq!(
            plan_put(Known::Absent, Some(held), size, 501, MAX),
            Err(PutRefused::Gap { received: 500 })
        );
        assert_eq!(
            plan_put(Known::Absent, None, size, 7, MAX),
            Err(PutRefused::Gap { received: 0 })
        );
        assert_eq!(
            plan_put(Known::Absent, Some(held), sealed_len(CHUNK), 500, MAX),
            Err(PutRefused::SizeMismatch { declared: size })
        );
    }

    #[test]
    fn limits_come_before_anything_is_stored() {
        assert_eq!(
            plan_put(Known::Absent, None, sealed_len(3 * CHUNK), 0, 1 << 20),
            Err(PutRefused::TooLarge)
        );
        assert_eq!(
            plan_put(Known::Absent, None, 1000, 0, MAX),
            Err(PutRefused::Framing)
        );
        let size = sealed_len(CHUNK);
        assert_eq!(
            plan_put(Known::Absent, None, size, size + 1, MAX),
            Err(PutRefused::Overrun)
        );
    }

    #[test]
    fn an_offset_at_the_end_publishes_a_complete_upload_and_nothing_else() {
        let size = sealed_len(CHUNK);
        let complete = Staged {
            declared: size,
            received: size,
        };
        assert_eq!(
            plan_put(Known::Absent, Some(complete), size, size, MAX),
            Ok(Put::Resume)
        );
        let short = Staged {
            declared: size,
            received: size - 1,
        };
        assert_eq!(
            plan_put(Known::Absent, Some(short), size, size, MAX),
            Err(PutRefused::Gap { received: size - 1 })
        );
        assert_eq!(
            plan_put(Known::Absent, None, size, size, MAX),
            Err(PutRefused::Gap { received: 0 })
        );
    }

    #[test]
    fn a_range_is_one_span_inside_the_blob() {
        let r = |header: &str| parse_range(Some(header), 1000);
        assert_eq!(parse_range(None, 1000), Ok(None));
        assert_eq!(r("bytes=0-9"), Ok(Some(ByteRange { start: 0, end: 9 })));
        assert_eq!(
            r("bytes=990-"),
            Ok(Some(ByteRange {
                start: 990,
                end: 999
            }))
        );
        assert_eq!(
            r("bytes=-10"),
            Ok(Some(ByteRange {
                start: 990,
                end: 999
            }))
        );
        assert_eq!(r("bytes=-5000"), Ok(Some(ByteRange { start: 0, end: 999 })));
        assert_eq!(
            r("bytes=900-5000"),
            Ok(Some(ByteRange {
                start: 900,
                end: 999
            }))
        );
        assert_eq!(ByteRange { start: 0, end: 9 }.count(), 10);
        for bad in [
            "bytes=1000-",
            "bytes=5-4",
            "bytes=-0",
            "bytes=0-1,5-6",
            "items=0-1",
            "bytes=a-b",
            "bytes=",
            "bytes=--5",
            "0-5",
            "bytes=18446744073709551616-",
        ] {
            assert!(r(bad).is_err(), "{bad}");
        }
        assert!(parse_range(Some("bytes=0-0"), 0).is_err());
    }

    #[test]
    fn a_deleted_blob_goes_when_it_is_older_than_the_retention_and_one_interval() {
        let (retention, interval) = (30_000, 1_000);
        assert!(!removable(100, 100 + 30_999, retention, interval));
        assert!(removable(100, 100 + 31_000, retention, interval));
        assert!(!removable(u64::MAX, u64::MAX - 1, retention, interval));
    }
}
