//! Slice 5c, the pack job against a fake server on localhost (ADR-0021): no request without a token, the dismissed prompt, the calls with their
//! exact URLs and headers for each form of the link, progress, and cancel. The failures, the caps and the log are in `stickers_bot_failures.rs`.
//! Nothing here talks to Telegram, and the test token does not have the real token shape.

#![cfg(test)]

mod common;

use common::bot_env::*;
use kinige_domain::ErrorCode;
use kinige_lib::stickers::secret::{SecretError, SecretStore};
use std::sync::atomic::Ordering;

#[test]
fn without_a_token_there_is_no_request_and_the_secret_store_is_read_once() {
    let env = env();
    env.secrets.delete().unwrap();
    let before = env.secrets.calls();
    let server = healthy(2);
    let error = env.run(&server.base(), LINK).unwrap_err();
    assert_eq!(code(error), ErrorCode::StickerPackNoToken);
    assert_eq!(server.count(), 0);
    assert_eq!(env.secrets.calls() - before, 1);
    env.nothing_written();
    // A token in the store that is not of the shape is the same as none.
    env.secrets.set("not a token").unwrap();
    assert_eq!(
        code(env.run(&server.base(), LINK).unwrap_err()),
        ErrorCode::StickerPackNoToken
    );
    assert_eq!(server.count(), 0);
}

#[test]
fn a_dismissed_prompt_ends_the_job_with_its_own_error_and_writes_nothing() {
    let env = env();
    let server = healthy(2);
    env.secrets.fail_with(Some(SecretError::Denied));
    assert_eq!(
        code(env.run(&server.base(), LINK).unwrap_err()),
        ErrorCode::SecretStoreDenied
    );
    env.secrets.fail_with(Some(SecretError::Unavailable));
    assert_eq!(
        code(env.run(&server.base(), LINK).unwrap_err()),
        ErrorCode::SecretStoreUnavailable
    );
    assert_eq!(server.count(), 0);
    env.nothing_written();
}

#[test]
fn a_pack_is_fetched_by_each_form_of_the_link_with_exact_urls_and_headers() {
    for link in [LINK, "t.me/addstickers/TestPack", "TestPack"] {
        let env = env();
        let server = healthy(3);
        let added = env.run(&server.base(), link).unwrap();
        assert_eq!(
            (added.title.as_str(), added.added, added.skipped),
            ("Test Pack", 3, 0)
        );
        let seen = server.seen();
        assert_eq!(
            seen.len(),
            1 + 3 + 3,
            "one set, then a getFile and a download per sticker"
        );
        assert_eq!(
            seen[0].target,
            format!("/bot{TOKEN}/getStickerSet?name=TestPack")
        );
        assert_eq!(seen[1].target, format!("/bot{TOKEN}/getFile?file_id=S0"));
        assert_eq!(seen[2].target, format!("/file/bot{TOKEN}/stickers/S0.webp"));
        for request in &seen {
            assert_eq!(request.method, "GET");
            let mut names: Vec<&str> = request.headers.iter().map(|(n, _)| n.as_str()).collect();
            names.sort_unstable();
            assert_eq!(
                names,
                ["accept", "host", "user-agent"],
                "no Accept-Encoding, no cookie, nothing else"
            );
            let agent = &request
                .headers
                .iter()
                .find(|(n, _)| n == "user-agent")
                .unwrap()
                .1;
            assert_eq!(agent, concat!("kinige/", env!("CARGO_PKG_VERSION")));
        }
        let progress = env.progress.lock().unwrap().clone();
        assert_eq!(progress.first().map(|p| (p.done, p.total)), Some((0, 3)));
        assert_eq!(progress.last().map(|p| (p.done, p.total)), Some((3, 3)));
        assert_eq!(
            env.store.sticker_library(false).unwrap().packs[0]
                .stickers
                .len(),
            3
        );
        assert!(
            env.store.telegram_token_saved().unwrap(),
            "a token that reads corrects the flag"
        );
    }
}

#[test]
fn cancel_adds_nothing_and_leaves_no_file() {
    let env = env();
    let server = healthy(3);
    env.cancel.store(true, Ordering::SeqCst);
    assert_eq!(
        code(env.run(&server.base(), LINK).unwrap_err()),
        ErrorCode::StickerPackCancelled
    );
    env.nothing_written();
}
