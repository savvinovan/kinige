//! The account flows against an in-memory server (appendix §B items 5, 6 and 8; the profile rule, items 7 and 14, is in `profiles.rs`).
#![cfg(test)]

mod common;

use common::*;
use kinige_crypto::keys::rc_auth_key;
use kinige_crypto::words::Words;
use s4b_keys::client::{ClientError, Pending};
use s4b_keys::link::Link;
use s4b_keys::server::{AuthFailed, RecoverError};

#[test]
fn registration_then_login_on_a_new_device_reads_everything() {
    let (mut server, log) = world();
    let (a, _words) = register(&mut server, &log, LOGIN, PASSWORD);
    write_envelope(&a, &mut server, 1);
    write_envelope(&a, &mut server, 2);

    let mut b = device(&log);
    login(&mut server, &mut b, LOGIN, PASSWORD).expect("login on a new device");
    assert_eq!(read_all(&b, &server).len(), 2);
    assert!(
        b.dek()
            .expect("dek")
            .key()
            .ct_eq(a.dek().expect("dek").key())
    );
    // A wrong password and an unknown login are the same refusal.
    let mut c = device(&log);
    assert_eq!(
        login(&mut server, &mut c, LOGIN, "fake wrong password"),
        Err(ClientError::Auth)
    );
    assert_eq!(
        login(&mut server, &mut c, "nobody@example.test", PASSWORD),
        Err(ClientError::Auth)
    );
}

#[test]
fn a_password_change_does_not_reencrypt_and_old_entries_stay_readable() {
    let (mut server, log) = world();
    let (mut a, _words) = register(&mut server, &log, LOGIN, PASSWORD);
    let before: Vec<Vec<u8>> = (1..=3)
        .map(|n| write_envelope(&a, &mut server, n))
        .collect();

    a.change_password(
        &mut Link::new(&mut server),
        &password(PASSWORD),
        &password(NEW_PASSWORD),
        false,
    )
    .expect("change");

    let after = server.pull_envelopes(&token(&a)).expect("pull");
    assert_eq!(
        after, before,
        "the stored envelopes are the very same bytes: nothing was re-encrypted"
    );
    let mut b = device(&log);
    login(&mut server, &mut b, LOGIN, NEW_PASSWORD).expect("login with the new password");
    let texts = read_all(&b, &server);
    assert_eq!(
        texts.len(),
        3,
        "a device that logged in with the new password reads every envelope written before the change"
    );
    assert!(texts[0].contains("fake entry number 1"));
    let mut c = device(&log);
    assert_eq!(
        login(&mut server, &mut c, LOGIN, PASSWORD),
        Err(ClientError::Auth),
        "the old password no longer logs in"
    );
}

#[test]
fn recovery_with_the_24_words_gives_back_the_data_and_ends_other_sessions() {
    let (mut server, log) = world();
    let (a, words) = register(&mut server, &log, LOGIN, PASSWORD);
    write_envelope(&a, &mut server, 1);
    let stale_token = token(&a);

    // A device that has neither the password nor a session: only the words.
    let mut c = device(&log);
    let new_pw = password(NEW_PASSWORD);
    c.begin_recovery(&mut Link::new(&mut server), LOGIN, &words, &new_pw)
        .expect("begin");
    c.confirm_saved().expect("save");
    c.send_recovery(&mut Link::new(&mut server))
        .expect("recover");

    assert_eq!(
        read_all(&c, &server).len(),
        1,
        "the DEK came out of wrap_rc, so the old entries open"
    );
    assert_eq!(
        server.session_count(&account_id(&c)),
        1,
        "every other session was revoked"
    );
    assert!(
        server.push_envelope(&stale_token, b"x").is_err(),
        "the old session is dead"
    );
    let mut d = device(&log);
    login(&mut server, &mut d, LOGIN, NEW_PASSWORD).expect("the new password works");
    let mut e = device(&log);
    assert_eq!(
        login(&mut server, &mut e, LOGIN, PASSWORD),
        Err(ClientError::Auth),
        "the old password is gone"
    );
}

#[test]
fn after_recovery_the_old_words_are_invalid_and_the_new_ones_work() {
    let (mut server, log) = world();
    let (_a, old_words) = register(&mut server, &log, LOGIN, PASSWORD);
    let mut c = device(&log);
    c.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &old_words,
        &password(NEW_PASSWORD),
    )
    .expect("begin");
    let new_words = c.shown_words().expect("new words").phrase().to_string();
    assert_ne!(new_words, old_words, "recovery issues new words");
    c.confirm_saved().expect("save");
    c.send_recovery(&mut Link::new(&mut server))
        .expect("recover");

    let mut d = device(&log);
    assert_eq!(
        d.begin_recovery(
            &mut Link::new(&mut server),
            LOGIN,
            &old_words,
            &password("fake password three")
        ),
        Err(ClientError::Auth),
        "the old words are invalid"
    );
    d.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &new_words,
        &password("fake password three"),
    )
    .expect("the new words work");
}

