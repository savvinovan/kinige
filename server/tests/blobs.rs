//! Blobs over HTTP (ADR-0001 §7, appendix §A.2 and §A.6, ADR-0020): upload by offset, resume, overwrite of an unfinished upload, publication,
//! "already exists", download with a range, the delete and the delayed removal, the purge of an attachment, the limits and the order of the
//! checks. Every scenario runs against the disk store and, when `KINIGE_TEST_S3_URL` is set, against an S3-compatible store.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::body::Body;
use axum::http::StatusCode;
use common::blobs::*;
use common::*;
use kinige_crypto::blob::open_blob;
use kinige_crypto::secret::BlobKey;
use kinige_server::Clock;
use kinige_server::blobs::sweep;
use kinige_server::config::Config;
use kinige_server::testing::{BlobBackend, blob_backends};
use serde_json::json;

const MIB: usize = 1 << 20;

fn backends() -> Vec<BlobBackend> {
    blob_backends().expect("the blob backends")
}

#[tokio::test]
async fn upload_resume_overwrite_publish_download_range_and_already_exists() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        // 2.5 MiB of plaintext: three chunks, 3 MiB of blob.
        let blob = sealed(MIB * 5 / 2, 7);
        let id = blob_uuid(1);
        let n = blob.len();

        assert_eq!(status(&rig, &acct, &id).await["state"], "none");

        let (code, answer) = upload(&rig, &acct, &id, &blob, 0, 1_000_000).await;
        assert_eq!(
            (code, &answer["result"]),
            (StatusCode::OK, &json!("partial")),
            "{backend:?}"
        );
        assert_eq!(answer["received"], 1_000_000);
        let state = status(&rig, &acct, &id).await;
        assert_eq!(
            (
                state["state"].as_str(),
                state["size"].as_u64(),
                state["received"].as_u64()
            ),
            (Some("uploading"), Some(n as u64), Some(1_000_000)),
        );

        // Not published while unfinished: no download, no row, no object.
        assert_eq!(
            download(&rig, &acct, &id, None).await.0,
            StatusCode::NOT_FOUND
        );
        assert!(blob_row(&rig, &acct, &id).await.is_none());
        assert!(object_size(&rig, &acct, &id).await.is_none());

        // A gap is refused with the place to continue from, and the unfinished upload is untouched.
        let (code, answer) = upload(&rig, &acct, &id, &blob, 1_000_001, 1_100_000).await;
        assert_eq!(
            (code, answer["error"].as_str()),
            (StatusCode::CONFLICT, Some("offset_gap"))
        );
        assert_eq!(answer["received"], 1_000_000);

        // Resume inside what the server holds overwrites the tail: 600 000 .. 2 000 000.
        let (_, answer) = upload(&rig, &acct, &id, &blob, 600_000, 2_000_000).await;
        assert_eq!(answer["received"], 2_000_000);

        // Another size at an offset other than 0 is refused; at 0 the upload starts over.
        let other_size = kinige_server::rules::blob::sealed_len(3 * MIB as u64) as usize;
        let (code, answer) = upload_raw(
            &rig,
            &acct,
            &id,
            other_size,
            2_000_000,
            Body::from(vec![0u8; 10]),
        )
        .await;
        assert_eq!(
            (code, answer["error"].as_str()),
            (StatusCode::CONFLICT, Some("size_mismatch"))
        );

        let (_, answer) = upload(&rig, &acct, &id, &blob, 2_000_000, n).await;
        assert_eq!(
            (answer["result"].as_str(), answer["received"].as_u64()),
            (Some("published"), Some(n as u64))
        );

        // Published: a row, an object of the right size, no staging file, and the download is the blob, byte for byte.
        assert_eq!(blob_row(&rig, &acct, &id).await, Some((n as i64, None)));
        assert_eq!(object_size(&rig, &acct, &id).await, Some(n as u64));
        assert!(
            rig.state
                .blobs
                .staging
                .find(&acct.creds.account_id, &id)
                .await
                .is_none()
        );
        let (code, headers, bytes) = download(&rig, &acct, &id, None).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(headers["content-length"], n.to_string().as_str());
        assert_eq!(headers["accept-ranges"], "bytes");
        assert_eq!(bytes, blob, "{backend:?}");
        let key = BlobKey::from_bytes([7; 32]);
        assert_eq!(
            open_blob(&bytes, &key).expect("opens"),
            plain(MIB * 5 / 2, 7)
        );

        // Ranges: a span across a chunk boundary, an open end, a suffix.
        let (code, headers, bytes) =
            download(&rig, &acct, &id, Some("bytes=1048000-1049999")).await;
        assert_eq!(code, StatusCode::PARTIAL_CONTENT);
        assert_eq!(
            headers["content-range"],
            format!("bytes 1048000-1049999/{n}").as_str()
        );
        assert_eq!(bytes, blob[1_048_000..=1_049_999]);
        let (_, _, bytes) = download(&rig, &acct, &id, Some(&format!("bytes={}-", n - 10))).await;
        assert_eq!(bytes, blob[n - 10..]);
        let (_, _, bytes) = download(&rig, &acct, &id, Some("bytes=-25")).await;
        assert_eq!(bytes, blob[n - 25..]);
        for bad in [
            &format!("bytes={n}-"),
            "bytes=5-1",
            "bytes=0-1,4-5",
            "pages=1-2",
            "bytes=x-y",
        ] {
            let (code, headers, _) = download(&rig, &acct, &id, Some(bad)).await;
            assert_eq!(code, StatusCode::RANGE_NOT_SATISFIABLE, "{bad}");
            assert_eq!(headers["content-range"], format!("bytes */{n}").as_str());
        }

        // Publishing the same id again is "already exists", a success, and nothing in the body is read or compared.
        let (code, answer) = upload_all(&rig, &acct, &id, &sealed(MIB * 5 / 2, 9)).await;
        assert_eq!(
            (code, answer["result"].as_str()),
            (StatusCode::OK, Some("exists"))
        );
        let (_, answer) = upload_raw(&rig, &acct, &id, n, 0, Body::from("not even a blob")).await;
        assert_eq!(answer["result"], "exists");
        assert_eq!(
            download(&rig, &acct, &id, None).await.2,
            blob,
            "the first bytes stay"
        );

        // An unfinished upload can be overwritten from 0, also with another size.
        let other = blob_uuid(2);
        let first = sealed(2000, 3);
        let second = sealed(MIB + 3000, 4);
        upload(&rig, &acct, &other, &first, 0, 1500).await;
        let (_, answer) = upload(&rig, &acct, &other, &second, 0, 500).await;
        assert_eq!(answer["received"], 500);
        let (_, answer) = upload(&rig, &acct, &other, &second, 500, second.len()).await;
        assert_eq!(answer["result"], "published");
        assert_eq!(download(&rig, &acct, &other, None).await.2, second);

        // A publication that failed after the last byte left a complete staging file: a request at the end with no body finishes it.
        let third = blob_uuid(3);
        let whole = sealed(4000, 5);
        let (mut file, _) = rig
            .state
            .blobs
            .staging
            .open(&acct.creds.account_id, &third, whole.len() as u64, 0)
            .await
            .expect("staging file");
        tokio::io::AsyncWriteExt::write_all(&mut file, &whole)
            .await
            .expect("write");
        drop(file);
        assert_eq!(status(&rig, &acct, &third).await["received"], whole.len());
        let (_, answer) =
            upload_raw(&rig, &acct, &third, whole.len(), whole.len(), Body::empty()).await;
        assert_eq!(answer["result"], "published", "{backend:?}");
        assert_eq!(download(&rig, &acct, &third, None).await.2, whole);
    }
}

