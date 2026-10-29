//! `kinige-mcp --socket <путь>` — мост MCP: строки JSON-RPC из stdin идут в Unix-сокет Kinige, ответы — в stdout
//! (ADR-0009, слайс 4a). Запускает его MCP-клиент (Claude Desktop, Claude Code). Базу мост не открывает.

mod bridge;
mod frames;
mod host;
mod log;
mod scan;

use bridge::{Bridge, Config};
use host::Host;
use std::path::PathBuf;
use std::time::Duration;
use std::{env, io};

/// Сколько ждать сокет приложения: оно может ещё запускаться.
const WAIT: Duration = Duration::from_secs(10);
/// Подмена ожидания в тестах моста, мс.
const TEST_WAIT_MS: &str = "KINIGE_MCP_TEST_WAIT_MS";

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

fn socket_argument() -> Option<PathBuf> {
    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--socket" {
            return args.next().map(PathBuf::from);
        }
    }
    None
}
