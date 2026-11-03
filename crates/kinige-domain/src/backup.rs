//! Backup of the database file (PRD §2.1, §10.4, §12.3, slice 3): file names, rotation and types for windows. No IO;
//! taking the backup and the integrity check are in `kinige-store`.
//!
//! Backups live in the `backups` folder next to the database:
//! - `daily-YYYY-MM-DD.db` is the daily backup (local date), the last 7 are kept;
//! - `pre-migration-v<schema>-YYYYMMDDTHHMMSSZ.db` is the backup before migrating an existing database, where `<schema>`
//!   is the version we migrate from; the last 3 are kept;
//! - `pre-import-YYYYMMDDTHHMMSSZ.db` is the backup before an import from Telegram (slice 4); the last 3 are kept.

use serde::Serialize;
use ts_rs::TS;

/// How many daily backups are kept (PRD §2.1).
pub const KEEP_DAILY: usize = 7;
/// How many pre-migration backups are kept.
pub const KEEP_PRE_MIGRATION: usize = 3;
/// How many pre-import backups are kept.
pub const KEEP_PRE_IMPORT: usize = 3;

const DAILY_PREFIX: &str = "daily-";
const PRE_PREFIX: &str = "pre-migration-v";
const IMPORT_PREFIX: &str = "pre-import-";
const SUFFIX: &str = ".db";

/// The kind of backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "BackupKind.ts")]
pub enum BackupKind {
    /// The daily backup.
    Daily,
    /// The backup before a database migration.
    PreMigration,
    /// The backup before an import from Telegram.
    PreImport,
}

/// A backup in the list of the "Database is corrupted" window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "BackupInfo.ts")]
pub struct BackupInfo {
    /// The file name in the backup folder; restoring is requested by it.
    pub name: String,
    pub kind: BackupKind,
    /// When it was taken (ms).
    #[ts(type = "number")]
    pub created_at: i64,
}

/// Why a backup is not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "BackupError.ts")]
pub enum BackupError {
    /// The backup folder cannot be created or opened.
    FolderUnavailable,
    /// The backup could not be written.
    WriteFailed,
    /// The integrity check found corruption: the daily backup is not taken, so that a damaged one does not push out the good ones.
    IntegrityFailed,
    /// The trash auto-purge could not remove an entry (it is damaged): the entry stays in the trash, the check and the backup go on.
    PurgeFailed,
}

/// The state of backups for "Settings".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "BackupStatus.ts")]
pub struct BackupStatus {
    /// The backup folder.
    pub folder: Option<String>,
    /// When the last backup was taken (ms); `null` means there are no backups yet.
    #[ts(type = "number | null")]
    pub last_at: Option<i64>,
    /// Backups in the folder.
    #[ts(type = "number")]
    pub count: i64,
    pub error: Option<BackupError>,
}

/// The startup state: whether the recovery page is needed instead of the journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "StartupState.ts")]
pub struct StartupState {
    /// The database is corrupted: the journal is not open, the "Database is corrupted" window is shown.
    pub needs_recovery: bool,
    /// The backups it can be restored from, newest first.
    pub backups: Vec<BackupInfo>,
    /// The database was created by a newer version of Kinige (§10.4): the journal is not open, backups will not help, the app must be updated.
    pub database_too_new: bool,
    /// The journal is open, but the background check in this process has already found corruption in it. The Rust event
    /// "database is corrupted" may arrive before the page subscribes to it (the check runs at startup), so after
    /// subscribing the page asks for the state once more. It is not set while a restore is running.
    pub corruption_found: bool,
}

/// The result of restoring from a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "RestoreOutcome.ts")]
pub struct RestoreOutcome {
    /// Where the previous Markdown copy folder was moved (`<name>-before-restore-<stamp>`); `null` means there was no folder.
    pub previous_mirror: Option<String>,
    /// Where the copy folder named in `settings.json` was moved, if it is a different one (slice 4b); `null` means there was no such folder.
    pub settings_file_mirror: Option<String>,
}

