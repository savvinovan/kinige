//! The push body (ADR-0001 appendix §A.6) and its reader. A body is a run of frames, each with the `entity`, `id` and `op` the sender
//! declares next to the envelope:
//!
//! ```text
//! u8 entity_len (1..=32) | entity (ASCII) | id (16 raw bytes) | op (16 raw bytes) | u32 BE envelope_len | envelope
//! ```
//!
//! The reader judges every length before it buffers what the length announces (§A.4 step 1): the declared size against the cap, then
//! the nine prefix bytes with `Layout::parse`, then the header, which is parsed before the rest of the envelope is read. The whole
//! body is limited as it is consumed, so a client cannot make the server hold more than the cap whatever it declares.

use kinige_crypto::CryptoError;
use kinige_crypto::envelope::{Layout, MAX_ENVELOPE, PREFIX_LEN, peek};
use kinige_crypto::header::Header;
use kinige_domain::sync::conflict::uuid_bytes;
use libsodium_rs::utils::{self, BASE64_VARIANT_URLSAFE_NO_PADDING};

const MAX_ENTITY: usize = 32;

/// Why a body was refused. `TooLarge` is a 413, the rest are 400s named by their code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireError {
    TooLarge,
    Malformed(&'static str),
}

/// One envelope of a push request, with what the sender declared and what the server read from the open header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub entity: String,
    pub id: String,
    pub op: String,
    pub bytes: Vec<u8>,
    /// Parsed from the open header; unauthenticated (§5.2): it routes the envelope, nothing more.
    pub header: Header,
}

/// A source of body chunks, so that the reader is tested without HTTP.
pub trait Source {
    /// The next chunk, `None` at the end of the body, `Err` if the transport failed.
    fn next_chunk(&mut self) -> impl Future<Output = Result<Option<Vec<u8>>, WireError>> + Send;
}

/// Reads all frames of a body of at most `max_body` bytes and `max_frames` envelopes. An empty body is an error.
pub async fn read_frames(
    source: impl Source,
    max_body: usize,
    max_frames: usize,
) -> Result<Vec<Frame>, WireError> {
    let mut reader = Reader {
        source,
        pending: Vec::new(),
        used: 0,
        consumed: 0,
        max_body,
    };
    let mut frames = Vec::new();
    while !reader.at_end().await? {
        if frames.len() >= max_frames {
            return Err(WireError::TooLarge);
        }
        frames.push(read_frame(&mut reader).await?);
    }
    if frames.is_empty() {
        return Err(WireError::Malformed("empty_body"));
    }
    Ok(frames)
}

async fn read_frame<S: Source>(reader: &mut Reader<S>) -> Result<Frame, WireError> {
    let entity_len = usize::from(reader.take(1).await?.first().copied().unwrap_or(0));
    if entity_len == 0 || entity_len > MAX_ENTITY {
        return Err(WireError::Malformed("frame_entity"));
    }
    let entity = String::from_utf8(reader.take(entity_len).await?)
        .map_err(|_| WireError::Malformed("frame_entity"))?;
    let id = uuid_text(&reader.take(16).await?);
    let op = uuid_text(&reader.take(16).await?);
    let len_bytes = <[u8; 4]>::try_from(reader.take(4).await?.as_slice())
        .map_err(|_| WireError::Malformed("frame_length"))?;
    let len = usize::try_from(u32::from_be_bytes(len_bytes)).unwrap_or(usize::MAX);
    // The declared size against the cap, before a byte of the envelope is read.
    if len > MAX_ENVELOPE {
        return Err(WireError::TooLarge);
    }
    let mut bytes = reader.take(len.min(PREFIX_LEN)).await?;
    let layout = Layout::parse(&bytes, len).map_err(layout_error)?;
    bytes.extend(reader.take(layout.header_len).await?);
    // The header is read and judged before the (up to 1 MiB) rest of the envelope.
    let header_bytes = bytes
        .get(PREFIX_LEN..)
        .ok_or(WireError::Malformed("envelope_length"))?;
    let header =
        Header::parse(header_bytes).map_err(|_| WireError::Malformed("envelope_header"))?;
    if header.entity != entity || header.id != id || header.op != op {
        return Err(WireError::Malformed("header_mismatch"));
    }
    let rest = len
        .checked_sub(bytes.len())
        .ok_or(WireError::Malformed("envelope_length"))?;
    bytes.extend(reader.take(rest).await?);
    // The final word on the whole envelope, with the same function the S4b opener starts with.
    peek(&bytes).map_err(layout_error)?;
    Ok(Frame {
        entity,
        id,
        op,
        bytes,
        header,
    })
}

fn layout_error(error: CryptoError) -> WireError {
    match error {
        CryptoError::TooLarge => WireError::TooLarge,
        CryptoError::UnknownFormat(_) => WireError::Malformed("unsupported_format"),
        _ => WireError::Malformed("envelope_length"),
    }
}

