//! Helpers for the blob suites: real sealed blobs (the format of ADR-0001 §A.2, made by `kinige-crypto`) and the requests of appendix §A.6.
#![allow(dead_code)]

use super::{Acct, Rig};
use axum::body::Body;
use axum::http::{HeaderMap, StatusCode};
use kinige_crypto::blob::seal_blob;
use kinige_crypto::secret::BlobKey;
use serde_json::Value;

/// A synthetic plaintext of `len` bytes that differs with `salt`.
pub fn plain(len: usize, salt: u8) -> Vec<u8> {
    (0..len)
        .map(|i| u8::try_from(i % 251).unwrap_or(0).wrapping_add(salt))
        .collect()
}

/// A real blob (§A.2) of the plaintext `plain(len, salt)`: header, then chunks of `1 MiB + 17`.
pub fn sealed(len: usize, salt: u8) -> Vec<u8> {
    let key = BlobKey::from_bytes([salt; 32]);
    seal_blob(&plain(len, salt), &key).expect("a blob")
}

pub fn blob_uuid(n: u8) -> String {
    format!("0193{n:04x}-0000-7000-8000-{n:012x}")
}

/// `PUT bytes[from..to]` as a part of `blob`, declaring its whole size. The answer's status and JSON.
pub async fn upload(
    rig: &Rig,
    acct: &Acct,
    id: &str,
    blob: &[u8],
    from: usize,
    to: usize,
) -> (StatusCode, Value) {
    let part = blob.get(from..to).expect("a part inside the blob").to_vec();
    let length = part.len();
    upload_sized(rig, acct, id, blob.len(), from, Body::from(part), Some(length)).await
}

/// A request with a body of any kind and no `Content-Length` (a router called in process adds none).
pub async fn upload_raw(
    rig: &Rig,
    acct: &Acct,
    id: &str,
    size: usize,
    offset: usize,
    body: Body,
) -> (StatusCode, Value) {
    upload_sized(rig, acct, id, size, offset, body, None).await
}

pub async fn upload_sized(
    rig: &Rig,
    acct: &Acct,
    id: &str,
    size: usize,
    offset: usize,
    body: Body,
    content_length: Option<usize>,
) -> (StatusCode, Value) {
    let mut headers = vec![
        ("x-kinige-blob-size", size.to_string()),
        ("x-kinige-blob-offset", offset.to_string()),
    ];
    if let Some(length) = content_length {
        headers.push(("content-length", length.to_string()));
    }
    let (status, _, bytes) = rig
        .http
        .request(
            "PUT",
            &format!("/v1/blobs/{id}"),
            Some(&acct.token),
            &headers,
            body,
        )
        .await;
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

/// The whole blob in one request.
pub async fn upload_all(rig: &Rig, acct: &Acct, id: &str, blob: &[u8]) -> (StatusCode, Value) {
    upload(rig, acct, id, blob, 0, blob.len()).await
}

pub async fn download(
    rig: &Rig,
    acct: &Acct,
    id: &str,
    range: Option<&str>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let headers: Vec<(&str, String)> = range
        .map(|r| vec![("range", r.to_string())])
        .unwrap_or_default();
    rig.http
        .request(
            "GET",
            &format!("/v1/blobs/{id}"),
            Some(&acct.token),
            &headers,
            Body::empty(),
        )
        .await
}

pub async fn status(rig: &Rig, acct: &Acct, id: &str) -> Value {
    let (code, _, bytes) = rig
        .http
        .request(
            "GET",
            &format!("/v1/blobs/{id}/status"),
            Some(&acct.token),
            &[],
            Body::empty(),
        )
        .await;
    assert_eq!(code, StatusCode::OK);
    serde_json::from_slice(&bytes).expect("a status")
}

pub async fn delete(rig: &Rig, acct: &Acct, id: &str) -> StatusCode {
    rig.http
        .request(
            "DELETE",
            &format!("/v1/blobs/{id}"),
            Some(&acct.token),
            &[],
            Body::empty(),
        )
        .await
        .0
}

/// The object store's size for a blob, `None` if the object is not there.
pub async fn object_size(rig: &Rig, acct: &Acct, id: &str) -> Option<u64> {
    let key = kinige_server::blobs::object_key(&acct.creds.account_id, id);
    rig.state.blobs.objects.size(&key).await.expect("a size")
}

/// The row of the `blobs` table: `(size, deleted_ms)`.
pub async fn blob_row(rig: &Rig, acct: &Acct, id: &str) -> Option<(i64, Option<i64>)> {
    let client = rig.state.pool.get().await.expect("connection");
    client
        .query_opt(
            "SELECT size, deleted_ms FROM blobs WHERE account_id = $1 AND blob_id = $2",
            &[&acct.creds.account_id, &id],
        )
        .await
        .expect("a row")
        .map(|row| (row.get(0), row.get(1)))
}
