//! Helpers for the backup and restore suites: seeding an installation, taking and restoring backups, and reading tables as text.
#![allow(dead_code)]

use super::blobs::{blob_uuid, sealed, upload_all};
use super::*;
use kinige_server::backup::{self, Manifest, Report};
use kinige_server::dto::LoginRequest;
use std::path::PathBuf;

/// What `seed` put into an installation.
pub struct Seeded {
    pub blob_id: String,
    pub blob: Vec<u8>,
}

/// Entities `n` and `n + 1` for the account: one is pushed and purged, the other stays; 181 days pass and the log is compacted, so a
/// drop (epoch 1) exists. Then a blob is published. The virtual clock ends 181 days later than it began.
pub async fn seed(rig: &Rig, acct: &Acct, maker: &Maker, n: u8) -> Seeded {
    put(rig, acct, n, n + 100, &msg(maker, n, n + 100, T0)).await;
    let purge = maker.purged("message", &id(n), &id(n + 101), &[("body", hlc(T0, 0))]);
    put(rig, acct, n, n + 101, &purge).await;
    put(rig, acct, n + 1, n + 102, &msg(maker, n + 1, n + 102, T0)).await;
    rig.clock.advance(181 * DAY);
    compact_now(rig, acct).await;
    let blob = sealed(4000 + usize::from(n), n);
    let blob_id = blob_uuid(n);
    let (code, answer) = upload_all(rig, acct, &blob_id, &blob).await;
    assert_eq!(code, StatusCode::OK, "{answer}");
    Seeded { blob_id, blob }
}

/// A backup of the rig into a new directory; returns `(backup dir, root, manifest)`.
pub async fn take(rig: &Rig) -> (PathBuf, Manifest) {
    let root = rig.db.scratch_dir("backups").expect("a directory");
    take_into(rig, &root).await
}

pub async fn take_into(rig: &Rig, root: &std::path::Path) -> (PathBuf, Manifest) {
    backup::create(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, root)
        .await
        .expect("a backup")
}

pub async fn restore_into(rig: &Rig, dir: &std::path::Path) -> Report {
    backup::restore(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, dir)
        .await
        .expect("a restore")
}

/// A clean installation: a new database and an empty blob store, on the same virtual clock value.
pub async fn clean_like(rig: &Rig) -> Rig {
    let clean = rig_with(Config::default()).await;
    clean.clock.set(kinige_server::Clock::now_ms(&*rig.clock));
    clean
}

/// A new session for the account's credentials.
pub async fn login_token(rig: &Rig, creds: &kinige_server::testing::Creds) -> String {
    let request = LoginRequest {
        login: creds.login.clone(),
        auth_key: b64(&creds.auth_key),
        replaces: None,
    };
    let (status, answer) = rig
        .http
        .post(
            "/v1/login",
            None,
            &serde_json::to_value(&request).expect("json"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    answer["token"].as_str().expect("a token").to_string()
}

/// A number field of a row that `rows` returned.
pub fn field(row: &str, name: &str) -> u64 {
    let after = row.split(&format!("\"{name}\":")).nth(1).expect("a field");
    after
        .trim_end_matches(['}', ' '])
        .split(',')
        .next()
        .expect("a value")
        .parse()
        .expect("a number")
}

/// The status of a login with `key` (no session is kept).
pub async fn login_status(
    rig: &Rig,
    creds: &kinige_server::testing::Creds,
    key: &[u8; 32],
) -> StatusCode {
    rig.http
        .post(
            "/v1/login",
            None,
            &serde_json::json!({"login": creds.login, "auth_key": b64(key)}),
        )
        .await
        .0
}

/// `POST /v1/account/delete` with a session and the password's key.
pub async fn delete_account(
    rig: &Rig,
    acct: &Acct,
    key: &[u8; 32],
) -> (StatusCode, serde_json::Value) {
    rig.http
        .post(
            "/v1/account/delete",
            Some(&acct.token),
            &serde_json::json!({"auth_key": b64(key)}),
        )
        .await
}

/// A password change from `old` (credential version `version`) to a new random password; returns the new key.
pub async fn change_password(rig: &Rig, acct: &Acct, version: u64, old: &[u8; 32]) -> [u8; 32] {
    let (part, new_key, _, _) = acct.creds.new_password(1);
    let (status, answer) = rig
        .http
        .post(
            "/v1/password/change",
            Some(&acct.token),
            &serde_json::json!({"auth_key": b64(old), "expected_version": version, "new": part, "revoke_others": false}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    new_key
}

/// A table (or any query) as JSON text, one string per row, in the order the SQL gives.
pub async fn rows(rig: &Rig, sql: &str) -> Vec<String> {
    let client = rig.state.pool.get().await.expect("connection");
    client
        .query(&format!("SELECT row_to_json(t)::text FROM ({sql}) t"), &[])
        .await
        .expect("rows")
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect()
}

pub async fn count(rig: &Rig, table: &str) -> i64 {
    let client = rig.state.pool.get().await.expect("connection");
    client
        .query_one(&format!("SELECT count(*) FROM {table}"), &[])
        .await
        .expect("count")
        .get(0)
}

/// Every file under `dir`, relative paths, sorted.
pub fn files_under(dir: &std::path::Path) -> Vec<String> {
    fn walk(base: &std::path::Path, at: &std::path::Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(at).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.push(
                    path.strip_prefix(base)
                        .expect("inside")
                        .to_string_lossy()
                        .into_owned(),
                );
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// Copies the directory `from` and everything under it to `to`.
pub fn copy_dir(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("dir");
    for entry in std::fs::read_dir(from).expect("dir") {
        let entry = entry.expect("entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy");
        }
    }
}
