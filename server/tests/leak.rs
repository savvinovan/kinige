//! PRD §14 (criteria S4c): no plain text in the server database. A run writes synthetic entries whose texts, chat titles, `journal_date`
//! and tags are known, and goes through every credential flow with real keys; then every table is dumped and the whole server log is read,
//! and both are scanned for every text and every secret in every encoding (ADR-0001 §8.8, appendix §A.4 "Logs"). The scanner is proven first:
//! it finds a deliberate leak in each encoding, finds the hashes the database must hold, and finds a plain text planted in a new table.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use common::leak::{Secret, encodings, find_leaks};
use common::*;
use kinige_crypto::ids::auth_hash;
use kinige_crypto::secret::{Password, SecretBytes};
use kinige_crypto::words::Words;
use kinige_crypto::{Profile, keys, wrap};
use kinige_server::Clock;
use kinige_server::dto::RegisterRequest;
use kinige_server::store::compact;
use kinige_server::wire::{to_b64, uuid_text};
use libsodium_rs::random;
use serde_json::json;

const LOGIN: &str = "fake.leaktest@example.test";
const ENTRY_TEXT: &str = "fake entry text about a fake journey";
const CHAT_TITLE: &str = "Fake Chat Title For The Test";
const JOURNAL_DATE: &str = "2031-07-19";
const TAG: &str = "#fakeleaktag";

fn password(text: &str) -> Password {
    SecretBytes::new(text.as_bytes().to_vec())
}

/// Every table, every row, as the text Postgres would print it (`bytea` as hex): what a stolen database looks like.
async fn dump(rig: &Rig) -> String {
    let client = rig.state.pool.get().await.expect("connection");
    let tables = client
        .query(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE' ORDER BY 1",
            &[],
        )
        .await
        .expect("tables");
    let mut out = String::new();
    for table in tables {
        let name: String = table.get(0);
        let rows = client
            .query(
                &format!("SELECT row_to_json(t)::text FROM \"{name}\" t"),
                &[],
            )
            .await
            .expect("rows");
        out.push_str(&format!("-- {name}\n"));
        for row in rows {
            out.push_str(row.get::<_, &str>(0));
            out.push('\n');
        }
    }
    out
}

#[tokio::test]
async fn the_scanner_finds_a_deliberate_leak_in_every_encoding() {
    let key: Vec<u8> = (0..32u8)
        .map(|i| i.wrapping_mul(11).wrapping_add(5))
        .collect();
    let secrets = [Secret::new("auth_key", &key)];
    for form in encodings(&key) {
        assert_eq!(
            find_leaks(&format!("a row {form} ends"), &secrets),
            vec!["auth_key"],
            "{form}"
        );
    }
    assert_eq!(
        find_leaks(&format!("request {key:?}"), &secrets),
        vec!["auth_key"],
        "the Debug of the bytes"
    );
    assert!(find_leaks("a clean line, account 1111", &secrets).is_empty());
    let text = [Secret::text("title", CHAT_TITLE)];
    assert_eq!(
        find_leaks(&format!("{{\"t\":\"{CHAT_TITLE}\"}}"), &text),
        vec!["title"]
    );
    assert!(
        find_leaks("Fake Chat", &text).is_empty(),
        "plain text counts only whole"
    );
}

