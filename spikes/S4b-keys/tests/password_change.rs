//! A lost answer to a password change (ADR-0001 §8.5): the server may have applied it and the client never learned. A retry with the old
//! password cannot tell, so the client settles the question by logging in with the new password, as for a recovery (§A.5).
#![cfg(test)]

mod common;

use common::*;
use s4b_keys::client::{ClientError, Pending, Resumed};
use s4b_keys::link::{Fault, Link};

const OLD: &str = PASSWORD;
const NEW: &str = NEW_PASSWORD;

#[test]
fn a_lost_answer_after_the_server_applied_the_change_is_settled_by_the_new_password() {
    let (mut server, log) = world();
    let (mut a, _words) = register(&mut server, &log, LOGIN, OLD);
    write_envelope(&a, &mut server, 1);

    // The first call (the salt and profile) works, the change itself is applied and its answer is lost.
    let mut link = Link::with_faults(&mut server, [None, Some(Fault::LoseReply)]);
    let result = a.change_password(&mut link, &password(OLD), &password(NEW), false);
    assert_eq!(result, Err(ClientError::Lost));
    assert!(matches!(a.store.pending, Some(Pending::PasswordChange)));

    // A retry with the old password fails: the server already has the new salt, so the old key is wrong.
    let retry = a.change_password(
        &mut Link::new(&mut server),
        &password(OLD),
        &password(NEW),
        false,
    );
    assert_eq!(
        retry,
        Err(ClientError::Auth),
        "the old password is already wrong"
    );

    // The new password confirms the change.
    let settled = a.settle_password_change(&mut Link::new(&mut server), &password(NEW));
    assert_eq!(settled, Ok(Resumed::Applied));
    assert!(a.store.pending.is_none());
    assert_eq!(
        read_all(&a, &server).len(),
        1,
        "the session and the DEK are good"
    );

    let mut b = device(&log);
    login(&mut server, &mut b, LOGIN, NEW).expect("the new password works everywhere");
    let mut c = device(&log);
    assert_eq!(
        login(&mut server, &mut c, LOGIN, OLD),
        Err(ClientError::Auth)
    );
}

#[test]
fn a_lost_request_leaves_the_old_password_valid_and_the_change_can_be_sent_again() {
    let (mut server, log) = world();
    let (mut a, _words) = register(&mut server, &log, LOGIN, OLD);

    let mut link = Link::with_faults(&mut server, [None, Some(Fault::LoseRequest)]);
    let result = a.change_password(&mut link, &password(OLD), &password(NEW), false);
    assert_eq!(result, Err(ClientError::Lost));

    // The server never saw it: the new password does not log in, the old one does.
    let settled = a.settle_password_change(&mut Link::new(&mut server), &password(NEW));
    assert_eq!(settled, Ok(Resumed::NotApplied));
    assert!(a.store.pending.is_none());
    let mut b = device(&log);
    login(&mut server, &mut b, LOGIN, OLD).expect("the old password still works");

    a.change_password(
        &mut Link::new(&mut server),
        &password(OLD),
        &password(NEW),
        false,
    )
    .expect("sent again");
    let mut c = device(&log);
    login(&mut server, &mut c, LOGIN, NEW).expect("now the new one");
}

#[test]
fn settling_needs_a_pending_change() {
    let (mut server, log) = world();
    let (mut a, _words) = register(&mut server, &log, LOGIN, OLD);
    assert_eq!(
        a.settle_password_change(&mut Link::new(&mut server), &password(NEW)),
        Err(ClientError::NothingPending)
    );
}
