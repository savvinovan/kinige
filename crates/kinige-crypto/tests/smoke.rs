//! The binding gate of S4b (appendix §B item 11): the libsodium binding of ADR-0001 §11 builds on macOS, Windows and Linux
//! and every primitive the envelope scheme uses works. Runs in the `rust`, `macos` and `windows` CI jobs.
#![cfg(test)]

use libsodium_rs::crypto_aead::xchacha20poly1305 as aead;
use libsodium_rs::crypto_generichash::generichash;
use libsodium_rs::crypto_kdf;
use libsodium_rs::crypto_pwhash::argon2id;
use libsodium_rs::crypto_secretstream::xchacha20poly1305 as stream;
use libsodium_rs::{ensure_init, random, utils, version};

#[test]
fn the_binding_links_libsodium_1_0_22() {
    ensure_init().expect("init");
    assert_eq!(version::version_string(), "1.0.22");
    assert!(
        !version::library_minimal(),
        "the full library is linked, not the minimal build"
    );
}

#[test]
fn every_primitive_of_adr_0001_section_11_round_trips() {
    ensure_init().expect("init");

    let key = aead::Key::generate();
    let nonce = aead::Nonce::generate();
    let sealed = aead::encrypt(b"hello", Some(b"ad"), &nonce, &key).expect("seal");
    assert_eq!(sealed.len(), 5 + aead::ABYTES);
    assert_eq!(
        aead::decrypt(&sealed, Some(b"ad"), &nonce, &key).expect("open"),
        b"hello"
    );
    assert!(aead::decrypt(&sealed, Some(b"AD"), &nonce, &key).is_err());

    let padded = utils::pad(b"abc", 1024).expect("pad");
    assert_eq!(padded.len(), 1024);
    assert_eq!(utils::unpad(&padded, 1024).expect("unpad"), b"abc");

    let master = crypto_kdf::Key::from_slice(&[7u8; 32]).expect("kdf key");
    let sub1 = crypto_kdf::derive_from_key(32, 1, b"kinigePW", &master).expect("sub1");
    let sub2 = crypto_kdf::derive_from_key(32, 2, b"kinigePW", &master).expect("sub2");
    assert_ne!(sub1, sub2);

    assert_eq!(generichash(b"x", None, 32).expect("hash").len(), 32);
    assert!(utils::memcmp(&sub1, &sub1));
    assert!(!utils::memcmp(&sub1, &sub2));
    let mut buf = [1u8; 8];
    utils::memzero(&mut buf);
    assert_eq!(buf, [0u8; 8]);
    assert_eq!(random::bytes(24).len(), 24);

    let skey = stream::Key::generate();
    let (mut push, header) = stream::PushState::init_push(&skey).expect("init_push");
    let c = push.push(b"data", None, stream::TAG_FINAL).expect("push");
    let mut pull = stream::PullState::init_pull(&header, &skey).expect("init_pull");
    assert_eq!(
        pull.pull(&c, None).expect("pull"),
        (b"data".to_vec(), stream::TAG_FINAL)
    );

    // The smallest Argon2id the library accepts: this checks that the function links and runs, the profiles are measured later.
    let out = argon2id::pwhash(
        32,
        b"fake",
        &[0u8; argon2id::SALTBYTES],
        argon2id::OPSLIMIT_MIN,
        argon2id::MEMLIMIT_MIN,
    )
    .expect("pwhash");
    assert_eq!(out.len(), 32);
}
