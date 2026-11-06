//! CLAUDE.md rule 12 for the S4c-2 endpoints: random methods, ids, sizes, offsets, ranges and bodies on the blob routes, and random JSON on the
//! account deletion, give a 4xx and never a 5xx or a panic, and leave nothing half-published behind. A proptest at a fixed count and seed
//! (`GARBAGE_CASES`, default 300, `GARBAGE_SEED`, default 1). The pure parts have unit tests (`rules::blob`, `cli`, `backup::manifest`);
//! garbage backups are in `restore_rules.rs`.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::blobs::{blob_uuid, sealed};
use common::*;
use proptest::prelude::*;
use proptest::test_runner::{
    Config as ProptestConfig, RngAlgorithm, RngSeed, TestError, TestRunner,
};
use serde_json::json;

#[derive(Debug, Clone)]
struct Case {
    method: &'static str,
    id: String,
    status: bool,
    session: bool,
    size: Option<String>,
    offset: Option<String>,
    range: Option<String>,
    body: Vec<u8>,
}

fn header_text() -> impl Strategy<Value = Option<String>> {
    prop::option::of(prop_oneof![
        "[0-9]{1,12}",
        "-?[0-9]{1,25}",
        "[ -~]{0,20}",
        Just("1065".to_string()),
        Just("0".to_string()),
    ])
}

fn case() -> impl Strategy<Value = Case> {
    (
        prop::sample::select(vec!["PUT", "GET", "DELETE", "POST", "HEAD", "PATCH"]),
        prop_oneof![
            Just(blob_uuid(1)),
            Just(blob_uuid(2)),
            "[0-9a-f-]{36}",
            "[ -~]{0,50}",
            "\\PC{0,20}",
            Just("..".to_string()),
            Just("%2e%2e".to_string()),
        ],
        any::<bool>(),
        any::<bool>(),
        header_text(),
        header_text(),
        prop::option::of(prop_oneof![
            "bytes=[0-9]{0,6}-[0-9]{0,6}",
            "bytes=-[0-9]{0,6}",
            "[ -~]{0,20}",
        ]),
        prop::collection::vec(any::<u8>(), 0..2500),
    )
        .prop_map(
            |(method, id, status, session, size, offset, range, body)| Case {
                method,
                id,
                status,
                session,
                size,
                offset,
                range,
                body,
            },
        )
}

#[test]
fn garbage_on_the_blob_routes_is_a_4xx_and_never_a_5xx() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    let rig = runtime.block_on(rig());
    let acct = runtime.block_on(account(&rig));
    // One real blob, so that GETs and DELETEs have something to find.
    let real = sealed(3000, 1);
    runtime.block_on(common::blobs::upload_all(&rig, &acct, &blob_uuid(1), &real));

    let cases: u32 = std::env::var("GARBAGE_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let seed: u64 = std::env::var("GARBAGE_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let mut runner = TestRunner::new(ProptestConfig {
        cases,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    let outcome = runner.run(&case(), |case| {
        runtime.block_on(async {
            let path = if case.status {
                format!("/v1/blobs/{}/status", case.id)
            } else {
                format!("/v1/blobs/{}", case.id)
            };
            let mut builder = Request::builder().method(case.method).uri(&path);
            if case.session {
                builder = builder.header("authorization", format!("Bearer {}", acct.token));
            }
            for (name, value) in [
                ("x-kinige-blob-size", &case.size),
                ("x-kinige-blob-offset", &case.offset),
                ("range", &case.range),
            ] {
                if let Some(value) = value {
                    builder = builder.header(name, value);
                }
            }
            let Ok(request) = builder.body(Body::from(case.body.clone())) else {
                // A path or header the HTTP library itself refuses never reaches the server.
                return Ok(());
            };
            let (status, _) = rig.http.send(request).await;
            prop_assert!(status.as_u16() < 500, "{} {path}: {status}", case.method);
            if !case.session {
                prop_assert!(
                    status == StatusCode::UNAUTHORIZED
                        || status == StatusCode::NOT_FOUND
                        || status == StatusCode::METHOD_NOT_ALLOWED,
                    "{path}: without a session got {status}"
                );
            }
            Ok(())
        })
    });
    match outcome {
        Ok(()) => {}
        Err(TestError::Fail(reason, case)) => panic!(
            "garbage test failed\n  seed {seed}, cases {cases}\n  reason: {reason}\n  case: {case:?}"
        ),
        Err(TestError::Abort(reason)) => panic!("garbage test aborted: {reason}"),
    }

    // The real blob is where it was, the account is alive, and a garbage body for the deletion is a 4xx.
    runtime.block_on(async {
        let (code, _, bytes) = common::blobs::download(&rig, &acct, &blob_uuid(1), None).await;
        assert!(
            code == StatusCode::OK && bytes == real || code == StatusCode::NOT_FOUND,
            "{code}"
        );
        for body in [
            json!({}),
            json!({"auth_key": 5}),
            json!({"auth_key": "short"}),
            json!([1, 2]),
            json!({"auth_key": b64(&[0u8; 32])}),
        ] {
            let (status, _) = rig
                .http
                .post("/v1/account/delete", Some(&acct.token), &body)
                .await;
            assert!(status.is_client_error(), "{body}: {status}");
        }
        let (status, _) = rig
            .http
            .post_raw("/v1/account/delete", Some(&acct.token), vec![0xff; 40])
            .await;
        assert!(status.is_client_error());
        assert_eq!(
            pull(&rig, &acct, generation(&rig).await, 0, 0, 1).await.0,
            StatusCode::OK,
            "the account is still there"
        );
    });
}
