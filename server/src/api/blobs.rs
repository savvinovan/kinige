//! The blob endpoints (ADR-0001 §7, appendix §A.2 and §A.6, ADR-0020). Every request is authenticated first, as a push is: a client with no
//! session gets 401 and the server reads no body for it. Then the id and the declared sizes are judged, and only then is a byte written.

use super::authed;
use super::body::{BodyError, GuardedBody};
use crate::blobs::object_key;
use crate::blobs::objects::ObjectError;
use crate::dto::{BlobPutOutcome, BlobPutResponse, BlobState, BlobStatus};
use crate::error::{ApiError, json};
use crate::rules::blob::{ByteRange, Known, Put, PutRefused, parse_range, plan_put};
use crate::state::AppState;
use crate::store::blobs::{self, Published};
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::StreamExt;
use kinige_domain::hlc::is_uuid;
use tokio::io::AsyncWriteExt;

/// The total size of the blob as stored, in bytes: the framing of §A.2 is checked against it.
const SIZE_HEADER: &str = "x-kinige-blob-size";
/// Where in the blob this request's body starts; absent means 0.
const OFFSET_HEADER: &str = "x-kinige-blob-offset";

/// `PUT /v1/blobs/{id}`: upload the bytes of the body at the offset, resuming an unfinished upload or starting one; publishes the blob
/// when its last byte arrives.
pub async fn put(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    let (account, _) = authed(&app, &headers).await?;
    let blob = blob_id(id)?;
    let size = number(&headers, SIZE_HEADER)?.ok_or(ApiError::BadRequest("blob_size"))?;
    let offset = number(&headers, OFFSET_HEADER)?.unwrap_or(0);
    // One writer per upload: a second request for the same id is told so and nothing is read for it.
    let _busy = app
        .blobs
        .staging
        .try_busy(&account, &blob)
        .ok_or(ApiError::Conflict("busy"))?;
    let (known, _) = blobs::state(&app.pool, &account, &blob).await?;
    let held = app.blobs.staging.find(&account, &blob).await;
    let plan = plan_put(
        known,
        held.as_ref().map(|found| found.state),
        size,
        offset,
        app.config.blob_max,
    )
    .map_err(refusal)?;
    let remaining = size.saturating_sub(offset);
    if plan == Put::Exists {
        return Ok(put_answer(BlobPutOutcome::Exists, size));
    }
    // Declared lengths are judged before a byte is read (the request cap and what the blob still needs).
    let declared = number(&headers, header::CONTENT_LENGTH.as_str())?;
    if declared.is_some_and(|len| len > remaining.min(app.config.blob_request_max)) {
        return Err(ApiError::TooLarge);
    }
    let keep = if plan == Put::Start { 0 } else { offset };
    // A start reserves its room in the quota and creates its staging file in one step under the account's lock; a resume has its room.
    let open = || async {
        app.blobs
            .staging
            .open(&account, &blob, size, keep)
            .await
            .map_err(|error| {
                log::error!("the staging file cannot be opened: {:?}", error.kind());
                ApiError::Internal
            })
    };
    let (mut file, path) = if plan == Put::Start {
        blobs::reserve(
            &app.pool,
            &account,
            size,
            app.config.blob_quota,
            || app.blobs.staging.declared_total(&account, &blob),
            open,
        )
        .await?
    } else {
        open().await?
    };
    let limit = remaining.min(app.config.blob_request_max);
    let mut written = 0u64;
    let mut source = GuardedBody::new(body, &app.config);
    let ended = loop {
        match source.next_chunk().await {
            Ok(None) => break Ok(()),
            Ok(Some(chunk)) => {
                let len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
                if written.saturating_add(len) > limit {
                    break Err(ApiError::TooLarge);
                }
                if let Err(error) = file.write_all(&chunk).await {
                    log::error!("the staging file cannot be written: {:?}", error.kind());
                    break Err(ApiError::Internal);
                }
                written += len;
            }
            Err(BodyError::Timeout) => break Err(ApiError::Timeout),
            Err(BodyError::Broken) => break Err(ApiError::BadRequest("body")),
        }
    };
    // What arrived stays, also when the request failed: the client resumes at `received`.
    if file.sync_data().await.is_err() {
        log::error!("the staging file cannot be synced");
        return Err(ApiError::Internal);
    }
    drop(file);
    if ended.is_err() && keep == 0 && written == 0 {
        // A failed start that stored nothing leaves nothing behind.
        app.blobs.staging.remove(&account, &blob).await;
    }
    ended?;
    let received = offset.saturating_add(written);
    if received < size {
        return Ok(put_answer(BlobPutOutcome::Partial, received));
    }
    publish(&app, &account, &blob, size, &path).await
}

