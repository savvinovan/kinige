//! How a blob ends (ADR-0001 §7, ADR-0020): a delete marks it and the object leaves the store after the backup retention plus one interval,
//! the purge of an attachment marks the blob its open header names, unfinished uploads and objects without a row are cleaned after their
//! age, and a publication never deletes the object of a row that exists. Upload and download are in `blobs.rs`. Every scenario runs against
//! the disk store and, with `KINIGE_TEST_S3_URL`, an S3-compatible store.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::blobs::*;
use common::*;
use kinige_server::Clock;
use kinige_server::blobs::sweep;
use kinige_server::config::Config;
use kinige_server::rules::blob::Known;
use kinige_server::store::blobs::{self as rows_of_blobs, Published};
use kinige_server::testing::{BlobBackend, blob_backends};

#[tokio::test]
async fn a_delete_marks_the_blob_and_the_object_goes_after_the_retention_and_one_interval() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        let blob = sealed(5000, 3);
        let id = blob_uuid(1);
        upload_all(&rig, &acct, &id, &blob).await;

        // An unfinished upload is deleted at once, with its staging file.
        let part = blob_uuid(2);
        upload(&rig, &acct, &part, &blob, 0, 100).await;
        assert_eq!(delete(&rig, &acct, &part).await, StatusCode::OK);
        assert_eq!(status(&rig, &acct, &part).await["state"], "none");

        assert_eq!(delete(&rig, &acct, &id).await, StatusCode::OK);
        assert_eq!(
            delete(&rig, &acct, &id).await,
            StatusCode::OK,
            "deleting twice is a success"
        );
        assert_eq!(
            delete(&rig, &acct, &blob_uuid(99)).await,
            StatusCode::OK,
            "so is deleting what was never there"
        );
        assert_eq!(
            download(&rig, &acct, &id, None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(status(&rig, &acct, &id).await["state"], "deleted");
        let (code, answer) = upload_all(&rig, &acct, &id, &blob).await;
        assert_eq!(
            (code, answer["error"].as_str()),
            (StatusCode::CONFLICT, Some("gone")),
            "a deleted id is replaced under a new one"
        );
        let deleted_at = rig.clock.now_ms();
        assert_eq!(
            blob_row(&rig, &acct, &id).await.map(|r| r.1),
            Some(Some(deleted_at as i64))
        );
        // Marked, not removed.
        assert_eq!(
            object_size(&rig, &acct, &id).await,
            Some(blob.len() as u64),
            "{backend:?}"
        );

        // 30 days and 23 hours: still there. 31 days: gone, with its row; the id is free again.
        let config = rig.state.config.clone();
        rig.clock.advance(31 * DAY - 3_600_000);
        let swept = sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
            .await
            .expect("sweep");
        assert_eq!(swept.removed, 0);
        assert_eq!(object_size(&rig, &acct, &id).await, Some(blob.len() as u64));
        rig.clock.advance(3_600_000);
        let swept = sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
            .await
            .expect("sweep");
        assert_eq!(swept.removed, 1);
        assert!(object_size(&rig, &acct, &id).await.is_none());
        assert!(blob_row(&rig, &acct, &id).await.is_none());
    }
}

#[tokio::test]
async fn the_objects_of_a_deleted_account_stay_for_the_retention_plus_one_interval_and_then_go() {
    // ADR-0001 §8.9, PRD §10.3: the account is gone at once, its blob objects are not (an older backup may still need them).
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        let blob = sealed(4000, 6);
        let id = blob_uuid(1);
        upload_all(&rig, &acct, &id, &blob).await;
        let (code, _) = delete_account_of(&rig, &acct).await;
        assert_eq!(code, StatusCode::OK);
        let config = rig.state.config.clone();
        rig.clock.advance(30 * DAY);
        sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
            .await
            .expect("sweep");
        assert_eq!(
            object_size(&rig, &acct, &id).await,
            Some(blob.len() as u64),
            "thirty days after the deletion the object is still there ({backend:?})"
        );
        rig.clock.advance(DAY + 1);
        let swept = sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
            .await
            .expect("sweep");
        assert_eq!(swept.removed, 1);
        assert!(object_size(&rig, &acct, &id).await.is_none());
    }
}

