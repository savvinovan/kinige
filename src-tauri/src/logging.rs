//! Журнал приложения: крейт `log` + `tauri-plugin-log`. В .app, запущенном из Finder, stderr никуда не попадает,
//! поэтому основной приёмник — файл в каталоге логов ОС (macOS: `~/Library/Logs/me.savvinov.kinige/kinige.log`).
//!
//! В журнал никогда не попадают тексты записей, названия чатов, пути к файлам пользователя и содержимое импорта:
//! только виды событий и категории ошибок (`StoreError::category`).

use log::LevelFilter;
use tauri::Runtime;
use tauri::plugin::TauriPlugin;
use tauri_plugin_log::{Builder, RotationStrategy, Target, TargetKind, TimezoneStrategy};

/// Метка загрузки страницы для `scripts/measure-cold-start.sh`: пишется только при `KINIGE_TIMING=1`, только в stderr.
pub const TIMING_TARGET: &str = "kinige-timing";
/// Жизнь окна захвата для проверки в живом окне: пишется только при `KINIGE_TRACE=1`, только в stderr.
pub const TRACE_TARGET: &str = "kinige-trace";

/// Уровень журнала по умолчанию; `KINIGE_LOG=debug` (или `trace`, `warn`, `error`, `off`) меняет его для кода Kinige.
const DEFAULT_LEVEL: LevelFilter = LevelFilter::Info;
/// Размер одного файла и число хранимых файлов (текущий + архивные): вместе не больше ~5 МБ.
const MAX_FILE_BYTES: u128 = 1_000_000;
const KEPT_FILES: usize = 5;
const FILE_NAME: &str = "kinige";
/// Модули кода Kinige: их уровень задаёт `KINIGE_LOG`; чужие крейты (tauri, tao, wry) пишут только с `warn`.
const OWN_MODULES: [&str; 3] = ["kinige_lib", "kinige_store", "kinige_domain"];

/// Плагин журнала: файл с ротацией по размеру; stderr — в dev-сборке и когда задан `KINIGE_LOG`, `KINIGE_TIMING`
/// или `KINIGE_TRACE` (запуск из терминала).
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
            // Метки замера и трассировка окна в файл не пишутся.
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
