//! Push and pull (ADR-0001 §6.1, §A.3, §A.4, §A.6). A push is read frame by frame; the lengths and the open header of every envelope are
//! judged before its body is buffered (step 1), then the session (step 2), then each envelope goes through steps 3-8 on its own.

use super::{authed, parse};
use crate::dto::{
    PullRequest, PullResponse, PulledEnvelope, PushOutcome, PushResponse, PushResult,
};
use crate::error::{ApiError, json};
use crate::rules::pull::{Cursor, Plan};
use crate::rules::push::Answer;
use crate::state::AppState;
use crate::store::envelopes::{self, PushContext};
use crate::wire::{Source, WireError, read_frames, to_b64};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;

/// The request header with the client's protocol version (§4); a push without it is below every minimum.
const PROTOCOL_HEADER: &str = "x-kinige-protocol";
/// `<generation>.<seq>.<epoch>`: the pusher's cursor and the drop epoch it last saw (appendix §A.3, `stale`).
const CURSOR_HEADER: &str = "x-kinige-cursor";

pub async fn push(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    let protocol = protocol_of(&headers)?;
    let cursor = cursor_of(&headers)?;
    if declared_length(&headers).is_some_and(|len| len > app.config.max_push_body) {
        return Err(ApiError::TooLarge);
    }
    // Step 1: lengths and headers, frame by frame, before the bodies are buffered.
    let frames = read_frames(
        BodySource(body),
        app.config.max_push_body,
        app.config.max_batch,
    )
    .await
    .map_err(wire_error)?;
    // Step 2: the session.
    let (account, _) = authed(&app, &headers).await?;
    let ctx = PushContext {
        now_ms: app.clock.now_ms(),
        protocol_ok: protocol >= app.config.min_protocol,
        cursor,
    };
    let mut results = Vec::with_capacity(frames.len());
    for frame in &frames {
        let answer = envelopes::push_one(&app.pool, &account, frame, &ctx).await?;
        results.push(result_of(&frame.op, answer));
    }
    Ok(json(StatusCode::OK, &PushResponse { results }))
}

pub async fn pull(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
    let (account, _) = authed(&app, &headers).await?;
    let request: PullRequest = parse(&body)?;
    let cursor = Cursor {
        generation: request
            .generation
            .parse()
            .map_err(|_| ApiError::BadRequest("generation"))?,
        after: bounded(request.after, "after")?,
        known_epoch: bounded(request.known_epoch, "known_epoch")?,
    };
    let limit = request
        .limit
        .unwrap_or(app.config.page_default)
        .clamp(1, app.config.page_max);
    let out = envelopes::pull(&app.pool, &account, &cursor, limit, app.config.page_bytes).await?;
    let generation = out.generation.to_string();
    let response = match out.plan {
        Plan::GenerationChanged { stale } => PullResponse::GenerationChanged {
            generation,
            stale,
            epoch: out.epoch,
        },
        Plan::Stale => PullResponse::Stale {
            generation,
            epoch: out.epoch,
        },
        Plan::Page => PullResponse::Page {
            generation,
            after: cursor.after,
            next: out.next,
            head: out.head,
            epoch: out.epoch,
            items: out
                .items
                .into_iter()
                .map(|item| PulledEnvelope {
                    seq: item.seq,
                    entity: item.entity,
                    id: item.id,
                    op: item.op,
                    envelope: to_b64(&item.bytes),
                })
                .collect(),
        },
    };
    Ok(json(StatusCode::OK, &response))
}

fn result_of(op: &str, answer: Answer) -> PushResult {
    let (result, seq, purged_of) = match answer {
        Answer::Ok { seq, purged_of } => (PushOutcome::Ok, Some(seq), purged_of),
        Answer::Duplicate { seq } => (PushOutcome::Duplicate, Some(seq), None),
        Answer::OpConflict => (PushOutcome::OpConflict, None, None),
        Answer::Purged => (PushOutcome::Purged, None, None),
        Answer::StalePurge => (PushOutcome::StalePurge, None, None),
        Answer::ClockSkew => (PushOutcome::ClockSkew, None, None),
        Answer::TooOld => (PushOutcome::TooOld, None, None),
        Answer::Stale => (PushOutcome::Stale, None, None),
    };
    PushResult {
        op: op.to_string(),
        result,
        seq,
        purged_of,
    }
}

fn wire_error(error: WireError) -> ApiError {
    match error {
        WireError::TooLarge => ApiError::TooLarge,
        WireError::Malformed(code) => ApiError::BadRequest(code),
    }
}

/// A missing protocol header is version 0, which is below every minimum; one that is not a number is a malformed request.
fn protocol_of(headers: &HeaderMap) -> Result<u32, ApiError> {
    match headers.get(PROTOCOL_HEADER) {
        None => Ok(0),
        Some(value) => value
            .to_str()
            .ok()
            .and_then(|text| text.trim().parse().ok())
            .ok_or(ApiError::BadRequest("protocol")),
    }
}

fn cursor_of(headers: &HeaderMap) -> Result<Cursor, ApiError> {
    let bad = ApiError::BadRequest("cursor");
    let text = headers
        .get(CURSOR_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::BadRequest("cursor"))?;
    let mut parts = text.trim().split('.');
    let (Some(generation), Some(after), Some(epoch), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad);
    };
    Ok(Cursor {
        generation: generation
            .parse()
            .map_err(|_| ApiError::BadRequest("cursor"))?,
        after: bounded(
            after.parse().map_err(|_| ApiError::BadRequest("cursor"))?,
            "cursor",
        )?,
        known_epoch: bounded(
            epoch.parse().map_err(|_| ApiError::BadRequest("cursor"))?,
            "cursor",
        )?,
    })
}

/// A number the database stores as `bigint`.
fn bounded(value: u64, what: &'static str) -> Result<u64, ApiError> {
    if i64::try_from(value).is_ok() {
        Ok(value)
    } else {
        Err(ApiError::BadRequest(what))
    }
}

fn declared_length(headers: &HeaderMap) -> Option<usize> {
    headers
        .get(header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .trim()
        .parse()
        .ok()
}

/// The request body as chunks for `wire::read_frames`.
struct BodySource(Body);

impl Source for BodySource {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, WireError> {
        loop {
            match self.0.frame().await {
                None => return Ok(None),
                Some(Err(_)) => return Err(WireError::Malformed("body")),
                Some(Ok(frame)) => {
                    if let Ok(data) = frame.into_data()
                        && !data.is_empty()
                    {
                        return Ok(Some(data.to_vec()));
                    }
                }
            }
        }
    }
}
