//! The backup lock (ADR-0020, "the cut time"). A backup has a cut time, and the branch-cut rule (ADR-0017) and the deletion journal
//! (ADR-0001 §8.9) compare it with the times the server stamped on a compaction and on an account deletion: "stamped before the cut
//! time" must mean "inside the backup". A stamp is read before the transaction commits, so without a lock a compaction could stamp
//! 10:00:00, the backup read its cut time 10:00:01 and start its snapshot, and the compaction commit afterwards: the stamp says "in the
//! backup", the backup does not have it.
//!
//! The fix is one advisory lock. A compaction and an account deletion take it **shared**, first in their transaction, and read the
//! clock after that. A backup takes it **exclusive** on a connection of its own, reads its cut time, opens its snapshot, and lets go.
//! Every compaction then either committed before the backup got the lock (it is in the snapshot, and its stamp is below the cut time)
//! or took its lock after the backup let go (it is not in the snapshot, and its stamp is at or above the cut time). Pushes and uploads
//! do not take it, so the exclusive hold lasts as long as the slowest running compaction and no longer.

use crate::error::ApiError;
use deadpool_postgres::Transaction;
use tokio_postgres::Client;

/// A constant that no other application takes (next to the migration lock).
pub const BACKUP_LOCK: i64 = 0x004b_494e_4947_4502;

/// First statement of a compaction and of an account deletion. Held to the end of the transaction.
pub async fn shared(tx: &Transaction<'_>) -> Result<(), ApiError> {
    tx.execute("SELECT pg_advisory_xact_lock_shared($1)", &[&BACKUP_LOCK])
        .await?;
    Ok(())
}

/// For a backup, on a connection that stays open until [`release`]: waits for the running compactions and deletions, and keeps new
/// ones out.
pub async fn exclusive(client: &Client) -> Result<(), ApiError> {
    client
        .execute("SELECT pg_advisory_lock($1)", &[&BACKUP_LOCK])
        .await?;
    Ok(())
}

pub async fn release(client: &Client) -> Result<(), ApiError> {
    client
        .execute("SELECT pg_advisory_unlock($1)", &[&BACKUP_LOCK])
        .await?;
    Ok(())
}
