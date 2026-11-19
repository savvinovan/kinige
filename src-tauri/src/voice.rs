//! Voice notes in the Tauri layer (slice 7a, PRD §5, §12.3, ADR-0015): the `send_voice` command, the fixed-URL command that opens
//! the system's microphone privacy page, and the decision about the web view's own permission prompts. There are no business
//! rules here (they are in `kinige-domain` and `kinige-store`).

use crate::attachments::{api, header_text};
use crate::capture::{CAPTURE_LABEL, MAIN_LABEL, notify_entries_changed};
use kinige_domain::attachment::{MAX_PASTE_BYTES, waveform_from_hex};
use kinige_domain::groups::INBOX_ID;
use kinige_domain::{ApiError, ErrorCode, MessageView};
use kinige_store::{Now, Store};
use std::sync::{Mutex, MutexGuard, PoisonError};
use tauri::ipc::{InvokeBody, Request, Response};
use tauri::webview::{PageLoadEvent, PermissionKind, PermissionResponse};
use tauri::{AppHandle, Manager, Runtime, State};
#[cfg(any(target_os = "macos", target_os = "windows"))]
use tauri_plugin_opener::OpenerExt;

type ApiResult<T> = Result<T, ApiError>;

/// Which window has the microphone open (or is waiting for the system's answer to the first request). The device is shared by the two
/// windows that record, the main one and the capture window, and each page has its own recorder: this is what keeps them to one
/// recording at a time (KINIG-153). It also tells the capture window not to hide on a loss of focus while its recording runs.
#[derive(Default)]
pub struct MicrophoneClaim(Mutex<Option<&'static str>>);

impl MicrophoneClaim {
    /// Takes the microphone for `label`. False when the other window holds it; taking it again is fine.
    pub fn hold(&self, label: &'static str) -> bool {
        let mut holder = self.holder();
        match *holder {
            Some(other) if other != label => false,
            _ => {
                *holder = Some(label);
                true
            }
        }
    }

    /// Gives the microphone back. A window cannot release the other one's claim.
    pub fn release(&self, label: &str) {
        let mut holder = self.holder();
        if *holder == Some(label) {
            *holder = None;
        }
    }

    pub fn is_held_by(&self, label: &str) -> bool {
        *self.holder() == Some(label)
    }

    fn holder(&self) -> MutexGuard<'_, Option<&'static str>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

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
    let bytes = recording_body(&request)?;
    let chat_id = header_text(&request, "x-chat-id").ok_or(ErrorCode::ChatNotFound)?;
    let (duration_ms, waveform) = recording_headers(&request)?;
    let reply_to = header_text(&request, "x-reply-to");
    store
        .send_voice_with_waveform(
            &chat_id,
            reply_to.as_deref(),
            bytes,
            duration_ms,
            waveform.as_deref(),
            &Now::system(),
        )
        .map_err(api)
}

/// The quick capture window's recording (KINIG-153): the same body and headers as `send_voice` but no chat and no reply, because this
/// window writes only to Inbox and has no way to name another chat (§12.3). The text draft of the window is not touched. The main
/// window hears about the new entry like it does about a text note.
#[tauri::command(async)]
pub fn capture_send_voice<R: Runtime>(
    app: AppHandle<R>,
    store: State<'_, Store>,
    request: Request<'_>,
) -> ApiResult<MessageView> {
    let bytes = recording_body(&request)?;
    let (duration_ms, waveform) = recording_headers(&request)?;
    let sent = store
        .send_voice_with_waveform(
            INBOX_ID,
            None,
            bytes,
            duration_ms,
            waveform.as_deref(),
            &Now::system(),
        )
        .map_err(api)?;
    notify_entries_changed(&app);
    Ok(sent)
}

/// The raw body of a recording: not JSON, and under the 64 MiB cap of the IPC.
fn recording_body<'a>(request: &'a Request<'_>) -> ApiResult<&'a [u8]> {
    let InvokeBody::Raw(bytes) = request.body() else {
        return Err(ErrorCode::AttachmentUnreadable.into());
    };
    if bytes.len() > MAX_PASTE_BYTES {
        return Err(ErrorCode::AttachmentTooLarge.into());
    }
    Ok(bytes)
}

/// `x-duration-ms` (the product limits are the store's) and `x-waveform`. The waveform is cosmetic and local: a header that does not read
/// as 64 peaks is ignored, the note is still saved.
fn recording_headers(request: &Request<'_>) -> ApiResult<(i64, Option<Vec<u8>>)> {
    let duration_ms = header_text(request, "x-duration-ms")
        .as_deref()
        .and_then(parse_duration_ms)
        .ok_or(ErrorCode::AttachmentUnreadable)?;
    let waveform = header_text(request, "x-waveform")
        .as_deref()
        .and_then(waveform_from_hex)
        .filter(|peaks| !peaks.is_empty());
    Ok((duration_ms, waveform))
}

/// A page that starts loading has no recorder yet: a claim left by a page that was reloaded must not lock the other window out. Called
/// from `on_page_load` with every event; only `Started` counts.
pub fn on_page_load_event<R: Runtime>(webview: &tauri::Webview<R>, event: PageLoadEvent) {
    if event == PageLoadEvent::Started {
        release_for(webview.app_handle(), webview.label());
    }
}

/// macOS: the web content process of a page died (WebKit's `webViewWebContentProcessDidTerminate`). With no handler set, tauri-runtime-wry
/// 2.12.3 reloads the page (and `Started` then releases the claim); a handler replaces that default, so this one does both, in the other
/// order of importance: the dead page's claim goes first (its recorder is gone, and the other window must not say "unavailable" until a
/// restart), a capture window that was on screen is hidden (a blank panel that ignores everything must not stay), then the page reloads.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn on_page_terminated<R: Runtime>(webview: &tauri::Webview<R>) {
    release_for(webview.app_handle(), webview.label());
    if webview.label() == CAPTURE_LABEL {
        crate::capture::hide_capture(webview.app_handle());
    }
    if webview.reload().is_err() {
        log::error!("could not reload a page after its web content process ended");
    }
}