/// `GET /v1/blobs/{id}`: the blob, or the part a `Range` header names (206).
pub async fn get(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (account, _) = authed(&app, &headers).await?;
    let blob = blob_id(id)?;
    let (known, size) = blobs::state(&app.pool, &account, &blob).await?;
    let (Known::Published, Some(size)) = (known, size) else {
        return Err(ApiError::NotFound);
    };
    let asked = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    if headers.get(header::RANGE).is_some() && asked.is_none() {
        return Ok(unsatisfiable(size));
    }
    let Ok(range) = parse_range(asked, size) else {
        return Ok(unsatisfiable(size));
    };
    let whole = range.is_none();
    let range = range.unwrap_or(ByteRange {
        start: 0,
        end: size.saturating_sub(1),
    });
    let stream = match app
        .blobs
        .objects
        .read(&object_key(&account, &blob), range)
        .await
    {
        Ok(stream) => stream,
        Err(ObjectError::NotFound) => {
            log::error!("a published blob is missing from the store");
            return Err(ApiError::NotFound);
        }
        Err(ObjectError::Failed) => return Err(ApiError::Internal),
    };
    let body = Body::from_stream(
        stream.map(|chunk| chunk.map_err(|_| std::io::Error::other("the object store failed"))),
    );
    let mut response = Response::new(body);
    *response.status_mut() = if whole {
        StatusCode::OK
    } else {
        StatusCode::PARTIAL_CONTENT
    };
    let out = response.headers_mut();
    out.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    out.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    out.insert(header::CONTENT_LENGTH, HeaderValue::from(range.count()));
    if !whole
        && let Ok(value) =
            HeaderValue::from_str(&format!("bytes {}-{}/{size}", range.start, range.end))
    {
        out.insert(header::CONTENT_RANGE, value);
    }
    Ok(response)
}

/// `GET /v1/blobs/{id}/status`: what the server holds of the id.
pub async fn status(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (account, _) = authed(&app, &headers).await?;
    let blob = blob_id(id)?;
    let (known, size) = blobs::state(&app.pool, &account, &blob).await?;
    let answer = match known {
        Known::Published => BlobStatus {
            state: BlobState::Published,
            size: size.unwrap_or(0),
            received: size.unwrap_or(0),
        },
        Known::Deleted => BlobStatus {
            state: BlobState::Deleted,
            size: 0,
            received: 0,
        },
        Known::Absent => match app.blobs.staging.find(&account, &blob).await {
            Some(found) => BlobStatus {
                state: BlobState::Uploading,
                size: found.state.declared,
                received: found.state.received,
            },
            None => BlobStatus {
                state: BlobState::None,
                size: 0,
                received: 0,
            },
        },
    };
    Ok(json(StatusCode::OK, &answer))
}

