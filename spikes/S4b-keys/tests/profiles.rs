//! The Argon2id profile rule against a server that sends the wrong thing (ADR-0001 §8.2; appendix §B items 7 and 14, S4a items 28 and 29):
//! unknown or lower than seen is refused before anything is derived, and a new password never gets a lower profile.
#![cfg(test)]

mod common;

use common::*;
use kinige_crypto::{CryptoError, Profile, ProfileError};
use s4b_keys::client::{Client, ClientError, Resumed};
use s4b_keys::link::{Fault, Link};

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
fn an_uncertain_recovery_on_a_device_that_saw_profile_2_is_not_applied_when_the_server_still_sends_1()
 {
    // The request of the recovery was lost, so the server is unchanged and sends profile 1 to a device that has seen 2. The login that
    // settles the question is refused by the profile rule, and that means "not applied": enter the old words again (§A.5).
    let (mut server, log) = world();
    let (_a, words) = register(&mut server, &log, LOGIN, PASSWORD);
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
    let mut link = Link::with_faults(&mut server, [Some(Fault::LoseRequest)]);
    assert_eq!(x.send_recovery(&mut link), Err(ClientError::Lost));

    let mut x = x.restart();
    let runs = x.derive_runs();
    let settled =
        x.resume_recovery_after_restart(&mut Link::new(&mut server), &password(NEW_PASSWORD));
    assert_eq!(settled, Ok(Resumed::NotApplied));
    assert_eq!(x.derive_runs(), runs, "refused before deriving");
    assert!(x.store.pending.is_none(), "the unsent package is dropped");

    // The old words still work, and now give the new password profile 2.
    x.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &words,
        &password(NEW_PASSWORD),
    )
    .expect("the old words still work");
}
