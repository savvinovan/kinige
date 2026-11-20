//! The Bot API client (slice 5c, ADR-0021): the app's only outbound HTTP. It runs in Rust, so the web view never makes the request and the
//! CSP does not change. The host is fixed (`api.telegram.org`, https only); every URL is built with a URL builder (path segments and query
//! pairs), never by concatenation; no redirect is followed; no `Accept-Encoding` is sent and any other `Content-Encoding` than `identity` is
//! refused; the User-Agent is `kinige/<version>`; the trust anchors are the OS store and the system proxy is honoured (HTTPS through a proxy
//! is a CONNECT tunnel, so the token stays inside TLS). A body is counted while it streams and never allocated beyond its cap.
//!
//! The token is in the path of every URL, so a failure is turned into a category before anything else happens (`without_url`, then a
//! match on the kind of failure): no error text is formatted, logged or returned.

use body::read_capped;
use reqwest::header::{CONTENT_ENCODING, HeaderMap};
use reqwest::redirect::Policy;
use reqwest::{Client, Response, StatusCode, Url};
use std::time::Duration;

/// The fixed host of the real API.
pub const API_BASE: &str = "https://api.telegram.org";
/// How long to wait for a connection, and for a whole request (the body included).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The caps of the two small answers.
pub const MAX_SET_BYTES: u64 = 1024 * 1024;
pub const MAX_FILE_ANSWER_BYTES: u64 = 16 * 1024;

/// Why a call failed. A category only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotError {
    /// HTTP 401: the token is not accepted.
    TokenRefused,
    /// HTTP 400 or 404: there is no such pack (or file).
    NotFound,
    /// HTTP 429: Telegram asks to wait. Nothing is retried.
    Wait,
    /// No connection, a proxy or TLS failure, a timeout, a dropped body.
    Unreachable,
    /// Any other status, a redirect, a content encoding we did not ask for.
    Bad,
    /// A body over its cap.
    TooLarge,
}

pub struct BotClient {
    http: Client,
    base: Url,
}

impl BotClient {
    /// The client of the real API.
    pub fn production() -> Result<BotClient, BotError> {
        #[cfg(feature = "test-hooks")]
        if let Ok(base) = std::env::var("KINIGE_TEST_BOT_API") {
            log::warn!("kinige-bot-test-hooks-5c: the Bot API base is overridden");
            return BotClient::with_base(&base, false);
        }
        BotClient::with_base(API_BASE, true)
    }

    /// A client for `base`; `https_only` is true for the real one. Only the `test-hooks` feature and the tests call it with another base.
    pub fn with_base(base: &str, https_only: bool) -> Result<BotClient, BotError> {
        BotClient::with_timeouts(base, https_only, CONNECT_TIMEOUT, REQUEST_TIMEOUT)
    }

    /// The same with shorter limits, so that a test of a stalled server does not wait 30 s.
    pub fn with_timeouts(
        base: &str,
        https_only: bool,
        connect: Duration,
        request: Duration,
    ) -> Result<BotClient, BotError> {
        let base = Url::parse(base).map_err(|_| BotError::Bad)?;
        let http = Client::builder()
            .redirect(Policy::none())
            .connect_timeout(connect)
            .timeout(request)
            .https_only(https_only)
            .user_agent(concat!("kinige/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|_| BotError::Unreachable)?;
        Ok(BotClient { http, base })
    }

    /// `getStickerSet?name=<name>`: the body, at most 1 MiB.
    pub async fn get_sticker_set(&self, token: &str, name: &str) -> Result<Vec<u8>, BotError> {
        let url = self.method_url(token, "getStickerSet", ("name", name))?;
        self.get(url, MAX_SET_BYTES).await
    }

    /// `getFile?file_id=<id>`: the body, at most 16 KiB.
    pub async fn get_file(&self, token: &str, file_id: &str) -> Result<Vec<u8>, BotError> {
        let url = self.method_url(token, "getFile", ("file_id", file_id))?;
        self.get(url, MAX_FILE_ANSWER_BYTES).await
    }

    /// `/file/bot<token>/<file_path>`: the bytes, at most `cap`.
    pub async fn download(
        &self,
        token: &str,
        file_path: &str,
        cap: u64,
    ) -> Result<Vec<u8>, BotError> {
        let mut url = self.base.clone();
        {
            let mut segments = url.path_segments_mut().map_err(|()| BotError::Bad)?;
            segments.clear().push("file").push(&format!("bot{token}"));
            segments.extend(file_path.split('/'));
        }
        self.get(url, cap).await
    }

    /// `/bot<token>/<method>?<key>=<value>`, built piece by piece: a value with `&`, `#`, `%` or `/` stays inside its own piece.
    fn method_url(&self, token: &str, method: &str, query: (&str, &str)) -> Result<Url, BotError> {
        let mut url = self.base.clone();
        url.path_segments_mut()
            .map_err(|()| BotError::Bad)?
            .clear()
            .push(&format!("bot{token}"))
            .push(method);
        url.query_pairs_mut().append_pair(query.0, query.1);
        Ok(url)
    }

    async fn get(&self, url: Url, cap: u64) -> Result<Vec<u8>, BotError> {
        let response = self.http.get(url).send().await.map_err(classify)?;
        check(&response)?;
        read_capped(response, cap).await
    }
}

/// The status and the headers decide before a byte of the body is read.
fn check(response: &Response) -> Result<(), BotError> {
    match response.status() {
        StatusCode::UNAUTHORIZED => return Err(BotError::TokenRefused),
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND => return Err(BotError::NotFound),
        StatusCode::TOO_MANY_REQUESTS => return Err(BotError::Wait),
        status if status.is_success() => {}
        _ => return Err(BotError::Bad),
    }
    if !identity_encoding(response.headers()) {
        return Err(BotError::Bad);
    }
    Ok(())
}

/// No `Content-Encoding`, or `identity`: nothing else is decoded here.
fn identity_encoding(headers: &HeaderMap) -> bool {
    headers
        .get_all(CONTENT_ENCODING)
        .iter()
        .all(|value| value.as_bytes().eq_ignore_ascii_case(b"identity"))
}

/// The first step of every error: the URL (which holds the token) is dropped, then only the kind of failure is read.
fn classify(error: reqwest::Error) -> BotError {
    let error = error.without_url();
    if error.is_redirect() {
        BotError::Bad
    } else {
        // A timeout, a refused connection, a TLS or proxy failure, a body that broke off: the network is the reason.
        BotError::Unreachable
    }
}

/// Reading a body with a cap.
mod body {
    use super::{BotError, classify};
    use reqwest::Response;

    /// Streams the body in chunks and stops at `cap` bytes: a declared length over the cap is refused before the first chunk, and the count
    /// is the bytes really received, so a lying `Content-Length` changes nothing. Nothing is allocated for a size the server announces.
    pub async fn read_capped(mut response: Response, cap: u64) -> Result<Vec<u8>, BotError> {
        if response.content_length().is_some_and(|length| length > cap) {
            return Err(BotError::TooLarge);
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(classify)? {
            let total = u64::try_from(body.len())
                .unwrap_or(u64::MAX)
                .saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
            if total > cap {
                return Err(BotError::TooLarge);
            }
            body.extend_from_slice(&chunk);
        }
        Ok(body)
    }
}
