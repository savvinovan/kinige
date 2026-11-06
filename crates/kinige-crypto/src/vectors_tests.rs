//! The test vectors made by libsodium.js (appendix §B items 1 and 12). `vectors/vectors.json` and `vectors/blobs/*.bin` are produced
//! by `spikes/S4b-keys/vectors/gen.mjs` (see the README of the spike); every input is fixed and obviously fake. Rust must reproduce
//! each byte, and must open what libsodium.js sealed.

use crate::envelope::{ad_env, seal_with_nonce};
use crate::header::{Header, build_payload};
use crate::ids::{AccountId, auth_hash, conflict_revision_id, unknown_login_salt};
use crate::keys::{self, Dek, SALT_LEN, subkey};
use crate::operation::open_operation;
use crate::profile::Profile;
use crate::secret::{DekKey, KekPw, KekRc, Password, RevKey, SecretBytes, ServerSecret};
use crate::words::Words;
use crate::wrap::{self, Kind, ad_wrap};
use kinige_domain::jcs;
use kinige_domain::sync::{Group, conflict_revision};
use libsodium_rs::crypto_generichash::generichash;
use libsodium_rs::{utils, version};
use serde_json::{Value, json};

const VECTORS: &str = include_str!("../vectors/vectors.json");
const ACCOUNT: &str = "11111111-1111-4111-8111-111111111111";

/// The string of `gen.mjs` (`NASTY`), spelled with Rust escapes: non-ASCII, a surrogate pair, quotes, a backslash, controls, DEL,
/// U+2028 and `<>&/`.
const NASTY: &str = "Privet \u{43f}\u{440}\u{438}\u{432}\u{435}\u{442} \u{1f642} \"q\" \\ \n\t\r\u{8}\u{c} \u{1}\u{1f} \u{7f} \u{2028} <>&/ caf\u{e9}";

#[test]
fn the_vectors_come_from_the_same_libsodium_version_as_the_binding() {
    assert_eq!(
        vectors()["libsodium_js"]["version"].as_str(),
        Some(version::version_string())
    );
}

#[test]
fn argon2id_and_every_subkey_match_libsodium_js() {
    let keys = &vectors()["keys"];
    let password: Password = SecretBytes::new(text(keys, "password").into_bytes());
    let salt = array16(&bin(keys, "salt"));

    let root1 = keys::derive_pw_root(&password, &salt, Profile::P1).expect("profile 1");
    assert_eq!(root1.as_bytes().as_slice(), bin(keys, "pw_root_profile1"));
    let root2 = keys::derive_pw_root(&password, &salt, Profile::P2).expect("profile 2");
    assert_eq!(root2.as_bytes().as_slice(), bin(keys, "pw_root_profile2"));

    assert_eq!(
        keys::pw_auth_key(&root1)
            .expect("auth")
            .as_bytes()
            .as_slice(),
        bin(keys, "auth_key")
    );
    assert_eq!(
        keys::pw_kek(&root1).expect("kek").as_bytes().as_slice(),
        bin(keys, "kek_pw")
    );
    let rc_root = Words::parse(&words_phrase(0))
        .expect("words")
        .rc_root()
        .expect("root");
    assert_eq!(rc_root.as_bytes().as_slice(), bin(keys, "rc_root"));
    assert_eq!(
        keys::rc_auth_key(&rc_root)
            .expect("rc auth")
            .as_bytes()
            .as_slice(),
        bin(keys, "rc_auth_key")
    );
    assert_eq!(
        keys::rc_kek(&rc_root)
            .expect("rc kek")
            .as_bytes()
            .as_slice(),
        bin(keys, "kek_rc")
    );

    let dek = dek();
    assert_eq!(dek.env_key().as_bytes().as_slice(), bin(keys, "k_env"));
    assert_eq!(dek.rev_key().as_bytes().as_slice(), bin(keys, "k_rev"));
    let reserved = subkey(dek.key().as_bytes(), 2, b"kinigeDK").expect("subkey 2");
    assert_eq!(reserved.as_slice(), bin(keys, "k_reserved"));

    let auth = array32(&bin(keys, "auth_key"));
    assert_eq!(
        auth_hash(&auth).expect("hash").as_slice(),
        bin(keys, "auth_hash")
    );
    let secret = ServerSecret::from_slice(&bin(keys, "server_secret")).expect("secret");
    assert_eq!(
        unknown_login_salt(&secret, &text(keys, "login"))
            .expect("salt")
            .as_slice(),
        bin(keys, "unknown_login_salt")
    );
}

