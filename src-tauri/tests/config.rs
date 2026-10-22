//! Проверки безопасности самих конфигов: CSP и список разрешений (§6, §12.3 ТЗ).

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
        .expect("CSP должен быть задан");

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

/// `allow-list-messages` для команды `list_messages`.
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
    let joined: Vec<&str> = kinige_lib::MAIN_COMMANDS
        .iter()
        .chain(kinige_lib::CAPTURE_COMMANDS)
        .copied()
        .collect();
    assert_eq!(joined, kinige_lib::COMMANDS);
    for command in kinige_lib::CAPTURE_COMMANDS {
        assert!(
            command.starts_with("capture_"),
            "{command}: команды окна захвата — с префиксом"
        );
        assert!(
            !kinige_lib::MAIN_COMMANDS.contains(command),
            "{command} выдана и главному окну"
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
    // Слушать событие «записи изменились» (§12.3); отправлять события и управлять окнами главному окну нельзя.
    expected.push("core:event:allow-listen".into());
    expected.push("core:event:allow-unlisten".into());
    assert_eq!(
        granted, expected,
        "главному окну выданы только свои команды и подписка на событие: без core:default, масок и разрешений плагинов"
    );
    for permission in &granted {
        assert!(
            permission.starts_with("allow-")
                || matches!(
                    permission.as_str(),
                    "core:event:allow-listen" | "core:event:allow-unlisten"
                ),
            "{permission}: разрешения плагинов (opener:, dialog:, global-shortcut:) и прочие core: окну не выдаются"
        );
        assert!(
            !permission.contains("capture"),
            "{permission}: команд захвата у главного окна нет"
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
            "{permission}: окну захвата не выдаются ни чужие команды, ни core:, ни плагины (события в том числе)"
        );
    }
}

#[test]
fn there_are_exactly_two_capability_files() {
    let dir = format!("{}/capabilities", env!("CARGO_MANIFEST_DIR"));
    let mut files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(
        files,
        ["capture.json", "main.json"],
        "у каждого окна своя capability"
    );
}

#[test]
fn build_script_manifest_matches_the_command_list() {
    let build =
        std::fs::read_to_string(format!("{}/build.rs", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let list = build
        .split("commands(&[")
        .nth(1)
        .and_then(|rest| rest.split("])").next())
        .expect("список команд в build.rs");
    // Между кавычками — имена команд: нечётные части при разбиении по «"».
    let declared: Vec<&str> = list.split('"').skip(1).step_by(2).collect();
    assert_eq!(
        declared,
        kinige_lib::COMMANDS,
        "список в build.rs должен совпадать с kinige_lib::COMMANDS"
    );
}

#[test]
fn macos_private_api_is_on_only_for_the_capture_panel() {
    // tauri-nspanel (окно захвата как NSPanel, ADR-0006) требует фичу `macos-private-api`; tauri-build сверяет её с конфигом.
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
        "главное окно грузит только страницу приложения"
    );

    let capture = &windows[1];
    assert_eq!(capture["label"], "capture");
    let url = capture["url"].as_str().unwrap();
    assert!(
        !url.contains("://") && !url.starts_with("//"),
        "окно захвата грузит страницу приложения, а не внешний адрес: {url}"
    );
    assert_eq!(url, "index.html#capture");
    assert_eq!(
        capture["visible"], false,
        "создаётся скрытым: показ мгновенный"
    );
    assert_eq!(capture["decorations"], false);
    assert_eq!(capture["alwaysOnTop"], true);
    assert_eq!(capture["visibleOnAllWorkspaces"], true);
    assert_eq!(capture["skipTaskbar"], true);
    assert_eq!(capture["resizable"], false);
    assert_eq!(capture["width"], 560);
}
