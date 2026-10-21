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

#[test]
fn main_capability_lists_exactly_the_commands_of_the_slice() {
    let capability = read("capabilities/main.json");
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    let granted: Vec<String> = capability["permissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p.as_str().unwrap().to_string())
        .collect();
    let expected: Vec<String> = kinige_lib::COMMANDS.iter().map(|c| permission(c)).collect();
    assert_eq!(
        granted, expected,
        "главному окну выданы только команды слайса: без core:default, масок и разрешений плагинов"
    );
    for permission in &granted {
        assert!(
            permission.starts_with("allow-") && !permission.contains(':'),
            "{permission}: разрешения плагинов (opener:, dialog:) и core: окну не выдаются"
        );
    }
}

#[test]
fn there_is_exactly_one_capability_file() {
    let dir = format!("{}/capabilities", env!("CARGO_MANIFEST_DIR"));
    let files: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        files,
        ["main.json"],
        "второго окна и своей capability в слайсе 1 нет"
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
fn only_one_window_without_remote_urls() {
    let conf = read("tauri.conf.json");
    let windows = conf["app"]["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0]["label"], "main");
    assert!(
        windows[0].get("url").is_none(),
        "окно грузит только страницу приложения"
    );
}
