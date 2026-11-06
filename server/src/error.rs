//! Every refusal is a status code and a short code in a JSON body, `{"error":"<code>"}`. "No such login" and "wrong key" are the same
//! `401 auth` (ADR-0001 §8.6). Database errors are logged by SQLSTATE only: their messages can carry the value that clashed.

use crate::rate::Limited;
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, PartialEq, Eq)]
pub enum ApiError {
    /// 400: the request is not what the protocol allows. The code names the rule, never the content.
    BadRequest(&'static str),
    /// 401: no session, a bad token, or a wrong key. Nothing says which.
    Auth,
    /// 409: the request is well formed but the state refuses it.
    Conflict(&'static str),
    /// 409: a password change derived against an older credential version (§8.5).
    VersionConflict { current: u64 },
    /// 413: a body or an envelope above its cap, refused before it is buffered.
    TooLarge,
    /// 429.
    RateLimited(Limited),
    /// 500: a database or pool failure. Never the answer to a malformed request.
    Internal,
}

#[derive(Serialize)]
struct Body<'a> {
    error: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_version: Option<u64>,
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Auth => StatusCode::UNAUTHORIZED,
            ApiError::Conflict(_) | ApiError::VersionConflict { .. } => StatusCode::CONFLICT,
            ApiError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::RateLimited(_) => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(code) | ApiError::Conflict(code) => code,
            ApiError::Auth => "auth",
            ApiError::VersionConflict { .. } => "version_conflict",
            ApiError::TooLarge => "too_large",
            ApiError::RateLimited(_) => "rate_limited",
            ApiError::Internal => "internal",
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let current_version = match self {
            ApiError::VersionConflict { current } => Some(current),
            _ => None,
        };
        let body = Body {
            error: self.code(),
            current_version,
        };
        let mut response = json(self.status(), &body);
        if let ApiError::RateLimited(limited) = self
            && let Ok(value) = HeaderValue::from_str(&limited.retry_after_secs.to_string())
        {
            response.headers_mut().insert(header::RETRY_AFTER, value);
        }
        response
    }
}

impl From<tokio_postgres::Error> for ApiError {
    fn from(error: tokio_postgres::Error) -> ApiError {
        match error.code() {
            Some(state) => log::error!("database error, sqlstate {}", state.code()),
            None => log::error!("database error, no sqlstate (connection or protocol)"),
        }
        ApiError::Internal
    }
}

impl From<deadpool_postgres::PoolError> for ApiError {
    fn from(_: deadpool_postgres::PoolError) -> ApiError {
        log::error!("database pool error");
        ApiError::Internal
    }
}

/// A JSON response with the given status. A body that cannot be serialized is a bug and becomes an empty 500.
pub fn json<T: Serialize>(status: StatusCode, body: &T) -> Response {
    match serde_json::to_vec(body) {
        Ok(bytes) => {
            let mut response = (status, bytes).into_response();
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            );
            response
        }
        Err(_) => {
            log::error!("a response body did not serialize");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_and_codes_of_every_error() {
        let limited = Limited {
            retry_after_secs: 7,
        };
        let cases = [
            (ApiError::BadRequest("x"), 400, "x"),
            (ApiError::Auth, 401, "auth"),
            (ApiError::Conflict("login_taken"), 409, "login_taken"),
            (
                ApiError::VersionConflict { current: 3 },
                409,
                "version_conflict",
            ),
            (ApiError::TooLarge, 413, "too_large"),
            (ApiError::RateLimited(limited), 429, "rate_limited"),
            (ApiError::Internal, 500, "internal"),
        ];
        for (error, status, code) in cases {
            assert_eq!(error.status().as_u16(), status);
            assert_eq!(error.code(), code);
        }
    }

    #[test]
    fn a_rate_limit_carries_retry_after() {
        let response = ApiError::RateLimited(Limited {
            retry_after_secs: 7,
        })
        .into_response();
        assert_eq!(
            response
                .headers()
                .get(header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok()),
            Some("7")
        );
    }
}
