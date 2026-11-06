//! CLAUDE.md rule 12: a new IPC command comes with a test on garbage input. Random and truncated bodies, random JSON and random headers on
//! every endpoint give a 4xx and never a 5xx or a panic. A proptest at a fixed count and seed (`GARBAGE_CASES`, default 300, `GARBAGE_SEED`,
//! default 1); a failure prints the seed and the shrunk case.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::*;
use kinige_server::config::Config;
use kinige_server::rate::Rule;
use proptest::prelude::*;
use proptest::test_runner::{
    Config as ProptestConfig, RngAlgorithm, RngSeed, TestError, TestRunner,
};
use serde_json::{Map, Value, json};

const PATHS: [&str; 10] = [
    "/v1/register",
    "/v1/login/params",
    "/v1/login",
    "/v1/logout",
    "/v1/password/change",
    "/v1/recovery/fetch",
    "/v1/recovery",
    "/v1/words/reissue",
    "/v1/pull",
    "/v1/push",
];

const FIELDS: [&str; 22] = [
    "login",
    "auth_key",
    "rc_auth_key",
    "account_id",
    "salt",
    "profile",
    "wrap_pw",
    "wrap_rc",
    "key_id",
    "new",
    "replaces",
    "expected_version",
    "revoke_others",
    "old_rc_auth_key",
    "new_rc_auth_key",
    "generation",
    "after",
    "known_epoch",
    "limit",
    "token",
    "envelope",
    "result",
];

/// What one request is made of.
#[derive(Debug, Clone)]
enum Garbage {
    /// Bytes that are nothing: the response must be a 4xx.
    Bytes(Vec<u8>),
    /// JSON of any shape or an object with the protocol's own field names and wrong values: a 4xx, or a 200 where it happens to be valid.
    Json(Value),
    /// A real push body, cut, flipped or extended: never a 5xx.
    Push {
        cut: usize,
        flip: Option<(usize, u8)>,
        extra: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
struct Case {
    path: usize,
    session: bool,
    garbage: Garbage,
    /// Header values for the cursor and the protocol, printable ASCII.
    cursor: String,
    protocol: String,
    /// A GET instead of a POST, or a path that is not one.
    odd_route: Option<bool>,
}

fn value() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|n| json!(n)),
        any::<u64>().prop_map(|n| json!(n)),
        any::<f64>()
            .prop_filter("finite", |f| f.is_finite())
            .prop_map(|f| json!(f)),
        "\\PC{0,40}".prop_map(Value::String),
        Just(json!("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")),
        Just(json!("fake.user@example.test")),
        Just(json!("11111111-1111-4111-8111-111111111111")),
    ];
    leaf.prop_recursive(3, 24, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..4).prop_map(Value::Array),
            prop::collection::btree_map("[a-z_]{1,12}", inner, 0..5)
                .prop_map(|m| Value::Object(m.into_iter().collect())),
        ]
    })
}

fn field_object() -> impl Strategy<Value = Value> {
    prop::collection::vec((prop::sample::select(FIELDS.to_vec()), value()), 0..10).prop_map(
        |pairs| {
            let mut object = Map::new();
            for (name, v) in pairs {
                object.insert(name.to_string(), v);
            }
            Value::Object(object)
        },
    )
}

fn case() -> impl Strategy<Value = Case> {
    let garbage = prop_oneof![
        prop::collection::vec(any::<u8>(), 0..3000).prop_map(Garbage::Bytes),
        value().prop_map(Garbage::Json),
        field_object().prop_map(Garbage::Json),
        (
            any::<prop::sample::Index>(),
            any::<Option<(prop::sample::Index, u8)>>(),
            prop::collection::vec(any::<u8>(), 0..40)
        )
            .prop_map(|(cut, flip, extra)| Garbage::Push {
                cut: cut.index(usize::MAX),
                flip: flip.map(|(at, byte)| (at.index(usize::MAX), byte)),
                extra,
            }),
    ];
    (
        0..PATHS.len(),
        any::<bool>(),
        garbage,
        "[ -~]{0,30}",
        "[ -~]{0,6}",
        prop::option::weighted(0.05, any::<bool>()),
    )
        .prop_map(
            |(path, session, garbage, cursor, protocol, odd_route)| Case {
                path,
                session,
                garbage,
                cursor,
                protocol,
                odd_route,
            },
        )
}

