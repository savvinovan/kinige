//! Slice 5c, the Bot API client against garbage and failure (ADR-0021): every status and every odd answer has its category and adds nothing,
//! a stalled server and a TLS failure are "unreachable", the caps are refused before a file is requested, hostile ids and paths never become a
//! request, a 401 or 429 in the middle stops the job, and the token, the link and the names never reach the log.

#![cfg(test)]

mod common;

use common::bot_env::*;
use common::fake_bot::*;
use kinige_domain::ErrorCode;
use kinige_lib::stickers::BotClient;
use kinige_lib::stickers::secret::SecretError;
use log::{LevelFilter, Log, Metadata, Record};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

struct Capture;

impl Log for Capture {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        CAPTURED
            .lock()
            .unwrap()
            .push(format!("[{}] {}", record.level(), record.args()));
    }

    fn flush(&self) {}
}

fn install_logger() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        log::set_logger(&Capture).expect("the logger is installed once");
        log::set_max_level(LevelFilter::Trace);
    });
}

#[test]
fn every_failure_has_its_category_and_adds_nothing() {
    let cases: Vec<(&str, Reply, ErrorCode)> = vec![
        (
            "401",
            Reply::Json(401, "{}".into()),
            ErrorCode::StickerPackTokenRefused,
        ),
        (
            "400",
            Reply::Json(400, "{}".into()),
            ErrorCode::StickerPackNotFound,
        ),
        (
            "404",
            Reply::Json(404, "{}".into()),
            ErrorCode::StickerPackNotFound,
        ),
        (
            "429",
            Reply::Json(429, "{}".into()),
            ErrorCode::StickerPackWait,
        ),
        (
            "500",
            Reply::Json(500, "{}".into()),
            ErrorCode::StickerPackBadResponse,
        ),
        (
            "not JSON",
            Reply::Raw(200, vec![], b"<html>".to_vec()),
            ErrorCode::StickerPackBadResponse,
        ),
        (
            "gzip",
            Reply::Raw(
                200,
                vec![("content-encoding".into(), "gzip".into())],
                b"\x1f\x8b".to_vec(),
            ),
            ErrorCode::StickerPackBadResponse,
        ),
        (
            "redirect elsewhere",
            Reply::Redirect(302, "https://example.invalid/x".into()),
            ErrorCode::StickerPackBadResponse,
        ),
        (
            "redirect to http",
            Reply::Redirect(301, "http://127.0.0.1:1/x".into()),
            ErrorCode::StickerPackBadResponse,
        ),
        (
            "1 GiB declared",
            Reply::Declared(1 << 30, b"{".to_vec()),
            ErrorCode::StickerPackTooLarge,
        ),
        (
            "a body that breaks off",
            Reply::Declared(1000, b"{\"ok\"".to_vec()),
            ErrorCode::StickerPackUnreachable,
        ),
        ("hang up", Reply::Hangup, ErrorCode::StickerPackUnreachable),
    ];
    for (name, reply, want) in cases {
        let env = env();
        let reply = Mutex::new(Some(reply));
        let server = FakeBot::start(Arc::new(move |_| {
            reply.lock().unwrap().take().unwrap_or(Reply::Hangup)
        }));
        let error = env.run(&server.base(), LINK).unwrap_err();
        assert_eq!(code(error), want, "{name}");
        assert_eq!(server.count(), 1, "{name}: no retry and no second request");
        env.nothing_written();
    }
}

#[test]
fn a_stalled_server_and_a_tls_failure_are_unreachable() {
    let env = env();
    let stall = FakeBot::start(Arc::new(|_| Reply::Stall));
    let client = BotClient::with_timeouts(
        &stall.base(),
        false,
        Duration::from_millis(300),
        Duration::from_millis(400),
    )
    .unwrap();
    assert_eq!(
        code(env.run_with(client, LINK).unwrap_err()),
        ErrorCode::StickerPackUnreachable
    );
    // A plain-HTTP server behind an https address: the handshake fails.
    let plain = healthy(1);
    let https = format!("https://{}", plain.addr);
    let client =
        BotClient::with_timeouts(&https, true, Duration::from_secs(2), Duration::from_secs(2))
            .unwrap();
    assert_eq!(
        code(env.run_with(client, LINK).unwrap_err()),
        ErrorCode::StickerPackUnreachable
    );
    // The real client refuses an http base.
    let client = BotClient::with_base(&plain.base(), true).unwrap();
    assert_eq!(
        code(env.run_with(client, LINK).unwrap_err()),
        ErrorCode::StickerPackUnreachable
    );
    assert_eq!(plain.count(), 0, "nothing reached the plain server");
    env.nothing_written();
}

#[test]
fn the_caps_of_a_pack_are_refused_before_a_file_is_requested() {
    let env = env();
    let too_many = FakeBot::start(Arc::new(|_| Reply::Json(200, set_json("Big", 201, &[]))));
    assert_eq!(
        code(env.run(&too_many.base(), LINK).unwrap_err()),
        ErrorCode::StickerPackTooLarge
    );
    assert_eq!(too_many.count(), 1);
    let heavy: Vec<serde_json::Value> = (0..40)
        .map(|i| serde_json::json!({"file_id": format!("H{i}"), "width": 512, "height": 512, "file_size": 2 * 1024 * 1024}))
        .collect();
    let over = FakeBot::start(Arc::new(move |_| {
        Reply::Json(200, set_json("Heavy", 0, &heavy))
    }));
    assert_eq!(
        code(env.run(&over.base(), LINK).unwrap_err()),
        ErrorCode::StickerPackTooLarge
    );
    assert_eq!(over.count(), 1);
    let animated =
        serde_json::json!({"file_id": "A1", "width": 512, "height": 512, "is_animated": true});
    let only = FakeBot::start(Arc::new(move |_| {
        Reply::Json(200, set_json("Moving", 0, std::slice::from_ref(&animated)))
    }));
    assert_eq!(
        code(env.run(&only.base(), LINK).unwrap_err()),
        ErrorCode::StickerPackOnlyAnimated
    );
    env.nothing_written();
}

