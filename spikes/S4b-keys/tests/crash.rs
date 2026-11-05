//! Crashes and lost answers between the steps of registration, recovery and reissue (appendix §B item 13, §A.5).
#![cfg(test)]

mod common;

use common::*;
use kinige_crypto::words::Words;
use s4b_keys::client::{ClientError, Pending, Resumed};
use s4b_keys::link::{Fault, Link};

#[test]
fn nothing_goes_out_before_save_is_confirmed() {
    let (mut server, log) = world();
    let mut client = device(&log);
    client
        .begin_registration(LOGIN, &password(PASSWORD))
        .expect("begin");
    assert!(client.shown_words().is_some());
    let mut link = Link::new(&mut server);
    assert_eq!(
        client.send_registration(&mut link),
        Err(ClientError::SaveNotConfirmed)
    );
    assert_eq!(link.sent(), 0, "no request, not even one that would fail");
    assert!(
        server.stored_view(&client_package_id(&client)).is_none(),
        "no account on the server"
    );

    // The same gate holds for a recovery and for new words.
    let (mut other, other_log) = world();
    let (mut a, words) = register(&mut other, &other_log, LOGIN, PASSWORD);
    let mut recovering = device(&other_log);
    recovering
        .begin_recovery(
            &mut Link::new(&mut other),
            LOGIN,
            &words,
            &password(NEW_PASSWORD),
        )
        .expect("begin");
    let mut link = Link::new(&mut other);
    assert_eq!(
        recovering.send_recovery(&mut link),
        Err(ClientError::SaveNotConfirmed)
    );
    assert_eq!(link.sent(), 0);
    a.begin_reissue().expect("begin");
    let mut link = Link::new(&mut other);
    assert_eq!(
        a.send_reissue(&mut link, &password(PASSWORD)),
        Err(ClientError::SaveNotConfirmed)
    );
    assert_eq!(link.sent(), 0);
}

#[test]
fn a_cancelled_save_does_not_count() {
    let (mut server, log) = world();
    let mut client = device(&log);
    client
        .begin_registration(LOGIN, &password(PASSWORD))
        .expect("begin");
    // The user opened the dialog and cancelled: `confirm_saved` is not called, however often the button is tried.
    for _ in 0..3 {
        assert_eq!(
            client.send_registration(&mut Link::new(&mut server)),
            Err(ClientError::SaveNotConfirmed)
        );
    }
    client.confirm_saved().expect("a successful write");
    client
        .send_registration(&mut Link::new(&mut server))
        .expect("now it goes out");
}

#[test]
fn a_crash_between_showing_the_new_words_and_the_request_changes_nothing_on_the_server() {
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
    let new_words = c.shown_words().expect("shown").phrase().to_string();
    // The process dies here: the words were on the screen, the user never pressed "Save".
    let c = c.restart();
    assert!(
        c.shown_words().is_none(),
        "the words are not in memory any more"
    );
    assert!(
        matches!(c.store.pending, Some(Pending::Recovery(_))),
        "the package survived in the store"
    );

    let mut d = device(&log);
    d.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &old_words,
        &password("fake password six"),
    )
    .expect("the old words still work");
    let mut e = device(&log);
    login(&mut server, &mut e, LOGIN, PASSWORD).expect("and so does the old password");
    let mut f = device(&log);
    assert_eq!(
        f.begin_recovery(
            &mut Link::new(&mut server),
            LOGIN,
            &new_words,
            &password("fake password six")
        ),
        Err(ClientError::Auth),
        "the new words mean nothing yet"
    );
}

#[test]
fn a_lost_answer_to_registration_and_a_restart_give_one_account_and_the_words_of_the_pdf_work() {
    let (mut server, log) = world();
    let mut client = device(&log);
    client
        .begin_registration(LOGIN, &password(PASSWORD))
        .expect("begin");
    let pdf_words = client.shown_words().expect("shown").phrase().to_string();
    client.confirm_saved().expect("save");
    let id = client_package_id(&client);

    let mut link = Link::with_faults(&mut server, [Some(Fault::LoseReply)]);
    assert_eq!(client.send_registration(&mut link), Err(ClientError::Lost));
    assert!(server.stored_view(&id).is_some(), "the server did register");

    // The client restarts and does NOT build new keys and words: it repeats the saved package.
    let mut client = client.restart();
    client
        .send_registration(&mut Link::new(&mut server))
        .expect("the repeat is a success, not 'login taken'");
    assert_eq!(client_account_id(&client), id, "the same account id");
    assert!(server.stored_view(&id).is_some());

    let mut recovering = device(&log);
    recovering
        .begin_recovery(
            &mut Link::new(&mut server),
            LOGIN,
            &pdf_words,
            &password(NEW_PASSWORD),
        )
        .expect("the words from the PDF work");
}

