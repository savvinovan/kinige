//! Push and pull (ADR-0001 §6.1, §A.3, §A.4, §A.6). A push authenticates first (S4c, 2026-11-06: no body is read for a client without a
//! session), then reads the body frame by frame, judging the lengths and the open header of every envelope before its body is buffered,
//! then each envelope goes through steps 3-8 on its own.

use super::body::{BodyError, GuardedBody};
use super::{authed, parse};
use crate::dto::{
    LineageEntry, PullRequest, PullResponse, PulledEnvelope, PushOutcome, PushResponse, PushResult,
};
use crate::error::{ApiError, json};
use crate::rules::pull::{Cursor, Generation, MAX_LINEAGE, Plan};
use crate::rules::push::Answer;
use crate::state::AppState;
use crate::store::envelopes::{self, PushContext};
use crate::wire::{Source, WireError, read_frames, to_b64};
use axum::body::{Body, Bytes};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;

/// The request header with the client's protocol version (§4); a push without it is below every minimum.
const PROTOCOL_HEADER: &str = "x-kinige-protocol";
/// `<generation>.<seq>.<epoch>`: the pusher's cursor and the drop epoch it last saw (appendix §A.3, `stale`).
const CURSOR_HEADER: &str = "x-kinige-cursor";

pub async fn push(
    State(app): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    // Step 1 (S4c, 2026-11-06): the session, before a byte of the body is read: the shape of the bearer token (no database), then the
    // session lookup. A client with no valid session gets 401 whatever it sends.
    let (account, _) = authed(&app, &headers).await?;
    let protocol = protocol_of(&headers)?;
    let cursor = cursor_of(&headers)?;
    if declared_length(&headers).is_some_and(|len| len > app.config.max_push_body) {
        return Err(ApiError::TooLarge);
    }
    // Step 2: lengths and headers, frame by frame, before the bodies are buffered.
    let frames = read_frames(
        BodySource(GuardedBody::new(body, &app.config)),
        app.config.max_push_body,
        app.config.max_batch,
    )
    .await
    .map_err(wire_error)?;
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
    let generation: u64 = request
        .generation
        .parse()
        .map_err(|_| ApiError::BadRequest("generation"))?;
    let cursor = Cursor {
        generation,
        after: bounded(request.after, "after")?,
        known_epoch: bounded(request.known_epoch, "known_epoch")?,
        lineage: lineage_of(request.lineage.unwrap_or_default(), generation)?,
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
            lineage: out
                .lineage
                .iter()
                .map(|g| LineageEntry {
                    generation: g.id.to_string(),
                    cut_ms: g.cut_ms,
                })
                .collect(),
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
        WireError::Timeout => ApiError::Timeout,
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
    // Exactly three parts. A push carries no lineage: a pusher whose generation is not the current one is stale whatever its lineage
    // says (§A.3), so the lineage of S4c-2's branch-cut rule travels in the pull request only.
    let (Some(generation), Some(after), Some(epoch), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad);
    };
    Ok(Cursor {
        lineage: Vec::new(),
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

/// The device's lineage from a pull request: at most `MAX_LINEAGE` entries, ids and cut times that are numbers the database could hold,
/// and, if there is any, **ending with the cursor's generation** (the lineage is the one that put the device into that generation; one
/// that ends elsewhere is malformed, a `400 lineage`, and not guessed at; round 2 of the review of S4c-2). An absent or empty lineage is
/// allowed: it means "unknown", and the rule then counts every drop as missed.
fn lineage_of(entries: Vec<LineageEntry>, generation: u64) -> Result<Vec<Generation>, ApiError> {
    if entries.len() > MAX_LINEAGE {
        return Err(ApiError::BadRequest("lineage"));
    }
    let parsed: Vec<Generation> = entries
        .into_iter()
        .map(|entry| {
            Ok(Generation {
                id: bounded(
                    entry
                        .generation
                        .parse()
                        .map_err(|_| ApiError::BadRequest("lineage"))?,
                    "lineage",
                )?,
                cut_ms: bounded(entry.cut_ms, "lineage")?,
            })
        })
        .collect::<Result<_, ApiError>>()?;
    if parsed.last().is_some_and(|last| last.id != generation) {
        return Err(ApiError::BadRequest("lineage"));
    }
    Ok(parsed)
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

/// The request body as chunks for `wire::read_frames`, with the clock of `api::body` on it.
struct BodySource(GuardedBody);

impl Source for BodySource {
    async fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, WireError> {
        match self.0.next_chunk().await {
            Ok(chunk) => Ok(chunk.map(|data| data.to_vec())),
            Err(BodyError::Timeout) => Err(WireError::Timeout),
            Err(BodyError::Broken) => Err(WireError::Malformed("body")),
        }
    }
}
