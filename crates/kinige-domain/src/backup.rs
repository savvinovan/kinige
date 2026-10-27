//! Копия файла базы (ТЗ §2.1, §10.4, §12.3, слайс 3): имена файлов, ротация и типы для окон. Без IO; снятие копии и
//! проверка целостности — в `kinige-store`.
//!
//! Копии лежат в папке `backups` рядом с базой:
//! - `daily-YYYY-MM-DD.db` — копия дня (локальная дата), хранятся 7 последних;
//! - `pre-migration-v<схема>-YYYYMMDDTHHMMSSZ.db` — копия перед миграцией существующей базы, где `<схема>` — версия, из
//!   которой мигрируем; хранятся 3 последних.

use serde::Serialize;
use ts_rs::TS;

/// Сколько копий дня хранится (ТЗ §2.1).
pub const KEEP_DAILY: usize = 7;
/// Сколько копий перед миграцией хранится.
pub const KEEP_PRE_MIGRATION: usize = 3;

const DAILY_PREFIX: &str = "daily-";
const PRE_PREFIX: &str = "pre-migration-v";
const SUFFIX: &str = ".db";

/// Вид копии.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "BackupKind.ts")]
pub enum BackupKind {
    /// Копия дня.
    Daily,
    /// Копия перед миграцией базы.
    PreMigration,
}

/// Копия в списке окна «База повреждена».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "BackupInfo.ts")]
pub struct BackupInfo {
    /// Имя файла в папке копий; по нему просят восстановление.
    pub name: String,
    pub kind: BackupKind,
    /// Когда снята (мс).
    #[ts(type = "number")]
    pub created_at: i64,
}

/// Почему копия не снимается.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "BackupError.ts")]
pub enum BackupError {
    /// Папку копий нельзя создать или открыть.
    FolderUnavailable,
    /// Копию записать не удалось.
    WriteFailed,
    /// Проверка целостности нашла повреждение: копия дня не снимается, чтобы испорченная не вытеснила хорошие.
    IntegrityFailed,
    /// Автоудаление корзины не смогло убрать запись (повреждена): запись остаётся в корзине, проверка и копия идут дальше.
    PurgeFailed,
}

/// Состояние копий для «Настроек».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "BackupStatus.ts")]
pub struct BackupStatus {
    /// Папка копий.
    pub folder: Option<String>,
    /// Когда снята последняя копия (мс); `null` — копий ещё нет.
    #[ts(type = "number | null")]
    pub last_at: Option<i64>,
    /// Копий в папке.
    #[ts(type = "number")]
    pub count: i64,
    pub error: Option<BackupError>,
}

/// Состояние запуска: нужна ли страница восстановления вместо журнала.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "StartupState.ts")]
pub struct StartupState {
    /// База повреждена: журнал не открыт, показывается окно «База повреждена».
    pub needs_recovery: bool,
    /// Копии, из которых можно восстановить, новые первыми.
    pub backups: Vec<BackupInfo>,
    /// База создана более новой версией Kinige (§10.4): журнал не открыт, копии не помогут, нужно обновить приложение.
    pub database_too_new: bool,
    /// Папка Markdown-копии, которая при восстановлении будет переименована в `<имя>-before-restore-<метка>` рядом с ней
    /// (записи после снимка копии иначе были бы затёрты); `null` — копия выключена.
    pub mirror_folder: Option<String>,
}

/// Итог восстановления из копии.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "RestoreOutcome.ts")]
pub struct RestoreOutcome {
    /// Куда переехала прежняя папка Markdown-копии (`<имя>-before-restore-<метка>`); `null` — папки не было.
    pub previous_mirror: Option<String>,
}

/// Имя копии дня за локальную дату `YYYY-MM-DD`.
pub fn daily_name(local_date: &str) -> String {
    format!("{DAILY_PREFIX}{local_date}{SUFFIX}")
}

