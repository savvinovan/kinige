//! The credential flows of ADR-0001 §8.3-§8.7 and appendix §A.5 over HTTP against a real Postgres: registration, login, password
//! change, recovery, reissuing the words, logout, the rate limits. Every test is named after the rule it pins.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::*;
use kinige_crypto::ids::auth_hash;
use kinige_server::config::Config;
use kinige_server::rate::Rule;
use kinige_server::testing::{Creds, array32, random_wrap};
use serde_json::{Value, json};

async fn login(rig: &Rig, creds: &Creds, key: &[u8; 32]) -> (StatusCode, Value) {
    rig.http
        .post(
            "/v1/login",
            None,
            &json!({"login": creds.login, "auth_key": b64(key)}),
        )
        .await
}

fn token_of(answer: &Value) -> String {
    answer["token"].as_str().expect("a token").to_string()
}

/// A pull with the token: 200 while the session lives, 401 after it ends.
async fn session_status(rig: &Rig, token: &str) -> StatusCode {
    let acct = Acct {
        creds: Creds::random(),
        token: token.to_string(),
    };
    pull(rig, &acct, generation(rig).await, 0, 0, 1).await.0
}

#[tokio::test]
async fn registration_then_login_gives_the_wrap_the_version_and_a_session() {
    let rig = rig().await;
    let acct = account(&rig).await;
    assert_eq!(session_status(&rig, &acct.token).await, StatusCode::OK);
    let (status, answer) = login(&rig, &acct.creds, &acct.creds.auth_key).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(answer["account_id"], acct.creds.account_id);
    assert_eq!(answer["wrap_pw"], b64(&acct.creds.wrap_pw));
    assert_eq!(answer["key_id"], 1);
    assert_eq!(answer["cred_version"], 1);
    assert_ne!(
        token_of(&answer),
        acct.token,
        "every login is a new session"
    );
    let (status, params) = rig
        .http
        .post(
            "/v1/login/params",
            None,
            &json!({"login": acct.creds.login.to_uppercase()}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        params["salt"],
        b64(&acct.creds.salt),
        "logins are case-insensitive"
    );
    assert_eq!(params["profile"], 1);
}

#[tokio::test]
async fn registration_is_idempotent_by_account_id_and_a_taken_login_is_its_own_error() {
    let rig = rig().await;
    let creds = Creds::random();
    let body = serde_json::to_value(creds.register_request()).expect("json");
    let (first, a) = rig.http.post("/v1/register", None, &body).await;
    let (again, b) = rig.http.post("/v1/register", None, &body).await;
    assert_eq!((first, again), (StatusCode::OK, StatusCode::OK), "{a} {b}");
    assert_ne!(a["token"], b["token"]);

    let mut other_package = creds.register_request();
    other_package.wrap_pw = b64(&random_wrap());
    let (status, answer) = rig
        .http
        .post(
            "/v1/register",
            None,
            &serde_json::to_value(other_package).expect("json"),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_id_taken"))
    );

    let mut new_id_same_login = Creds::random().register_request();
    new_id_same_login.login = creds.login.clone();
    let (status, answer) = rig
        .http
        .post(
            "/v1/register",
            None,
            &serde_json::to_value(new_id_same_login).expect("json"),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("login_taken"))
    );
}