#[tokio::test]
async fn nothing_the_user_wrote_and_no_secret_is_in_any_table_or_in_the_log() {
    let rig = rig().await;
    let maker = Maker::new();
    let account = *maker.account();
    let dek = maker.dek();
    let mut secrets: Vec<Secret> = Vec::new();

    // Registration with real keys: Argon2id of profile 1, the words, the wraps.
    let first_password = password("fake password number one");
    let salt: [u8; 16] = random::bytes(16).try_into().expect("16 bytes");
    let root = keys::derive_pw_root(&first_password, &salt, Profile::P1).expect("root");
    let auth = keys::pw_auth_key(&root).expect("auth key");
    let kek = keys::pw_kek(&root).expect("kek");
    let words = Words::generate().expect("words");
    let rc_root = words.rc_root().expect("rc root");
    let rc_auth = keys::rc_auth_key(&rc_root).expect("rc auth key");
    let wrap_pw = wrap::wrap_pw(dek, &kek, &account, &salt, Profile::P1).expect("wrap");
    let wrap_rc =
        wrap::wrap_rc(dek, &keys::rc_kek(&rc_root).expect("kek"), &account).expect("wrap");
    for (name, bytes) in [
        ("auth_key", auth.as_bytes().as_slice()),
        ("rc_auth_key", rc_auth.as_bytes()),
        ("pw_root", root.as_bytes()),
        ("kek_pw", kek.as_bytes()),
        ("rc_root", rc_root.as_bytes()),
        ("dek", dek.key().as_bytes()),
        ("k_env", dek.env_key().as_bytes()),
        ("k_rev", dek.rev_key().as_bytes()),
    ] {
        secrets.push(Secret::new(name, bytes));
    }
    secrets.push(Secret::text("password 1", "fake password number one"));
    secrets.push(Secret::text("words 1", words.phrase()));
    let account_text = uuid_text(account.as_bytes());
    let request = RegisterRequest {
        account_id: account_text.clone(),
        login: LOGIN.to_string(),
        salt: to_b64(&salt),
        profile: 1,
        auth_key: to_b64(auth.as_bytes()),
        rc_auth_key: to_b64(rc_auth.as_bytes()),
        wrap_pw: to_b64(&wrap_pw),
        wrap_rc: to_b64(&wrap_rc),
        key_id: 1,
    };
    let (status, registered) = rig
        .http
        .post(
            "/v1/register",
            None,
            &serde_json::to_value(&request).expect("json"),
        )
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{registered}");
    let token = registered["token"].as_str().expect("token").to_string();
    secrets.push(Secret::new("token", &from_b64(&token)));

    // The user writes: entries with a text, a title, a date and a tag, sealed by the real envelope under the account's DEK.
    let account_row = Acct {
        creds: kinige_server::testing::Creds {
            account_id: account_text.clone(),
            login: LOGIN.to_string(),
            ..kinige_server::testing::Creds::random()
        },
        token: token.clone(),
    };
    let content = format!("{CHAT_TITLE} | {JOURNAL_DATE} | {TAG} | {ENTRY_TEXT}");
    for n in 1..=6u8 {
        let bytes = maker.upsert(
            "message",
            &uuid(n),
            &uuid(100 + n),
            &[
                ("body", hlc(T0 - 100, u32::from(n))),
                ("placement", hlc(T0 - 100, u32::from(n))),
            ],
            &format!("{content} number {n}"),
        );
        let result = push(
            &rig,
            &account_row,
            &Item {
                entity: "message",
                id: &uuid(n),
                op: &uuid(100 + n),
                bytes: &bytes,
            },
        )
        .await;
        assert_eq!(outcome(&result), "ok");
    }
    let purge = maker.purged(
        "message",
        &uuid(1),
        &uuid(110),
        &[("body", hlc(T0 - 100, 1)), ("placement", hlc(T0 - 100, 1))],
    );
    push(
        &rig,
        &account_row,
        &Item {
            entity: "message",
            id: &uuid(1),
            op: &uuid(110),
            bytes: &purge,
        },
    )
    .await;
    rig.clock.advance(181 * DAY);
    compact::account(&rig.state.pool, &account_text, rig.clock.now_ms())
        .await
        .expect("compaction");
    for (name, text) in [
        ("entry text", ENTRY_TEXT),
        ("chat title", CHAT_TITLE),
        ("journal_date", JOURNAL_DATE),
        ("tag", TAG),
    ] {
        secrets.push(Secret::text(name, text));
    }

    // The flows: a second login, a password change, a recovery with the words, new words, a failed login, a logout.
    let (_, second) = rig
        .http
        .post(
            "/v1/login",
            None,
            &json!({"login": LOGIN, "auth_key": to_b64(auth.as_bytes())}),
        )
        .await;
    let second_token = second["token"].as_str().expect("token").to_string();
    secrets.push(Secret::new("token 2", &from_b64(&second_token)));
    let second_password = password("fake password number two");
    let salt2: [u8; 16] = random::bytes(16).try_into().expect("16 bytes");
    let root2 = keys::derive_pw_root(&second_password, &salt2, Profile::P1).expect("root");
    let auth2 = keys::pw_auth_key(&root2).expect("auth key");
    let wrap_pw2 = wrap::wrap_pw(
        dek,
        &keys::pw_kek(&root2).expect("kek"),
        &account,
        &salt2,
        Profile::P1,
    )
    .expect("wrap");
    for (name, bytes) in [
        ("auth_key 2", auth2.as_bytes().as_slice()),
        ("pw_root 2", root2.as_bytes()),
        ("kek_pw 2", keys::pw_kek(&root2).expect("kek").as_bytes()),
    ] {
        secrets.push(Secret::new(name, bytes));
    }
    secrets.push(Secret::text("password 2", "fake password number two"));
    let part = json!({"salt": to_b64(&salt2), "profile": 1, "auth_key": to_b64(auth2.as_bytes()), "wrap_pw": to_b64(&wrap_pw2)});
    let (status, changed) = rig
        .http
        .post("/v1/password/change", Some(&token), &json!({"auth_key": to_b64(auth.as_bytes()), "expected_version": 1, "new": part, "revoke_others": true}))
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{changed}");
    let _ = rig
        .http
        .post(
            "/v1/login",
            None,
            &json!({"login": LOGIN, "auth_key": to_b64(&[7u8; 32])}),
        )
        .await;

    let new_words = Words::generate().expect("words");
    let new_rc_root = new_words.rc_root().expect("rc root");
    let new_rc_auth = keys::rc_auth_key(&new_rc_root).expect("rc auth key");
    let third_password = password("fake password number three");
    let salt3: [u8; 16] = random::bytes(16).try_into().expect("16 bytes");
    let root3 = keys::derive_pw_root(&third_password, &salt3, Profile::P1).expect("root");
    let auth3 = keys::pw_auth_key(&root3).expect("auth key");
    let wrap_pw3 = wrap::wrap_pw(
        dek,
        &keys::pw_kek(&root3).expect("kek"),
        &account,
        &salt3,
        Profile::P1,
    )
    .expect("wrap");
    let new_wrap_rc =
        wrap::wrap_rc(dek, &keys::rc_kek(&new_rc_root).expect("kek"), &account).expect("wrap");
    for (name, bytes) in [
        ("auth_key 3", auth3.as_bytes().as_slice()),
        ("rc_auth_key 2", new_rc_auth.as_bytes()),
        ("rc_root 2", new_rc_root.as_bytes()),
        ("pw_root 3", root3.as_bytes()),
    ] {
        secrets.push(Secret::new(name, bytes));
    }
    secrets.push(Secret::text("password 3", "fake password number three"));
    secrets.push(Secret::text("words 2", new_words.phrase()));
    let (status, recovered) = rig
        .http
        .post(
            "/v1/recovery",
            None,
            &json!({"login": LOGIN, "old_rc_auth_key": to_b64(rc_auth.as_bytes()),
                    "new": {"salt": to_b64(&salt3), "profile": 1, "auth_key": to_b64(auth3.as_bytes()), "wrap_pw": to_b64(&wrap_pw3)},
                    "rc_auth_key": to_b64(new_rc_auth.as_bytes()), "wrap_rc": to_b64(&new_wrap_rc)}),
        )
        .await;
    assert_eq!(status, axum::http::StatusCode::OK, "{recovered}");
    let recovered_token = recovered["token"].as_str().expect("token").to_string();
    secrets.push(Secret::new("token 3", &from_b64(&recovered_token)));
    let (status, _) = rig
        .http
        .post("/v1/logout", Some(&recovered_token), &json!({}))
        .await;
    assert_eq!(status, axum::http::StatusCode::OK);
    // One session stays alive at the end, so that the database holds a token hash to find.
    let (_, live) = rig
        .http
        .post(
            "/v1/login",
            None,
            &json!({"login": LOGIN, "auth_key": to_b64(auth3.as_bytes())}),
        )
        .await;
    let live_token = from_b64(live["token"].as_str().expect("token"));
    secrets.push(Secret::new("token 4", &live_token));
    assert!(secrets.len() >= 24, "{} things scanned for", secrets.len());

    // The scan: the whole database and the whole log of the process.
    let database = dump(&rig).await;
    let log = logged().join("\n");
    assert!(
        database.len() > 4000,
        "the dump is the whole state: {} bytes",
        database.len()
    );
    assert!(
        logged().len() >= 15,
        "the run logged its events: {}",
        logged().len()
    );
    let in_database = find_leaks(&database, &secrets);
    assert!(
        in_database.is_empty(),
        "secrets or text in the database: {in_database:?}"
    );
    let in_log = find_leaks(&log, &secrets);
    assert!(
        in_log.is_empty(),
        "secrets or text in the server log: {in_log:?}"
    );
    let with_login: Vec<&str> = log.lines().filter(|line| line.contains(LOGIN)).collect();
    assert!(
        with_login.is_empty(),
        "the login is personal data too: not in the log: {with_login:?}"
    );

    // Positive controls. The dump holds what it should: the hashes, the salt, the login, the session hashes, the envelope bytes.
    let row = {
        let client = rig.state.pool.get().await.expect("connection");
        client
            .query_one(
                "SELECT auth_hash, rc_hash, salt FROM accounts WHERE id = $1",
                &[&account_text],
            )
            .await
            .expect("row")
    };
    let (stored_auth, stored_rc, stored_salt): (Vec<u8>, Vec<u8>, Vec<u8>) =
        (row.get(0), row.get(1), row.get(2));
    assert_eq!(
        stored_auth,
        auth_hash(auth3.as_bytes()).expect("hash").to_vec(),
        "after the recovery the account holds the hash of the third password's key"
    );
    let expected = [
        Secret::new("auth_hash", &stored_auth),
        Secret::new("rc_hash", &stored_rc),
        Secret::new("salt", &stored_salt),
        Secret::new(
            "token hash",
            &auth_hash(&<[u8; 32]>::try_from(live_token.as_slice()).expect("32")).expect("hash"),
        ),
        Secret::text("login", LOGIN),
        Secret::new("an envelope", &sample_envelope(&rig, &account_text).await),
    ];
    let found = find_leaks(&database, &expected);
    assert_eq!(
        found,
        expected.iter().map(|s| s.name).collect::<Vec<_>>(),
        "the scan sees what the database really holds"
    );

    // The scan covers a table nobody listed: a plain text planted in a new one is flagged, by name.
    {
        let client = rig.state.pool.get().await.expect("connection");
        client
            .batch_execute("CREATE TABLE zz_planted (note text)")
            .await
            .expect("table");
        client
            .execute(
                "INSERT INTO zz_planted (note) VALUES ($1)",
                &[&format!("{CHAT_TITLE} on {JOURNAL_DATE}")],
            )
            .await
            .expect("insert");
    }
    let planted = find_leaks(&dump(&rig).await, &secrets);
    assert_eq!(planted, vec!["chat title", "journal_date"]);
    // And the log: a line with a key in it is flagged.
    let leaky = format!("{log}\nregister body: {}", to_b64(auth.as_bytes()));
    assert_eq!(find_leaks(&leaky, &secrets), vec!["auth_key"]);
}

fn from_b64(text: &str) -> Vec<u8> {
    kinige_server::wire::from_b64(text).expect("base64")
}

/// The first 64 bytes of a stored envelope: enough to be found, and not the whole.
async fn sample_envelope(rig: &Rig, account: &str) -> Vec<u8> {
    let client = rig.state.pool.get().await.expect("connection");
    let row = client
        .query_one(
            "SELECT bytes FROM envelopes WHERE account_id = $1 ORDER BY seq LIMIT 1",
            &[&account],
        )
        .await
        .expect("an envelope");
    let bytes: Vec<u8> = row.get(0);
    bytes.get(..64).expect("64 bytes").to_vec()
}
