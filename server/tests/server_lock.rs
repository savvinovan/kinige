//! The server's presence lock (`store::lock::ServerLock`, round 2 of the review of S4c-2). A session advisory lock dies with its
//! connection; a server that went on without it would let a restore start under it. The lock is taken again when its connection
//! drops, and if a restore got in first, the server is told to stop.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use common::backup::*;
use common::*;
use kinige_server::backup::{self, BackupError};
use kinige_server::store::lock::{self, Retake, SERVER_LOCK, ServerLock};
use std::time::Duration;
use tokio::time::{sleep, timeout};

const STEP: Duration = Duration::from_millis(20);

#[tokio::test]
async fn a_lock_whose_connection_was_killed_is_taken_again_and_a_restore_is_refused_again() {
    let rig = rig().await;
    account(&rig).await;
    let (dir, _) = take(&rig).await;
    let mut held = ServerLock::acquire_with(
        &rig.db.pg_config(),
        Retake {
            after_loss: Duration::ZERO,
            retry: Duration::from_millis(50),
        },
    )
    .await
    .expect("the lock");
    let before = holders(&rig).await;
    assert_eq!(before.len(), 1, "one session holds it");
    assert!(restore_refused(&rig, &dir).await);

    // A Postgres restart, a network cut or an administrator's terminate: the session, and with it the lock, ends.
    terminate(&rig, before[0]).await;
    let again = timeout(Duration::from_secs(10), async {
        loop {
            let now = holders(&rig).await;
            if now.len() == 1 && now[0] != before[0] {
                return now[0];
            }
            sleep(STEP).await;
        }
    })
    .await
    .expect("the lock is held again by a new session");
    assert_ne!(again, before[0]);
    assert!(
        restore_refused(&rig, &dir).await,
        "a restore is refused again"
    );
    assert!(
        timeout(Duration::from_millis(200), held.lost())
            .await
            .is_err(),
        "and the server was not told to stop"
    );
}

#[tokio::test]
async fn a_restore_that_gets_the_lock_in_the_gap_stops_the_server() {
    let rig = rig().await;
    let mut held = ServerLock::acquire_with(
        &rig.db.pg_config(),
        Retake {
            // A wide gap, so that the restore below is in it. In production there is none.
            after_loss: Duration::from_millis(800),
            retry: Duration::from_millis(50),
        },
    )
    .await
    .expect("the lock");
    let before = holders(&rig).await;
    terminate(&rig, before[0]).await;

    let restoring = rig.state.pool.get().await.expect("connection");
    let got = timeout(Duration::from_millis(700), async {
        loop {
            if lock::try_restore(&restoring).await.expect("a try") {
                return;
            }
            sleep(STEP).await;
        }
    })
    .await;
    assert!(got.is_ok(), "the restore got the lock in the gap");

    timeout(Duration::from_secs(10), held.lost())
        .await
        .expect("the server is told that it lost the lock and must stop");
    lock::release_restore(&restoring).await.expect("release");
}

/// Whether a restore of `dir` is refused because a server holds the lock.
async fn restore_refused(rig: &Rig, dir: &std::path::Path) -> bool {
    matches!(
        backup::restore(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, dir).await,
        Err(BackupError::ServerRunning)
    )
}

/// The backends of this database that hold the server lock.
async fn holders(rig: &Rig) -> Vec<i32> {
    let client = rig.state.pool.get().await.expect("connection");
    client
        .query(
            "SELECT pid FROM pg_locks WHERE locktype = 'advisory' AND granted \
             AND database = (SELECT oid FROM pg_database WHERE datname = current_database()) \
             AND ((classid::bigint << 32) | objid::bigint) = $1",
            &[&SERVER_LOCK],
        )
        .await
        .expect("locks")
        .iter()
        .map(|row| row.get(0))
        .collect()
}

async fn terminate(rig: &Rig, pid: i32) {
    let client = rig.state.pool.get().await.expect("connection");
    client
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .await
        .expect("terminate");
}