/// Takes back the claim of a window label (not the other window's).
fn release_for<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Some(claim) = app.try_state::<MicrophoneClaim>() {
        claim.release(label);
    }
}

/// `held = true`: the window is about to open the microphone; the answer says whether it may (false: the other window records). `held =
/// false`: it let the device go. Both windows have their own command, so the claim is the caller's without a label argument that a page
/// could lie about.
#[tauri::command]
pub fn microphone_hold(claim: State<'_, MicrophoneClaim>, held: bool) -> bool {
    hold_for(&claim, MAIN_LABEL, held)
}

/// The capture window's `microphone_hold`.
#[tauri::command]
pub fn capture_hold_microphone(claim: State<'_, MicrophoneClaim>, held: bool) -> bool {
    hold_for(&claim, CAPTURE_LABEL, held)
}

fn hold_for(claim: &MicrophoneClaim, label: &'static str, held: bool) -> bool {
    if held {
        claim.hold(label)
    } else {
        claim.release(label);
        true
    }
}

/// The stored bytes of a voice note, for the player's waveform decoder (ADR-0015): the same checks as the `audio` route plus a cap of
/// 16 MiB. Raw bytes in the answer. An IPC command and not a fetch of the route, because the route is deliberately not cross-origin
/// readable and the page's `connect-src` stays `ipc:` only.
#[tauri::command(async)]
pub fn read_voice_for_waveform(
    store: State<'_, Store>,
    attachment_id: String,
) -> ApiResult<Response> {
    store
        .audio_for_waveform(&attachment_id)
        .map(Response::new)
        .map_err(api)
}

/// Saves the peaks the player decoded (`peaks` is hex: 128 digits, or empty for "cannot be decoded, do not try again"). Local-only
/// data: it never reaches `core`, the Markdown copy or sync. Garbage is an error and writes nothing.
#[tauri::command(async)]
pub fn set_attachment_waveform(
    store: State<'_, Store>,
    attachment_id: String,
    peaks: String,
) -> ApiResult<()> {
    let peaks = waveform_from_hex(&peaks).ok_or(ErrorCode::AttachmentUnreadable)?;
    store
        .set_attachment_waveform(&attachment_id, &peaks)
        .map_err(api)
}

/// Opens the system's microphone privacy page ("Microphone access is off" → "Open settings"). It only opens a settings page: no
/// dialog, no argument, no data in or out.
#[tauri::command(async)]
pub fn open_microphone_settings<R: Runtime>(app: AppHandle<R>) -> ApiResult<()> {
    open_settings_page(&app)
}

/// The capture window's `open_microphone_settings`: the same fixed page, so its "Microphone access is off" line can offer the button too.
#[tauri::command(async)]
pub fn capture_open_microphone_settings<R: Runtime>(app: AppHandle<R>) -> ApiResult<()> {
    open_settings_page(&app)
}

