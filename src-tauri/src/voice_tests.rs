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
