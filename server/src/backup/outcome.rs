//! What a restore that failed leaves to do (KINIG-102). The restore's lock and its load transaction are on one connection, so a lost
//! connection is a lost lock and a rolled-back transaction at once. This tells a lost connection from any other failure, and decides
//! whether the objects the restore copied into the store stay.
//!
//! | Failure         | Connection | Result                          | Copied objects |
//! |-----------------|------------|---------------------------------|----------------|
//! | before `COMMIT` | usable     | the error it was                | removed        |
//! | before `COMMIT` | lost       | [`BackupError::LockLost`]       | removed        |
//! | at `COMMIT`     | usable     | the error it was (rolled back)  | removed        |
//! | at `COMMIT`     | lost       | [`BackupError::OutcomeUnknown`] | kept           |
//!
//! Kept objects are safe: a rerun finds them by hash, and the sweep adopts the unreferenced ones.

use super::BackupError;
use crate::blobs::objects::Objects;
use deadpool_postgres::Object;

/// Why the load failed, and whether it was the commit that failed.
pub(super) struct Failed {
    pub(super) error: BackupError,
    pub(super) at_commit: bool,
}

impl<E: Into<BackupError>> From<E> for Failed {
    fn from(error: E) -> Failed {
        Failed {
            error: error.into(),
            at_commit: false,
        }
    }
}

/// After a failed load: tells a lost connection from any other failure by a trivial statement on it (`Client::is_closed` stays false for a
/// while after the server terminated the backend), removes the copied objects unless the outcome is unknown, and returns the error.
pub(super) async fn settle(
    guard: &Object,
    objects: &Objects,
    copied: Vec<String>,
    failed: Failed,
) -> BackupError {
    let lost = guard.query_one("SELECT 1", &[]).await.is_err();
    let error = match (lost, failed.at_commit) {
        (false, _) => failed.error,
        (true, false) => BackupError::LockLost,
        (true, true) => BackupError::OutcomeUnknown,
    };
    if lost {
        log::error!("the restore's database connection ended: {error}");
    }
    if error != BackupError::OutcomeUnknown {
        for key in copied {
            let _ = objects.delete(&key).await;
        }
    }
    error
}
