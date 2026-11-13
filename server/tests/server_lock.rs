//! The server's presence lock (`store::lock::ServerLock`, round 2 of the review of S4c-2). A session advisory lock dies with its
//! connection; it is taken again when the connection drops, and if a restore got in first, the server is told to stop. And the other
//! side (KINIG-102): a restore holds its lock on the connection that writes, so the lock and the transaction end together. Its tests
//! stop a restore with triggers in the test's own database and terminate the backend that holds the lock: after the `TRUNCATE` (a
//! trigger on `lineage` waits for a key the test holds), at the `COMMIT` (deferred triggers), and in the orphan scan (a trigger on `blobs`).
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::blobs::{blob_row, blob_uuid, object_size, sealed, upload_all};
use common::*;
use kinige_server::backup::{self, BackupError, Report};
use kinige_server::blobs::object_key;
use kinige_server::store::lock::{self, Retake, SERVER_LOCK, ServerLock};
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

const STEP: Duration = Duration::from_millis(20);

/// The key the pause triggers wait for. Not one of the server's own.
const PAUSE: i64 = 0x004b_494e_4947_45f1;

const TABLES: [&str; 11] = [
    "accounts",
    "envelopes",
    "ops",
    "entity_known",
    "purged_entities",
    "drops",
    "sessions",
    "blobs",
    "deletions",
    "credential_changes",
    "lineage",
];

#[tokio::test]
async fn a_restore_that_loses_its_lock_after_the_truncate_replaces_nothing_and_runs_again() {
    let s = scene().await;
    install(&s.rig, &pause_trigger("lineage", "")).await;
    let (before, head) = (snapshot(&s.rig).await, pulled_head(&s).await);
    let gate = s.rig.state.pool.get().await.expect("connection");
    gate.execute("SELECT pg_advisory_lock($1)", &[&PAUSE])
        .await
        .expect("the pause key");

    // The restore is past its TRUNCATE and waits in the trigger. The backend that waits is the one that holds the server lock:
    // the lock and the transaction are on one connection.
    let restoring = start_restore(&s);
    let waiter = wait_for_waiter(&s.rig, PAUSE).await;
    assert_eq!(holders(&s.rig).await, vec![waiter]);
    assert!(object_size(&s.rig, &s.acct, &s.backed_up).await.is_some());
    terminate(&s.rig, waiter).await;
    let server = start_server(&s.rig).await;
    gate.execute("SELECT pg_advisory_unlock($1)", &[&PAUSE])
        .await
        .expect("release");

    let result = finished(restoring).await;
    assert_eq!(result.expect_err("a lost lock"), BackupError::LockLost);
    assert_eq!(snapshot(&s.rig).await, before, "nothing was replaced");
    assert_eq!(
        object_size(&s.rig, &s.acct, &s.backed_up).await,
        None,
        "and the object the restore had copied is gone again"
    );
    assert_eq!(
        pulled_head(&s).await,
        head,
        "a session from before the restore still gets the head from before"
    );

    // The server stops, and the restore runs again. A dropped `ServerLock` is released asynchronously.
    drop(server);
    let report = restored_when_free(&s).await;
    assert_eq!(count(&s.rig, "lineage").await, 2, "{report:?}");
    assert!(object_size(&s.rig, &s.acct, &s.backed_up).await.is_some());
}

#[tokio::test]
async fn a_restore_that_loses_its_lock_in_the_orphan_scan_is_done_and_adopts_nothing() {
    let s = scene().await;
    // Held in the scan after the commit: the trigger fires for the rows the scan inserts (published_ms = 0, already deleted).
    let adopting = "WHEN (NEW.published_ms = 0 AND NEW.deleted_ms IS NOT NULL)";
    install(&s.rig, &pause_trigger("blobs", adopting)).await;
    let gate = s.rig.state.pool.get().await.expect("connection");
    gate.execute("SELECT pg_advisory_lock($1)", &[&PAUSE])
        .await
        .expect("the pause key");

    let restoring = start_restore(&s);
    let waiter = wait_for_waiter(&s.rig, PAUSE).await;
    assert_eq!(
        holders(&s.rig).await,
        vec![waiter],
        "the scan is on the lock's connection"
    );
    assert_eq!(
        count(&s.rig, "lineage").await,
        2,
        "the restore has committed"
    );
    terminate(&s.rig, waiter).await;
    // A server starts in the gap and may publish a blob under the key the scan is about to adopt as deleted.
    let _server = start_server(&s.rig).await;
    gate.execute("SELECT pg_advisory_unlock($1)", &[&PAUSE])
        .await
        .expect("release");

    let report = finished(restoring).await.expect("the restore is done");
    assert!(report.orphan_scan_failed, "{report:?}");
    assert_eq!(report.orphans, 0);
    assert_eq!(
        blob_row(&s.rig, &s.acct, &blob_uuid(7)).await,
        None,
        "the object of the blob uploaded after the backup was not adopted as deleted"
    );
}

#[tokio::test]
async fn a_commit_that_fails_with_the_connection_usable_is_the_error_it_was_and_cleans_up() {
    let s = scene().await;
    install(
        &s.rig,
        "CREATE FUNCTION refuse() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'no'; END $$; \
         CREATE CONSTRAINT TRIGGER refuse AFTER INSERT ON lineage DEFERRABLE INITIALLY DEFERRED \
         FOR EACH ROW EXECUTE FUNCTION refuse();",
    )
    .await;
    let before = snapshot(&s.rig).await;

    let result = finished(start_restore(&s)).await;
    assert_eq!(result.expect_err("a refused commit"), BackupError::Database);
    assert_eq!(snapshot(&s.rig).await, before, "rolled back");
    assert_eq!(object_size(&s.rig, &s.acct, &s.backed_up).await, None);
}