#[tokio::test]
async fn registration_refuses_what_the_protocol_does_not_allow() {
    let rig = rig().await;
    let good = Creds::random().register_request();
    type Mutation = Box<dyn Fn(&mut kinige_server::dto::RegisterRequest)>;
    let mutations: Vec<(&str, Mutation)> = vec![
        (
            "account_id",
            Box::new(|r| r.account_id = "NOT-A-UUID".to_string()),
        ),
        ("login", Box::new(|r| r.login = "has space".to_string())),
        ("login", Box::new(|r| r.login = "a".repeat(255))),
        ("profile", Box::new(|r| r.profile = 3)),
        ("profile", Box::new(|r| r.profile = 0)),
        ("salt", Box::new(|r| r.salt = b64(&[1; 15]))),
        ("auth_key", Box::new(|r| r.auth_key = b64(&[1; 31]))),
        (
            "rc_auth_key",
            Box::new(|r| r.rc_auth_key = "***".to_string()),
        ),
        ("wrap_pw", Box::new(|r| r.wrap_pw = b64(&[2; 73]))),
        ("wrap_rc", Box::new(|r| r.wrap_rc = b64(&[1; 72]))),
        ("key_id", Box::new(|r| r.key_id = 0)),
    ];
    for (code, mutate) in mutations {
        let mut request = serde_json::from_value::<kinige_server::dto::RegisterRequest>(
            serde_json::to_value(&good).expect("json"),
        )
        .expect("copy");
        mutate(&mut request);
        let (status, answer) = rig
            .http
            .post(
                "/v1/register",
                None,
                &serde_json::to_value(request).expect("json"),
            )
            .await;
        assert_eq!(
            (status, answer["error"].as_str()),
            (StatusCode::BAD_REQUEST, Some(code))
        );
    }
}

#[tokio::test]
async fn an_unknown_login_looks_exactly_like_a_known_one() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let (_, known) = rig
        .http
        .post(
            "/v1/login/params",
            None,
            &json!({"login": acct.creds.login}),
        )
        .await;
    let (status, unknown) = rig
        .http
        .post(
            "/v1/login/params",
            None,
            &json!({"login": "nobody.here@example.test"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let shape = |v: &Value| {
        let mut keys: Vec<String> = v.as_object().expect("object").keys().cloned().collect();
        keys.sort();
        (keys, v["salt"].as_str().map(str::len), v["profile"].clone())
    };
    assert_eq!(
        shape(&known),
        shape(&unknown),
        "same fields, a 16-byte salt, the default profile"
    );
    let (_, again) = rig
        .http
        .post(
            "/v1/login/params",
            None,
            &json!({"login": "nobody.here@example.test"}),
        )
        .await;
    assert_eq!(unknown, again, "the salt of an unknown login repeats");

    // After a password change the answer for a known login still differs from an unknown one's in the salt only: no credential version,
    // nothing that tells an account that changed its password from one that did not.
    let (part, _, _, _) = acct.creds.new_password(1);
    let (status, _) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &json!({"auth_key": b64(&acct.creds.auth_key), "expected_version": 1, "new": part, "revoke_others": false}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, changed) = rig
        .http
        .post(
            "/v1/login/params",
            None,
            &json!({"login": acct.creds.login}),
        )
        .await;
    let without_salt = |v: &Value| {
        let mut object = v.as_object().expect("object").clone();
        object.remove("salt");
        object
    };
    assert_eq!(without_salt(&changed), without_salt(&unknown));
    assert_eq!(
        without_salt(&changed).keys().collect::<Vec<_>>(),
        vec!["profile"]
    );
    assert_ne!(changed["salt"], unknown["salt"]);

    // The test runs on one thread, so the lines of this thread are the lines of these two requests.
    let before = logged_here().len();
    let wrong_key = login(&rig, &acct.creds, &array32()).await;
    let ghost = Creds::random();
    let no_account = login(&rig, &ghost, &array32()).await;
    assert_eq!(wrong_key, no_account, "same status, same body");
    assert_eq!(wrong_key.0, StatusCode::UNAUTHORIZED);
    // The server's own records, which are at INFO.
    let lines: Vec<String> = logged_here()
        .into_iter()
        .skip(before)
        .filter(|l| l.starts_with("INFO "))
        .collect();
    assert_eq!(
        lines,
        vec![
            "INFO login failed",
            "INFO POST /v1/login -> 401",
            "INFO login failed",
            "INFO POST /v1/login -> 401"
        ]
    );
}

#[tokio::test]
async fn a_password_change_swaps_on_the_version_it_was_derived_against() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let (part, new_key, _, _) = acct.creds.new_password(1);
    let change = |version: u64, old: [u8; 32], part: &kinige_server::dto::PasswordPart| json!({"auth_key": b64(&old), "expected_version": version, "new": part, "revoke_others": false});
    let (status, answer) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &change(1, acct.creds.auth_key, &part),
        )
        .await;
    assert_eq!(
        (status, &answer["cred_version"]),
        (StatusCode::OK, &json!(2)),
        "{answer}"
    );
    assert_eq!(
        login(&rig, &acct.creds, &acct.creds.auth_key).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(login(&rig, &acct.creds, &new_key).await.0, StatusCode::OK);

    // A second change derived against version 1 loses: another change landed first. Nothing is written.
    let (part2, key2, _, _) = acct.creds.new_password(1);
    let (status, answer) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &change(1, acct.creds.auth_key, &part2),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        (answer["error"].as_str(), &answer["current_version"]),
        (Some("version_conflict"), &json!(2))
    );
    assert_eq!(
        login(&rig, &acct.creds, &key2).await.0,
        StatusCode::UNAUTHORIZED
    );

    // The right version with the wrong old key is `auth`.
    let (status, answer) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &change(2, array32(), &part2),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("auth"))
    );
}

