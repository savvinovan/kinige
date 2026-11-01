//! The bridge on a platform without Unix sockets (Windows, slice 10a): the binary builds and, when started, says in one
//! stderr line that there is no AI-agent access and exits with code 2. It reads nothing from stdin and touches neither
//! the network nor the disk.

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