struct Reader<S> {
    source: S,
    pending: Vec<u8>,
    /// How much of `pending` has been handed out.
    used: usize,
    /// Bytes handed out in total, against `max_body`.
    consumed: usize,
    max_body: usize,
}

impl<S: Source> Reader<S> {
    /// Whether the body is over. Reads a chunk if nothing is waiting.
    async fn at_end(&mut self) -> Result<bool, WireError> {
        while self.used >= self.pending.len() {
            match self.source.next_chunk().await? {
                None => return Ok(true),
                Some(chunk) => {
                    self.pending = chunk;
                    self.used = 0;
                }
            }
        }
        Ok(false)
    }

    /// Exactly `n` bytes. The cap is checked before any of them is buffered; a body that ends early is `frame_truncated`.
    async fn take(&mut self, n: usize) -> Result<Vec<u8>, WireError> {
        let total = self.consumed.checked_add(n).ok_or(WireError::TooLarge)?;
        if total > self.max_body {
            return Err(WireError::TooLarge);
        }
        let mut out = Vec::new();
        while out.len() < n {
            if self.at_end().await? {
                return Err(WireError::Malformed("frame_truncated"));
            }
            let available = self.pending.get(self.used..).unwrap_or_default();
            let want = n - out.len();
            let part = available
                .get(..want.min(available.len()))
                .unwrap_or_default();
            out.extend_from_slice(part);
            self.used += part.len();
        }
        self.consumed = total;
        Ok(out)
    }
}

/// Builds a push body (the client side; the tests and the S4a adapter use it). `None` if an entity or an id is not what a frame holds.
pub fn encode_frames(items: &[(&str, &str, &str, &[u8])]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for (entity, id, op, envelope) in items {
        if entity.is_empty() || entity.len() > MAX_ENTITY {
            return None;
        }
        out.push(u8::try_from(entity.len()).ok()?);
        out.extend_from_slice(entity.as_bytes());
        out.extend_from_slice(&uuid_bytes(id)?);
        out.extend_from_slice(&uuid_bytes(op)?);
        out.extend_from_slice(&u32::try_from(envelope.len()).ok()?.to_be_bytes());
        out.extend_from_slice(envelope);
    }
    Some(out)
}

