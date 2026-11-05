//! Words, wraps, the blob and the revision id: the negative tests of appendix §B items 4, 6 (bytes), 9 and the redaction of
//! secrets. Inputs are synthetic.

use crate::blob::{self, CHUNK};
use crate::error::{BlobError, CryptoError, WordsError};
use crate::ids::{AccountId, conflict_revision_id};
use crate::keys::{self, Dek, SALT_LEN};
use crate::profile::Profile;
use crate::secret::{BlobKey, DekKey, KekPw, KekRc, Password, PwRoot, RevKey, SecretBytes};
use crate::words::Words;
use crate::wrap;
use kinige_domain::sync::{Group, conflict_revision};

const ACCOUNT: [u8; 16] = [0x11; 16];
const SALT: [u8; SALT_LEN] = [0x22; SALT_LEN];

#[test]
fn generated_words_are_24_and_give_the_same_root_every_time() {
    let words = Words::generate().expect("generate");
    assert_eq!(words.phrase().split(' ').count(), 24);
    let again = Words::parse(words.phrase()).expect("parse");
    assert_eq!(again.phrase(), words.phrase());
    assert!(
        words
            .rc_root()
            .expect("root")
            .ct_eq(&again.rc_root().expect("root"))
    );
    assert!(
        !words
            .rc_root()
            .expect("root")
            .ct_eq(&Words::generate().expect("other").rc_root().expect("root"))
    );
}

#[test]
fn input_is_normalized_before_parsing() {
    let words = Words::generate().expect("generate");
    let messy = format!(
        "  {}\n",
        words.phrase().to_uppercase().replace(' ', " \t  ")
    );
    assert_eq!(
        Words::parse(&messy).expect("parse").phrase(),
        words.phrase()
    );
}

#[test]
fn a_typo_is_unknown_word_and_a_swapped_dictionary_word_is_a_checksum_error_most_of_the_time() {
    let words = Words::generate().expect("generate");
    let mut list: Vec<&str> = words.phrase().split(' ').collect();
    let original = list[5];
    list[5] = "zzzzzz";
    assert_eq!(
        Words::parse(&list.join(" ")).unwrap_err(),
        CryptoError::Words(WordsError::UnknownWord(5)),
        "a position, not the word"
    );
    // Replace one word by each of 16 other dictionary words: a checksum catches all but about 1 in 256.
    list[5] = original;
    let others = [
        "abandon", "ability", "able", "about", "above", "absent", "absorb", "abstract", "absurd",
        "abuse", "access", "accident", "account", "accuse", "achieve", "acid",
    ];
    let (mut caught, mut tried) = (0, 0);
    for other in others.into_iter().filter(|other| *other != original) {
        list[5] = other;
        tried += 1;
        match Words::parse(&list.join(" ")) {
            Err(CryptoError::Words(WordsError::Checksum)) => caught += 1,
            Ok(_) => {}
            Err(other) => panic!("unexpected {other:?}"),
        }
    }
    assert!(caught >= tried - 1, "{caught} of {tried}");
}

#[test]
fn only_24_words_are_accepted() {
    // A valid 12-word BIP-39 phrase has 128 bits of entropy: it is not our key and must not become one.
    let twelve = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    assert_eq!(
        Words::parse(twelve).unwrap_err(),
        CryptoError::Words(WordsError::WordCount(12))
    );
    assert_eq!(
        Words::parse("").unwrap_err(),
        CryptoError::Words(WordsError::WordCount(0))
    );
    let known_24 = format!("{}art", "abandon ".repeat(23));
    assert!(
        Words::parse(&known_24).is_ok(),
        "the all-zero entropy phrase is valid"
    );
    assert_eq!(
        Words::parse(&known_24)
            .expect("parse")
            .rc_root()
            .expect("root")
            .as_bytes(),
        &[0u8; 32]
    );
}

#[test]
fn words_print_as_redacted() {
    let words = Words::generate().expect("generate");
    assert_eq!(format!("{words:?}"), "Words(<redacted>)");
}

#[test]
fn the_password_and_the_words_branches_are_separate_keys() {
    let root = PwRoot::from_bytes([0x33; 32]);
    let auth = keys::pw_auth_key(&root).expect("auth");
    let kek = keys::pw_kek(&root).expect("kek");
    assert_ne!(auth.as_bytes(), kek.as_bytes());
    assert_ne!(auth.as_bytes(), root.as_bytes());
    let rc = crate::secret::RcRoot::from_bytes([0x33; 32]);
    assert_ne!(
        keys::rc_auth_key(&rc).expect("rc auth").as_bytes(),
        auth.as_bytes(),
        "another context"
    );
}

