//! Voice notes in the Tauri layer (slice 7a, PRD §5, §12.3, ADR-0015): the `send_voice` command, the fixed-URL command that opens
//! the system's microphone privacy page, and the decision about the web view's own permission prompts. There are no business
//! rules here (they are in `kinige-domain` and `kinige-store`).

use crate::attachments::{api, header_text};
use crate::capture::MAIN_LABEL;
use kinige_domain::attachment::MAX_PASTE_BYTES;
use kinige_domain::{ApiError, ErrorCode, MessageView};
use kinige_store::{Now, Store};
use tauri::ipc::{InvokeBody, Request};
use tauri::webview::{PermissionKind, PermissionResponse};
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_opener::OpenerExt;

type ApiResult<T> = Result<T, ApiError>;

/// The page of the system settings with the microphone switches. A fixed URL: the command takes no argument, so nothing the web
/// view says can reach the opener (`open_external_url` would refuse these schemes, which is why this is a separate command).
#[cfg(target_os = "macos")]
const MICROPHONE_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone";
#[cfg(target_os = "windows")]
const MICROPHONE_SETTINGS_URL: &str = "ms-settings:privacy-microphone";

/// Saves a recording as a new entry (slice 7a): the bytes are the raw body; `x-chat-id`, `x-duration-ms` (the recorder's own clock,
/// decimal digits) and, optionally, `x-reply-to` are headers. The store decides the container by the first bytes and checks the
/// duration, the size, the chat and the reply target; nothing is written on an error.
#[tauri::command(async)]
pub fn send_voice(store: State<'_, Store>, request: Request<'_>) -> ApiResult<MessageView> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(ErrorCode::AttachmentUnreadable.into());
    };
    if bytes.len() > MAX_PASTE_BYTES {
        return Err(ErrorCode::AttachmentTooLarge.into());
    }
    let chat_id = header_text(&request, "x-chat-id").ok_or(ErrorCode::ChatNotFound)?;
    let duration_ms = header_text(&request, "x-duration-ms")
        .as_deref()
        .and_then(parse_duration_ms)
        .ok_or(ErrorCode::AttachmentUnreadable)?;
    let reply_to = header_text(&request, "x-reply-to");
    store
        .send_voice(
            &chat_id,
            reply_to.as_deref(),
            bytes,
            duration_ms,
            &Now::system(),
        )
        .map_err(api)
}

/// Opens the system's microphone privacy page ("Microphone access is off" → "Open settings"). It only opens a settings page: no
/// dialog, no argument, no data in or out.
#[tauri::command(async)]
pub fn open_microphone_settings<R: Runtime>(app: AppHandle<R>) -> ApiResult<()> {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    {
        app.opener()
            .open_url(MICROPHONE_SETTINGS_URL, None::<&str>)
            .map_err(|_| {
                log::warn!("could not open the microphone settings");
                ApiError::from(ErrorCode::Internal)
            })
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let _ = app;
        Err(ErrorCode::Unsupported.into())
    }
}

/// The web view's own permission prompts (`tauri::Builder::on_permission_request`). Only the main window may use the microphone;
/// the camera, screen capture and geolocation are refused to every window; anything else keeps the platform default (which is what
/// keeps the clipboard-read allowance wry grants on Windows). Without a handler wry on macOS grants every capture request, camera
/// included. On macOS only camera and microphone requests reach the handler; the refusals of geolocation and screen capture are
/// effective on Windows and Linux (ADR-0015).
pub fn permission_decision(label: &str, kind: PermissionKind) -> PermissionResponse {
    match kind {
        PermissionKind::Microphone if label == MAIN_LABEL => PermissionResponse::Allow,
        PermissionKind::Microphone
        | PermissionKind::Camera
        | PermissionKind::DisplayCapture
        | PermissionKind::Geolocation => PermissionResponse::Deny,
        _ => PermissionResponse::Default,
    }
}

/// `x-duration-ms`: decimal digits only (no sign, no space, no exponent), at most `i64::MAX`.
fn parse_duration_ms(text: &str) -> Option<i64> {
    if text.is_empty() || text.len() > 19 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_duration_header_is_digits_only() {
        assert_eq!(parse_duration_ms("40000"), Some(40_000));
        assert_eq!(parse_duration_ms("0"), Some(0));
        for bad in [
            "",
            "-1",
            "+5",
            " 5",
            "5 ",
            "1e3",
            "4.5",
            "0x10",
            "９",
            "9223372036854775808",
            "99999999999999999999",
        ] {
            assert_eq!(parse_duration_ms(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn only_the_main_window_may_use_the_microphone() {
        use PermissionKind::*;
        use PermissionResponse::*;
        assert_eq!(permission_decision("main", Microphone), Allow);
        for label in ["capture", "settings", "", "Main", "other"] {
            assert_eq!(permission_decision(label, Microphone), Deny, "{label}");
        }
    }

    #[test]
    fn the_camera_screen_capture_and_geolocation_are_refused_to_every_window() {
        use PermissionKind::*;
        use PermissionResponse::*;
        for label in ["main", "capture", "settings"] {
            for kind in [Camera, DisplayCapture, Geolocation] {
                assert_eq!(permission_decision(label, kind), Deny, "{label} {kind:?}");
            }
        }
    }

    #[test]
    fn a_page_asking_for_audio_and_video_together_is_refused() {
        // The web view answers such a request by asking the handler for each kind; one Deny makes it a Deny.
        use PermissionKind::*;
        use PermissionResponse::*;
        let answers = [Microphone, Camera].map(|kind| permission_decision("main", kind));
        assert_eq!(answers, [Allow, Deny]);
        assert!(answers.contains(&Deny));
    }

    #[test]
    fn everything_else_keeps_the_platform_default() {
        use PermissionKind::*;
        use PermissionResponse::*;
        for label in ["main", "capture", "settings"] {
            for kind in [Notifications, ClipboardRead, Midi, Sensors, LocalFonts] {
                assert_eq!(
                    permission_decision(label, kind),
                    Default,
                    "{label} {kind:?}"
                );
            }
        }
    }
}