/// Имя копии перед миграцией: версия схемы, из которой мигрируем, и момент (UTC, `YYYYMMDDTHHMMSSZ`).
pub fn pre_migration_name(from_version: i64, utc_stamp: &str) -> String {
    format!("{PRE_PREFIX}{from_version}-{utc_stamp}{SUFFIX}")
}

/// Вид копии по имени файла; чужие имена (в том числе временные `.tmp` и `corrupt-…`) — `None`.
pub fn classify(name: &str) -> Option<BackupKind> {
    let stem = name.strip_suffix(SUFFIX)?;
    if let Some(date) = stem.strip_prefix(DAILY_PREFIX) {
        return crate::journal::is_valid_journal_date(date).then_some(BackupKind::Daily);
    }
    let rest = stem.strip_prefix(PRE_PREFIX)?;
    let (version, stamp) = rest.split_once('-')?;
    let valid = !version.is_empty()
        && version.bytes().all(|b| b.is_ascii_digit())
        && stamp.len() == 16
        && stamp.ends_with('Z')
        && stamp.as_bytes()[8] == b'T'
        && stamp[..8].bytes().all(|b| b.is_ascii_digit())
        && stamp[9..15].bytes().all(|b| b.is_ascii_digit());
    valid.then_some(BackupKind::PreMigration)
}

/// Что удалить после снятия новой копии: старше [`KEEP_DAILY`] копий дня и [`KEEP_PRE_MIGRATION`] копий перед миграцией.
/// Порядок — по имени (дата и метка времени в имени сортируются как строки); чужие файлы не трогаются.
pub fn expired(names: &[String]) -> Vec<String> {
    let mut stale = Vec::new();
    for (kind, keep) in [
        (BackupKind::Daily, KEEP_DAILY),
        (BackupKind::PreMigration, KEEP_PRE_MIGRATION),
    ] {
        let mut of_kind: Vec<&String> = names
            .iter()
            .filter(|name| classify(name) == Some(kind))
            .collect();
        of_kind.sort();
        let cut = of_kind.len().saturating_sub(keep);
        stale.extend(of_kind.into_iter().take(cut).cloned());
    }
    stale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_through_classify() {
        assert_eq!(classify(&daily_name("2026-10-27")), Some(BackupKind::Daily));
        assert_eq!(
            classify(&pre_migration_name(2, "20261027T131500Z")),
            Some(BackupKind::PreMigration)
        );
    }

    #[test]
    fn foreign_files_are_not_backups() {
        for name in [
            "daily-2026-10-27.db.tmp",
            ".daily-2026-10-27.db.tmp",
            "daily-2026-13-27.db",
            "daily-.db",
            "corrupt-1792563120000.db",
            "kinige.db",
            "pre-migration-v-20261027T131500Z.db",
            "pre-migration-v2-20261027131500Z.db",
            "pre-migration-v2-20261027T13150Z.db",
            "notes.txt",
        ] {
            assert_eq!(classify(name), None, "{name}");
        }
    }

    #[test]
    fn rotation_keeps_the_last_seven_daily_and_three_pre_migration() {
        let mut names: Vec<String> = (1..=10)
            .map(|day| daily_name(&format!("2026-10-{day:02}")))
            .collect();
        names.extend((0..5).map(|i| pre_migration_name(1, &format!("2026102{i}T000000Z"))));
        names.push("corrupt-1.db".into());
        names.push("daily-2026-10-05.db.tmp".into());
        let mut stale = expired(&names);
        stale.sort();
        let mut expected: Vec<String> = (1..=3)
            .map(|day| daily_name(&format!("2026-10-{day:02}")))
            .collect();
        expected.extend((0..2).map(|i| pre_migration_name(1, &format!("2026102{i}T000000Z"))));
        expected.sort();
        assert_eq!(stale, expected);
    }

    #[test]
    fn rotation_does_nothing_below_the_limit() {
        let names: Vec<String> = (1..=7)
            .map(|day| daily_name(&format!("2026-10-{day:02}")))
            .collect();
        assert!(expired(&names).is_empty());
        assert!(expired(&[]).is_empty());
    }
}