/// `DELETE /v1/blobs/{id}`: deletes a blob of the caller's own account. The row is marked and the object stays until the backup retention
/// is over (ADR-0020); an unfinished upload goes at once. Deleting what is not there is a success. This is the request of the re-upload
/// after `missing` (ADR-0001 §7): the old blob goes after the ack of the new `core`. A purge of an attachment does not need it: the
/// open header of the `purged` envelope names the blob (§5.2) and the push marks it.
pub async fn delete(
    State(app): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (account, _) = authed(&app, &headers).await?;
    let blob = blob_id(id)?;
    let _busy = app
        .blobs
        .staging
        .try_busy(&account, &blob)
        .ok_or(ApiError::Conflict("busy"))?;
    app.blobs.staging.remove(&account, &blob).await;
    let marked = blobs::mark_deleted(&app.pool, &account, &blob, app.clock.now_ms()).await?;
    log::info!("blob delete, marked: {marked}");
    Ok(json(StatusCode::OK, &serde_json::json!({})))
}

/// The last byte is in: the object goes to the store (durable and visible), then the row is written, then the staging file goes.
async fn publish(
    app: &AppState,
    account: &str,
    blob: &str,
    size: u64,
    path: &std::path::Path,
) -> Result<Response, ApiError> {
    let key = object_key(account, blob);
    app.blobs
        .objects
        .put_file(&key, path)
        .await
        .map_err(|_| ApiError::Internal)?;
    match blobs::publish(&app.pool, account, blob, size, app.clock.now_ms()).await? {
        Published::New => {
            app.blobs.staging.remove(account, blob).await;
            log::info!("blob published");
            Ok(put_answer(BlobPutOutcome::Published, size))
        }
        Published::AlreadyThere(known) => {
            // Cannot happen under the busy flag; the object just put is the same id's, so it stays.
            app.blobs.staging.remove(account, blob).await;
            match known {
                Known::Deleted => Err(ApiError::Conflict("gone")),
                _ => Ok(put_answer(BlobPutOutcome::Exists, size)),
            }
        }
        Published::AccountGone => {
            // Only here, where no row of this id exists and no account can have one: never the object of a row that exists.
            let _ = app.blobs.objects.delete(&key).await;
            app.blobs.staging.remove(account, blob).await;
            Err(ApiError::Auth)
        }
    }
}

fn put_answer(result: BlobPutOutcome, received: u64) -> Response {
    json(StatusCode::OK, &BlobPutResponse { result, received })
}

fn refusal(refused: PutRefused) -> ApiError {
    match refused {
        PutRefused::Gone => ApiError::Conflict("gone"),
        PutRefused::Framing => ApiError::BadRequest("blob_framing"),
        PutRefused::TooLarge => ApiError::TooLarge,
        PutRefused::Gap { received } => ApiError::Offset { received },
        PutRefused::SizeMismatch { .. } => ApiError::Conflict("size_mismatch"),
        PutRefused::Overrun => ApiError::BadRequest("blob_offset"),
    }
}

/// 416 with the size the client may ask about.
fn unsatisfiable(size: u64) -> Response {
    let mut response = StatusCode::RANGE_NOT_SATISFIABLE.into_response();
    if let Ok(value) = HeaderValue::from_str(&format!("bytes */{size}")) {
        response.headers_mut().insert(header::CONTENT_RANGE, value);
    }
    response
}

/// The id in the path: a UUID, which is also what makes it safe as a file name and an object key.
fn blob_id(id: String) -> Result<String, ApiError> {
    if is_uuid(&id) {
        Ok(id)
    } else {
        Err(ApiError::BadRequest("blob_id"))
    }
}

/// A header that holds a number; absent is `None`, anything else that is not a number is a malformed request.
fn number(headers: &HeaderMap, name: &str) -> Result<Option<u64>, ApiError> {
    match headers.get(name) {
        None => Ok(None),
        Some(value) => value
            .to_str()
            .ok()
            .and_then(|text| text.trim().parse::<u64>().ok())
            .filter(|n| i64::try_from(*n).is_ok())
            .map(Some)
            .ok_or(ApiError::BadRequest("header")),
    }
}