#[tokio::test]
async fn two_password_changes_on_one_version_let_exactly_one_through() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let mut tasks = Vec::new();
    for _ in 0..6 {
        let (part, _, _, _) = acct.creds.new_password(1);
        let http = rig.http.clone();
        let token = acct.token.clone();
        let old = acct.creds.auth_key;
        tasks.push(tokio::spawn(async move {
            http.post(
                "/v1/password/change",
                Some(&token),
                &json!({"auth_key": b64(&old), "expected_version": 1, "new": part, "revoke_others": false}),
            )
            .await
            .0
        }));
    }
    let mut statuses = Vec::new();
    for task in tasks {
        statuses.push(task.await.expect("task"));
    }
    assert_eq!(
        statuses.iter().filter(|s| **s == StatusCode::OK).count(),
        1,
        "{statuses:?}"
    );
    assert_eq!(
        statuses
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        5,
        "{statuses:?}"
    );
}

#[tokio::test]
async fn the_server_refuses_a_lower_profile_on_a_password_change_and_on_a_recovery() {
    let rig = rig().await;
    let acct = account(&rig).await;
    // Raise the account to profile 2 first.
    let (up, up_key, _, _) = acct.creds.new_password(2);
    let (status, _) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &json!({"auth_key": b64(&acct.creds.auth_key), "expected_version": 1, "new": up, "revoke_others": false}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, params) = rig
        .http
        .post(
            "/v1/login/params",
            None,
            &json!({"login": acct.creds.login}),
        )
        .await;
    assert_eq!(params["profile"], 2);
    // A change derived against version 2 back to profile 1.
    let (down, _, _, _) = acct.creds.new_password(1);
    let (status, answer) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &json!({"auth_key": b64(&up_key), "expected_version": 2, "new": down, "revoke_others": false}),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("profile_downgrade"))
    );

    let (down, _, _, _) = acct.creds.new_password(1);
    let (status, answer) = rig
        .http
        .post(
            "/v1/recovery",
            None,
            &json!({"login": acct.creds.login, "old_rc_auth_key": b64(&acct.creds.rc_auth_key), "new": down,
                    "rc_auth_key": b64(&array32()), "wrap_rc": b64(&random_wrap())}),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("profile_downgrade"))
    );
    let (_, still) = rig
        .http
        .post(
            "/v1/login/params",
            None,
            &json!({"login": acct.creds.login}),
        )
        .await;
    assert_eq!(still["profile"], 2, "nothing was written");
}

