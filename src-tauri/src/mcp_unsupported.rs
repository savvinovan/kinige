//! The MCP server on platforms without Unix sockets (Windows, slice 10a): there is none. AI-agent access comes with
//! named pipes (slice 10). The module repeats the part of `mcp` that the rest of the app calls, but listens on nothing,
//! creates no files and starts no threads: the "AI agent access" page sees the error `unsupported` and says "not
//! available yet".

use kinige_domain::AgentSocket;
use std::path::Path;
use tauri::{AppHandle, Runtime};

/// The code in `AgentSocket::error`: instead of the switch the page shows a line saying the feature is not available here.
pub const UNSUPPORTED: &str = "unsupported";

/// Starts nothing; one line without paths goes to the process log.
pub fn start<R: Runtime>(_app: &AppHandle<R>, _data_dir: &Path) {
    log::info!("mcp: not available on this platform");
}

/// Nothing to remove: there was no socket.
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
