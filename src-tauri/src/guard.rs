//! Навигация главного окна (ТЗ §6, §12.3): webview не уходит со страницы приложения. Ссылки из записей открывает
//! системный браузер (`open_external_url`); «Open Link» из системного контекстного меню WKWebView пытается перейти по
//! адресу прямо в окне — этот переход отменяется здесь.

use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};
use url::Url;

/// Порт dev-сервера Vite (`devUrl` в `tauri.conf.json`).
const DEV_PORT: u16 = 1420;

/// Можно ли webview перейти по адресу. Пускаем только страницу самого приложения:
/// `tauri://localhost` (macOS, Linux), `http(s)://tauri.localhost` (Windows) и в dev-режиме `http://localhost:1420`.
/// Всё остальное — внешние сайты, `file:`, `data:`, `javascript:`, `about:` — нет.
pub fn navigation_allowed(url: &Url, dev: bool) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => {
            url.host_str() == Some("tauri.localhost")
                || (dev
                    && url.scheme() == "http"
                    && url.host_str() == Some("localhost")
                    && url.port() == Some(DEV_PORT))
        }
        _ => false,
    }
}

/// Плагин, который ставит предикат на каждую навигацию окон приложения.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("kinige-guard")
        .on_navigation(|_webview, url| navigation_allowed(url, tauri::is_dev()))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(url: &str, dev: bool) -> bool {
        navigation_allowed(&Url::parse(url).unwrap(), dev)
    }

    #[test]
    fn app_pages_are_allowed() {
        assert!(allowed("tauri://localhost/", false));
        assert!(allowed("tauri://localhost/index.html", false));
        assert!(allowed("http://tauri.localhost/", false));
        assert!(allowed("https://tauri.localhost/index.html", false));
        assert!(allowed("http://localhost:1420/", true));
        assert!(allowed("http://localhost:1420/#/x", true));
    }

    #[test]
    fn dev_server_is_not_allowed_in_release() {
        assert!(!allowed("http://localhost:1420/", false));
    }

    #[test]
    fn everything_else_is_cancelled() {
        for url in [
            "https://example.com/",
            "http://example.com/",
            "https://tauri.localhost.evil.example/",
            "https://evil.example/tauri.localhost",
            "http://localhost:8080/",
            "http://localhost/",
            "https://localhost:1420/",
            "http://localhost.evil.example:1420/",
            "tauri://evil/",
            "file:///etc/passwd",
            "data:text/html,<h1>x</h1>",
            "javascript:alert(1)",
            "about:blank",
            "mailto:me@example.com",
            "ipc://localhost/app_info",
            "asset://localhost/x",
            "blob:https://tauri.localhost/uuid",
        ] {
            assert!(!allowed(url, true), "{url} (dev)");
            assert!(!allowed(url, false), "{url}");
        }
    }
}
