//! Сервер MCP на платформах без Unix-сокетов (Windows, слайс 10a): его нет. Доступ ИИ-агентов появится вместе с именованными
//! каналами (слайс 10). Модуль повторяет ту часть `mcp`, которую зовёт остальное приложение, но ничего не слушает, не создаёт
//! файлов и не запускает потоков: окно «Доступ ИИ-агентов» видит ошибку `unsupported` и показывает «пока недоступно».

use kinige_domain::AgentSocket;
use std::path::Path;
use tauri::{AppHandle, Runtime};

/// Код в `AgentSocket::error`: окно показывает вместо выключателя строку «Недоступно на этой платформе».
pub const UNSUPPORTED: &str = "unsupported";

/// Ничего не запускает; в журнал процесса — одна строка без путей.
pub fn start<R: Runtime>(_app: &AppHandle<R>, _data_dir: &Path) {
    log::info!("mcp: на этой платформе недоступен");
}

/// Удалять нечего: сокета не было.
pub fn shutdown<R: Runtime>(_app: &AppHandle<R>) {}

pub fn socket_info<R: Runtime>(_app: &AppHandle<R>) -> AgentSocket {
    AgentSocket {
        error: Some(UNSUPPORTED.to_string()),
        ..AgentSocket::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::mock_app;

    #[test]
    fn the_socket_info_says_unsupported_and_offers_no_connection_snippets() {
        let app = mock_app();
        let info = socket_info(app.handle());
        assert!(!info.listening);
        assert_eq!(info.error.as_deref(), Some(UNSUPPORTED));
        assert!(info.bridge_path.is_none() && info.socket_path.is_none());
        assert!(info.desktop_config.is_none() && info.code_command.is_none());
    }
}
