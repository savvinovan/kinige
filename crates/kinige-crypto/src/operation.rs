//! An operation (header and groups) in and out of envelope bytes: the two calls a client makes (ADR-0001 §5.4, §A.4). The pieces
//! (`envelope`, `header`) stay public for the server, which reads headers without a key.

use crate::envelope;
use crate::error::CryptoError;
use crate::header::{Header, build_payload, parse_payload};
use crate::ids::AccountId;
use crate::keys::Dek;
use std::collections::BTreeMap;

/// What an envelope carries: the open header and, for an `upsert`, each group's exact bytes (JSON text).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operation {
    pub header: Header,
    pub groups: BTreeMap<String, String>,
}

/// Header, payload, padding, AEAD. The header must validate and the groups must be exactly the header's.
pub fn seal_operation(
    account: &AccountId,
    dek: &Dek,
    operation: &Operation,
) -> Result<Vec<u8>, CryptoError> {
    let header = operation.header.to_bytes()?;
    let payload = build_payload(&operation.header, &operation.groups)?;
    envelope::seal(account, dek, &header, &payload)
}

/// The client's side of a pull (§A.4 steps 1-3 and 5): lengths, format and key id, the AEAD, then the strict parse of the header
/// and the payload. Errors tell the caller what to do: `UnknownFormat` and `UnknownKeyId` go to `pending_unknown`, `Authentication`
/// stops the page, `Padding` and `Malformed` are quarantined as `malformed`. Matching `op` and `id` against the transport (step 4)
/// is the caller's: it has the transport's values.
pub fn open_operation(
    bytes: &[u8],
    account: &AccountId,
    dek: &Dek,
) -> Result<Operation, CryptoError> {
    let opened = envelope::open(bytes, account, dek)?;
    let header = Header::parse(&opened.header)?;
    let groups = parse_payload(opened.payload.as_slice(), &header)?;
    Ok(Operation { header, groups })
}