#[tokio::test]
async fn an_accepted_purge_of_an_attachment_marks_the_blob_it_names_in_its_open_header() {
    let rig = rig().await;
    let (a, b) = (account(&rig).await, account(&rig).await);
    let maker = Maker::new();
    let blob = sealed(3000, 1);
    let (blob_id, attachment) = (blob_uuid(1), id(40));
    upload_all(&rig, &a, &blob_id, &blob).await;
    upload_all(&rig, &b, &blob_id, &blob).await;

    // A purge from B names the same id, and affects only B's blob.
    let purge_b = maker.purged_attachment(&attachment, &id(60), &blob_id);
    let (op_b, body_b) = (id(60), purge_b);
    let result = push(
        &rig,
        &b,
        &Item {
            entity: "attachment",
            id: &attachment,
            op: &op_b,
            bytes: &body_b,
        },
    )
    .await;
    assert_eq!(outcome(&result), "ok", "{result}");
    assert_eq!(
        blob_row(&rig, &b, &blob_id).await.map(|r| r.1.is_some()),
        Some(true)
    );
    assert_eq!(
        blob_row(&rig, &a, &blob_id).await.map(|r| r.1.is_some()),
        Some(false)
    );
    assert_eq!(download(&rig, &a, &blob_id, None).await.2, blob);
    assert_eq!(
        download(&rig, &b, &blob_id, None).await.0,
        StatusCode::NOT_FOUND
    );

    // A purge that names no blob leaves blobs alone.
    let second = blob_uuid(2);
    upload_all(&rig, &a, &second, &blob).await;
    let other_attachment = id(41);
    let op = id(61);
    let no_blob = maker.purged(
        "attachment",
        &other_attachment,
        &op,
        &[
            ("core", hlc(T0, 0)),
            ("order", hlc(T0, 0)),
            ("deleted", hlc(T0, 0)),
        ],
    );
    let result = push(
        &rig,
        &a,
        &Item {
            entity: "attachment",
            id: &other_attachment,
            op: &op,
            bytes: &no_blob,
        },
    )
    .await;
    assert_eq!(outcome(&result), "ok");
    assert_eq!(
        blob_row(&rig, &a, &second).await.map(|r| r.1.is_some()),
        Some(false)
    );

    // The purge that does name it, once accepted, marks it.
    let (third_attachment, op3) = (id(42), id(62));
    let purge = maker.purged_attachment(&third_attachment, &op3, &second);
    let result = push(
        &rig,
        &a,
        &Item {
            entity: "attachment",
            id: &third_attachment,
            op: &op3,
            bytes: &purge,
        },
    )
    .await;
    assert_eq!(outcome(&result), "ok");
    assert_eq!(
        blob_row(&rig, &a, &second).await.map(|r| r.1.is_some()),
        Some(true)
    );
}

#[tokio::test]
async fn unfinished_uploads_and_objects_without_a_row_are_cleaned_after_their_age() {
    let rig = rig_with(Config {
        orphan_upload_ms: 0,
        ..Config::default()
    })
    .await;
    let acct = account(&rig).await;
    let blob = sealed(5000, 3);
    // An upload nobody finishes.
    let id = blob_uuid(1);
    upload(&rig, &acct, &id, &blob, 0, 100).await;
    assert_eq!(status(&rig, &acct, &id).await["state"], "uploading");
    // An object that no row refers to (a crash between the put and the row).
    let lost = blob_uuid(2);
    let key = kinige_server::blobs::object_key(&acct.creds.account_id, &lost);
    let staged = rig.db.scratch_dir("lost").expect("dir").join("f");
    std::fs::write(&staged, &blob).expect("file");
    rig.state
        .blobs
        .objects
        .put_file(&key, &staged)
        .await
        .expect("put");

    let config = rig.state.config.clone();
    let swept = sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
        .await
        .expect("sweep");
    assert_eq!((swept.uploads, swept.adopted), (1, 1));
    assert_eq!(status(&rig, &acct, &id).await["state"], "none");
    // The object is now a deleted blob and goes the delayed way.
    assert_eq!(
        blob_row(&rig, &acct, &lost).await.map(|r| r.1.is_some()),
        Some(true)
    );
    assert!(object_size(&rig, &acct, &lost).await.is_some());
    rig.clock.advance(31 * DAY);
    sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
        .await
        .expect("sweep");
    assert!(object_size(&rig, &acct, &lost).await.is_none());
}

#[tokio::test]
async fn a_publication_tells_a_row_that_exists_from_an_account_that_is_gone() {
    // Round 1 of the review: the insert returned "nothing inserted" for both, and the caller deleted the object. Only an account that is
    // gone (no row of this id can exist) may cost the object; a row that exists, published or deleted, is `AlreadyThere` and the object
    // stays.
    let rig = rig().await;
    let acct = account(&rig).await;
    let (account_id, blob) = (acct.creds.account_id.as_str(), blob_uuid(1));
    let now = rig.clock.now_ms();
    let publish = |account: &str, blob: &str| {
        let pool = rig.state.pool.clone();
        let (account, blob) = (account.to_string(), blob.to_string());
        async move {
            rows_of_blobs::publish(&pool, &account, &blob, 1065, now)
                .await
                .expect("publish")
        }
    };
    assert_eq!(publish(account_id, &blob).await, Published::New);
    assert_eq!(
        publish(account_id, &blob).await,
        Published::AlreadyThere(Known::Published)
    );
    assert!(
        rows_of_blobs::mark_deleted(&rig.state.pool, account_id, &blob, now)
            .await
            .expect("mark")
    );
    assert_eq!(
        publish(account_id, &blob).await,
        Published::AlreadyThere(Known::Deleted)
    );
    let stranger = "00000009-0000-4000-8000-000000000009";
    assert_eq!(
        publish(stranger, &blob_uuid(2)).await,
        Published::AccountGone
    );
}

fn backends() -> Vec<BlobBackend> {
    blob_backends().expect("the blob backends")
}

async fn delete_account_of(rig: &Rig, acct: &Acct) -> (StatusCode, serde_json::Value) {
    rig.http
        .post(
            "/v1/account/delete",
            Some(&acct.token),
            &serde_json::json!({"auth_key": b64(&acct.creds.auth_key)}),
        )
        .await
}
