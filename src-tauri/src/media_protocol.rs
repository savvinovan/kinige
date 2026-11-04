//! The `kinige-media` protocol (slice 7, ADR-0014): the only way the webview reads attachment files.
//! `kinige-media://localhost/{thumb|full|audio}/<uuid>` (on Windows `http://kinige-media.localhost/...`). `audio` (slice 7a) serves a
//! voice note with `Range` support; see `audio_route`.
//!
//! A custom protocol does not go through the capability system, so the window is checked here: only the webview labelled `main`
//! is answered, the others get 403. The label is fixed when the webview is created, it is not read from the request. The path
//! must be exactly `thumb` or `full` and a lower-case UUID v4; anything else, a purged id or a missing file is 404. A URL never
//! carries a filesystem path. `full` serves only what the store allows (an image of a format we show, up to 64 MiB, never SVG).

use crate::audio_route;
use crate::capture::MAIN_LABEL;
use kinige_store::{Media, MediaVariant, Store};
use tauri::http::{Method, Response, StatusCode, header};
use tauri::{Manager, Runtime};

/// The scheme name.
pub const SCHEME: &str = "kinige-media";
/// How long the webview may keep an answer: thumbnails are rebuilt rarely, ids are never reused.
const CACHE_CONTROL: &str = "private, max-age=300";

/// Registers the protocol on the app builder.
pub fn register<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder.register_asynchronous_uri_scheme_protocol(SCHEME, |ctx, request, responder| {
        let label = ctx.webview_label().to_string();
        let path = request.uri().path().to_string();
        let method = request.method().clone();
        let range = request
            .headers()
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        match ctx.app_handle().try_state::<Store>() {
            Some(store) => answer_range(
                &store,
                &label,
                &method,
                &path,
                range.as_deref(),
                move |response| {
                    responder.respond(response);
                },
            ),
            None => responder.respond(empty(StatusCode::SERVICE_UNAVAILABLE)),
        }
    })
}

/// Answers one request that carries no `Range` header. `done` gets the response, from this thread for a refusal and from the media
/// pool otherwise.
pub fn answer(
    store: &Store,
    label: &str,
    method: &Method,
    path: &str,
    done: impl FnOnce(Response<Vec<u8>>) + Send + 'static,
) {
    answer_range(store, label, method, path, None, done);
}

/// The same with the request's `Range` header, which only the `audio` route reads (slice 7a).
pub fn answer_range(
    store: &Store,
    label: &str,
    method: &Method,
    path: &str,
    range: Option<&str>,
    done: impl FnOnce(Response<Vec<u8>>) + Send + 'static,
) {
    if label != MAIN_LABEL {
        return done(empty(StatusCode::FORBIDDEN));
    }
    if method != Method::GET {
        return done(empty(StatusCode::METHOD_NOT_ALLOWED));
    }
    if let Some(id) = parse_audio(path) {
        let range = range.map(str::to_string);
        return store.audio_async(id, move |audio| match audio {
            Some(audio) => done(audio_route::respond(audio, range.as_deref())),
            None => done(empty(StatusCode::NOT_FOUND)),
        });
    }
    let Some((variant, id)) = parse(path) else {
        return done(empty(StatusCode::NOT_FOUND));
    };
    store.media_async(id, variant, move |media| match media {
        Some(media) => done(found(media)),
        None => done(empty(StatusCode::NOT_FOUND)),
    });
}

/// `/thumb/<uuid>` or `/full/<uuid>`; nothing else. An encoded dot (`%2e%2e`) is not a UUID.
fn parse(path: &str) -> Option<(MediaVariant, String)> {
    let rest = path.strip_prefix('/')?;
    let (kind, id) = rest.split_once('/')?;
    let variant = match kind {
        "thumb" => MediaVariant::Thumb,
        "full" => MediaVariant::Full,
        _ => return None,
    };
    kinige_domain::hlc::is_uuid_v4(id).then(|| (variant, id.to_string()))
}

/// `/audio/<uuid>`; nothing else (slice 7a).
fn parse_audio(path: &str) -> Option<String> {
    let id = path.strip_prefix("/audio/")?;
    kinige_domain::hlc::is_uuid_v4(id).then(|| id.to_string())
}

fn found(media: Media) -> Response<Vec<u8>> {
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, media.content_type)
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, CACHE_CONTROL);
    // Not a script, not a frame: the answer is a picture for an <img>.
    builder = builder.header(
        header::CONTENT_SECURITY_POLICY,
        "default-src 'none'; sandbox",
    );
    builder
        .body(media.bytes)
        .unwrap_or_else(|_| empty(StatusCode::INTERNAL_SERVER_ERROR))
}

/// A refusal: a status and no body. No `Access-Control-Allow-Origin`, ever.
fn empty(status: StatusCode) -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = status;
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d";

    #[test]
    fn only_thumb_and_full_with_a_lower_case_uuid_v4_are_parsed() {
        assert!(matches!(
            parse(&format!("/thumb/{ID}")),
            Some((MediaVariant::Thumb, _))
        ));
        assert!(matches!(
            parse(&format!("/full/{ID}")),
            Some((MediaVariant::Full, _))
        ));
        for bad in [
            "",
            "/",
            "/thumb",
            "/thumb/",
            "/thumb/..",
            "/thumb/%2e%2e",
            "/thumb/%2E%2E%2F",
            "/../full/5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d",
            "/other/5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d",
            "//thumb/5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d",
            "/thumb/5B1C0D9E-3F2A-4A7B-9C8D-1E2F3A4B5C6D",
            "/thumb/5b1c0d9e-3f2a-7a7b-9c8d-1e2f3a4b5c6d",
            "/thumb/5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d/",
            "/thumb/5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d/extra",
            "/thumb/5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d.jpg",
            "/thumb/%35b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d",
            "/thumb/\0",
        ] {
            assert!(parse(bad).is_none(), "{bad:?}");
        }
        assert!(parse(&format!("/thumb/{}", "a".repeat(1 << 16))).is_none());
    }
}
