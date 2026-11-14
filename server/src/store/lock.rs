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

/// Another constant of ours, for the server's presence (see [`ServerLock`]).
pub const SERVER_LOCK: i64 = 0x004b_494e_4947_4503;

/// What a running server holds, for as long as it runs: a **session** advisory lock, shared, on a connection of its own. A restore tries
/// the same lock exclusively and refuses to run when it cannot get it: a restore replaces the tables under a server that is serving, and
/// a blob publication that straddles it would write rows of one installation about the objects of another.
///
/// **A session lock dies with its connection** (a Postgres restart, a network cut, an administrator's `pg_terminate_backend`), and a
/// server that kept running without it would let a restore start under it (round 2 of the review). So the lock is supervised: when
/// its connection ends, the supervisor takes the lock again on a new one. If that is refused because a restore got the lock in the
/// gap, the server must stop serving ([`ServerLock::lost`] resolves, and `serve` exits with an error); if the database cannot be
/// reached, the supervisor keeps trying (nothing is served meanwhile: the pool fails too).
pub struct ServerLock {
    supervisor: tokio::task::JoinHandle<()>,
    lost: tokio::sync::oneshot::Receiver<()>,
}

/// How a lost lock is retaken: the pause before the first attempt (none in production; a test widens the gap to put a restore in it)
/// and the pause between attempts while the database cannot be reached.
#[derive(Debug, Clone, Copy)]
pub struct Retake {
    pub after_loss: std::time::Duration,
    pub retry: std::time::Duration,
}

impl Default for Retake {
    fn default() -> Retake {
        Retake {
            after_loss: std::time::Duration::ZERO,
            retry: std::time::Duration::from_secs(1),
        }
    }
}

/// The lock on a connection of its own, and the task that drives the connection.
struct Held {
    _client: tokio_postgres::Client,
    driver: tokio::task::JoinHandle<()>,
}

impl ServerLock {
    /// Connects and takes the lock. `Conflict("restore_running")` if a restore holds it.
    pub async fn acquire(config: &tokio_postgres::Config) -> Result<ServerLock, ApiError> {
        ServerLock::acquire_with(config, Retake::default()).await
    }

    pub async fn acquire_with(
        config: &tokio_postgres::Config,
        retake: Retake,
    ) -> Result<ServerLock, ApiError> {
        let held = Held::take(config).await?;
        let (tell, lost) = tokio::sync::oneshot::channel();
        let supervisor = tokio::spawn(supervise(config.clone(), held, retake, tell));
        Ok(ServerLock { supervisor, lost })
    }

    /// Resolves when the lock was lost and could not be taken again because a restore holds it: the server must stop.
    pub async fn lost(&mut self) {
        // A dropped sender is the supervisor ending, which only happens after it has told us or when we are dropped.
        let _ = (&mut self.lost).await;
    }
}

impl Drop for ServerLock {
    /// Ending the supervisor ends its connection, and with it the lock.
    fn drop(&mut self) {
        self.supervisor.abort();
    }
}

impl Held {
    async fn take(config: &tokio_postgres::Config) -> Result<Held, ApiError> {
        let (client, connection) = config.connect(NoTls).await?;
        let driver = tokio::spawn(async move {
            let _ = connection.await;
        });
        let held = Held {
            _client: client,
            driver,
        };
        let row = held
            ._client
            .query_one("SELECT pg_try_advisory_lock_shared($1)", &[&SERVER_LOCK])
            .await?;
        if !row.try_get::<_, bool>(0)? {
            return Err(ApiError::Conflict("restore_running"));
        }
        Ok(held)
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        self.driver.abort();
    }
}

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

/// A time to stamp on a drop, a deletion, a credential change or a backup's cut (round 1 of the review of S4c-2). The base is the
/// **database's** clock (`clock_timestamp()`, read inside the locked transaction) when `clock` is the production clock, so that every
/// process that touches the installation, on whatever host, stamps on one clock, and the injected time of a test or of the simulation
/// otherwise. Either way it is raised to the **floor**: one more than the newest cut time in the lineage, so that nothing is ever
/// stamped below the cut time of a restore (a restore into a clean installation on a host whose clock is behind the backup's, an NTP step
/// back). A stamp below a cut time would say "inside that backup" of something that is not.
///
/// **Only for the three things a restore compares with a cut time** (`compacted_ms`, `deleted_ms`, `changed_ms`). It must not be used
/// for an *age*: after a restore from a backup of the future the floor runs ahead of real time, and an age computed from it would make
/// everything old at once (the 180 days of a `purged`, the retention of a blob, the window of the credential journal). Ages use [`now`].
pub async fn stamp(client: &impl GenericClient, clock: &dyn Clock) -> Result<u64, ApiError> {
    let base = now(client, clock).await?;
    let floor = client
        .query_one("SELECT COALESCE(max(cut_ms), 0) + 1 FROM lineage", &[])
        .await?;
    Ok(base.max(to_u64(floor.try_get(0)?)))
}

/// The time **without the floor**: the database's clock (`clock_timestamp()`) when `clock` is the production clock, so that every
/// process stamps on one clock, and the injected time of a test or of the simulation otherwise. This is the time for every age: the
/// 180-day rule of compaction, the retention of a deleted blob, the window of the credential journal (round 2 of the review).
pub async fn now(client: &impl GenericClient, clock: &dyn Clock) -> Result<u64, ApiError> {
    if clock.is_wall_clock() {
        let row = client
            .query_one(
                "SELECT (extract(epoch FROM clock_timestamp()) * 1000)::bigint",
                &[],
            )
            .await?;
        Ok(to_u64(row.try_get(0)?))
    } else {
        Ok(clock.now_ms())
    }
}

/// Waits for the connection of `held` to end, takes the lock again, and so on for as long as the server runs.
async fn supervise(
    config: tokio_postgres::Config,
    mut held: Held,
    retake: Retake,
    tell: tokio::sync::oneshot::Sender<()>,
) {
    loop {
        let _ = (&mut held.driver).await;
        log::warn!("the server lock's connection ended; taking the lock again");
        tokio::time::sleep(retake.after_loss).await;
        held = loop {
            match Held::take(&config).await {
                Ok(again) => {
                    log::info!("the server lock is held again");
                    break again;
                }
                Err(ApiError::Conflict(_)) => {
                    log::error!(
                        "the server lock was taken by a restore while the server lost it: stopping"
                    );
                    let _ = tell.send(());
                    return;
                }
                Err(_) => tokio::time::sleep(retake.retry).await,
            }
        };
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
