//! The read timeout on request bodies (S4c-1 carry-over, `api/body.rs`): a client that sends a byte a minute is cut off. The numbers are
//! scaled down so the test takes a second: a chunk that takes longer than `body_idle_ms` or an average rate under `body_min_rate` after
//! `body_rate_grace_ms` gets a 408, on a push and on a blob upload alike, and what a blob upload had received stays for the resume.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::body::{Body, Bytes};
use axum::http::StatusCode;
use common::blobs::*;
use common::*;
use futures_util::stream;
use kinige_server::config::Config;
use std::time::Duration;

fn quick() -> Config {
    Config {
        body_idle_ms: 150,
        body_rate_grace_ms: 300,
        body_min_rate: 1000,
        ..Config::default()
    }
}

/// A body that sends `pieces` one after another, waiting `gap` before each.
fn trickle(pieces: Vec<Vec<u8>>, gap: Duration) -> Body {
    Body::from_stream(stream::unfold(
        pieces.into_iter(),
        move |mut rest| async move {
            let next = rest.next()?;
            tokio::time::sleep(gap).await;
            Some((Ok::<_, std::io::Error>(Bytes::from(next)), rest))
        },
    ))
}

#[tokio::test]
async fn a_client_that_goes_quiet_or_trickles_is_cut_off_with_a_408_on_a_blob_upload() {
    let rig = rig_with(quick()).await;
    let acct = account(&rig).await;
    let blob = sealed(5000, 1);
    let id = blob_uuid(1);

    // Quiet: one piece at once, then nothing for longer than the idle limit.
    let first = blob[..400].to_vec();
    let (code, answer) = upload_raw(
        &rig,
        &acct,
        &id,
        blob.len(),
        0,
        Body::from_stream(stream::unfold(0u8, move |step| {
            let first = first.clone();
            async move {
                match step {
                    0 => Some((Ok::<_, std::io::Error>(Bytes::from(first)), 1)),
                    _ => {
                        tokio::time::sleep(Duration::from_millis(600)).await;
                        None
                    }
                }
            }
        })),
    )
    .await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::REQUEST_TIMEOUT, Some("timeout"))
    );
    // What arrived stays, and the upload resumes from there.
    let state = status(&rig, &acct, &id).await;
    assert_eq!(
        (state["state"].as_str(), state["received"].as_u64()),
        (Some("uploading"), Some(400))
    );

    // Trickling: a few bytes at a time, each within the idle limit, but far under the minimum rate once the grace period is over.
    let pieces: Vec<Vec<u8>> = blob[400..].chunks(20).map(<[u8]>::to_vec).collect();
    let (code, answer) = upload_raw(
        &rig,
        &acct,
        &id,
        blob.len(),
        400,
        trickle(pieces, Duration::from_millis(60)),
    )
    .await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::REQUEST_TIMEOUT, Some("timeout")),
        "{answer}"
    );
    let received = status(&rig, &acct, &id).await["received"]
        .as_u64()
        .expect("received") as usize;
    assert!(received > 400 && received < blob.len(), "{received}");

    // A client that is fast enough finishes from where the server holds it.
    let (code, answer) = upload(&rig, &acct, &id, &blob, received, blob.len()).await;
    assert_eq!(
        (code, answer["result"].as_str()),
        (StatusCode::OK, Some("published"))
    );
    assert_eq!(download(&rig, &acct, &id, None).await.2, blob);
}

#[tokio::test]
async fn a_client_that_goes_quiet_or_trickles_is_cut_off_with_a_408_on_a_push() {
    let rig = rig_with(quick()).await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let envelope = msg(&maker, 1, 50, T0);
    let (item_id, item_op) = (id(1), id(50));
    let whole = body(&[Item {
        entity: "message",
        id: &item_id,
        op: &item_op,
        bytes: &envelope,
    }]);
    let cursor = cursor(generation(&rig).await, 0, 0);
    let send = |pieces: Vec<Vec<u8>>, gap: Duration| {
        let (rig, acct, cursor) = (&rig, &acct, cursor.clone());
        async move {
            let (status, _, bytes) = rig
                .http
                .request(
                    "POST",
                    "/v1/push",
                    Some(&acct.token),
                    &[
                        ("x-kinige-cursor", cursor),
                        ("x-kinige-protocol", "1".to_string()),
                    ],
                    trickle(pieces, gap),
                )
                .await;
            (
                status,
                serde_json::from_slice::<serde_json::Value>(&bytes).unwrap_or_default(),
            )
        }
    };

    // A byte at a time, 60 ms apart: no chunk is idle for long, the rate is far under the minimum.
    let single: Vec<Vec<u8>> = whole.iter().map(|b| vec![*b]).collect();
    let (status, answer) = send(single, Duration::from_millis(60)).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::REQUEST_TIMEOUT, Some("timeout")),
        "{answer}"
    );
    assert!(
        log_seqs(&rig, &acct).await.is_empty(),
        "a cut-off push stored nothing"
    );

    // Silence before the first byte.
    let (status, answer) = send(vec![whole[..30].to_vec()], Duration::from_millis(600)).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::REQUEST_TIMEOUT, Some("timeout")),
        "{answer}"
    );

    // The same body in one piece is accepted.
    let (status, answer) = send(vec![whole.clone()], Duration::ZERO).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["results"][0]["result"], "ok");
}