#[tokio::test]
async fn revoke_others_ends_the_other_sessions_and_keeps_this_one() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let (_, other) = login(&rig, &acct.creds, &acct.creds.auth_key).await;
    let other = token_of(&other);
    let (part, _, _, _) = acct.creds.new_password(1);
    let (status, _) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &json!({"auth_key": b64(&acct.creds.auth_key), "expected_version": 1, "new": part, "revoke_others": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session_status(&rig, &acct.token).await, StatusCode::OK);
    assert_eq!(session_status(&rig, &other).await, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn recovery_replaces_everything_ends_all_sessions_and_is_idempotent() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let (_, second) = login(&rig, &acct.creds, &acct.creds.auth_key).await;
    let second = token_of(&second);

    // Step 1: the word check. A wrong key and an unknown login are the same answer.
    let fetch = |login: &str, key: [u8; 32]| json!({"login": login, "rc_auth_key": b64(&key)});
    let (status, fetched) = rig
        .http
        .post(
            "/v1/recovery/fetch",
            None,
            &fetch(&acct.creds.login, acct.creds.rc_auth_key),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{fetched}");
    assert_eq!(fetched["wrap_rc"], b64(&acct.creds.wrap_rc));
    let wrong = rig
        .http
        .post(
            "/v1/recovery/fetch",
            None,
            &fetch(&acct.creds.login, array32()),
        )
        .await;
    let ghost = rig
        .http
        .post(
            "/v1/recovery/fetch",
            None,
            &fetch("ghost@example.test", array32()),
        )
        .await;
    assert_eq!(wrong, ghost);
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED);

    // Step 4: one atomic replacement.
    let (part, new_pw_key, _, _) = acct.creds.new_password(1);
    let new_rc = array32();
    let new_wrap_rc = random_wrap();
    let request = json!({"login": acct.creds.login, "old_rc_auth_key": b64(&acct.creds.rc_auth_key), "new": part,
                         "rc_auth_key": b64(&new_rc), "wrap_rc": b64(&new_wrap_rc)});
    let (status, done) = rig.http.post("/v1/recovery", None, &request).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["cred_version"], 2);
    let session = token_of(&done);
    assert_eq!(
        session_status(&rig, &acct.token).await,
        StatusCode::UNAUTHORIZED,
        "the other sessions are revoked"
    );
    assert_eq!(
        session_status(&rig, &second).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(session_status(&rig, &session).await, StatusCode::OK);
    assert_eq!(
        login(&rig, &acct.creds, &new_pw_key).await.0,
        StatusCode::OK,
        "the new password works"
    );
    assert_eq!(
        login(&rig, &acct.creds, &acct.creds.auth_key).await.0,
        StatusCode::UNAUTHORIZED
    );
    let old_words = rig
        .http
        .post(
            "/v1/recovery/fetch",
            None,
            &fetch(&acct.creds.login, acct.creds.rc_auth_key),
        )
        .await;
    assert_eq!(
        old_words.0,
        StatusCode::UNAUTHORIZED,
        "the old words are invalid"
    );

    // A lost answer: the same package again is a success and does not apply twice.
    let (status, again) = rig.http.post("/v1/recovery", None, &request).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["cred_version"], 2, "the version did not move");

    // The word checks have a budget per login; a new window.
    rig.clock.advance(61_000);
    // The same new words behind another package are a separate error.
    let (other_part, _, _, _) = acct.creds.new_password(1);
    let clash = json!({"login": acct.creds.login, "old_rc_auth_key": b64(&acct.creds.rc_auth_key), "new": other_part,
                       "rc_auth_key": b64(&new_rc), "wrap_rc": b64(&new_wrap_rc)});
    let (status, answer) = rig.http.post("/v1/recovery", None, &clash).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("package_mismatch"))
    );

    // An unknown login and a wrong old key are the same `auth`.
    let wrong_old = json!({"login": acct.creds.login, "old_rc_auth_key": b64(&array32()), "new": part,
                           "rc_auth_key": b64(&array32()), "wrap_rc": b64(&random_wrap())});
    let ghost = json!({"login": "ghost@example.test", "old_rc_auth_key": b64(&array32()), "new": part,
                       "rc_auth_key": b64(&array32()), "wrap_rc": b64(&random_wrap())});
    let wrong = rig.http.post("/v1/recovery", None, &wrong_old).await;
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong, rig.http.post("/v1/recovery", None, &ghost).await);
}

