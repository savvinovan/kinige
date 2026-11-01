//! `kinige-mcp --socket <путь>` — мост MCP: строки JSON-RPC из stdin идут в Unix-сокет Kinige, ответы — в stdout
//! (ADR-0009, слайс 4a). Запускает его MCP-клиент (Claude Desktop, Claude Code). Базу мост не открывает.
//!
//! Только Unix. На Windows (слайс 10a) доступа ИИ-агентов нет до именованных каналов (слайс 10): бинарник собирается, но на
//! первом же запуске говорит об этом одной строкой и завершается с кодом 2.

#[cfg(unix)]
mod bridge;
#[cfg(unix)]
mod frames;
#[cfg(unix)]
mod host;
mod log;
#[cfg(unix)]
mod scan;

#[cfg(unix)]
use bridge::{Bridge, Config};
#[cfg(unix)]
use host::Host;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::time::Duration;
#[cfg(unix)]
use std::{env, io};

/// Сколько ждать сокет приложения: оно может ещё запускаться.
#[cfg(unix)]
const WAIT: Duration = Duration::from_secs(10);
/// Подмена ожидания в тестах моста, мс.
#[cfg(unix)]
const TEST_WAIT_MS: &str = "KINIGE_MCP_TEST_WAIT_MS";

/// Что мост говорит там, где ему нечем соединяться с приложением.
#[cfg(not(unix))]
const UNSUPPORTED: &str = "unsupported: AI agent access is not available on this platform yet";

#[cfg(unix)]
fn main() {
    let Some(socket) = socket_argument() else {
        log::note("usage: kinige-mcp --socket <path>", None);
        std::process::exit(2);
    };
    let wait = env::var(TEST_WAIT_MS)
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .map_or(WAIT, Duration::from_millis);
    let bridge = Bridge::new(
        Config::new(socket, wait),
        Host::locate(),
        Box::new(io::stdout()),
    );
    bridge.run(io::stdin().lock());
}

#[cfg(not(unix))]
fn main() {
    log::note(UNSUPPORTED, None);
    std::process::exit(2);
}

#[cfg(unix)]
fn socket_argument() -> Option<PathBuf> {
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--socket" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
