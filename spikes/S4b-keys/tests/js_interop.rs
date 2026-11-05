//! Rust to libsodium.js (appendix §B items 1 and 12, the direction the committed vectors do not cover): what the real flows seal is
//! opened by libsodium.js with its own code. Needs node and `npm ci` in `vectors/`; it runs when `S4B_NODE=1` (CI sets it) and
//! says so when it does not.
#![cfg(test)]

mod common;

use common::*;
use kinige_crypto::blob::seal_blob;
use kinige_crypto::keys::{self, derive_pw_root};
use kinige_crypto::secret::BlobKey;
use kinige_crypto::words::Words;
use kinige_crypto::{Profile, open_operation};
use s4b_keys::client::Pending;
use s4b_keys::link::Link;
use std::process::Command;

#[test]
fn libsodium_js_opens_what_the_real_flow_sealed() {
    if std::env::var("S4B_NODE").ok().as_deref() != Some("1") {
        eprintln!("S4B_NODE is not 1: the libsodium.js interop test is skipped (CI runs it)");
        return;
    }
    let (mut server, log) = world();
    let mut a = device(&log);
    a.begin_registration(LOGIN, &password(PASSWORD))
        .expect("begin");
    let words = a.shown_words().expect("words").phrase().to_string();
    let Some(Pending::Registration(pending)) = a.store.pending.clone() else {
        panic!("pending");
    };
    a.confirm_saved().expect("save");
    a.send_registration(&mut Link::new(&mut server))
        .expect("register");
    let envelope = write_envelope(&a, &mut server, 3);

    let root =
        derive_pw_root(&password(PASSWORD), &pending.package.salt, Profile::P1).expect("root");
    let rc_root = Words::parse(&words)
        .expect("words")
        .rc_root()
        .expect("rc root");
    let operation = open_operation(&envelope, &account_id(&a), &dek(&a)).expect("open");
    let header_len =
        u32::from_be_bytes([envelope[5], envelope[6], envelope[7], envelope[8]]) as usize;
    let blob_key = BlobKey::from_bytes([0x5a; 32]);
    let file: Vec<u8> = (0..1_048_577usize).map(|i| (i % 251) as u8).collect();
    let blob = seal_blob(&file, &blob_key).expect("blob");

    let dir = std::env::temp_dir().join(format!("kinige-s4b-interop-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    std::fs::write(dir.join("blob.bin"), &blob).expect("blob file");
    let hex = |bytes: &[u8]| libsodium_rs::utils::bin2hex(bytes);
    let input = serde_json::json!({
        "account_id_hex": hex(account_id(&a).as_bytes()),
        "key_id": 1,
        "profile": 1,
        "password": PASSWORD,
        "salt": hex(&pending.package.salt),
        "pw_root": hex(root.as_bytes()),
        "auth_key": hex(keys::pw_auth_key(&root).expect("auth").as_bytes()),
        "rc_root": hex(rc_root.as_bytes()),
        "rc_auth_key": hex(keys::rc_auth_key(&rc_root).expect("rc auth").as_bytes()),
        "dek": hex(dek(&a).key().as_bytes()),
        "wrap_pw": hex(&pending.package.wrap_pw),
        "wrap_rc": hex(&pending.package.wrap_rc),
        "envelope": hex(&envelope),
        "header": String::from_utf8(envelope[9..9 + header_len].to_vec()).expect("utf8 header"),
        "payload": format!(r#"{{"groups":{{"body":{}}}}}"#, operation.groups["body"]),
        "blob_key": hex(blob_key.as_bytes()),
        "blob_file_size": file.len(),
    });
    std::fs::write(dir.join("interop.json"), input.to_string()).expect("input file");

    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/verify.mjs");
    let output = Command::new("node")
        .arg(script)
        .arg(&dir)
        .output()
        .expect("node runs");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        output.status.success(),
        "libsodium.js refused what Rust sealed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
