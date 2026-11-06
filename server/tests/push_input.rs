//! What a push request has to look like (ADR-0001 appendix §A.4, server steps 1 and 2, and §A.6): the session first, then the lengths and
//! the open header of every envelope before its body is buffered; the cursor header; the batch cap.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use http_body::Frame;
use kinige_server::config::Config;
use kinige_server::wire::encode_frames;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

/// A request body that remembers whether anyone asked it for data.
struct Tripwire(Arc<AtomicBool>);

impl http_body::Body for Tripwire {
    type Data = axum::body::Bytes;
    type Error = std::convert::Infallible;

    fn poll_frame(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        self.0.store(true, Ordering::SeqCst);
        Poll::Ready(None)
    }
}

#[tokio::test]
async fn the_session_is_judged_before_a_byte_of_the_body_is_read() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let current = cursor(generation(&rig).await, 0, 0);
    let touched = Arc::new(AtomicBool::new(false));
    let request = |token: Option<&str>| {
        let mut builder = Request::builder()
            .method("POST")
            .uri("/v1/push")
            .header("x-kinige-cursor", &current)
            .header("x-kinige-protocol", "1");
        if let Some(token) = token {
            builder = builder.header("authorization", token);
        }
        builder
            .body(Body::new(Tripwire(Arc::clone(&touched))))
            .expect("request")
    };
    // No header, a header that is not a bearer, a bearer that is not a token, and a well-formed token that is not a session.
    let unknown = format!("Bearer {}", b64(&[7; 32]));
    for header in [
        None,
        Some("Basic abc"),
        Some("Bearer not-a-token"),
        Some(unknown.as_str()),
    ] {
        let (status, answer) = rig.http.send(request(header)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{header:?}");
        assert!(
            String::from_utf8_lossy(&answer).contains("auth"),
            "{header:?}"
        );
        assert!(
            !touched.load(Ordering::SeqCst),
            "the body was polled for {header:?}"
        );
    }
    // With a session the body is read (here it is empty, so it is a 400 for that reason).
    let (status, _) = rig
        .http
        .send(request(Some(&format!("Bearer {}", acct.token))))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(touched.load(Ordering::SeqCst));
}

#[tokio::test]
async fn lengths_and_headers_are_judged_before_the_bodies_are_buffered() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let bytes = msg(&maker, 1, 50, T0);
    let (item_id, item_op) = (id(1), id(50));
    let good = body(&[Item {
        entity: "message",
        id: &item_id,
        op: &item_op,
        bytes: &bytes,
    }]);
    let current = cursor(generation(&rig).await, 0, 0);

    // The session comes first: a valid body without one is a 401, and so is garbage without one.
    let (status, _) = rig.http.push(None, &current, Some("1"), good.clone()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = rig
        .http
        .push(
            None,
            &current,
            Some("1"),
            b"garbage that is not a frame".to_vec(),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // With a session, garbage is a 400 that names the rule.
    let (status, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &current,
            Some("1"),
            b"garbage that is not a frame".to_vec(),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("frame_entity"))
    );

    // The declared envelope size is refused with nothing of the envelope sent.
    let mut huge = vec![7u8];
    huge.extend_from_slice(b"message");
    huge.extend_from_slice(&[0x11; 32]);
    huge.extend_from_slice(&u32::MAX.to_be_bytes());
    let (status, answer) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), huge)
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, Some("too_large"))
    );
    // A body that declares more than the cap in its Content-Length is refused before any of it is read.
    let request = Request::builder()
        .method("POST")
        .uri("/v1/push")
        .header("x-kinige-cursor", &current)
        .header("x-kinige-protocol", "1")
        .header("authorization", format!("Bearer {}", acct.token))
        .header(
            "content-length",
            (Config::default().max_push_body + 1).to_string(),
        )
        .body(Body::from(good.clone()))
        .expect("request");
    assert_eq!(
        rig.http.send(request).await.0,
        StatusCode::PAYLOAD_TOO_LARGE
    );

    // The header must say what the frame declares; an unknown format byte and a truncated body are 400s.
    let wrong = encode_frames(&[("message", &id(2), &id(50), &bytes[..])]).expect("frame");
    let (status, answer) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), wrong)
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("header_mismatch"))
    );
    let mut future = bytes.clone();
    future[0] = 2;
    let (_, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &current,
            Some("1"),
            body(&[Item {
                entity: "message",
                id: &item_id,
                op: &item_op,
                bytes: &future,
            }]),
        )
        .await;
    assert_eq!(answer["error"], "unsupported_format");
    let (_, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &current,
            Some("1"),
            good[..good.len() - 1].to_vec(),
        )
        .await;
    assert_eq!(answer["error"], "frame_truncated");
    assert!(
        log_seqs(&rig, &acct).await.is_empty(),
        "nothing of a refused request was written"
    );

    // The cursor header is required and strict.
    for bad in [
        "",
        "1.2",
        // Exactly three parts: a push carries no lineage (S4c-2), a fourth part of any kind is malformed.
        "1.2.3.4",
        "1.2.3.4.5",
        "1.2.3.x",
        "1.2.3.-4",
        "1.2.3.99999999999999999999",
        "1.2.3.",
        "a.b.c",
        "1.-2.3",
        "1.2.99999999999999999999",
    ] {
        let (status, answer) = rig
            .http
            .push(Some(&acct.token), bad, Some("1"), good.clone())
            .await;
        assert_eq!(
            (status, answer["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some("cursor")),
            "{bad:?}"
        );
    }
    let (status, _) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), good)
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_batch_has_a_cap_and_every_envelope_of_a_batch_is_answered_in_order() {
    let rig = rig_with(Config {
        max_batch: 3,
        ..Config::default()
    })
    .await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    let envelopes: Vec<Vec<u8>> = (1..=4u8).map(|n| msg(&maker, n, 100 + n, T0)).collect();
    let ids: Vec<(String, String)> = (1..=4u8).map(|n| (id(n), id(100 + n))).collect();
    let items: Vec<Item<'_>> = envelopes
        .iter()
        .zip(&ids)
        .map(|(bytes, (i, o))| Item {
            entity: "message",
            id: i,
            op: o,
            bytes,
        })
        .collect();
    let current = cursor(generation(&rig).await, 0, 0);
    let (status, _) = rig
        .http
        .push(Some(&acct.token), &current, Some("1"), body(&items))
        .await;
    assert_eq!(
        status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "four frames, a cap of three"
    );
    let (status, answer) = rig
        .http
        .push(
            Some(&acct.token),
            &current,
            Some("1"),
            body(items.get(..3).expect("three")),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let results = parse_push(&answer).results;
    assert_eq!(
        results.iter().map(|r| r.op.clone()).collect::<Vec<_>>(),
        vec![id(101), id(102), id(103)]
    );
    assert_eq!(
        results.iter().filter_map(|r| r.seq).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}
