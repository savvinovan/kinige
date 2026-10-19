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

#[test]
fn main_capability_lists_commands_explicitly() {
    let capability = read("capabilities/main.json");
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert_eq!(
        capability["permissions"],
        serde_json::json!(["allow-app-info"]),
        "главному окну выданы только перечисленные команды, без core:default и масок"
    );
}
