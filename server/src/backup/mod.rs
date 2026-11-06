//! Backup and restore (PRD §10.1, ADR-0001 §6.3, §8.9, ADR-0020). `kinige-server backup <dir>` writes one directory per backup under
//! `<dir>`: a dump of the database taken in one `REPEATABLE READ` snapshot (the server's own, `COPY` as text per table, so a restore
//! needs no `pg_dump` of a matching version), the object of every published blob, and a manifest with the cut time and the hash of every
//! file. `kinige-server restore <dir>/<backup>` loads it, into the installation it came from or into a clean one, by the same code.
//!
//! What the dump leaves out, on purpose: **sessions** (a restore must not bring back a session that a logout, a password change or a
//! recovery had ended; every device logs in again) and unfinished uploads (they are files in the staging area, and the client starts
//! over).

mod create;
pub mod manifest;
mod restore;

pub use create::{Pruned, create, prune};
pub use manifest::Manifest;
pub use restore::{Report, restore};

use crate::error::ApiError;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum BackupError {
    /// The database failed; the SQLSTATE was logged where it happened.
    Database,
    /// A file operation failed.
    Io(&'static str),
    /// The object store failed or an object a row refers to is not there.
    Store,
    /// `manifest.json` is missing, cut off or not what this server writes.
    Manifest(&'static str),
    /// A file the manifest lists is not in the directory.
    Missing(String),
    /// A file's length or hash is not the manifest's.
    Hash(String),
    /// The backup was taken from another schema than this server's.
    Schema { backup: i32, server: i32 },
}

impl From<ApiError> for BackupError {
    fn from(_: ApiError) -> BackupError {
        BackupError::Database
    }
}

impl From<tokio_postgres::Error> for BackupError {
    fn from(error: tokio_postgres::Error) -> BackupError {
        match error.code() {
            Some(state) => log::error!("database error, sqlstate {}", state.code()),
            None => log::error!("database error, no sqlstate (connection or protocol)"),
        }
        BackupError::Database
    }
}

impl From<deadpool_postgres::PoolError> for BackupError {
    fn from(_: deadpool_postgres::PoolError) -> BackupError {
        log::error!("database pool error");
        BackupError::Database
    }
}

impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackupError::Database => write!(f, "the database failed"),
            BackupError::Io(what) => write!(f, "a file operation failed ({what})"),
            BackupError::Store => write!(f, "the blob store failed or is missing a blob"),
            BackupError::Manifest(why) => write!(f, "the manifest is not usable: {why}"),
            BackupError::Missing(path) => {
                write!(f, "{path} is listed in the manifest and is not there")
            }
            BackupError::Hash(path) => {
                write!(f, "{path} is not what the manifest says (length or hash)")
            }
            BackupError::Schema { backup, server } => write!(
                f,
                "the backup is of schema {backup} and this server is of schema {server}"
            ),
        }
    }
}

impl std::error::Error for BackupError {}
