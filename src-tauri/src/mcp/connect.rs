//! Сведения для окна «Доступ ИИ-агентов»: состояние сокета и фрагменты подключения (ADR-0009). Конфиги клиентов
//! приложение не правит: пользователь копирует фрагмент сам.

use super::McpHandle;
use kinige_domain::AgentSocket;
use serde_json::json;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

/// Имя моста рядом с главным исполняемым файлом (`Kinige.app/Contents/MacOS/kinige-mcp`).
const BRIDGE_NAME: &str = "kinige-mcp";

/// Мост рядом с приложением. В сборке `dev` его может не быть.
fn bridge_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let bridge = exe.parent()?.join(BRIDGE_NAME);
    bridge.is_file().then_some(bridge)
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

/// Фрагмент `claude_desktop_config.json`.
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

/// Команда Claude Code. `--scope user`: журнал один на все проекты; без этого сервер появился бы только в текущем.
pub(crate) fn code_command(bridge: &Path, socket: &Path) -> String {
    format!(
        "claude mcp add --scope user kinige -- {} --socket {}",
        shell_quote(&bridge.to_string_lossy()),
        shell_quote(&socket.to_string_lossy())
    )
}

/// Одинарные кавычки, если в пути есть что-то кроме безопасных знаков.
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