#[test]
fn hostile_ids_and_paths_never_become_a_request_and_a_big_file_is_skipped() {
    let env = env();
    let extra = vec![
        serde_json::json!({"file_id": "a&b=c#d", "width": 512, "height": 512}),
        serde_json::json!({"file_id": "%2e%2e%2f", "width": 512, "height": 512}),
        serde_json::json!({"file_id": "S0", "width": 512, "height": 512}),
        serde_json::json!({"file_id": "TRAVERSE", "width": 512, "height": 512}),
        serde_json::json!({"file_id": "BIG", "width": 512, "height": 512}),
    ];
    let server = FakeBot::start(Arc::new(move |request| {
        let target = request.target.as_str();
        if target.contains("/getStickerSet?") {
            Reply::Json(200, set_json("Hostile", 2, &extra))
        } else if target.contains("file_id=TRAVERSE") {
            Reply::Json(
                200,
                serde_json::json!({"ok": true, "result": {"file_path": "../../etc/passwd"}})
                    .to_string(),
            )
        } else if target.contains("/getFile?") {
            let id = target.split("file_id=").nth(1).unwrap_or_default();
            Reply::Json(200, file_json(id))
        } else if target.contains("/BIG.webp") {
            Reply::Declared(1 << 30, vec![0; 16])
        } else {
            Reply::Raw(200, vec![], sticker_for(target))
        }
    }));
    let added = env.run(&server.base(), LINK).unwrap();
    // S0 and S1 are good (S0 once, though it is listed twice), TRAVERSE and BIG are skipped, the two with bad ids are skipped before any request.
    assert_eq!((added.added, added.skipped), (2, 4));
    let seen = server.seen();
    let asked: Vec<&str> = seen
        .iter()
        .map(|r| r.target.as_str())
        .filter(|t| t.contains("/getFile?"))
        .collect();
    assert_eq!(asked.len(), 4, "{asked:?}");
    assert!(
        seen.iter()
            .all(|r| !r.target.contains('#') && !r.target.contains(".."))
    );
}

#[test]
fn a_401_or_a_429_in_the_middle_stops_the_job_and_adds_nothing() {
    for (status, want) in [
        (401, ErrorCode::StickerPackTokenRefused),
        (429, ErrorCode::StickerPackWait),
    ] {
        let env = env();
        let server = FakeBot::start(Arc::new(move |request| {
            let target = request.target.as_str();
            if target.contains("/getStickerSet?") {
                Reply::Json(200, set_json("Mid", 3, &[]))
            } else if target.contains("file_id=S1") {
                Reply::Json(status, "{}".into())
            } else if target.contains("/getFile?") {
                Reply::Json(
                    200,
                    file_json(target.split("file_id=").nth(1).unwrap_or_default()),
                )
            } else {
                Reply::Raw(200, vec![], sticker_for(target))
            }
        }));
        assert_eq!(code(env.run(&server.base(), LINK).unwrap_err()), want);
        assert!(
            !server.seen().iter().any(|r| r.target.contains("S2")),
            "the job stopped"
        );
        env.nothing_written();
    }
}

#[test]
fn the_token_the_link_and_the_names_never_reach_the_log_on_any_failure_path() {
    install_logger();
    let mark = CAPTURED.lock().unwrap().len();
    let replies: Vec<Box<dyn Fn() -> Reply + Send + Sync>> = vec![
        Box::new(|| Reply::Json(401, "{}".into())),
        Box::new(|| Reply::Json(429, "{}".into())),
        Box::new(|| Reply::Raw(200, vec![], b"garbage".to_vec())),
        Box::new(|| Reply::Redirect(302, "http://127.0.0.1:1/x".into())),
        Box::new(|| Reply::Hangup),
        Box::new(|| Reply::Json(200, set_json("Secret Title Zebra", 2, &[]))),
    ];
    for reply in replies {
        let env = env();
        let server = FakeBot::start(Arc::new(move |request| {
            if request.target.contains("/getFile?") || request.target.starts_with("/file/") {
                Reply::Hangup
            } else {
                reply()
            }
        }));
        let _ = env.run(&server.base(), LINK);
    }
    // A connection that is refused and one that is not TLS.
    let env = env();
    let _ = env.run("http://127.0.0.1:9", LINK);
    let _ = env.run_with(
        BotClient::with_base("https://127.0.0.1:9", true).unwrap(),
        LINK,
    );
    env.secrets.fail_with(Some(SecretError::Denied));
    let _ = env.run("http://127.0.0.1:9", LINK);
    let lines = CAPTURED.lock().unwrap()[mark..].to_vec();
    for line in &lines {
        for secret in [
            TOKEN,
            "fake-token-for-tests",
            "TestPack",
            "addstickers",
            "Zebra",
        ] {
            assert!(!line.contains(secret), "{secret} in a log line: {line}");
        }
    }
}