#[test]
fn reissuing_the_words_replaces_them() {
    let (mut server, log) = world();
    let (mut a, old_words) = register(&mut server, &log, LOGIN, PASSWORD);
    write_envelope(&a, &mut server, 1);
    a.begin_reissue().expect("begin");
    let new_words = a.shown_words().expect("words").phrase().to_string();
    assert_ne!(new_words, old_words);
    a.confirm_saved().expect("save");
    a.send_reissue(&mut Link::new(&mut server), &password(PASSWORD))
        .expect("reissue");

    let mut c = device(&log);
    assert_eq!(
        c.begin_recovery(
            &mut Link::new(&mut server),
            LOGIN,
            &old_words,
            &password(NEW_PASSWORD)
        ),
        Err(ClientError::Auth)
    );
    c.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &new_words,
        &password(NEW_PASSWORD),
    )
    .expect("the new words");
    c.confirm_saved().expect("save");
    c.send_recovery(&mut Link::new(&mut server))
        .expect("recover");
    assert_eq!(read_all(&c, &server).len(), 1);
}

#[test]
fn an_unknown_login_looks_like_a_known_one() {
    let (mut server, log) = world();
    let (a, _words) = register(&mut server, &log, LOGIN, PASSWORD);
    let known = server.login_params(LOGIN);
    let unknown = server.login_params("nobody@example.test");
    assert_eq!(unknown.salt.len(), known.salt.len());
    assert_eq!(unknown.salt.len(), 16);
    assert_eq!(
        unknown.profile, known.profile,
        "the same profile (an account with profile 2 differs: accepted, §8.4)"
    );
    assert_eq!(
        server.login_params("nobody@example.test").salt,
        unknown.salt,
        "the same unknown login gets the same salt every time"
    );
    assert_ne!(
        server.login_params("other.nobody@example.test").salt,
        unknown.salt
    );
    assert_ne!(known.salt, unknown.salt);

    // Recovery: "no such login" and "wrong key" are the same value and write the same log line.
    let words_of_nobody = {
        let (_, words) = register(
            &mut server,
            &log,
            "second@example.test",
            "fake password four",
        );
        words
    };
    let mut wrong = device(&log);
    let before = log.len();
    let wrong_key = wrong.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &words_of_nobody,
        &password(NEW_PASSWORD),
    );
    let wrong_lines = log.lines()[before..].to_vec();
    let mut nobody = device(&log);
    let before = log.len();
    let no_login = nobody.begin_recovery(
        &mut Link::new(&mut server),
        "nobody@example.test",
        &words_of_nobody,
        &password(NEW_PASSWORD),
    );
    assert_eq!(wrong_key, Err(ClientError::Auth));
    assert_eq!(wrong_key, no_login);
    assert_eq!(
        wrong_lines
            .iter()
            .filter(|line| line.starts_with("server"))
            .count(),
        log.lines()[before..]
            .iter()
            .filter(|line| line.starts_with("server"))
            .count()
    );
    // Straight against the server the answer is one value that carries nothing.
    let unknown_key = rc_auth_key(
        &Words::parse(&words_of_nobody)
            .expect("words")
            .rc_root()
            .expect("root"),
    )
    .expect("key");
    assert_eq!(
        server
            .recovery_fetch("nobody@example.test", &unknown_key)
            .err(),
        Some(AuthFailed)
    );
    assert_eq!(
        server.recovery_fetch(LOGIN, &unknown_key).err(),
        Some(AuthFailed)
    );
    drop(a);
}

#[test]
fn another_package_behind_an_applied_rc_auth_key_is_its_own_error() {
    let (mut server, log) = world();
    let (_a, words) = register(&mut server, &log, LOGIN, PASSWORD);
    let mut c = device(&log);
    c.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &words,
        &password(NEW_PASSWORD),
    )
    .expect("begin");
    let Some(Pending::Recovery(pending)) = c.store.pending.clone() else {
        panic!("a recovery is pending");
    };
    c.confirm_saved().expect("save");
    c.send_recovery(&mut Link::new(&mut server))
        .expect("recover");
    // The same new rc_auth_key with a different password part: not the same package.
    let mut mismatch = pending.request.clone();
    mismatch.password.salt = [0x77; 16];
    let old = rc_auth_key(
        &Words::parse(&words)
            .expect("words")
            .rc_root()
            .expect("root"),
    )
    .expect("key");
    assert_eq!(
        server.recover(LOGIN, &old, &mismatch).err(),
        Some(RecoverError::PackageMismatch)
    );
    assert!(
        server.recover(LOGIN, &old, &pending.request).is_ok(),
        "the very same package again is a success"
    );
}