#[test]
fn a_wrap_opens_only_with_its_kek_account_key_id_salt_and_profile() {
    let account = AccountId::from_bytes(ACCOUNT);
    let dek = Dek::new(1, DekKey::from_bytes([0x44; 32])).expect("dek");
    let kek = KekPw::from_bytes([0x55; 32]);
    let wrapped = wrap::wrap_pw(&dek, &kek, &account, &SALT, Profile::P1).expect("wrap");
    assert_eq!(wrapped.len(), 73);
    let opened = wrap::unwrap_pw(&wrapped, &kek, &account, 1, &SALT, Profile::P1).expect("unwrap");
    assert!(opened.key().ct_eq(dek.key()));
    assert_eq!(opened.key_id(), 1);

    let other_salt = [0x23; SALT_LEN];
    let other_account = AccountId::from_bytes([0x12; 16]);
    let refusals = [
        (
            "wrong kek",
            wrap::unwrap_pw(
                &wrapped,
                &KekPw::from_bytes([0x56; 32]),
                &account,
                1,
                &SALT,
                Profile::P1,
            ),
        ),
        (
            "substituted salt",
            wrap::unwrap_pw(&wrapped, &kek, &account, 1, &other_salt, Profile::P1),
        ),
        (
            "substituted profile",
            wrap::unwrap_pw(&wrapped, &kek, &account, 1, &SALT, Profile::P2),
        ),
        (
            "another account",
            wrap::unwrap_pw(&wrapped, &kek, &other_account, 1, &SALT, Profile::P1),
        ),
        (
            "another key id",
            wrap::unwrap_pw(&wrapped, &kek, &account, 2, &SALT, Profile::P1),
        ),
    ];
    for (what, result) in refusals {
        assert_eq!(result.unwrap_err(), CryptoError::Authentication, "{what}");
    }
    for position in 0..wrapped.len() {
        let mut changed = wrapped.clone();
        changed[position] ^= 1;
        assert!(
            wrap::unwrap_pw(&changed, &kek, &account, 1, &SALT, Profile::P1).is_err(),
            "byte {position}"
        );
    }
    assert_eq!(
        wrap::unwrap_pw(&wrapped[..72], &kek, &account, 1, &SALT, Profile::P1).unwrap_err(),
        CryptoError::CiphertextLength
    );
    // A future wrap version may have another length: it is "unknown version", not "wrong length".
    for length in [1, 10, 73, 200] {
        let mut future = vec![0u8; length];
        future[0] = 2;
        assert_eq!(
            wrap::unwrap_pw(&future, &kek, &account, 1, &SALT, Profile::P1).unwrap_err(),
            CryptoError::UnknownWrapVersion(2),
            "length {length}"
        );
    }
    assert_eq!(
        wrap::unwrap_pw(&[], &kek, &account, 1, &SALT, Profile::P1).unwrap_err(),
        CryptoError::Truncated
    );
}

#[test]
fn the_password_wrap_and_the_recovery_wrap_are_not_interchangeable() {
    let account = AccountId::from_bytes(ACCOUNT);
    let dek = Dek::new(1, DekKey::from_bytes([0x44; 32])).expect("dek");
    let same_bytes = [0x66; 32];
    let wrapped = wrap::wrap_rc(&dek, &KekRc::from_bytes(same_bytes), &account).expect("wrap");
    assert!(
        wrap::unwrap_pw(
            &wrapped,
            &KekPw::from_bytes(same_bytes),
            &account,
            1,
            &SALT,
            Profile::P1
        )
        .is_err()
    );
    assert!(wrap::unwrap_rc(&wrapped, &KekRc::from_bytes(same_bytes), &account, 1).is_ok());
}

#[test]
fn argon2id_profile_1_gives_a_stable_root_and_another_password_another_root() {
    let password: Password = SecretBytes::new(b"fake password for tests".to_vec());
    let one = keys::derive_pw_root(&password, &SALT, Profile::P1).expect("derive");
    let again = keys::derive_pw_root(&password, &SALT, Profile::P1).expect("derive");
    assert!(one.ct_eq(&again));
    let other = keys::derive_pw_root(
        &SecretBytes::new(b"another fake password".to_vec()),
        &SALT,
        Profile::P1,
    )
    .expect("derive");
    assert!(!one.ct_eq(&other));
}

#[test]
fn a_blob_round_trips_at_the_chunk_boundaries() {
    let key = BlobKey::from_bytes([0x77; 32]);
    for size in [
        0,
        1,
        1023,
        1024,
        CHUNK - 1025,
        CHUNK - 1024 - 1,
        CHUNK - 1,
        CHUNK,
        CHUNK + 1,
        2 * CHUNK + 5,
    ] {
        let file: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let sealed = blob::seal_blob(&file, &key).expect("seal");
        assert_eq!(
            blob::open_blob(&sealed, &key).expect("open"),
            file,
            "size {size}"
        );
    }
}

