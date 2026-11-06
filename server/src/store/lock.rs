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

use super::to_u64;
use crate::clock::Clock;
use crate::error::ApiError;
use deadpool_postgres::Transaction;
use tokio_postgres::{Client, GenericClient, NoTls};

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

/// Another constant of ours, for the server's presence (see [`ServerLock`]).
pub const SERVER_LOCK: i64 = 0x004b_494e_4947_4503;

/// A time to stamp on a drop, a deletion, a credential change or a backup's cut (round 1 of the review of S4c-2). The base is the
/// **database's** clock (`clock_timestamp()`, read inside the locked transaction) when `clock` is the production clock, so that every
/// process that touches the installation, on whatever host, stamps on one clock, and the injected time of a test or of the simulation
/// otherwise. Either way it is raised to the **floor**: one more than the newest cut time in the lineage, so that nothing is ever
/// stamped below the cut time of a restore (a restore into a clean installation on a host whose clock is behind the backup's, an NTP step
/// back). A stamp below a cut time would say "inside that backup" of something that is not.
pub async fn stamp(client: &impl GenericClient, clock: &dyn Clock) -> Result<u64, ApiError> {
    let base = if clock.is_wall_clock() {
        let row = client
            .query_one(
                "SELECT (extract(epoch FROM clock_timestamp()) * 1000)::bigint",
                &[],
            )
            .await?;
        to_u64(row.try_get(0)?)
    } else {
        clock.now_ms()
    };
    let floor = client
        .query_one("SELECT COALESCE(max(cut_ms), 0) + 1 FROM lineage", &[])
        .await?;
    Ok(base.max(to_u64(floor.try_get(0)?)))
}

/// What a running server holds, for as long as it runs: a **session** advisory lock, shared, on a connection of its own. A restore tries
/// the same lock exclusively and refuses to run when it cannot get it: a restore replaces the tables under a server that is serving, and
/// a blob publication that straddles it would write rows of one installation about the objects of another.
pub struct ServerLock {
    _client: tokio_postgres::Client,
    driver: tokio::task::JoinHandle<()>,
}

impl ServerLock {
    /// Connects and takes the lock. `Conflict("restore_running")` if a restore holds it.
    pub async fn acquire(config: &tokio_postgres::Config) -> Result<ServerLock, ApiError> {
        let (client, connection) = config.connect(NoTls).await?;
        let driver = tokio::spawn(async move {
            let _ = connection.await;
        });
        let row = client
            .query_one("SELECT pg_try_advisory_lock_shared($1)", &[&SERVER_LOCK])
            .await?;
        if !row.try_get::<_, bool>(0)? {
            driver.abort();
            return Err(ApiError::Conflict("restore_running"));
        }
        Ok(ServerLock {
            _client: client,
            driver,
        })
    }
}

impl Drop for ServerLock {
    /// Ending the connection ends the session, and with it the lock.
    fn drop(&mut self) {
        self.driver.abort();
    }
}

/// For a restore, on a connection that stays open until [`release_restore`]: false if a server holds the lock.
pub async fn try_restore(client: &Client) -> Result<bool, ApiError> {
    let row = client
        .query_one("SELECT pg_try_advisory_lock($1)", &[&SERVER_LOCK])
        .await?;
    Ok(row.try_get(0)?)
}

pub async fn release_restore(client: &Client) -> Result<(), ApiError> {
    client
        .execute("SELECT pg_advisory_unlock($1)", &[&SERVER_LOCK])
        .await?;
    Ok(())
}
