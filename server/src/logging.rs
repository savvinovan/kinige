//! The log sink of the binary: one line per record on stderr, `LEVEL target: message`. The container runtime or journald adds the time.
//! The default level is `info`; `KINIGE_LOG=debug` (or `warn`, `error`, `trace`) changes it **for the server's own records only**.
//! Records carry event kinds and error categories only (ADR-0001 §8.8): nothing here formats a body, a token or a key.
//!
//! Dependencies are capped at `warn` whatever the level: `tokio-postgres` logs every statement it runs **with its bound parameters** at
//! `debug` (`executing statement s5 with parameters: [...]`), which would write logins, key hashes and wraps into the log. The leak test
//! of S4c-1 found it; `permits` is what keeps it out.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::io::Write;
use std::sync::OnceLock;

/// The target prefix of this crate's own records (`module_path!()` of the crate).
const OWN_TARGET: &str = "kinige_server";

/// Whether a record is written when the configured level is `configured`: the server's own up to that level, everything else only
/// `warn` and `error`.
pub fn permits(level: Level, target: &str, configured: LevelFilter) -> bool {
    if target == OWN_TARGET || target.starts_with("kinige_server::") {
        level <= configured
    } else {
        level <= Level::Warn && level <= configured
    }
}

struct StderrLog {
    level: LevelFilter,
}

impl Log for StderrLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        permits(metadata.level(), metadata.target(), self.level)
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            // A failed write to stderr has nowhere to be reported.
            let _ = writeln!(
                std::io::stderr().lock(),
                "{} {}: {}",
                record.level(),
                record.target(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

/// Installs the sink once. A second call, or another logger already installed, is ignored.
pub fn init(filter: Option<&str>) {
    let level = filter
        .and_then(|text| text.trim().parse::<Level>().ok())
        .map_or(LevelFilter::Info, |level| level.to_level_filter());
    static SINK: OnceLock<StderrLog> = OnceLock::new();
    let sink = SINK.get_or_init(|| StderrLog { level });
    if log::set_logger(sink).is_ok() {
        log::set_max_level(level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependencies_are_capped_at_warn_at_every_level() {
        for configured in [LevelFilter::Info, LevelFilter::Debug, LevelFilter::Trace] {
            assert!(!permits(Level::Debug, "tokio_postgres::query", configured));
            assert!(!permits(
                Level::Trace,
                "tokio_postgres::connection",
                configured
            ));
            assert!(!permits(Level::Info, "hyper_util::server", configured));
            assert!(permits(Level::Warn, "tokio_postgres::query", configured));
            assert!(permits(Level::Error, "deadpool", configured));
        }
        assert!(!permits(Level::Warn, "tokio_postgres", LevelFilter::Error));
    }

    #[test]
    fn the_servers_own_records_follow_the_configured_level() {
        assert!(permits(
            Level::Info,
            "kinige_server::api",
            LevelFilter::Info
        ));
        assert!(!permits(
            Level::Debug,
            "kinige_server::api",
            LevelFilter::Info
        ));
        assert!(permits(Level::Debug, "kinige_server", LevelFilter::Debug));
        // A crate whose name only begins with ours is a dependency like any other.
        assert!(!permits(
            Level::Info,
            "kinige_server_other",
            LevelFilter::Info
        ));
        assert!(!permits(
            Level::Debug,
            "kinige_serverx::a",
            LevelFilter::Trace
        ));
    }
}
