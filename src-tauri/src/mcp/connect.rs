//! Information for the "AI agent access" window: the socket state and connection snippets (ADR-0009). The app does not edit
//! client configs: the user copies the snippet.

use super::McpHandle;
use kinige_domain::AgentSocket;
use serde_json::json;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

/// The bridge name next to the main executable (`Kinige.app/Contents/MacOS/kinige-mcp`).
const BRIDGE_NAME: &str = "kinige-mcp";

/// The bridge next to the app. A `dev` build may not have it.
fn bridge_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bridge = exe.parent()?.join(BRIDGE_NAME);
    // An empty file is a stub from build.rs (a build without a real bridge): the bridge is considered absent.
    let real = std::fs::metadata(&bridge).is_ok_and(|meta| meta.is_file() && meta.len() > 0);
    real.then_some(bridge)
}

pub fn socket_info<R: Runtime>(app: &AppHandle<R>) -> AgentSocket {
    let Some(handle) = app.try_state::<McpHandle>() else {
        return AgentSocket::default();
    };
    let state = &handle.0;
    let bridge = bridge_path();
    let error = state
        .error()
        .map(|error| error.code().to_string())
        .or_else(|| bridge.is_none().then(|| "bridge_missing".to_string()));
    let (desktop_config, code_command) = match (&bridge, &error) {
        (Some(bridge), None) => (
            Some(desktop_config(bridge, state.path())),
            Some(code_command(bridge, state.path())),
        ),
        _ => (None, None),
    };
    AgentSocket {
        listening: state.error().is_none(),
        error,
        bridge_path: bridge.map(|path| path.to_string_lossy().into_owned()),
        socket_path: Some(state.path().to_string_lossy().into_owned()),
        desktop_config,
        code_command,
    }
}

/// A snippet of `claude_desktop_config.json`.
pub(crate) fn desktop_config(bridge: &Path, socket: &Path) -> String {
    let config = json!({
        "mcpServers": {
            "kinige": {
                "command": bridge.to_string_lossy(),
                "args": ["--socket", socket.to_string_lossy()],
            }
        }
    });
    serde_json::to_string_pretty(&config).unwrap_or_default()
}

/// The Claude Code command. `--scope user`: the journal is one for all projects; without it the server would appear only in the current one.
pub(crate) fn code_command(bridge: &Path, socket: &Path) -> String {
    format!(
        "claude mcp add --scope user kinige -- {} --socket {}",
        shell_quote(&bridge.to_string_lossy()),
        shell_quote(&socket.to_string_lossy())
    )
}

/// Single quotes if the path has anything besides safe characters.
fn shell_quote(text: &str) -> String {
    let plain = !text.is_empty()
        && text.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '/' | '.' | '_' | '-' | '+' | ':' | '@' | '%' | ',')
        });
    if plain {
        text.to_string()
    } else {
        format!("'{}'", text.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippets_quote_paths_with_spaces_and_stay_valid_json() {
        let bridge = Path::new("/Applications/Kinige.app/Contents/MacOS/kinige-mcp");
        let socket =
            Path::new("/Users/me/Library/Application Support/me.savvinov.kinige/mcp/kinige.sock");
        let command = code_command(bridge, socket);
        assert_eq!(
            command,
            "claude mcp add --scope user kinige -- /Applications/Kinige.app/Contents/MacOS/kinige-mcp --socket '/Users/me/Library/Application Support/me.savvinov.kinige/mcp/kinige.sock'"
        );
        let config: serde_json::Value =
            serde_json::from_str(&desktop_config(bridge, socket)).unwrap();
        assert_eq!(
            config["mcpServers"]["kinige"]["args"][1],
            socket.to_str().unwrap()
        );
        assert!(code_command(bridge, Path::new("/it's/k.sock")).ends_with(r"'/it'\''s/k.sock'"));
    }
}