#[test]
fn the_official_bip39_phrases_give_their_entropy() {
    for (index, entry) in vectors()["words"]
        .as_array()
        .expect("words")
        .iter()
        .enumerate()
    {
        let words = Words::parse(&words_phrase(index)).expect("a valid phrase");
        assert_eq!(
            words.rc_root().expect("root").as_bytes().as_slice(),
            bin(entry, "entropy"),
            "phrase {index}"
        );
    }
}

#[test]
fn the_wraps_are_byte_for_byte_those_of_libsodium_js_and_open() {
    let wraps = &vectors()["wraps"];
    let keys = &vectors()["keys"];
    let account = account();
    let salt = array16(&bin(keys, "salt"));
    let (kek_pw, kek_rc) = (
        KekPw::from_slice(&bin(keys, "kek_pw")).expect("kek"),
        KekRc::from_slice(&bin(keys, "kek_rc")).expect("kek"),
    );

    let ad_pw = ad_wrap(
        &account,
        1,
        Kind::Password {
            salt: &salt,
            profile: Profile::P1,
        },
    );
    let ad_rc = ad_wrap(&account, 1, Kind::Recovery);
    assert_eq!(ad_pw, bin(wraps, "ad_wrap_pw"));
    assert_eq!(ad_rc, bin(wraps, "ad_wrap_rc"));

    let sealed_pw = wrap::seal(
        dek().key(),
        kek_pw.as_bytes(),
        &ad_pw,
        array24(&bin(wraps, "nonce_pw")),
    )
    .expect("seal");
    let sealed_rc = wrap::seal(
        dek().key(),
        kek_rc.as_bytes(),
        &ad_rc,
        array24(&bin(wraps, "nonce_rc")),
    )
    .expect("seal");
    assert_eq!(sealed_pw, bin(wraps, "wrap_pw"));
    assert_eq!(sealed_rc, bin(wraps, "wrap_rc"));

    let from_js = wrap::unwrap_pw(
        &bin(wraps, "wrap_pw"),
        &kek_pw,
        &account,
        1,
        &salt,
        Profile::P1,
    )
    .expect("open");
    assert!(from_js.key().ct_eq(dek().key()));
    let from_js = wrap::unwrap_rc(&bin(wraps, "wrap_rc"), &kek_rc, &account, 1).expect("open");
    assert!(from_js.key().ct_eq(dek().key()));
}

#[test]
fn padding_matches_libsodium_js() {
    for entry in vectors()["padding"].as_array().expect("padding") {
        let length = entry["input_length"].as_u64().expect("length") as usize;
        let input: Vec<u8> = (0..length).map(|i| ((i * 7 + 3) % 256) as u8).collect();
        let padded = utils::pad(&input, 1024).expect("pad");
        assert_eq!(
            padded.len() as u64,
            entry["padded_length"].as_u64().expect("padded length")
        );
        assert_eq!(
            generichash(&padded, None, 32).expect("hash"),
            bin(entry, "padded_blake2b256"),
            "length {length}"
        );
        assert_eq!(utils::unpad(&padded, 1024).expect("unpad"), input);
    }
}