#[tokio::test]
async fn a_large_blob_goes_to_the_store_in_parts_on_every_backend() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        // 9 MiB of plaintext: more than the 8 MiB single request of the store.
        let blob = sealed(9 * MIB, 11);
        let id = blob_uuid(1);
        for (from, to) in [(0, 4 * MIB), (4 * MIB, blob.len())] {
            upload(&rig, &acct, &id, &blob, from, to).await;
        }
        assert_eq!(
            object_size(&rig, &acct, &id).await,
            Some(blob.len() as u64),
            "{backend:?}"
        );
        assert_eq!(download(&rig, &acct, &id, None).await.2, blob);
        let (_, _, tail) = download(&rig, &acct, &id, Some("bytes=9000000-9000099")).await;
        assert_eq!(tail, blob[9_000_000..9_000_100]);
    }
}

#[tokio::test]
async fn the_server_checks_the_framing_it_can_see_and_nothing_else() {
    let rig = rig().await;
    let acct = account(&rig).await;
    // Lengths that are not a blob of §A.2 are refused at the first request, before a byte is stored.
    for (n, size) in [
        (1, 0usize),
        (2, 24),
        (3, 1000),
        (4, 24 + 1024 + 16),
        (5, 24 + 1025 + 17),
        (6, MIB + 24 + 17 + 1),
    ] {
        let id = blob_uuid(n);
        let (code, answer) = upload_raw(
            &rig,
            &acct,
            &id,
            size,
            0,
            Body::from(vec![1u8; size.min(100)]),
        )
        .await;
        assert_eq!(
            (code, answer["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("blob_framing")),
            "{size}"
        );
        assert_eq!(status(&rig, &acct, &id).await["state"], "none");
    }
    // The right length with other bytes is accepted: the server cannot open the stream and does not pretend to.
    let id = blob_uuid(9);
    let garbage = vec![0xAB; 24 + 1024 + 17];
    let (code, answer) = upload_all(&rig, &acct, &id, &garbage).await;
    assert_eq!(
        (code, answer["result"].as_str()),
        (StatusCode::OK, Some("published"))
    );
    assert_eq!(download(&rig, &acct, &id, None).await.2, garbage);
}

#[tokio::test]
async fn limits_come_before_any_byte_is_stored() {
    let rig = rig_with(Config {
        blob_max: 3 * MIB as u64,
        blob_quota: 4 * MIB as u64,
        blob_request_max: MIB as u64,
        ..Config::default()
    })
    .await;
    let acct = account(&rig).await;

    // The cap of one blob.
    let too_big = sealed(4 * MIB, 1);
    let (code, answer) = upload(&rig, &acct, &blob_uuid(1), &too_big, 0, 100).await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
    );
    assert!(
        rig.state
            .blobs
            .staging
            .find(&acct.creds.account_id, &blob_uuid(1))
            .await
            .is_none()
    );

    // The cap of one request: a body of 2 MiB (declared by Content-Length) is refused before it is read.
    let blob = sealed(2 * MIB + 5, 2);
    let (code, answer) = upload(&rig, &acct, &blob_uuid(2), &blob, 0, 2 * MIB).await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
    );
    assert!(
        rig.state
            .blobs
            .staging
            .find(&acct.creds.account_id, &blob_uuid(2))
            .await
            .is_none()
    );

    // A body longer than the blob still needs is refused too, also without a Content-Length (a stream), and what fits stays.
    let small = sealed(2000, 3);
    let id = blob_uuid(3);
    let stream = futures_util::stream::iter(vec![
        Ok::<_, std::io::Error>(axum::body::Bytes::from(small[..500].to_vec())),
        Ok(axum::body::Bytes::from(vec![0u8; small.len()])),
    ]);
    let (code, _) = upload_raw(&rig, &acct, &id, small.len(), 0, Body::from_stream(stream)).await;
    assert_eq!(code, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(status(&rig, &acct, &id).await["received"], 500);

    // The quota counts published blobs and the declared size of unfinished uploads, and a deleted blob frees its share.
    let a = sealed(3 * MIB - 100_000, 4);
    let b = sealed(2 * MIB, 5);
    for (n, part) in [(10, &a), (11, &b)] {
        let id = blob_uuid(n);
        let mut at = 0;
        while at < part.len() {
            let to = (at + MIB).min(part.len());
            let (code, answer) = upload(&rig, &acct, &id, part, at, to).await;
            if n == 11 {
                assert_eq!(
                    (code, answer["error"].as_str()),
                    (StatusCode::PAYLOAD_TOO_LARGE, Some("quota"))
                );
                break;
            }
            assert_eq!(code, StatusCode::OK, "{answer}");
            at = to;
        }
    }
    assert!(status(&rig, &acct, &blob_uuid(11)).await["state"] == "none");
    assert_eq!(delete(&rig, &acct, &blob_uuid(10)).await, StatusCode::OK);
    let (code, _) = upload(&rig, &acct, &blob_uuid(11), &b, 0, MIB).await;
    assert_eq!(code, StatusCode::OK, "a deleted blob no longer counts");
}

#[tokio::test]
async fn every_blob_request_is_authenticated_before_anything_is_read_or_decided() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let blob = sealed(2000, 1);
    let id = blob_uuid(1);

    // No token, a malformed one, an unknown one: 401, whatever the id, the headers and the body say.
    let unknown = kinige_server::wire::to_b64(&[7u8; 32]);
    for token in [None, Some("x"), Some(unknown.as_str())] {
        for (method, path) in [
            ("PUT", format!("/v1/blobs/{id}")),
            ("PUT", "/v1/blobs/not-an-id".to_string()),
            ("GET", format!("/v1/blobs/{id}")),
            ("GET", format!("/v1/blobs/{id}/status")),
            ("DELETE", format!("/v1/blobs/{id}")),
        ] {
            let (code, _, _) = rig
                .http
                .request(
                    method,
                    &path,
                    token,
                    &[("x-kinige-blob-size", "nope".to_string())],
                    Body::from("garbage"),
                )
                .await;
            assert_eq!(code, StatusCode::UNAUTHORIZED, "{method} {path} {token:?}");
        }
    }
    assert!(
        rig.state
            .blobs
            .staging
            .find(&acct.creds.account_id, &id)
            .await
            .is_none()
    );

    // With a session the id is judged next, then the headers.
    for bad in [
        "x",
        "../../etc/passwd",
        "00000000-0000-0000-0000-00000000000G",
        &id.to_uppercase(),
    ] {
        let (code, _, _) = rig
            .http
            .request(
                "GET",
                &format!("/v1/blobs/{bad}"),
                Some(&acct.token),
                &[],
                Body::empty(),
            )
            .await;
        assert!(
            code == StatusCode::BAD_REQUEST || code == StatusCode::NOT_FOUND,
            "{bad} {code}"
        );
    }
    let (code, answer) = upload_raw(&rig, &acct, &id, 0, 0, Body::empty()).await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("blob_framing"))
    );
    for (name, value) in [
        ("x-kinige-blob-size", "-1"),
        ("x-kinige-blob-size", "1e3"),
        ("x-kinige-blob-size", "99999999999999999999"),
    ] {
        let (code, _, _) = rig
            .http
            .request(
                "PUT",
                &format!("/v1/blobs/{id}"),
                Some(&acct.token),
                &[(name, value.to_string())],
                Body::empty(),
            )
            .await;
        assert_eq!(code, StatusCode::BAD_REQUEST, "{value}");
    }
    let (code, _, _) = rig
        .http
        .request(
            "PUT",
            &format!("/v1/blobs/{id}"),
            Some(&acct.token),
            &[],
            Body::empty(),
        )
        .await;
    assert_eq!(code, StatusCode::BAD_REQUEST, "the size header is required");
    upload_all(&rig, &acct, &id, &blob).await;
}

