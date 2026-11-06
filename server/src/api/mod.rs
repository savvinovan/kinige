//! The HTTP API (ADR-0001 appendix §A.6): the router, the middleware at the edge, and the helpers the handlers share. Account requests
//! are small JSON bodies read with a cap (`DefaultBodyLimit`); a push is read frame by frame in `wire.rs`.

mod accounts;
mod sync;

use crate::config::MAX_JSON_BODY;
use crate::error::ApiError;
use crate::state::AppState;
use crate::store::sessions::{self, Token};
use axum::Router;
use axum::extract::{ConnectInfo, DefaultBodyLimit, Request, State};
use axum::http::{HeaderMap, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, post};
use serde::de::DeserializeOwned;
use std::net::SocketAddr;

/// The client address the rate limits are keyed on: the connection's, or what a trusted proxy's header says.
#[derive(Debug, Clone)]
pub struct ClientAddr(pub String);

pub fn router(state: AppState) -> Router {
    let small = Router::new()
        .route("/v1/register", post(accounts::register))
        .route("/v1/login/params", post(accounts::login_params))
        .route("/v1/login", post(accounts::login))
        .route("/v1/logout", post(accounts::logout))
        .route("/v1/password/change", post(accounts::change_password))
        .route("/v1/recovery/fetch", post(accounts::recovery_fetch))
        .route("/v1/recovery", post(accounts::recover))
        .route("/v1/words/reissue", post(accounts::reissue_words))
        .route("/v1/pull", post(sync::pull))
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY));
    // The push body has no extractor and so no default limit: `wire::read_frames` enforces its own, as it consumes.
    let push = Router::new().route("/v1/push", post(sync::push));
    Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(small)
        .merge(push)
        .layer(middleware::from_fn_with_state(state.clone(), edge))
        .with_state(state)
}

/// Finds the client address and writes one log line per request: the method, a sanitized path and the status. No query, header or
/// body is ever logged (ADR-0001 §8.8).
async fn edge(State(app): State<AppState>, mut request: Request, next: Next) -> Response {
    let address = client_address(&app, &request);
    request.extensions_mut().insert(ClientAddr(address));
    let method = request.method().clone();
    let path = safe_path(request.uri().path()).to_string();
    let response = next.run(request).await;
    log::info!("{method} {path} -> {}", response.status().as_u16());
    response
}

fn client_address(app: &AppState, request: &Request) -> String {
    if let Some(name) = &app.client_addr_header
        && let Some(value) = request
            .headers()
            .get(name.as_ref())
            .and_then(|v| v.to_str().ok())
    {
        return value.trim().chars().take(64).collect();
    }
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(
            || "unknown".to_string(),
            |ConnectInfo(addr)| addr.ip().to_string(),
        )
}

/// The path for the log: only what a route can look like, so a hostile request line cannot write into the log.
fn safe_path(path: &str) -> &str {
    let plain = path.len() <= 40
        && path
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b == b'/' || b.is_ascii_digit());
    if plain { path } else { "other" }
}

/// A JSON body, or a 400 that names no content.
fn parse<T: DeserializeOwned>(body: &[u8]) -> Result<T, ApiError> {
    serde_json::from_slice(body).map_err(|_| ApiError::BadRequest("json"))
}

/// The session of the request: `Authorization: Bearer <token>`. Anything else is the same `401 auth`.
async fn authed(app: &AppState, headers: &HeaderMap) -> Result<(String, Token), ApiError> {
    let token = bearer(headers).ok_or(ApiError::Auth)?;
    let account = sessions::account_of(&app.pool, &token).await?;
    Ok((account, token))
}

fn bearer(headers: &HeaderMap) -> Option<Token> {
    let value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    Token::parse(value.strip_prefix("Bearer ")?.trim())
}