#[test]
fn the_envelopes_are_byte_for_byte_those_of_libsodium_js_and_open() {
    let envelopes = &vectors()["envelopes"];
    for (name, groups) in [("upsert", true), ("purged", false)] {
        let entry = &envelopes[name];
        let header = text(entry, "header");
        let parsed = Header::parse(header.as_bytes()).expect("header");
        assert_eq!(
            parsed.to_bytes().expect("to bytes"),
            header.as_bytes(),
            "{name}: the header serializes to the same text"
        );

        let group_texts = if groups {
            [
                ("body".to_string(), text(entry, "body_group")),
                ("core".to_string(), text(entry, "core_group")),
            ]
            .into_iter()
            .collect()
        } else {
            Default::default()
        };
        let payload = build_payload(&parsed, &group_texts).expect("payload");
        assert_eq!(
            payload,
            text(entry, "payload").as_bytes(),
            "{name}: the payload text"
        );

        let open_bytes_len = 9 + header.len();
        let expected = bin(entry, "envelope");
        assert_eq!(
            ad_env(&account(), &expected[..open_bytes_len]),
            bin(entry, "ad_env"),
            "{name}: AD_env"
        );
        let sealed = seal_with_nonce(
            &account(),
            &dek(),
            header.as_bytes(),
            &payload,
            array24(&bin(entry, "nonce")),
        )
        .expect("seal");
        assert_eq!(sealed, expected, "{name}: the envelope");

        let opened =
            open_operation(&expected, &account(), &dek()).expect("open what libsodium.js sealed");
        assert_eq!(opened.header, parsed);
        assert_eq!(opened.groups, group_texts);
    }
}

#[test]
fn the_conflict_revision_id_and_content_match_libsodium_js() {
    let revision = &vectors()["revision"];
    let loser = Group::new(
        &text(revision, "loser_hlc"),
        revision["loser_sv"].as_u64().expect("sv") as u32,
        &text(revision, "loser_raw"),
    )
    .expect("group");
    let made = conflict_revision(&text(revision, "message_id"), "body", &loser).expect("revision");
    assert_eq!(made.id_input, bin(revision, "id_input"));
    let key = RevKey::from_slice(&bin(&vectors()["keys"], "k_rev")).expect("key");
    assert_eq!(
        conflict_revision_id(&key, &made).expect("id"),
        text(revision, "id")
    );
    assert_eq!(
        made.core.raw,
        text(revision, "core"),
        "the revision's core group, with snapshot_json inside"
    );
    let core: Value = serde_json::from_str(&made.core.raw).expect("core json");
    assert_eq!(
        core["snapshot_json"].as_str(),
        Some(text(revision, "snapshot_json").as_str())
    );
}

#[test]
fn canonical_json_is_the_same_bytes_as_libsodium_js_makes() {
    let all = vectors();
    let canonical = all["canonical"].as_array().expect("canonical");
    let body = json!({"base": null, "body_md": NASTY, "edited_at": null, "supersedes": []});
    assert_eq!(
        jcs::canonical_string(&body).expect("jcs"),
        text(&canonical[0], "canonical")
    );
    let core = json!({"created_at": 1762300000000u64, "created_tz": "Europe/Moscow", "origin": "user", "reply_to_id": null});
    assert_eq!(
        jcs::canonical_string(&core).expect("jcs"),
        text(&canonical[1], "canonical")
    );
    let order = json!({"\u{1f642}": 1, "\u{ff5e}": 2, "b": [true, false, null], "a": {"z": 0, "y": 9007199254740991u64}});
    assert_eq!(
        jcs::canonical_string(&order).expect("jcs"),
        text(&canonical[2], "canonical")
    );
}

pub(crate) fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("vectors.json is valid json")
}

fn account() -> AccountId {
    AccountId::parse(ACCOUNT).expect("account")
}

fn dek() -> Dek {
    let key = DekKey::from_slice(&bin(&vectors()["keys"], "dek")).expect("dek");
    Dek::new(1, key).expect("dek")
}

fn words_phrase(index: usize) -> String {
    text(&vectors()["words"][index], "phrase")
}

fn text(value: &Value, field: &str) -> String {
    value[field]
        .as_str()
        .unwrap_or_else(|| panic!("{field} is a string"))
        .to_string()
}

pub(crate) fn bin(value: &Value, field: &str) -> Vec<u8> {
    utils::hex2bin(&text(value, field)).expect("hex")
}

fn array16(bytes: &[u8]) -> [u8; SALT_LEN] {
    bytes.try_into().expect("16 bytes")
}

fn array24(bytes: &[u8]) -> [u8; 24] {
    bytes.try_into().expect("24 bytes")
}

fn array32(bytes: &[u8]) -> [u8; 32] {
    bytes.try_into().expect("32 bytes")
}