#[test]
fn a_lost_request_to_registration_is_repeated_with_the_same_package() {
    let (mut server, log) = world();
    let mut client = device(&log);
    client
        .begin_registration(LOGIN, &password(PASSWORD))
        .expect("begin");
    client.confirm_saved().expect("save");
    let mut link = Link::with_faults(&mut server, [Some(Fault::LoseRequest)]);
    assert_eq!(client.send_registration(&mut link), Err(ClientError::Lost));
    assert!(server.stored_view(&client_package_id(&client)).is_none());
    client
        .send_registration(&mut Link::new(&mut server))
        .expect("repeat");
}

#[test]
fn a_taken_login_keeps_the_account_id_the_keys_and_the_words() {
    let (mut server, log) = world();
    let _taken = register(&mut server, &log, LOGIN, PASSWORD);
    let mut client = device(&log);
    client
        .begin_registration(LOGIN, &password("fake password seven"))
        .expect("begin");
    let words = client.shown_words().expect("shown").phrase().to_string();
    let id = client_package_id(&client);
    client.confirm_saved().expect("save");
    assert_eq!(
        client.send_registration(&mut Link::new(&mut server)),
        Err(ClientError::LoginTaken)
    );

    client
        .change_pending_login("another.user@example.test")
        .expect("another login");
    client
        .send_registration(&mut Link::new(&mut server))
        .expect("registered");
    assert!(client.shown_words().is_none(), "the words screen is over");
    assert_eq!(
        client_account_id(&client),
        id,
        "the account id did not change"
    );

    let mut recovering = device(&log);
    recovering
        .begin_recovery(
            &mut Link::new(&mut server),
            "another.user@example.test",
            &words,
            &password(NEW_PASSWORD),
        )
        .expect("the same words work");
}

#[test]
fn an_account_id_taken_by_another_package_gets_a_new_id_and_the_same_words() {
    let (mut server, log) = world();
    let (first, _words) = register(&mut server, &log, LOGIN, PASSWORD);
    let mut client = device(&log);
    client
        .begin_registration("second.user@example.test", &password("fake password eight"))
        .expect("begin");
    let words = client.shown_words().expect("shown").phrase().to_string();
    client.confirm_saved().expect("save");
    // Force the (2^-122) collision: the package is for an account id the server already has.
    if let Some(Pending::Registration(pending)) = &mut client.store.pending {
        pending.package.account_id = account_id(&first);
    }
    assert_eq!(
        client.send_registration(&mut Link::new(&mut server)),
        Err(ClientError::AccountIdTaken)
    );

    client
        .rebuild_after_account_id_taken(&password("fake password eight"), &words)
        .expect("rebuilt");
    client
        .send_registration(&mut Link::new(&mut server))
        .expect("registered");
    assert_ne!(client_account_id(&client), account_id(&first));
    let mut recovering = device(&log);
    recovering
        .begin_recovery(
            &mut Link::new(&mut server),
            "second.user@example.test",
            &words,
            &password(NEW_PASSWORD),
        )
        .expect("the words did not change");
    write_envelope(&client, &mut server, 1);
    assert_eq!(read_all(&client, &server).len(), 1);
}

