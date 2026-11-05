//! Appendix §B item 10: after a full run, nothing the client and the server logged contains a key, a token, a password, the words or
//! entry text, in any encoding the code could print. The scanner is proven first against deliberate leaks, so a pass means something.
#![cfg(test)]

mod common;

use common::*;
use kinige_crypto::Profile;
use kinige_crypto::keys;
use kinige_crypto::words::Words;
use s4b_keys::client::Pending;
use s4b_keys::leak::{Secret, encodings, find_leaks};
use s4b_keys::link::Link;

#[test]
fn the_scanner_finds_a_deliberate_leak_in_every_encoding() {
    let key: Vec<u8> = (0..32).map(|i| (i * 11 + 5) as u8).collect();
    let secrets = [Secret::new("auth_key", &key)];
    for form in encodings(&key) {
        let (_, log) = world();
        log.event(format!("register body: {form}"));
        assert_eq!(
            find_leaks(&log.text(), &secrets),
            vec!["auth_key"],
            "{form}"
        );
    }
    let (_, log) = world();
    log.event(format!("request {key:?}"));
    assert_eq!(
        find_leaks(&log.text(), &secrets),
        vec!["auth_key"],
        "the Debug of the bytes"
    );
}

#[test]
fn a_full_run_logs_no_secret_in_any_encoding() {
    let (mut server, log) = world();
    let mut secrets: Vec<Secret> = Vec::new();

    // Registration. The package holds the keys the server will receive.
    let mut a = device(&log);
    a.begin_registration(LOGIN, &password(PASSWORD))
        .expect("begin");
    let first_words = a.shown_words().expect("words").phrase().to_string();
    if let Some(Pending::Registration(pending)) = &a.store.pending {
        secrets.push(Secret::new("auth_key", pending.package.auth_key.as_bytes()));
        secrets.push(Secret::new(
            "rc_auth_key",
            pending.package.rc_auth_key.as_bytes(),
        ));
        secrets.push(Secret::new("dek", pending.dek.key().as_bytes()));
        secrets.push(Secret::new("k_env", pending.dek.env_key().as_bytes()));
        let root = keys::derive_pw_root(&password(PASSWORD), &pending.package.salt, Profile::P1)
            .expect("root");
        secrets.push(Secret::new("pw_root", root.as_bytes()));
        secrets.push(Secret::new(
            "kek_pw",
            keys::pw_kek(&root).expect("kek").as_bytes(),
        ));
        let rc = Words::parse(&first_words)
            .expect("words")
            .rc_root()
            .expect("root");
        secrets.push(Secret::new("rc_root", rc.as_bytes()));
        secrets.push(Secret::new(
            "kek_rc",
            keys::rc_kek(&rc).expect("kek").as_bytes(),
        ));
    }
    a.confirm_saved().expect("save");
    a.send_registration(&mut Link::new(&mut server))
        .expect("register");
    secrets.push(Secret::new("token", token(&a).as_bytes()));
    write_envelope(&a, &mut server, 7);

    // Login on a second device, a password change, recovery with the words, new words.
    let mut b = device(&log);
    login(&mut server, &mut b, LOGIN, PASSWORD).expect("login");
    secrets.push(Secret::new("token b", token(&b).as_bytes()));
    b.change_password(
        &mut Link::new(&mut server),
        &password(PASSWORD),
        &password(NEW_PASSWORD),
        false,
    )
    .expect("change");

    let mut c = device(&log);
    c.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &first_words,
        &password("fake password ten"),
    )
    .expect("recovery");
    let second_words = c.shown_words().expect("words").phrase().to_string();
    if let Some(Pending::Recovery(pending)) = &c.store.pending {
        secrets.push(Secret::new(
            "new auth_key",
            pending.request.password.auth_key.as_bytes(),
        ));
        secrets.push(Secret::new(
            "new rc_auth_key",
            pending.request.rc_auth_key.as_bytes(),
        ));
    }
    c.confirm_saved().expect("save");
    c.send_recovery(&mut Link::new(&mut server))
        .expect("recover");
    secrets.push(Secret::new("token c", token(&c).as_bytes()));
    c.begin_reissue().expect("reissue");
    let third_words = c.shown_words().expect("words").phrase().to_string();
    c.confirm_saved().expect("save");
    c.send_reissue(&mut Link::new(&mut server), &password("fake password ten"))
        .expect("reissue");

    for (name, text) in [
        ("password 1", PASSWORD),
        ("password 2", NEW_PASSWORD),
        ("password 3", "fake password ten"),
        ("words 1", &first_words),
        ("words 2", &second_words),
        ("words 3", &third_words),
        ("entry text", "fake entry number 7"),
    ] {
        secrets.push(Secret::new(name, text.as_bytes()));
    }
    assert!(secrets.len() >= 20, "{} secrets scanned for", secrets.len());
    assert!(log.len() >= 15, "the run logged its events: {}", log.len());
    let leaks = find_leaks(&log.text(), &secrets);
    assert!(leaks.is_empty(), "secrets in the log: {leaks:?}");
    // The login name is personal data too: not in the logs either.
    assert!(!log.text().contains("fake.user"));
}

#[test]
fn the_server_holds_hashes_and_no_secret() {
    let (mut server, log) = world();
    let mut a = device(&log);
    a.begin_registration(LOGIN, &password(PASSWORD))
        .expect("begin");
    let Some(Pending::Registration(pending)) = a.store.pending.clone() else {
        panic!("pending registration");
    };
    a.confirm_saved().expect("save");
    a.send_registration(&mut Link::new(&mut server))
        .expect("register");
    let view = server
        .stored_view(&pending.package.account_id)
        .expect("stored");
    let (auth_hash, rc_hash) = (view.auth_hash, view.rc_hash);
    assert_eq!(view.salt, pending.package.salt);
    assert_eq!(view.profile, 1);
    assert_ne!(
        &auth_hash,
        pending.package.auth_key.as_bytes(),
        "a hash, not the key"
    );
    assert_ne!(&rc_hash, pending.package.rc_auth_key.as_bytes());
    assert_eq!(
        auth_hash,
        kinige_crypto::ids::auth_hash(pending.package.auth_key.as_bytes()).expect("hash")
    );
}
