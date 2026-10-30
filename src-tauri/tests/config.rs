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
    // Сначала команды главного окна, затем только окна настроек, затем окна захвата.
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
            "{command} выдана и окну захвата"
        );
    }
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
        "окну настроек выданы только свои команды и подписка на события: без core:default, масок и плагинов"
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
            "{forbidden}: у окна настроек этого нет"
        );
    }
}

#[test]
fn the_settings_window_is_not_in_the_static_window_list() {
    // Окно создаётся при первом открытии (холодный старт не меняется), capability привязана к его имени.
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
    // Размер окна — из токенов дизайн-системы (`layout.capture.window-*`): страница рассчитана на ровно такое окно.
    let tokens = read("../design/tokens/layout.json");
    let px = |name: &str| -> u64 {
        tokens["layout"]["capture"][name]["$value"]
            .as_str()
            .and_then(|value| value.strip_suffix("px"))
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("нет токена layout.capture.{name} в px"))
    };
    assert_eq!(capture["width"], px("window-width"));
    assert_eq!(capture["height"], px("window-height"));
    // Окно прозрачное и без системной тени: форму панели и тень рисует CSS (поле под тень — `shadow-margin`).
    assert_eq!(capture["transparent"], true);
    assert_eq!(capture["shadow"], false);
    assert_eq!(
        px("window-width"),
        px("panel-width") + 2 * px("shadow-margin")
    );
    // Константы Rust (пределы `capture_resize`, ширина, положение верха панели) совпадают с токенами.
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
    // Рабочая область 3840 × 2160 без Dock: панель по центру, её низ на 12% высоты выше низа, окно ниже панели на 32 px.
    let (x, bottom) = anchor_for((0, 0, 3840, 2160), 584, 32.0);
    assert_eq!(x, 1628);
    assert_eq!(bottom, 1933); // 2160 × 0,88 + 32 = 1932,8
    // Рабочая область со смещением (menu bar сверху, Dock снизу) и второй монитор справа: считается от неё, а не от экрана.
    let (x, bottom) = anchor_for((1440, 30, 1440, 800), 584, 32.0);
    assert_eq!(x, 1440 + 428);
    assert_eq!(bottom, 766); // 30 + 800 × 0,88 + 32
    // Retina: поле под тень 64 физических px.
    assert_eq!(anchor_for((0, 0, 2880, 1800), 1168, 64.0), (856, 1648)); // 1800 × 0,88 + 64
    // Нижний край один и тот же при любой высоте окна: растёт только верх (вверх).
    let bottom = 1933;
    for height in [122_u32, 281, 333, 480, 640] {
        let top = top_for_bottom(bottom, height);
        assert_eq!(top + height as i32, bottom, "высота {height}");
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
