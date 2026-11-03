//! The app log: the `log` crate + `tauri-plugin-log`. In an .app started from Finder, stderr goes nowhere,
//! so the main sink is a file in the OS log directory (macOS: `~/Library/Logs/me.savvinov.kinige/kinige.log`).
//!
//! Entry texts, chat names, paths to the user's files and import contents never get into the log:
//! only event kinds and error categories (`StoreError::category`).

use log::LevelFilter;
use tauri::Runtime;
use tauri::plugin::TauriPlugin;
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind, TimezoneStrategy};

/// The page load mark for `scripts/measure-cold-start.sh`: written only with `KINIGE_TIMING=1`, only to stderr.
pub const TIMING_TARGET: &str = "kinige-timing";
/// The life of the capture window for checking in a live window: written only with `KINIGE_TRACE=1`, only to stderr.
pub const TRACE_TARGET: &str = "kinige-trace";

/// The default log level; `KINIGE_LOG=debug` (or `trace`, `warn`, `error`, `off`) changes it for Kinige code.
const DEFAULT_LEVEL: LevelFilter = LevelFilter::Info;
/// The size of one file and the number of kept files (current + archived): together at most ~5 MB.
const MAX_FILE_BYTES: u128 = 1_000_000;
const KEPT_FILES: usize = 5;
const FILE_NAME: &str = "kinige";
/// Kinige code modules: their level is set by `KINIGE_LOG`; foreign crates (tauri, tao, wry) write only from `warn`.
const OWN_MODULES: [&str; 3] = ["kinige_lib", "kinige_store", "kinige_domain"];

/// The log plugin: a file with size rotation; stderr in a dev build and when `KINIGE_LOG`, `KINIGE_TIMING`
/// or `KINIGE_TRACE` is set (a launch from a terminal).
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let level = level_from(std::env::var("KINIGE_LOG").ok().as_deref());
    let timing = env_set("KINIGE_TIMING");
    let trace = env_set("KINIGE_TRACE");

    let mut builder = Builder::new()
        .clear_targets()
        .level(LevelFilter::Warn)
        .level_for(
            TIMING_TARGET,
            if timing {
                LevelFilter::Info
            } else {
                LevelFilter::Off
            },
        )
        .level_for(
            TRACE_TARGET,
            if trace {
                LevelFilter::Debug
            } else {
                LevelFilter::Off
            },
        )
        .rotation_strategy(RotationStrategy::KeepSome(KEPT_FILES))
        .max_file_size(MAX_FILE_BYTES)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .target(
            // The measurement marks and the window trace are not written to the file.
            Target::new(TargetKind::LogDir {
                file_name: Some(FILE_NAME.into()),
            })
            .filter(|meta| meta.target() != TIMING_TARGET && meta.target() != TRACE_TARGET),
        );
    for module in OWN_MODULES {
        builder = builder.level_for(module, level);
    }
    if cfg!(debug_assertions) || timing || trace || std::env::var_os("KINIGE_LOG").is_some() {
        builder = builder.target(Target::new(TargetKind::Stderr));
    }
    builder.build()
}

fn env_set(name: &str) -> bool {
    std::env::var_os(name).is_some()
}

fn level_from(value: Option<&str>) -> LevelFilter {
    value
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(DEFAULT_LEVEL)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_defaults_to_info() {
        assert_eq!(level_from(None), LevelFilter::Info);
        assert_eq!(level_from(Some("")), LevelFilter::Info);
        assert_eq!(level_from(Some("громко")), LevelFilter::Info);
    }

    #[test]
    fn level_is_taken_from_the_variable() {
        assert_eq!(level_from(Some("debug")), LevelFilter::Debug);
        assert_eq!(level_from(Some(" TRACE ")), LevelFilter::Trace);
        assert_eq!(level_from(Some("off")), LevelFilter::Off);
    }
}
