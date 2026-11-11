//! The rules a restore applies (ADR-0001 §6.3, §8.9, ADR-0017, ADR-0020): an account deleted after a backup is deleted again and its id is
//! never registered again, and a restore refuses to run under a server. Over HTTP, on real dumps. The credentials changed after a backup
//! are in `restore_credentials.rs`, the placing of a branch by where it forked in `restore_branch_cut.rs`, and damaged backups in
//! `restore_garbage.rs`.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::blobs::*;
use common::*;
use kinige_server::Clock;
use kinige_server::backup::{self, BackupError};
use kinige_server::store::lock::ServerLock;
use kinige_server::testing::Creds;
use serde_json::json;
use std::time::Duration;

#[tokio::test]
async fn a_deleted_account_does_not_come_back_with_an_older_backup() {
    let rig = rig().await;
    let (one, two) = (account(&rig).await, account(&rig).await);
    let maker = Maker::new();
    let seeded = seed(&rig, &one, &maker, 1).await;
    put(&rig, &two, 30, 130, &msg(&maker, 30, 130, T0)).await;
    let (older, older_manifest) = take(&rig).await;
    let root = older.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);

    // An unfinished upload of the account, then the deletion: a wrong key deletes nothing; a right one removes the account, its wraps,
    // hashes, sessions and envelopes at once, marks its blobs, and puts its hash in the journal.
    upload(&rig, &one, &blob_uuid(9), &sealed(5000, 9), 0, 100).await;
    let (code, answer) = delete_account(&rig, &one, &[3u8; 32]).await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("auth"))
    );
    assert_eq!(accounts_left(&rig).await.len(), 2);
    let (code, _) = rig
        .http
        .post(
            "/v1/account/delete",
            None,
            &json!({"auth_key": b64(&one.creds.auth_key)}),
        )
        .await;
    assert_eq!(code, StatusCode::UNAUTHORIZED, "a session is needed");
    let (code, _) = delete_account(&rig, &one, &one.creds.auth_key).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(
        accounts_left(&rig).await,
        vec![two.creds.account_id.clone()]
    );
    for table in [
        "sessions",
        "envelopes",
        "ops",
        "entity_known",
        "purged_entities",
        "drops",
    ] {
        let left = rows(
            &rig,
            &format!(
                "SELECT 1 FROM {table} WHERE account_id = '{}'",
                one.creds.account_id
            ),
        )
        .await;
        assert!(left.is_empty(), "{table}");
    }
    assert_eq!(
        blob_row(&rig, &one, &seeded.blob_id)
            .await
            .map(|r| r.1.is_some()),
        Some(true)
    );
    assert!(
        rig.state
            .blobs
            .staging
            .find(&one.creds.account_id, &blob_uuid(9))
            .await
            .is_none()
    );
    assert_eq!(
        download(&rig, &one, &seeded.blob_id, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(count(&rig, "deletions").await, 1);
    let hash = kinige_server::hashing::hex(
        &kinige_server::hashing::account_hash(&one.creds.account_id).expect("hash"),
    );
    assert_eq!(
        rows(
            &rig,
            "SELECT encode(account_hash, 'hex') AS h FROM deletions"
        )
        .await,
        vec![format!("{{\"h\":\"{hash}\"}}")]
    );
    rig.clock.advance(DAY);
    let (newer, _) = take_into(&rig, &root).await;

    // In place: the journal of the installation is read before it is replaced.
    let report = restore_into(&rig, &older).await;
    assert_eq!(report.deletions_applied, 1);
    assert_eq!(
        accounts_left(&rig).await,
        vec![two.creds.account_id.clone()]
    );
    assert_eq!(count(&rig, "deletions").await, 1, "and the journal is kept");
    assert_eq!(
        blob_row(&rig, &one, &seeded.blob_id)
            .await
            .map(|r| r.1.is_some()),
        Some(true),
        "the blob goes the delayed way"
    );

    // Into a clean installation: no journal of its own; the newest backup next to the one restored has it.
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &older).await;
    assert_eq!(report.deletions_applied, 1);
    assert_eq!(
        accounts_left(&clean).await,
        vec![two.creds.account_id.clone()]
    );
    assert_eq!(count(&clean, "deletions").await, 1);

    // The honest limit: with no journal anywhere (the older backup alone), the account is restored. The operator keeps backups together.
    let alone_root = rig.db.scratch_dir("alone").expect("dir");
    copy_dir(&older, &alone_root.join("backup-alone"));
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &alone_root.join("backup-alone")).await;
    assert_eq!(report.deletions_applied, 0);
    assert_eq!(accounts_left(&clean).await.len(), 2);

    // A deletion made before the cut time is in the backup already: nothing to apply, and the account is simply not there.
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &newer).await;
    assert_eq!(report.deletions_applied, 0);
    assert_eq!(
        accounts_left(&clean).await,
        vec![two.creds.account_id.clone()]
    );
    assert_eq!(
        count(&clean, "deletions").await,
        1,
        "the journal travels with the backup"
    );
    assert!(older_manifest.cut_ms < rig.clock.now_ms());
}