fn raw_request(
    path: &str,
    method: &str,
    token: Option<&str>,
    cursor: &str,
    protocol: &str,
    body: Vec<u8>,
) -> Option<Request<Body>> {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("x-kinige-cursor", cursor)
        .header("x-kinige-protocol", protocol);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body)).ok()
}

#[test]
fn garbage_on_every_endpoint_is_a_4xx_and_never_a_5xx() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("runtime");
    // The limits are lifted so that the cases reach the handlers instead of the rate limiter; a 429 is a 4xx anyway.
    let config = Config {
        login_by_login: Rule::new(u32::MAX, 1),
        login_by_addr: Rule::new(u32::MAX, 1),
        recovery_by_login: Rule::new(u32::MAX, 1),
        recovery_by_addr: Rule::new(u32::MAX, 1),
        ..Config::default()
    };
    let rig = runtime.block_on(rig_with(config));
    let maker = Maker::new();
    let valid = maker.upsert(
        "message",
        &uuid(1),
        &uuid(2),
        &[("body", hlc(T0, 0))],
        "fake",
    );
    let valid_body = body(&[Item {
        entity: "message",
        id: &uuid(1),
        op: &uuid(2),
        bytes: &valid,
    }]);
    let creds = kinige_server::testing::Creds::random();
    runtime
        .block_on(creds.register(&rig.http))
        .expect("registration");

    let cases: u32 = std::env::var("GARBAGE_CASES")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let seed: u64 = std::env::var("GARBAGE_SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);
    let config = ProptestConfig {
        cases,
        rng_algorithm: RngAlgorithm::ChaCha,
        rng_seed: RngSeed::Fixed(seed),
        failure_persistence: None,
        ..ProptestConfig::default()
    };
    let mut runner = TestRunner::new(config);
    let outcome = runner.run(&case(), |case| {
        runtime.block_on(async {
            // A fresh session for every case: `/v1/logout` ends the one it is given.
            let token = if case.session {
                let (_, answer) = rig
                    .http
                    .post(
                        "/v1/login",
                        None,
                        &json!({"login": creds.login, "auth_key": b64(&creds.auth_key)}),
                    )
                    .await;
                answer["token"].as_str().map(str::to_string)
            } else {
                None
            };
            let path = PATHS[case.path];
            let (method, path) = match case.odd_route {
                Some(true) => ("GET", path),
                Some(false) => ("POST", "/v1/not-a-route"),
                None => ("POST", path),
            };
            let (bytes, must_fail) = match &case.garbage {
                Garbage::Bytes(bytes) => (bytes.clone(), true),
                Garbage::Json(value) => (serde_json::to_vec(value).unwrap_or_default(), false),
                Garbage::Push { cut, flip, extra } => {
                    let mut body = valid_body.clone();
                    if let Some((at, byte)) = flip {
                        let at = at % body.len();
                        body[at] ^= byte | 1;
                    }
                    body.truncate(cut % (body.len() + 1));
                    body.extend_from_slice(extra);
                    (body, false)
                }
            };
            let Some(request) = raw_request(
                path,
                method,
                token.as_deref(),
                &case.cursor,
                &case.protocol,
                bytes,
            ) else {
                return Ok(());
            };
            let (status, _) = rig.http.send(request).await;
            prop_assert!(status.as_u16() < 500, "{} {path}: {status}", case.path);
            if must_fail || method == "GET" || path == "/v1/not-a-route" {
                // Random bytes are not a request; `/v1/login/params` and `/v1/logout` with a session ignore the body or read `{}`.
                let ignores_body = matches!(case.path, 3);
                prop_assert!(
                    status.is_client_error() || (ignores_body && status == StatusCode::OK),
                    "{path}: random bytes got {status}"
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
    // The server still works after all of it.
    runtime.block_on(async {
        let (status, answer) = rig
            .http
            .post(
                "/v1/login",
                None,
                &json!({"login": creds.login, "auth_key": b64(&creds.auth_key)}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "login still works");
        let token = answer["token"].as_str().expect("token").to_string();
        assert_eq!(
            pull(
                &rig,
                &Acct {
                    creds: creds.clone(),
                    token
                },
                generation(&rig).await,
                0,
                0,
                1
            )
            .await
            .0,
            StatusCode::OK
        );
    });
}