#[tokio::test]
async fn reissuing_the_words_needs_the_session_and_the_password_and_is_idempotent() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let new_rc = array32();
    let wrap = random_wrap();
    let request = |key: [u8; 32], wrap: &[u8]| json!({"auth_key": b64(&key), "new_rc_auth_key": b64(&new_rc), "wrap_rc": b64(wrap)});
    let (status, _) = rig
        .http
        .post(
            "/v1/words/reissue",
            Some(&acct.token),
            &request(array32(), &wrap),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "a wrong password");
    let (status, _) = rig
        .http
        .post(
            "/v1/words/reissue",
            None,
            &request(acct.creds.auth_key, &wrap),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "no session");
    let (status, answer) = rig
        .http
        .post(
            "/v1/words/reissue",
            Some(&acct.token),
            &request(acct.creds.auth_key, &wrap),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let (status, _) = rig
        .http
        .post(
            "/v1/words/reissue",
            Some(&acct.token),
            &request(acct.creds.auth_key, &wrap),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "the same package again");
    let (status, answer) = rig
        .http
        .post(
            "/v1/words/reissue",
            Some(&acct.token),
            &request(acct.creds.auth_key, &random_wrap()),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("package_mismatch"))
    );
    let fetch = json!({"login": acct.creds.login, "rc_auth_key": b64(&acct.creds.rc_auth_key)});
    assert_eq!(
        rig.http.post("/v1/recovery/fetch", None, &fetch).await.0,
        StatusCode::UNAUTHORIZED,
        "the old words no longer work"
    );
}