/// The text of 16 UUID bytes, exactly as given (no version or variant bits are forced).
pub fn uuid_text(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(36);
    for (i, byte) in bytes.iter().enumerate() {
        if matches!(i, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

pub fn to_b64(bytes: &[u8]) -> String {
    utils::bin2base64(bytes, BASE64_VARIANT_URLSAFE_NO_PADDING)
}

pub fn from_b64(text: &str) -> Option<Vec<u8>> {
    utils::base642bin(text, BASE64_VARIANT_URLSAFE_NO_PADDING).ok()
}

/// A 32-byte key from its base64url text, `None` for any other length or text.
pub fn key32(text: &str) -> Option<[u8; 32]> {
    <[u8; 32]>::try_from(from_b64(text)?.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinige_crypto::header::{GroupStamp, Kind};
    use kinige_crypto::{AccountId, Dek, envelope};
    use std::collections::BTreeMap;

    const ID: &str = "33333333-3333-4333-8333-333333333333";
    const OP: &str = "55555555-5555-4555-8555-555555555555";
    const DEVICE: &str = "66666666-6666-4666-8666-666666666666";

    /// Chunks of a fixed size, to exercise frames that straddle chunks.
    struct Chunks(Vec<Vec<u8>>);

    impl Source for Chunks {
        async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, WireError> {
            Ok(if self.0.is_empty() {
                None
            } else {
                Some(self.0.remove(0))
            })
        }
    }

    fn chunked(body: &[u8], size: usize) -> Chunks {
        Chunks(body.chunks(size.max(1)).map(<[u8]>::to_vec).collect())
    }

    fn header() -> Header {
        Header {
            entity: "message".to_string(),
            id: ID.to_string(),
            op: OP.to_string(),
            kind: Kind::Upsert,
            groups: BTreeMap::from([(
                "body".to_string(),
                GroupStamp {
                    h: format!("1762300000000-00000-{DEVICE}"),
                    sv: 1,
                },
            )]),
            purged_of: None,
            blob: None,
        }
    }

    fn envelope() -> Vec<u8> {
        let dek = Dek::generate(1).expect("dek");
        let bytes = header().to_bytes().expect("header");
        envelope::seal(&AccountId::from_bytes([1; 16]), &dek, &bytes, b"{}").expect("seal")
    }

    fn body_of(envelope: &[u8]) -> Vec<u8> {
        encode_frames(&[("message", ID, OP, envelope)]).expect("frame")
    }

    fn read(body: &[u8], chunk: usize) -> Result<Vec<Frame>, WireError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        runtime.block_on(read_frames(chunked(body, chunk), 4 << 20, 10))
    }

    #[test]
    fn a_valid_frame_is_read_whatever_the_chunking() {
        let bytes = envelope();
        let body = body_of(&bytes);
        for chunk in [1, 2, 9, 10, 100, body.len()] {
            let frames = read(&body, chunk).expect("frames");
            assert_eq!(frames.len(), 1, "chunk {chunk}");
            assert_eq!(frames.first().map(|f| f.bytes.as_slice()), Some(&bytes[..]));
            assert_eq!(frames.first().map(|f| f.header.id.as_str()), Some(ID));
        }
    }

    #[test]
    fn two_frames_and_trailing_nothing() {
        let bytes = envelope();
        let item = ("message", ID, OP, &bytes[..]);
        let body = encode_frames(&[item, item]).expect("frames");
        assert_eq!(read(&body, 7).expect("frames").len(), 2);
    }

    #[test]
    fn the_declared_size_is_refused_before_the_envelope_is_read() {
        let mut body = vec![7u8];
        body.extend_from_slice(b"message");
        body.extend_from_slice(&uuid_bytes(ID).expect("id"));
        body.extend_from_slice(&uuid_bytes(OP).expect("op"));
        body.extend_from_slice(&u32::MAX.to_be_bytes());
        // Not a byte of the envelope follows: a reader that tried to take u32::MAX bytes would report a truncated frame instead.
        assert_eq!(read(&body, 64), Err(WireError::TooLarge));
        let just_over = u32::try_from(MAX_ENVELOPE + 1).expect("fits");
        let at = body.len() - 4;
        body.truncate(at);
        body.extend_from_slice(&just_over.to_be_bytes());
        assert_eq!(read(&body, 64), Err(WireError::TooLarge));
    }

    #[test]
    fn a_header_that_disagrees_with_the_declaration_is_refused() {
        let bytes = envelope();
        let other = "44444444-4444-4444-8444-444444444444";
        for (entity, id, op, what) in [
            ("chat", ID, OP, "entity"),
            ("message", other, OP, "id"),
            ("message", ID, other, "op"),
        ] {
            let body = encode_frames(&[(entity, id, op, &bytes[..])]).expect("frame");
            assert_eq!(
                read(&body, 64),
                Err(WireError::Malformed("header_mismatch")),
                "{what}"
            );
        }
    }

    #[test]
    fn short_bodies_unknown_formats_and_bad_lengths_are_400s() {
        let bytes = envelope();
        let body = body_of(&bytes);
        for cut in [0, 1, 8, 40, body.len() - 1] {
            assert!(
                matches!(
                    read(body.get(..cut).unwrap_or_default(), 16),
                    Err(WireError::Malformed(_))
                ),
                "cut at {cut}"
            );
        }
        let mut future = bytes.clone();
        future[0] = 2;
        assert_eq!(
            read(&body_of(&future), 64),
            Err(WireError::Malformed("unsupported_format"))
        );
        let mut huge_header = bytes.clone();
        huge_header[5..9].copy_from_slice(&5000u32.to_be_bytes());
        assert_eq!(
            read(&body_of(&huge_header), 64),
            Err(WireError::Malformed("envelope_length"))
        );
        assert_eq!(
            read(&body_of(&bytes[..bytes.len() - 1]), 64),
            Err(WireError::Malformed("envelope_length")),
            "a ciphertext that is not a multiple of 1024 plus 16"
        );
    }

    #[test]
    fn the_body_and_the_batch_are_capped_as_they_are_consumed() {
        let bytes = envelope();
        let item = ("message", ID, OP, &bytes[..]);
        let body = encode_frames(&[item, item, item]).expect("frames");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        assert_eq!(
            runtime.block_on(read_frames(chunked(&body, 100), body.len() - 1, 10)),
            Err(WireError::TooLarge)
        );
        assert_eq!(
            runtime.block_on(read_frames(chunked(&body, 100), body.len(), 2)),
            Err(WireError::TooLarge)
        );
        assert_eq!(
            runtime.block_on(read_frames(chunked(&[], 100), 100, 2)),
            Err(WireError::Malformed("empty_body"))
        );
    }

    #[test]
    fn uuid_text_round_trips_without_forcing_version_bits() {
        let text = "00000000-0000-7000-8000-000000000001";
        let bytes = uuid_bytes(text).expect("bytes");
        assert_eq!(uuid_text(&bytes), text);
    }

    #[test]
    fn keys_are_base64url_of_exactly_32_bytes() {
        let key = [9u8; 32];
        assert_eq!(key32(&to_b64(&key)), Some(key));
        assert_eq!(key32(&to_b64(&key[..31])), None);
        assert_eq!(key32("***"), None);
        assert_eq!(key32(&format!("{}=", to_b64(&key))), None, "no padding");
    }
}