#[test]
fn a_truncated_blob_or_one_without_final_or_with_trailing_bytes_is_an_error() {
    let key = BlobKey::from_bytes([0x77; 32]);
    let file: Vec<u8> = (0..CHUNK + 100).map(|i| (i % 251) as u8).collect();
    let sealed = blob::seal_blob(&file, &key).expect("seal");
    let first_frame = 24 + CHUNK + 17;
    // Cut exactly after the first chunk: the stream ends before TAG_FINAL.
    assert_eq!(
        blob::open_blob(&sealed[..first_frame], &key).unwrap_err(),
        CryptoError::Blob(BlobError::MissingFinal)
    );
    assert_eq!(
        blob::open_blob(&sealed[..24], &key).unwrap_err(),
        CryptoError::Blob(BlobError::MissingFinal)
    );
    assert_eq!(
        blob::open_blob(&sealed[..23], &key).unwrap_err(),
        CryptoError::Blob(BlobError::Truncated)
    );
    for cut in [
        sealed.len() - 1,
        sealed.len() - 17,
        first_frame + 1,
        first_frame + 16,
        25,
        100,
    ] {
        assert!(
            blob::open_blob(&sealed[..cut], &key).is_err(),
            "cut at {cut}"
        );
    }
    let mut trailing = sealed.clone();
    trailing.push(0);
    assert_eq!(
        blob::open_blob(&trailing, &key).unwrap_err(),
        CryptoError::Blob(BlobError::Chunk)
    );
    // A whole valid second stream after the final chunk is also "bytes after TAG_FINAL".
    let second = blob::seal_blob(b"x", &key).expect("seal");
    let mut doubled = sealed.clone();
    doubled.extend_from_slice(&second[24..]);
    assert!(blob::open_blob(&doubled, &key).is_err());
    // Chunks swapped or a chunk repeated: the stream is ordered.
    let mut repeated = sealed[..first_frame].to_vec();
    repeated.extend_from_slice(&sealed[24..first_frame]);
    assert!(blob::open_blob(&repeated, &key).is_err());
    assert!(blob::open_blob(&sealed, &BlobKey::from_bytes([0x78; 32])).is_err());
}

#[test]
fn bytes_after_the_final_chunk_are_an_error_whatever_the_size_of_that_chunk() {
    let key = BlobKey::from_bytes([0x77; 32]);
    // A final chunk of exactly 1 MiB + 17 bytes (a file of 1 MiB - 1 pads to exactly 1 MiB): the extra bytes are seen as such.
    let file = vec![5u8; CHUNK - 1];
    let mut sealed = blob::seal_blob(&file, &key).expect("seal");
    assert_eq!(sealed.len(), 24 + CHUNK + 17);
    assert!(blob::open_blob(&sealed, &key).is_ok());
    sealed.extend_from_slice(&[0, 1, 2]);
    assert_eq!(
        blob::open_blob(&sealed, &key).unwrap_err(),
        CryptoError::Blob(BlobError::TrailingBytes)
    );
    // A shorter final chunk is read as one chunk with everything that follows, which does not authenticate. Still an error.
    let one = blob::seal_blob(b"first", &key).expect("seal");
    let mut glued = one.clone();
    glued.extend_from_slice(&one[24..]);
    assert_eq!(
        blob::open_blob(&glued, &key).unwrap_err(),
        CryptoError::Blob(BlobError::Chunk)
    );
}

#[test]
fn the_revision_id_is_keyed_deterministic_and_a_valid_v4_uuid() {
    let message = "5b1c0d9e-3f2a-4a7b-9c8d-1e2f3a4b5c6d";
    let hlc = format!(
        "0000000001000-00000-{}",
        "66666666-6666-4666-8666-666666666666"
    );
    let loser = Group::new(
        &hlc,
        1,
        r#"{"base":null,"body_md":"fake","edited_at":null,"supersedes":[]}"#,
    )
    .expect("group");
    let revision = conflict_revision(message, "body", &loser).expect("revision");
    let key = RevKey::from_bytes([0x88; 32]);
    let id = conflict_revision_id(&key, &revision).expect("id");
    assert_eq!(id, conflict_revision_id(&key, &revision).expect("again"));
    assert!(kinige_domain::hlc::is_uuid_v4(&id), "{id}");
    assert_ne!(
        id,
        conflict_revision_id(&RevKey::from_bytes([0x89; 32]), &revision).expect("other key")
    );
    let placement = conflict_revision(message, "placement", &loser).expect("revision");
    assert_ne!(
        id,
        conflict_revision_id(&key, &placement).expect("other group")
    );
}