#[tokio::test]
async fn a_blob_belongs_to_its_account_and_a_second_writer_to_an_upload_is_told_so() {
    let rig = rig().await;
    let (a, b) = (account(&rig).await, account(&rig).await);
    let id = blob_uuid(1);
    let blob_a = sealed(3000, 1);
    let blob_b = sealed(5000, 2);
    upload_all(&rig, &a, &id, &blob_a).await;
    // The same id under another account is another blob: the store is keyed by (account_id, remote_blob_id).
    assert_eq!(download(&rig, &b, &id, None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        delete(&rig, &b, &id).await,
        StatusCode::OK,
        "nothing of A's is B's to delete"
    );
    assert_eq!(download(&rig, &a, &id, None).await.2, blob_a);
    upload_all(&rig, &b, &id, &blob_b).await;
    assert_eq!(download(&rig, &b, &id, None).await.2, blob_b);
    assert_eq!(download(&rig, &a, &id, None).await.2, blob_a);

    // One writer per upload.
    let busy_id = blob_uuid(2);
    let held = rig
        .state
        .blobs
        .staging
        .try_busy(&a.creds.account_id, &busy_id)
        .expect("free");
    let (code, answer) = upload(&rig, &a, &busy_id, &blob_a, 0, 100).await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("busy"))
    );
    drop(held);
    assert_eq!(
        upload(&rig, &a, &busy_id, &blob_a, 0, 100).await.0,
        StatusCode::OK
    );
}

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
