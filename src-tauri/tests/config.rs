//! Security checks of the configs themselves: the CSP and the permission list (PRD §6, §12.3).

// The whole root of the integration tests is test code: `cfg(test)` gives helpers outside #[test] the same relaxations
// of clippy.toml (unwrap/expect/panic/indexing) as the tests in src/.
#![cfg(test)]

use serde_json::Value;

fn read(path: &str) -> Value {
    let text = std::fs::read_to_string(format!("{}/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn production_csp_forbids_inline_and_remote_code() {
    let conf = read("tauri.conf.json");
    let csp = conf["app"]["security"]["csp"]
        .as_object()
        .expect("the CSP must be set");

    assert_eq!(csp["default-src"], "'self'");
    assert_eq!(csp["script-src"], "'self'");
    for (directive, value) in csp {
        let value = value.as_str().unwrap();
        assert!(
            !value.contains("unsafe-inline") && !value.contains("unsafe-eval"),
            "{directive}: {value}"
        );
        assert!(!value.contains('*'), "{directive}: {value}");
    }
}

#[test]
fn dev_csp_still_forbids_inline_scripts() {
    let conf = read("tauri.conf.json");
    let script = conf["app"]["security"]["devCsp"]["script-src"]
        .as_str()
        .unwrap();
    assert_eq!(script, "'self'");
}

/// `allow-list-messages` for the `list_messages` command.
fn permission(command: &str) -> String {
    format!("allow-{}", command.replace('_', "-"))
}

fn granted(path: &str) -> Vec<String> {
    read(path)["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_string())
        .collect()
}

#[test]
fn command_lists_are_consistent() {
    // First the main window commands, then the settings window only, then the capture window.
    let settings_only = kinige_lib::SETTINGS_COMMANDS
        .iter()
        .filter(|c| !kinige_lib::MAIN_COMMANDS.contains(c));
    let joined: Vec<&str> = kinige_lib::MAIN_COMMANDS
        .iter()
        .chain(settings_only)
        .chain(kinige_lib::CAPTURE_COMMANDS)
        .copied()
        .collect();
    assert_eq!(joined, kinige_lib::COMMANDS);
    for command in kinige_lib::SETTINGS_COMMANDS {
        assert!(
            !kinige_lib::CAPTURE_COMMANDS.contains(command),
            "{command} is also granted to the capture window"
        );
    }
    for command in kinige_lib::CAPTURE_COMMANDS {
        assert!(
            command.starts_with("capture_"),
            "{command}: the capture window commands have the prefix"
        );
        assert!(
            !kinige_lib::MAIN_COMMANDS.contains(command),
            "{command} is also granted to the main window"
        );
    }
}

#[test]
fn main_capability_lists_exactly_its_commands_and_the_event_subscription() {
    let capability = read("capabilities/main.json");
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    let granted = granted("capabilities/main.json");
    let mut expected: Vec<String> = kinige_lib::MAIN_COMMANDS
        .iter()
        .map(|c| permission(c))
        .collect();
    // Listening to the "entries changed" event (§12.3); the main window may not send events or manage windows.
    expected.push("core:event:allow-listen".into());
    expected.push("core:event:allow-unlisten".into());
    assert_eq!(
        granted, expected,
        "the main window is granted only its own commands and an event subscription: no core:default, no wildcards and no plugin permissions"
    );
    for permission in &granted {
        assert!(
            permission.starts_with("allow-")
                || matches!(
                    permission.as_str(),
                    "core:event:allow-listen" | "core:event:allow-unlisten"
                ),
            "{permission}: plugin permissions (opener:, dialog:, global-shortcut:) and other core: ones are not granted to the window"
        );
        assert!(
            !permission.contains("capture"),
            "{permission}: the main window has no capture commands"
        );
    }
}

#[test]
fn capture_capability_lists_exactly_the_capture_commands() {
    let capability = read("capabilities/capture.json");
    assert_eq!(capability["windows"], serde_json::json!(["capture"]));
    let granted = granted("capabilities/capture.json");
    let expected: Vec<String> = kinige_lib::CAPTURE_COMMANDS
        .iter()
        .map(|c| permission(c))
        .collect();
    assert_eq!(granted, expected);
    for permission in &granted {
        assert!(
            permission.starts_with("allow-capture-") && !permission.contains(':'),
            "{permission}: the capture window is granted neither foreign commands, nor core:, nor plugins (events included)"
        );
    }
}

#[test]
fn settings_capability_lists_exactly_its_commands_and_the_event_subscription() {
    let capability = read("capabilities/settings.json");
    assert_eq!(capability["windows"], serde_json::json!(["settings"]));
    let granted = granted("capabilities/settings.json");
    let mut expected: Vec<String> = kinige_lib::SETTINGS_COMMANDS
        .iter()
        .map(|c| permission(c))
        .collect();
    expected.push("core:event:allow-listen".into());
    expected.push("core:event:allow-unlisten".into());
    assert_eq!(
        granted, expected,
        "the settings window is granted only its own commands and an event subscription: no core:default, no wildcards and no plugins"
    );
    for forbidden in [
        "list-messages",
        "search-messages",
        "send-message",
        "restore-backup",
        "import",
        "capture",
    ] {
        assert!(
            !granted.iter().any(|p| p.contains(forbidden)),
            "{forbidden}: the settings window does not have this"
        );
    }
}

#[test]
fn settings_window_sizes_match_the_design_tokens() {
    use kinige_lib::settings_window::{HEIGHT, MIN_HEIGHT, MIN_WIDTH, WIDTH};
    let tokens = read("../design/tokens/layout.json");
    let px = |name: &str| -> f64 {
        tokens["layout"]["settings"][name]["$value"]
            .as_str()
            .and_then(|value| value.strip_suffix("px"))
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("no layout.settings.{name} token in px"))
    };
    assert_eq!(px("window-width"), WIDTH);
    assert_eq!(px("window-height"), HEIGHT);
    assert_eq!(px("window-min-width"), MIN_WIDTH);
    assert_eq!(px("window-min-height"), MIN_HEIGHT);
}

#[test]
fn the_settings_window_is_not_in_the_static_window_list() {
    // The window is created on first open (the cold start does not change), the capability is bound to its name.
    let conf = read("tauri.conf.json");
    let labels: Vec<&str> = conf["app"]["windows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["label"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["main", "capture"]);
}

#[test]
fn there_are_exactly_three_capability_files() {
    let dir = format!("{}/capabilities", env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(
        files,
        ["capture.json", "main.json", "settings.json"],
        "each window has its own capability"
    );
}

#[test]
fn build_script_manifest_matches_the_command_list() {
    let build =
        std::fs::read_to_string(format!("{}/build.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let list = build
        .split("let list: &[&'static str] = &[")
        .nth(1)
        .and_then(|rest| rest.split("];").next())
        .expect("the command list in build.rs");
    // Between the quotes are the command names: the odd parts when splitting on `"`.
    let declared: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
    assert_eq!(
        declared,
        kinige_lib::COMMANDS,
        "the list in build.rs must match kinige_lib::COMMANDS"
    );
}

#[test]
fn macos_private_api_is_on_only_for_the_capture_panel() {
    // tauri-nspanel (the capture window as an NSPanel, ADR-0006) requires the `macos-private-api` feature; tauri-build checks it against the config.
    let conf = read("tauri.conf.json");
    assert_eq!(conf["app"]["macOSPrivateApi"], true);
    let manifest =
        std::fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(manifest.contains("macos-private-api"));
    assert!(manifest.contains("tauri-nspanel"));
}

#[test]
fn two_windows_without_remote_urls_and_the_capture_window_is_a_hidden_floating_panel() {
    let conf = read("tauri.conf.json");
    let windows = conf["app"]["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0]["label"], "main");
    assert!(
        windows[0].get("url").is_none(),
        "the main window loads only the app page"
    );

    let capture = &windows[1];
    assert_eq!(capture["label"], "capture");
    let url = capture["url"].as_str().unwrap();
    assert!(
        !url.contains("://") && !url.starts_with("//"),
        "the capture window loads the app page, not an external address: {url}"
    );
    assert_eq!(url, "index.html#capture");
    assert_eq!(
        capture["visible"], false,
        "created hidden: showing is instant"
    );
    assert_eq!(capture["decorations"], false);
    assert_eq!(capture["alwaysOnTop"], true);
    assert_eq!(capture["visibleOnAllWorkspaces"], true);
    assert_eq!(capture["skipTaskbar"], true);
    assert_eq!(capture["resizable"], false);
    // The window size comes from the design system tokens (`layout.capture.window-*`): the page is designed for exactly such a window.
    let tokens = read("../design/tokens/layout.json");
    let px = |name: &str| -> u64 {
        tokens["layout"]["capture"][name]["$value"]
            .as_str()
            .and_then(|value| value.strip_suffix("px"))
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("no layout.capture.{name} token in px"))
    };
    assert_eq!(capture["width"], px("window-width"));
    assert_eq!(capture["height"], px("window-height"));
    // The window is transparent and has no system shadow: the panel shape and shadow are drawn by CSS (the shadow margin is `shadow-margin`).
    assert_eq!(capture["transparent"], true);
    assert_eq!(capture["shadow"], false);
    assert_eq!(
        px("window-width"),
        px("panel-width") + 2 * px("shadow-margin")
    );
    // The Rust constants (the `capture_resize` limits, the width, the panel top position) match the tokens.
    assert_eq!(px("window-width"), kinige_lib::capture::WINDOW_WIDTH as u64);
    assert_eq!(
        px("shadow-margin"),
        kinige_lib::capture::SHADOW_MARGIN as u64
    );
    assert_eq!(
        px("window-min-height"),
        kinige_lib::capture::MIN_HEIGHT as u64
    );
    assert_eq!(
        px("window-max-height"),
        kinige_lib::capture::MAX_HEIGHT as u64
    );
    let initial = px("window-height");
    assert!((px("window-min-height")..=px("window-max-height")).contains(&initial));
}

#[test]
fn capture_window_sits_low_centered_and_keeps_its_bottom_edge_for_any_height() {
    use kinige_lib::capture::{anchor_for, top_for_bottom};
    // A 3840 x 2160 work area without the Dock: the panel is centered, its bottom is 12% of the height above the bottom, the window is 32 px below the panel.
    let (x, bottom) = anchor_for((0, 0, 3840, 2160), 584, 32.0);
    assert_eq!(x, 1628);
    assert_eq!(bottom, 1933); // 2160 × 0,88 + 32 = 1932,8
    // An offset work area (the menu bar on top, the Dock at the bottom) and a second monitor on the right: it is computed from it, not from the screen.
    let (x, bottom) = anchor_for((1440, 30, 1440, 800), 584, 32.0);
    assert_eq!(x, 1440 + 428);
    assert_eq!(bottom, 766); // 30 + 800 × 0,88 + 32
    // Retina: the shadow margin is 64 physical px.
    assert_eq!(anchor_for((0, 0, 2880, 1800), 1168, 64.0), (856, 1648)); // 1800 × 0,88 + 64
    // The bottom edge is the same at any window height: only the top grows (upward).
    let bottom = 1933;
    for height in [122_u32, 281, 333, 480, 640] {
        let top = top_for_bottom(bottom, height);
        assert_eq!(top + height as i32, bottom, "height {height}");
    }
    assert!(top_for_bottom(bottom, 640) < top_for_bottom(bottom, 122));
}

#[test]
fn capture_resize_clamps_the_height_into_the_token_range() {
    use kinige_lib::capture::{MAX_HEIGHT, MIN_HEIGHT, clamp_height};
    assert_eq!(clamp_height(172.4), 172.0);
    assert_eq!(clamp_height(0.0), MIN_HEIGHT);
    assert_eq!(clamp_height(-50.0), MIN_HEIGHT);
    assert_eq!(clamp_height(1.0e9), MAX_HEIGHT);
    assert_eq!(clamp_height(f64::NAN), MIN_HEIGHT);
    assert_eq!(clamp_height(f64::INFINITY), MIN_HEIGHT);
}

/// The "never crash" safety net (KINIG-28, ADR-0009) relies on stack unwinding: `catch_unwind` around the writer, read,
/// maintenance and MCP tasks, the panic hook and the listener restart do not work with `panic = "abort"` in any profile.
#[test]
fn no_profile_aborts_on_panic_and_the_never_panic_lints_are_on() {
    let manifest =
        std::fs::read_to_string(format!("{}/../Cargo.toml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let code: Vec<&str> = manifest
        .lines()
        .map(|line| line.split('#').next().unwrap_or_default().trim())
        .collect();
    assert!(
        !code
            .iter()
            .any(|line| line.replace(' ', "").starts_with("panic=\"abort\"")),
        "panic = \"abort\" disables panic interception"
    );
    for lint in [
        "unwrap_used",
        "expect_used",
        "panic",
        "unreachable",
        "todo",
        "unimplemented",
        "indexing_slicing",
        "string_slice",
    ] {
        assert!(
            code.contains(&format!("{lint} = \"deny\"").as_str()),
            "{lint}"
        );
    }
    assert!(code.contains(&"unsafe_code = \"deny\""));
}

// ---- The screens driver's gate (KINIG-92, ADR-0016) ----

/// The capabilities `tauri-build` resolved for this very build: its own copy in `OUT_DIR`, which differs per feature set (the file in
/// `gen/schemas` is shared by all builds and holds whichever ran last).
fn resolved_capabilities() -> Vec<String> {
    let path = format!("{}/capabilities.json", env!("OUT_DIR"));
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    value.as_object().unwrap().keys().cloned().collect()
}

#[test]
fn the_driver_capability_grants_only_its_reply_and_the_event_subscription() {
    // The file sits in its own directory, which only a build with the feature reads (build.rs).
    let capability = read("capabilities-driver/screens-driver.json");
    assert_eq!(capability["identifier"], "screens-driver");
    assert_eq!(
        capability["windows"],
        serde_json::json!(["main", "settings", "capture"])
    );
    assert_eq!(
        granted("capabilities-driver/screens-driver.json"),
        [
            "allow-screens-driver-reply",
            "core:event:allow-listen",
            "core:event:allow-unlisten"
        ]
    );
    let dir = format!("{}/capabilities-driver", env!("CARGO_MANIFEST_DIR"));
    assert_eq!(std::fs::read_dir(dir).unwrap().count(), 1);
}

#[test]
fn the_driver_feature_is_off_by_default_and_build_rs_refuses_it_without_the_allow_variable() {
    let manifest =
        std::fs::read_to_string(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let features = manifest.split("[features]").nth(1).expect("[features]");
    let features = features.split("\n[").next().unwrap();
    assert!(
        !features
            .lines()
            .any(|line| line.trim_start().starts_with("default")),
        "no default feature: the driver is never on unless asked for"
    );
    assert!(features.contains("screens-driver"));
    let build =
        std::fs::read_to_string(format!("{}/build.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert!(build.contains("CARGO_FEATURE_SCREENS_DRIVER"));
    assert!(build.contains("KINIGE_ALLOW_SCREENS_DRIVER"));
}

#[cfg(not(feature = "screens-driver"))]
#[test]
fn the_default_build_has_no_driver_command_and_no_driver_capability() {
    assert!(
        !kinige_lib::COMMANDS
            .iter()
            .any(|command| command.contains("screens_driver"))
    );
    assert!(!resolved_capabilities().contains(&"screens-driver".to_string()));
    for path in [
        "capabilities/main.json",
        "capabilities/settings.json",
        "capabilities/capture.json",
    ] {
        assert!(
            !granted(path)
                .iter()
                .any(|permission| permission.contains("screens")),
            "{path}"
        );
    }
    let stale = format!(
        "{}/permissions/autogenerated/screens_driver_reply.toml",
        env!("CARGO_MANIFEST_DIR")
    );
    assert!(
        !std::path::Path::new(&stale).exists(),
        "a build without the feature removes the driver's permission file"
    );
}

#[cfg(feature = "screens-driver")]
#[test]
fn the_driver_build_has_its_one_command_and_its_capability_and_the_windows_keep_their_own() {
    assert_eq!(kinige_lib::DRIVER_COMMANDS, ["screens_driver_reply"]);
    assert!(
        !kinige_lib::COMMANDS.contains(&"screens_driver_reply"),
        "COMMANDS is the default build's list"
    );
    assert!(resolved_capabilities().contains(&"screens-driver".to_string()));
    // The three window capabilities are the same files as in the default build and do not mention the driver.
    for path in [
        "capabilities/main.json",
        "capabilities/settings.json",
        "capabilities/capture.json",
    ] {
        assert!(
            !granted(path)
                .iter()
                .any(|permission| permission.contains("screens")),
            "{path}"
        );
    }
}
