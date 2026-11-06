//! Blobs (ADR-0001 §7, appendix §A.2, ADR-0020): the bytes of attachments, end-to-end encrypted by the client, kept by `(account_id,
//! remote_blob_id)`. The server never opens one; it checks the length the format fixes and keeps the bytes.
//!
//! An upload is a file in [`staging`] until it is complete; publishing puts it into the [`objects`] store (disk or S3) and records a row
//! in `blobs`. After that the object never changes. A delete only marks the row; [`sweep`] removes the object when it is older than the
//! backup retention plus one backup interval, so every backup that can still be restored finds its blobs.

pub mod objects;
pub mod settings;
pub mod staging;
pub mod sweep;

pub use crate::rules::blob::sealed_len_max;
use kinige_domain::hlc::is_uuid;
use objects::Objects;
use staging::Staging;

/// The store and the staging area of the running server.
#[derive(Clone)]
pub struct Blobs {
    pub objects: Objects,
    pub staging: Staging,
}

impl Blobs {
    pub fn new(objects: Objects, staging: Staging) -> Blobs {
        Blobs { objects, staging }
    }
}

/// The key of a blob in the object store.
pub fn object_key(account: &str, blob: &str) -> String {
    format!("{account}/{blob}")
}

/// `(account_id, blob_id)` of an object key; `None` for a key that is not one of ours (the sweep leaves such objects alone).
pub fn parse_key(key: &str) -> Option<(String, String)> {
    let (account, blob) = key.split_once('/')?;
    (is_uuid(account) && is_uuid(blob)).then(|| (account.to_string(), blob.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "00000001-0000-4000-8000-000000000001";
    const B: &str = "00000002-0000-4000-8000-000000000002";

    #[test]
    fn a_key_round_trips_and_a_foreign_key_is_not_ours() {
        assert_eq!(
            parse_key(&object_key(A, B)),
            Some((A.to_string(), B.to_string()))
        );
        for foreign in ["", "x", "x/y", &format!("{A}/{B}/more"), &format!("../{B}")] {
            assert_eq!(parse_key(foreign), None, "{foreign}");
        }
    }
}