#[tokio::test]
async fn logout_ends_one_session_and_a_login_can_name_the_token_it_replaces() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let (_, second) = login(&rig, &acct.creds, &acct.creds.auth_key).await;
    let second = token_of(&second);
    let (status, _) = rig.http.post("/v1/logout", Some(&second), &json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        session_status(&rig, &second).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session_status(&rig, &acct.token).await,
        StatusCode::OK,
        "the other session lives"
    );
    let (status, _) = rig.http.post("/v1/logout", Some(&second), &json!({})).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "a token that is not a session"
    );

    // A login that carries the token it replaces ends it in the same step.
    let (status, replaced) = rig
        .http
        .post(
            "/v1/login",
            None,
            &json!({"login": acct.creds.login, "auth_key": b64(&acct.creds.auth_key), "replaces": acct.token}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        session_status(&rig, &acct.token).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        session_status(&rig, &token_of(&replaced)).await,
        StatusCode::OK
    );

    // The token of another account is not touched, and the login does not say so.
    let stranger = account(&rig).await;
    let (status, _) = rig
        .http
        .post(
            "/v1/login",
            None,
            &json!({"login": acct.creds.login, "auth_key": b64(&acct.creds.auth_key), "replaces": stranger.token}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(session_status(&rig, &stranger.token).await, StatusCode::OK);
}

#[tokio::test]
async fn login_attempts_are_limited_per_login_and_per_address_alike_for_unknown_logins() {
    let config = Config {
        login_by_login: Rule::new(3, 60_000),
        login_by_addr: Rule::new(8, 60_000),
        ..Config::default()
    };
    let rig = rig_with(config).await;
    let acct = account(&rig).await;
    for _ in 0..3 {
        assert_eq!(
            login(&rig, &acct.creds, &array32()).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let blocked = login(&rig, &acct.creds, &acct.creds.auth_key).await;
    assert_eq!(
        blocked.0,
        StatusCode::TOO_MANY_REQUESTS,
        "even the right key waits out the window"
    );
    assert_eq!(blocked.1["error"], "rate_limited");
    // An unknown login is limited by the same rule.
    let ghost = Creds::random();
    for _ in 0..3 {
        assert_eq!(
            login(&rig, &ghost, &array32()).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        login(&rig, &ghost, &array32()).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    // The address budget is shared by all logins (every request here has the same, unknown, address).
    for _ in 0..2 {
        let _ = login(&rig, &Creds::random(), &array32()).await;
    }
    assert_eq!(
        login(&rig, &Creds::random(), &array32()).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    rig.clock.advance(61_000);
    assert_eq!(
        login(&rig, &acct.creds, &acct.creds.auth_key).await.0,
        StatusCode::OK,
        "a new window"
    );
}

#[tokio::test]
async fn the_word_checks_share_one_budget() {
    let config = Config {
        recovery_by_login: Rule::new(2, 60_000),
        ..Config::default()
    };
    let rig = rig_with(config).await;
    let acct = account(&rig).await;
    let fetch = json!({"login": acct.creds.login, "rc_auth_key": b64(&array32())});
    assert_eq!(
        rig.http.post("/v1/recovery/fetch", None, &fetch).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        rig.http.post("/v1/recovery/fetch", None, &fetch).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        rig.http.post("/v1/recovery/fetch", None, &fetch).await.0,
        StatusCode::TOO_MANY_REQUESTS
    );
    let (part, _, _, _) = acct.creds.new_password(1);
    let recovery = json!({"login": acct.creds.login, "old_rc_auth_key": b64(&acct.creds.rc_auth_key), "new": part,
                          "rc_auth_key": b64(&array32()), "wrap_rc": b64(&random_wrap())});
    assert_eq!(
        rig.http.post("/v1/recovery", None, &recovery).await.0,
        StatusCode::TOO_MANY_REQUESTS,
        "the recovery itself is in the same budget"
    );
}

#[tokio::test]
async fn the_database_holds_the_hashes_of_the_keys_and_never_the_keys() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let client = rig.state.pool.get().await.expect("connection");
    let row = client
        .query_one(
            "SELECT auth_hash, rc_hash FROM accounts WHERE id = $1",
            &[&acct.creds.account_id],
        )
        .await
        .expect("row");
    let (auth, rc): (Vec<u8>, Vec<u8>) = (row.get(0), row.get(1));
    assert_eq!(
        auth,
        auth_hash(&acct.creds.auth_key).expect("hash").to_vec()
    );
    assert_eq!(
        rc,
        auth_hash(&acct.creds.rc_auth_key).expect("hash").to_vec()
    );
    assert_ne!(auth, acct.creds.auth_key.to_vec());
}

/// Eight registrations at once, all through the router; the statuses, sorted.
async fn race(rig: &Rig, requests: Vec<kinige_server::dto::RegisterRequest>) -> Vec<(u16, String)> {
    let mut tasks = Vec::new();
    for request in requests {
        let http = rig.http.clone();
        tasks.push(tokio::spawn(async move {
            let body = serde_json::to_value(request).expect("json");
            let (status, answer) = http.post("/v1/register", None, &body).await;
            (
                status.as_u16(),
                answer["error"].as_str().unwrap_or("").to_string(),
            )
        }));
    }
    let mut out = Vec::new();
    for task in tasks {
        out.push(task.await.expect("task"));
    }
    out.sort();
    out
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn racing_registrations_meet_a_clean_answer_and_never_the_unique_index() {
    let rig = rig().await;
    // The same login behind eight account ids: one wins, the others are told the login is taken (never a 500, and Postgres is never
    // asked to refuse a duplicate, so its own log has no `Key (login)=(...) already exists`).
    let login = Creds::random().login;
    let requests = (0..8)
        .map(|_| {
            let mut r = Creds::random().register_request();
            r.login = login.clone();
            r
        })
        .collect();
    let answers = race(&rig, requests).await;
    assert_eq!(
        answers.iter().filter(|a| a.0 == 200).count(),
        1,
        "{answers:?}"
    );
    assert_eq!(
        answers
            .iter()
            .filter(|a| *a == &(409, "login_taken".to_string()))
            .count(),
        7,
        "{answers:?}"
    );

    // The same package eight times at once is eight successes.
    let package = Creds::random().register_request();
    let copy = || {
        serde_json::from_value::<kinige_server::dto::RegisterRequest>(
            serde_json::to_value(&package).expect("json"),
        )
        .expect("copy")
    };
    let answers = race(&rig, (0..8).map(|_| copy()).collect()).await;
    assert!(answers.iter().all(|a| a.0 == 200), "{answers:?}");

    // One account id behind eight logins: one wins, the others are told the id is taken by another package.
    let id = Creds::random().account_id;
    let requests = (0..8)
        .map(|_| {
            let mut r = Creds::random().register_request();
            r.account_id = id.clone();
            r
        })
        .collect();
    let answers = race(&rig, requests).await;
    assert_eq!(
        answers.iter().filter(|a| a.0 == 200).count(),
        1,
        "{answers:?}"
    );
    assert_eq!(
        answers
            .iter()
            .filter(|a| *a == &(409, "account_id_taken".to_string()))
            .count(),
        7,
        "{answers:?}"
    );
}