#[tokio::test]
async fn a_backend_that_dies_during_the_commit_is_an_unknown_outcome_and_the_copied_object_is_kept()
{
    let s = scene().await;
    install(
        &s.rig,
        "CREATE FUNCTION die() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN \
         PERFORM pg_terminate_backend(pg_backend_pid()); PERFORM pg_sleep(0.05); RETURN NULL; END $$; \
         CREATE CONSTRAINT TRIGGER die AFTER INSERT ON lineage DEFERRABLE INITIALLY DEFERRED \
         FOR EACH ROW EXECUTE FUNCTION die();",
    )
    .await;

    let result = finished(start_restore(&s)).await;
    assert_eq!(
        result.expect_err("a lost commit"),
        BackupError::OutcomeUnknown
    );
    // Kept: the database may have been replaced and its rows may refer to the object. A rerun finds it by hash.
    assert!(object_size(&s.rig, &s.acct, &s.backed_up).await.is_some());
}

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
    pids(rig, SERVER_LOCK, true).await
}

/// The backends of this database that hold (`granted`) or wait for the advisory lock `key`.
async fn pids(rig: &Rig, key: i64, granted: bool) -> Vec<i32> {
    let client = rig.state.pool.get().await.expect("connection");
    client
        .query(
            "SELECT pid FROM pg_locks WHERE locktype = 'advisory' AND granted = $2 \
             AND database = (SELECT oid FROM pg_database WHERE datname = current_database()) \
             AND ((classid::bigint << 32) | objid::bigint) = $1",
            &[&key, &granted],
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

/// An installation that differs from its backup: one more message and one more blob after it, and a backed-up blob whose object the
/// store lacks (so that the restore copies it in).
struct Scene {
    rig: Rig,
    acct: Acct,
    dir: PathBuf,
    backed_up: String,
}

async fn scene() -> Scene {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let seeded = seed(&rig, &acct, &maker, 2).await;
    let (dir, _) = take(&rig).await;
    put(&rig, &acct, 9, 109, &msg(&maker, 9, 109, T0)).await;
    let (code, _) = upload_all(&rig, &acct, &blob_uuid(7), &sealed(4007, 7)).await;
    assert_eq!(code, StatusCode::OK);
    let key = object_key(&acct.creds.account_id, &seeded.blob_id);
    rig.state.blobs.objects.delete(&key).await.expect("delete");
    Scene {
        rig,
        acct,
        dir,
        backed_up: seeded.blob_id,
    }
}

fn start_restore(s: &Scene) -> JoinHandle<Result<Report, BackupError>> {
    let pool = s.rig.state.pool.clone();
    let objects = s.rig.state.blobs.objects.clone();
    let clock = s.rig.clock.clone();
    let dir = s.dir.clone();
    tokio::spawn(async move { backup::restore(&pool, &objects, &*clock, &dir).await })
}

async fn finished(
    restoring: JoinHandle<Result<Report, BackupError>>,
) -> Result<Report, BackupError> {
    timeout(Duration::from_secs(30), restoring)
        .await
        .expect("the restore ends")
        .expect("join")
}

/// A server that starts in the gap. The backend that held the lock may still be going away.
async fn start_server(rig: &Rig) -> ServerLock {
    timeout(Duration::from_secs(10), async {
        loop {
            if let Ok(server) = ServerLock::acquire(&rig.db.pg_config()).await {
                return server;
            }
            sleep(STEP).await;
        }
    })
    .await
    .expect("a server starts")
}

/// The restore again, once a dropped `ServerLock` is released (that is asynchronous).
async fn restored_when_free(s: &Scene) -> Report {
    for _ in 0..50 {
        match backup::restore(
            &s.rig.state.pool,
            &s.rig.state.blobs.objects,
            &*s.rig.clock,
            &s.dir,
        )
        .await
        {
            Err(BackupError::ServerRunning) => sleep(Duration::from_millis(100)).await,
            other => return other.expect("the second restore succeeds"),
        }
    }
    panic!("the server's lock was never released");
}

/// Every table the restore replaces, as text, in a fixed order.
async fn snapshot(rig: &Rig) -> Vec<Vec<String>> {
    let mut all = Vec::new();
    for table in TABLES {
        all.push(rows(rig, &format!("SELECT * FROM {table} t0 ORDER BY t0::text")).await);
    }
    all
}

/// The head a session of the account from before the restore gets in a pull from 0.
async fn pulled_head(s: &Scene) -> (StatusCode, Value) {
    let (code, page) = pull(&s.rig, &s.acct, generation(&s.rig).await, 0, 0, 10).await;
    (code, page["head"].clone())
}

/// A trigger that waits for the advisory key `PAUSE` before each row it is set for.
fn pause_trigger(table: &str, when: &str) -> String {
    format!(
        "CREATE FUNCTION pause() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN \
         PERFORM pg_advisory_xact_lock({PAUSE}); RETURN NEW; END $$; \
         CREATE TRIGGER pause BEFORE INSERT ON {table} FOR EACH ROW {when} EXECUTE FUNCTION pause();"
    )
}

async fn install(rig: &Rig, ddl: &str) {
    let client = rig.state.pool.get().await.expect("connection");
    client.batch_execute(ddl).await.expect("a trigger");
}

/// Waits until a backend waits for the advisory key.
async fn wait_for_waiter(rig: &Rig, key: i64) -> i32 {
    timeout(Duration::from_secs(20), async {
        loop {
            if let Some(pid) = pids(rig, key, false).await.first() {
                return *pid;
            }
            sleep(STEP).await;
        }
    })
    .await
    .expect("a backend waits on the key")
}
