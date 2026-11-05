//! The account flows against an in-memory server (appendix §B items 5, 6, 8 and 14, and S4a items 28 and 29 that are about keys).
#![cfg(test)]

mod common;

use common::*;
use kinige_crypto::words::Words;
use kinige_crypto::{CryptoError, Profile, ProfileError};
use s4b_keys::client::{Client, ClientError, Pending};
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
    let unknown_key = kinige_crypto::keys::rc_auth_key(
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
fn a_server_that_sends_a_lower_profile_than_seen_is_refused_before_deriving() {
    let (mut server, log) = world();
    let (a, words) = register(&mut server, &log, LOGIN, PASSWORD);

    let mut seen_two = device(&log);
    seen_two.store.kdf_profile_seen = Some(Profile::P2);
    let result = login(&mut server, &mut seen_two, LOGIN, PASSWORD);
    assert_eq!(
        result,
        Err(ClientError::Profile(CryptoError::Profile(
            ProfileError::LowerThanSeen { sent: 1, seen: 2 }
        )))
    );
    assert_eq!(
        seen_two.derive_runs(),
        0,
        "refused before the password was derived: no Argon2id ran"
    );
    assert!(seen_two.account().is_none());

    // The way out is the words, which do not depend on Argon2 for the key: the new password gets profile 2.
    seen_two
        .begin_recovery(
            &mut Link::new(&mut server),
            LOGIN,
            &words,
            &password(NEW_PASSWORD),
        )
        .expect("recovery works");
    seen_two.confirm_saved().expect("save");
    seen_two
        .send_recovery(&mut Link::new(&mut server))
        .expect("recover");
    assert_eq!(
        server.login_params(LOGIN).profile,
        2,
        "the profile only grows: B item 14"
    );
    drop(a);
}

#[test]
fn an_unknown_profile_number_is_refused_before_deriving() {
    let (mut server, log) = world();
    let (a, _words) = register(&mut server, &log, LOGIN, PASSWORD);
    for hostile in [0u8, 3, 9, 255] {
        server.tamper_profile(&account_id(&a), hostile);
        let mut b = device(&log);
        assert_eq!(
            login(&mut server, &mut b, LOGIN, PASSWORD),
            Err(ClientError::Profile(CryptoError::Profile(
                ProfileError::Unknown(hostile)
            )))
        );
        assert_eq!(b.derive_runs(), 0, "profile {hostile}: no Argon2id");
    }
}

#[test]
fn a_password_change_never_lowers_the_profile_and_another_device_logs_in_after_it() {
    let (mut server, log) = world();
    let (mut x, _words) = register(&mut server, &log, LOGIN, PASSWORD);
    assert_eq!(server.login_params(LOGIN).profile, 1);

    // A newer client version, whose default is profile 2, raises the account when the password changes (max(seen, sent, default)).
    let mut newer = Client::with_default_profile(log.clone(), Profile::P2);
    login(&mut server, &mut newer, LOGIN, PASSWORD).expect("login");
    newer
        .change_password(
            &mut Link::new(&mut server),
            &password(PASSWORD),
            &password(NEW_PASSWORD),
            true,
        )
        .expect("change");
    assert_eq!(server.login_params(LOGIN).profile, 2);

    // A device of the older version, which never saw profile 2, logs in with the new password and now has seen it.
    let mut y = device(&log);
    login(&mut server, &mut y, LOGIN, NEW_PASSWORD).expect("login after the change");
    assert_eq!(y.store.kdf_profile_seen, Some(Profile::P2));
    // The older version changes the password again: its default is 1, the account stays at 2 (never downgraded).
    y.change_password(
        &mut Link::new(&mut server),
        &password(NEW_PASSWORD),
        &password("fake password five"),
        false,
    )
    .expect("change");
    assert_eq!(server.login_params(LOGIN).profile, 2);
    // A device that is still on the first password and has not seen 2 is refused by the server, not by the profile rule.
    assert_eq!(
        login(&mut server, &mut x, LOGIN, PASSWORD),
        Err(ClientError::Auth)
    );
}

#[test]
fn a_password_change_on_a_device_that_saw_a_higher_profile_than_the_server_sends_is_refused_before_deriving()
 {
    // After a restore of the server from an older backup the account may really be at profile 1 again while this device saw 2:
    // the rule refuses to derive the old password at the lower profile. Recovery is the way out.
    let (mut server, log) = world();
    let (mut a, _words) = register(&mut server, &log, LOGIN, PASSWORD);
    a.store.kdf_profile_seen = Some(Profile::P2);
    let runs = a.derive_runs();
    let result = a.change_password(
        &mut Link::new(&mut server),
        &password(PASSWORD),
        &password(NEW_PASSWORD),
        false,
    );
    assert!(matches!(result, Err(ClientError::Profile(_))), "{result:?}");
    assert_eq!(
        a.derive_runs(),
        runs,
        "no Argon2id ran for the refused change"
    );
}

#[test]
fn recovery_on_a_device_that_saw_profile_2_when_1_is_sent_gives_profile_2_and_logs_in() {
    let (mut server, log) = world();
    let (_a, words) = register(&mut server, &log, LOGIN, PASSWORD);
    assert_eq!(server.login_params(LOGIN).profile, 1);

    let mut x = device(&log);
    x.store.kdf_profile_seen = Some(Profile::P2);
    x.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &words,
        &password(NEW_PASSWORD),
    )
    .expect("begin");
    x.confirm_saved().expect("save");
    x.send_recovery(&mut Link::new(&mut server))
        .expect("recover");
    assert_eq!(
        server.login_params(LOGIN).profile,
        2,
        "not downgraded to the 1 the server sent"
    );

    let mut again = x.restart();
    again.store.account = None;
    login(&mut server, &mut again, LOGIN, NEW_PASSWORD)
        .expect("the device logs in with its new password without a refusal");
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
    let old = kinige_crypto::keys::rc_auth_key(
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
