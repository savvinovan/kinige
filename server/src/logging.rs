//! The log sink of the binary: one line per record on stderr, `LEVEL target: message`. The container runtime or journald adds the time.
//! The default level is `info`; `KINIGE_LOG=debug` (or `warn`, `error`, `trace`) changes it. Records carry event kinds and error
//! categories only (ADR-0001 §8.8): nothing here formats a body, a token or a key.

use log::{Level, LevelFilter, Log, Metadata, Record};
use std::io::Write;
use std::sync::OnceLock;

struct StderrLog {
    level: LevelFilter,
}

impl Log for StderrLog {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.level
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