/// The name of the daily backup for the local date `YYYY-MM-DD`.
pub fn daily_name(local_date: &str) -> String {
    format!("{DAILY_PREFIX}{local_date}{SUFFIX}")
}

/// The name of the pre-migration backup: the schema version we migrate from and the moment (UTC, `YYYYMMDDTHHMMSSZ`).
pub fn pre_migration_name(from_version: i64, utc_stamp: &str) -> String {
    format!("{PRE_PREFIX}{from_version}-{utc_stamp}{SUFFIX}")
}

/// The name of the pre-import backup: the moment (UTC, `YYYYMMDDTHHMMSSZ`).
pub fn pre_import_name(utc_stamp: &str) -> String {
    format!("{IMPORT_PREFIX}{utc_stamp}{SUFFIX}")
}

/// `YYYYMMDDTHHMMSSZ`.
fn is_utc_stamp(stamp: &str) -> bool {
    let all_digits = |part: Option<&[u8]>| part.is_some_and(|p| p.iter().all(u8::is_ascii_digit));
    let bytes = stamp.as_bytes();
    bytes.len() == 16
        && bytes.last() == Some(&b'Z')
        && bytes.get(8) == Some(&b'T')
        && all_digits(bytes.get(..8))
        && all_digits(bytes.get(9..15))
}

/// The kind of backup by file name; foreign names (including temporary `.tmp` and `corrupt-…`) give `None`.
pub fn classify(name: &str) -> Option<BackupKind> {
    let stem = name.strip_suffix(SUFFIX)?;
    if let Some(date) = stem.strip_prefix(DAILY_PREFIX) {
        return crate::journal::is_valid_journal_date(date).then_some(BackupKind::Daily);
    }
    if let Some(stamp) = stem.strip_prefix(IMPORT_PREFIX) {
        return is_utc_stamp(stamp).then_some(BackupKind::PreImport);
    }
    let rest = stem.strip_prefix(PRE_PREFIX)?;
    let (version, stamp) = rest.split_once('-')?;
    let valid =
        !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit()) && is_utc_stamp(stamp);
    valid.then_some(BackupKind::PreMigration)
}

/// What to delete after taking a new backup: the daily backups beyond the newest [`KEEP_DAILY`], the pre-migration ones
/// beyond [`KEEP_PRE_MIGRATION`] and the pre-import ones beyond [`KEEP_PRE_IMPORT`]. The order is by name (the date and
/// the timestamp in the name sort as strings); foreign files are not touched.
pub fn expired(names: &[String]) -> Vec<String> {
    let mut stale = Vec::new();
    for (kind, keep) in [
        (BackupKind::Daily, KEEP_DAILY),
        (BackupKind::PreMigration, KEEP_PRE_MIGRATION),
        (BackupKind::PreImport, KEEP_PRE_IMPORT),
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
        assert_eq!(
            pre_import_name("20261028T125000Z"),
            "pre-import-20261028T125000Z.db"
        );
        assert_eq!(
            classify(&pre_import_name("20261028T125000Z")),
            Some(BackupKind::PreImport)
        );
    }

    #[test]
    fn rotation_keeps_three_pre_import_copies_apart_from_other_kinds() {
        let mut names: Vec<String> = (0..5)
            .map(|i| pre_import_name(&format!("2026102{i}T000000Z")))
            .collect();
        names.push(daily_name("2026-10-01"));
        names.push(pre_migration_name(2, "20261001T000000Z"));
        let mut stale = expired(&names);
        stale.sort();
        assert_eq!(
            stale,
            [
                pre_import_name("20261020T000000Z"),
                pre_import_name("20261021T000000Z")
            ]
        );
    }

    #[test]
    fn foreign_files_are_not_backups() {
        for name in [
            "pre-import-.db",
            "pre-import-20261028125000Z.db",
            "pre-import-20261028T125000Z.db.tmp",
            ".pre-import-20261028T125000Z.db.tmp",
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
