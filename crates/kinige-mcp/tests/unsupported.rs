//! Мост на платформе без Unix-сокетов (Windows, слайс 10a): бинарник собирается и на запуске говорит, что доступа ИИ-агентов
//! нет, одной строкой в stderr и кодом 2. Ничего не читает из stdin и не открывает сеть и диск.

#![cfg(test)]
#![cfg(not(unix))]

use std::process::{Command, Stdio};

#[test]
fn the_bridge_says_it_is_unsupported_and_exits_with_code_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_kinige-mcp-bridge"))
        .args(["--socket", "C:\\nowhere\\kinige.sock"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty(), "stdout belongs to the MCP client");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.starts_with("kinige-mcp: unsupported"), "{stderr}");
}

#[test]
fn garbage_arguments_do_not_change_the_answer() {
    let output = Command::new(env!("CARGO_BIN_EXE_kinige-mcp-bridge"))
        .args(["--socket", "--socket", "\u{1b}[31m", "-"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}