#[tokio::test]
async fn a_deleted_account_id_is_never_registered_again_so_one_journal_entry_is_enough() {
    // Round 1 of the review: if a deleted id could be registered again, a second deletion would need a second entry (the journal keeps one
    // per account), and an old backup restored between the two would bring the account back. The id is refused, in the installation and
    // after a restore of a backup older than the deletion.
    let rig = rig().await;
    let acct = account(&rig).await;
    let (before, _) = take(&rig).await;
    rig.clock.advance(DAY);
    let (code, _) = delete_account(&rig, &acct, &acct.creds.auth_key).await;
    assert_eq!(code, StatusCode::OK);

    let again = |rig: &Rig| {
        let body = serde_json::to_value(acct.creds.register_request()).expect("json");
        let http = rig.http.clone();
        async move { http.post("/v1/register", None, &body).await }
    };
    let (status, answer) = again(&rig).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_id_taken")),
        "{answer}"
    );
    // A new login for the same id, and the same login with another id, are not what is refused: only the id.
    let other_id = Creds {
        account_id: Creds::random().account_id,
        ..acct.creds.clone()
    };
    let (status, _) = rig
        .http
        .post(
            "/v1/register",
            None,
            &serde_json::to_value(other_id.register_request()).expect("json"),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the login is free again, under another id"
    );

    // The older backup restored in place: the account is deleted again, and its id is still refused.
    let report = restore_into(&rig, &before).await;
    assert_eq!(report.deletions_applied, 1);
    assert!(
        accounts_left(&rig)
            .await
            .iter()
            .all(|id| *id != acct.creds.account_id),
        "the account stays deleted"
    );
    let (status, answer) = again(&rig).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_id_taken"))
    );
}

#[tokio::test]
async fn a_restore_refuses_to_run_under_a_server_and_runs_when_it_is_gone() {
    let rig = rig().await;
    let acct = account(&rig).await;
    seed(&rig, &acct, &Maker::new(), 2).await;
    let (dir, _) = take(&rig).await;
    let before = rows(&rig, "SELECT * FROM accounts").await;
    let held = ServerLock::acquire(&rig.db.pg_config())
        .await
        .expect("the server lock");
    // Another server on the same database is fine (the lock is shared).
    let second = ServerLock::acquire(&rig.db.pg_config())
        .await
        .expect("a second server");
    drop(second);

    let refused = backup::restore(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, &dir)
        .await
        .expect_err("a restore under a server");
    assert_eq!(refused, BackupError::ServerRunning);
    assert_eq!(rows(&rig, "SELECT * FROM accounts").await, before);
    assert_eq!(
        generation_count(&rig).await,
        1,
        "no generation was appended"
    );

    // The server stops: its connection ends and the lock with it.
    drop(held);
    let mut done = false;
    for _ in 0..50 {
        if backup::restore(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, &dir)
            .await
            .is_ok()
        {
            done = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(done, "the restore runs once the server is gone");
    assert_eq!(generation_count(&rig).await, 2);
}

async fn generation_count(rig: &Rig) -> usize {
    rows(rig, "SELECT 1 FROM lineage").await.len()
}

async fn accounts_left(rig: &Rig) -> Vec<String> {
    rows(rig, "SELECT id FROM accounts ORDER BY id")
        .await
        .iter()
        .map(|row| row.split('"').nth(3).expect("an id").to_string())
        .collect()
}