fn open_settings_page<R: Runtime>(app: &AppHandle<R>) -> ApiResult<()> {
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

/// The web view's own permission prompts (`tauri::Builder::on_permission_request`). Only the main window and the quick capture window
/// (KINIG-153) may use the microphone; the camera, screen capture and geolocation are refused to every window; anything else keeps the
/// platform default (which is what keeps the clipboard-read allowance wry grants on Windows). Without a handler wry on macOS grants
/// every capture request, camera included. On macOS only camera and microphone requests reach the handler; the refusals of geolocation
/// and screen capture are effective on Windows and Linux (ADR-0015).
pub fn permission_decision(label: &str, kind: PermissionKind) -> PermissionResponse {
    match kind {
        PermissionKind::Microphone if label == MAIN_LABEL || label == CAPTURE_LABEL => {
            PermissionResponse::Allow
        }
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
    fn only_the_main_and_the_capture_windows_may_use_the_microphone() {
        use PermissionKind::*;
        use PermissionResponse::*;
        for label in ["main", "capture"] {
            assert_eq!(permission_decision(label, Microphone), Allow, "{label}");
        }
        for label in ["settings", "", "Main", "Capture", "other"] {
            assert_eq!(permission_decision(label, Microphone), Deny, "{label}");
        }
    }

    /// A mock app with the claim managed, a main and a capture window, and `holder` holding the microphone.
    fn app_with(
        holder: &'static str,
    ) -> (
        tauri::App<tauri::test::MockRuntime>,
        tauri::WebviewWindow<tauri::test::MockRuntime>,
        tauri::WebviewWindow<tauri::test::MockRuntime>,
    ) {
        let app = tauri::test::mock_app();
        app.manage(MicrophoneClaim::default());
        let make = |label: &str| {
            tauri::WebviewWindowBuilder::new(&app, label, Default::default())
                .build()
                .expect("a mock window")
        };
        let (main, capture) = (make(MAIN_LABEL), make(CAPTURE_LABEL));
        assert!(app.state::<MicrophoneClaim>().hold(holder));
        (app, main, capture)
    }

    #[test]
    fn a_page_that_starts_loading_drops_its_own_claim_and_not_the_others() {
        // A reloaded page has no recorder: its claim from before the reload must not lock the other window out.
        let (app, main, capture) = app_with(CAPTURE_LABEL);
        on_page_load_event(main.as_ref(), PageLoadEvent::Started);
        assert!(
            app.state::<MicrophoneClaim>().is_held_by(CAPTURE_LABEL),
            "the main page loading is not the capture page's business"
        );
        on_page_load_event(capture.as_ref(), PageLoadEvent::Finished);
        assert!(
            app.state::<MicrophoneClaim>().is_held_by(CAPTURE_LABEL),
            "only the start of a load counts"
        );
        on_page_load_event(capture.as_ref(), PageLoadEvent::Started);
        assert!(!app.state::<MicrophoneClaim>().is_held_by(CAPTURE_LABEL));
        assert!(app.state::<MicrophoneClaim>().hold(MAIN_LABEL));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_dead_web_content_process_gives_its_claim_back_and_the_page_reloads() {
        // The main page dies while recording: the capture window must be able to record at once, not "unavailable until a restart".
        let (app, main, capture) = app_with(MAIN_LABEL);
        on_page_terminated(main.as_ref());
        assert!(!app.state::<MicrophoneClaim>().is_held_by(MAIN_LABEL));
        assert!(app.state::<MicrophoneClaim>().hold(CAPTURE_LABEL));
        // The capture page dies while recording: its claim goes too, and the other window's is untouched.
        on_page_terminated(capture.as_ref());
        assert!(!app.state::<MicrophoneClaim>().is_held_by(CAPTURE_LABEL));
        assert!(app.state::<MicrophoneClaim>().hold(MAIN_LABEL));
        on_page_terminated(capture.as_ref());
        assert!(app.state::<MicrophoneClaim>().is_held_by(MAIN_LABEL));
    }

    #[test]
    fn the_microphone_has_one_holder_at_a_time() {
        let claim = MicrophoneClaim::default();
        assert!(claim.hold("main"));
        assert!(claim.hold("main"), "taking it again is fine");
        assert!(!claim.hold("capture"), "the other window is refused");
        assert!(claim.is_held_by("main") && !claim.is_held_by("capture"));
        claim.release("capture");
        assert!(
            claim.is_held_by("main"),
            "a window cannot release the other one's claim"
        );
        claim.release("main");
        assert!(claim.hold("capture"), "free again once the holder lets go");
        claim.release("main");
        assert!(claim.is_held_by("capture"));
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