#[test]
fn a_lost_answer_to_the_replacement_is_repeated_and_the_words_from_the_pdf_work() {
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
    let pdf_words = c.shown_words().expect("shown").phrase().to_string();
    c.confirm_saved().expect("save");

    let mut link = Link::with_faults(&mut server, [Some(Fault::LoseReply)]);
    assert_eq!(c.send_recovery(&mut link), Err(ClientError::Lost));
    // The same process still has the old rc_auth_key: the identical package is sent again, and that is a success.
    c.send_recovery(&mut Link::new(&mut server))
        .expect("idempotent by the new rc_auth_key");

    let mut d = device(&log);
    login(&mut server, &mut d, LOGIN, NEW_PASSWORD)
        .expect("one replacement, the new password works");
    let mut e = device(&log);
    e.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &pdf_words,
        &password("fake password nine"),
    )
    .expect("the words from the PDF work");
    let mut f = device(&log);
    assert_eq!(
        f.begin_recovery(
            &mut Link::new(&mut server),
            LOGIN,
            &old_words,
            &password("fake password nine")
        ),
        Err(ClientError::Auth)
    );
}

#[test]
fn after_a_restart_an_uncertain_recovery_is_settled_by_logging_in_with_the_new_password() {
    let (mut server, log) = world();
    let (_a, old_words) = register(&mut server, &log, LOGIN, PASSWORD);

    // The server did the replacement and the answer was lost; the client restarted and lost the old rc_auth_key.
    let mut c = device(&log);
    c.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &old_words,
        &password(NEW_PASSWORD),
    )
    .expect("begin");
    c.confirm_saved().expect("save");
    let mut link = Link::with_faults(&mut server, [Some(Fault::LoseReply)]);
    assert_eq!(c.send_recovery(&mut link), Err(ClientError::Lost));
    let mut c = c.restart();
    assert_eq!(
        c.send_recovery(&mut Link::new(&mut server)),
        Err(ClientError::Auth),
        "the old key is gone, so it cannot just repeat"
    );
    assert_eq!(
        c.resume_recovery_after_restart(&mut Link::new(&mut server), &password(NEW_PASSWORD)),
        Ok(Resumed::Applied)
    );
    assert!(c.account().is_some() && c.store.pending.is_none());

    // The request itself was lost: the server never changed, the new password does not log in, the old words are needed again.
    let (mut server2, log2) = world();
    let (_b, old_words2) = register(&mut server2, &log2, LOGIN, PASSWORD);
    let mut d = device(&log2);
    d.begin_recovery(
        &mut Link::new(&mut server2),
        LOGIN,
        &old_words2,
        &password(NEW_PASSWORD),
    )
    .expect("begin");
    d.confirm_saved().expect("save");
    let mut link = Link::with_faults(&mut server2, [Some(Fault::LoseRequest)]);
    assert_eq!(d.send_recovery(&mut link), Err(ClientError::Lost));
    let mut d = d.restart();
    assert_eq!(
        d.resume_recovery_after_restart(&mut Link::new(&mut server2), &password(NEW_PASSWORD)),
        Ok(Resumed::NotApplied)
    );
    let mut again = device(&log2);
    again
        .begin_recovery(
            &mut Link::new(&mut server2),
            LOGIN,
            &old_words2,
            &password(NEW_PASSWORD),
        )
        .expect("the old words still work");
}

#[test]
fn a_lost_answer_to_reissuing_the_words_is_repeated_safely() {
    let (mut server, log) = world();
    let (mut a, old_words) = register(&mut server, &log, LOGIN, PASSWORD);
    a.begin_reissue().expect("begin");
    let pdf_words = a.shown_words().expect("shown").phrase().to_string();
    a.confirm_saved().expect("save");
    let mut link = Link::with_faults(&mut server, [None, Some(Fault::LoseReply)]);
    assert_eq!(
        a.send_reissue(&mut link, &password(PASSWORD)),
        Err(ClientError::Lost)
    );
    a.send_reissue(&mut Link::new(&mut server), &password(PASSWORD))
        .expect("the same package again");

    let mut b = device(&log);
    b.begin_recovery(
        &mut Link::new(&mut server),
        LOGIN,
        &pdf_words,
        &password(NEW_PASSWORD),
    )
    .expect("new words work");
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
    assert!(Words::parse(&pdf_words).is_ok());
}

fn client_package_id(client: &s4b_keys::client::Client) -> kinige_crypto::AccountId {
    match &client.store.pending {
        Some(Pending::Registration(pending)) => pending.package.account_id,
        _ => client_account_id(client),
    }
}

fn client_account_id(client: &s4b_keys::client::Client) -> kinige_crypto::AccountId {
    client.account().map_or_else(
        || kinige_crypto::AccountId::from_bytes([0; 16]),
        |account| account.account_id,
    )
}
