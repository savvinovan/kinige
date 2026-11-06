//! The rules a restore applies (ADR-0001 §6.3, §8.9, ADR-0017, ADR-0020): an account deleted after a backup is deleted again and its id is
//! never registered again, a password changed after a backup is not the old one again, a branch that a restore abandoned is placed by where
//! it forked (also when it was itself restored from a backup of a branch), and a restore refuses to run under a server. Over HTTP, on real
//! dumps. Damaged backups are in `restore_garbage.rs`.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::blobs::*;
use common::*;
use kinige_server::Clock;
use kinige_server::backup::{self, BackupError};
use kinige_server::store::lock::ServerLock;
use kinige_server::testing::{Creds, array32, random_wrap};
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

#[tokio::test]
async fn a_deleted_account_does_not_come_back_with_an_older_backup() {
    let rig = rig().await;
    let (one, two) = (account(&rig).await, account(&rig).await);
    let maker = Maker::new();
    let seeded = seed(&rig, &one, &maker, 1).await;
    put(&rig, &two, 30, 130, &msg(&maker, 30, 130, T0)).await;
    let (older, older_manifest) = take(&rig).await;
    let root = older.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);

    // An unfinished upload of the account, then the deletion: a wrong key deletes nothing; a right one removes the account, its wraps,
    // hashes, sessions and envelopes at once, marks its blobs, and puts its hash in the journal.
    upload(&rig, &one, &blob_uuid(9), &sealed(5000, 9), 0, 100).await;
    let (code, answer) = delete_account(&rig, &one, &[3u8; 32]).await;
    assert_eq!(
        (code, answer["error"].as_str()),
        (StatusCode::UNAUTHORIZED, Some("auth"))
    );
    assert_eq!(accounts_left(&rig).await.len(), 2);
    let (code, _) = rig
        .http
        .post(
            "/v1/account/delete",
            None,
            &json!({"auth_key": b64(&one.creds.auth_key)}),
        )
        .await;
    assert_eq!(code, StatusCode::UNAUTHORIZED, "a session is needed");
    let (code, _) = delete_account(&rig, &one, &one.creds.auth_key).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(
        accounts_left(&rig).await,
        vec![two.creds.account_id.clone()]
    );
    for table in [
        "sessions",
        "envelopes",
        "ops",
        "entity_known",
        "purged_entities",
        "drops",
    ] {
        let left = rows(
            &rig,
            &format!(
                "SELECT 1 FROM {table} WHERE account_id = '{}'",
                one.creds.account_id
            ),
        )
        .await;
        assert!(left.is_empty(), "{table}");
    }
    assert_eq!(
        blob_row(&rig, &one, &seeded.blob_id)
            .await
            .map(|r| r.1.is_some()),
        Some(true)
    );
    assert!(
        rig.state
            .blobs
            .staging
            .find(&one.creds.account_id, &blob_uuid(9))
            .await
            .is_none()
    );
    assert_eq!(
        download(&rig, &one, &seeded.blob_id, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(count(&rig, "deletions").await, 1);
    let hash = kinige_server::hashing::hex(
        &kinige_server::hashing::account_hash(&one.creds.account_id).expect("hash"),
    );
    assert_eq!(
        rows(
            &rig,
            "SELECT encode(account_hash, 'hex') AS h FROM deletions"
        )
        .await,
        vec![format!("{{\"h\":\"{hash}\"}}")]
    );
    rig.clock.advance(DAY);
    let (newer, _) = take_into(&rig, &root).await;

    // In place: the journal of the installation is read before it is replaced.
    let report = restore_into(&rig, &older).await;
    assert_eq!(report.deletions_applied, 1);
    assert_eq!(
        accounts_left(&rig).await,
        vec![two.creds.account_id.clone()]
    );
    assert_eq!(count(&rig, "deletions").await, 1, "and the journal is kept");
    assert_eq!(
        blob_row(&rig, &one, &seeded.blob_id)
            .await
            .map(|r| r.1.is_some()),
        Some(true),
        "the blob goes the delayed way"
    );

    // Into a clean installation: no journal of its own; the newest backup next to the one restored has it.
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &older).await;
    assert_eq!(report.deletions_applied, 1);
    assert_eq!(
        accounts_left(&clean).await,
        vec![two.creds.account_id.clone()]
    );
    assert_eq!(count(&clean, "deletions").await, 1);

    // The honest limit: with no journal anywhere (the older backup alone), the account is restored. The operator keeps backups together.
    let alone_root = rig.db.scratch_dir("alone").expect("dir");
    copy_dir(&older, &alone_root.join("backup-alone"));
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &alone_root.join("backup-alone")).await;
    assert_eq!(report.deletions_applied, 0);
    assert_eq!(accounts_left(&clean).await.len(), 2);

    // A deletion made before the cut time is in the backup already: nothing to apply, and the account is simply not there.
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &newer).await;
    assert_eq!(report.deletions_applied, 0);
    assert_eq!(
        accounts_left(&clean).await,
        vec![two.creds.account_id.clone()]
    );
    assert_eq!(
        count(&clean, "deletions").await,
        1,
        "the journal travels with the backup"
    );
    assert!(older_manifest.cut_ms < rig.clock.now_ms());
}

#[tokio::test]
async fn a_deleted_account_id_is_never_registered_again_so_one_journal_entry_is_enough() {
    // Round 1 of the review: if a deleted id could be registered again, a second deletion would need a second entry (the journal keeps one
    // per account), and an old backup restored between the two would bring the account back. The id is refused, in the installation and
    // after a restore of a backup older than the deletion.
    let rig = rig().await;
    let acct = account(&rig).await;
    let (before, _) = take(&rig).await;
    rig.clock.advance(DAY);
    let (code, _) = delete_account(&rig, &acct, &acct.creds.auth_key).await;
    assert_eq!(code, StatusCode::OK);

    let again = |rig: &Rig| {
        let body = serde_json::to_value(acct.creds.register_request()).expect("json");
        let http = rig.http.clone();
        async move { http.post("/v1/register", None, &body).await }
    };
    let (status, answer) = again(&rig).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_id_taken")),
        "{answer}"
    );
    // A new login for the same id, and the same login with another id, are not what is refused: only the id.
    let other_id = Creds {
        account_id: Creds::random().account_id,
        ..acct.creds.clone()
    };
    let (status, _) = rig
        .http
        .post(
            "/v1/register",
            None,
            &serde_json::to_value(other_id.register_request()).expect("json"),
        )
        .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the login is free again, under another id"
    );

    // The older backup restored in place: the account is deleted again, and its id is still refused.
    let report = restore_into(&rig, &before).await;
    assert_eq!(report.deletions_applied, 1);
    assert!(
        accounts_left(&rig)
            .await
            .iter()
            .all(|id| *id != acct.creds.account_id),
        "the account stays deleted"
    );
    let (status, answer) = again(&rig).await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::CONFLICT, Some("account_id_taken"))
    );
}

#[tokio::test]
async fn a_password_changed_after_a_backup_is_not_the_old_one_again() {
    // Round 1 of the review: a restore replaced the credentials with the backup's, so a password changed because it leaked worked again.
    let rig = rig().await;
    let acct = account(&rig).await;
    let old = acct.creds.auth_key;
    let (older, _) = take(&rig).await;
    let root = older.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);
    let new = change_password(&rig, &acct, 1, &old).await;
    assert_eq!(
        login_status(&rig, &acct.creds, &old).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(login_status(&rig, &acct.creds, &new).await, StatusCode::OK);
    assert_eq!(count(&rig, "credential_changes").await, 1);
    rig.clock.advance(DAY);
    let (newer, _) = take_into(&rig, &root).await;

    // In place: the journal of the installation is read before it is replaced. The old password is refused, the new one works.
    let report = restore_into(&rig, &older).await;
    assert_eq!(report.credentials_applied, 1);
    assert_eq!(
        login_status(&rig, &acct.creds, &old).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(login_status(&rig, &acct.creds, &new).await, StatusCode::OK);
    let version = rows(&rig, "SELECT cred_version FROM accounts").await;
    assert_eq!(field(&version[0], "cred_version"), 2);
    assert_eq!(
        count(&rig, "credential_changes").await,
        1,
        "and the journal is kept"
    );

    // Into a clean installation beside the newer backup: the journal of the neighbour has the change.
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &older).await;
    assert_eq!(report.credentials_applied, 1);
    assert_eq!(
        login_status(&clean, &acct.creds, &old).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login_status(&clean, &acct.creds, &new).await,
        StatusCode::OK
    );

    // The newer backup carries the change itself, and it has nothing to apply.
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &newer).await;
    assert_eq!(report.credentials_applied, 0);
    assert_eq!(
        login_status(&clean, &acct.creds, &old).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login_status(&clean, &acct.creds, &new).await,
        StatusCode::OK
    );

    // The honest limit, as for deletions: the older backup alone, with no journal anywhere, is the old password.
    let alone_root = rig.db.scratch_dir("alone").expect("dir");
    copy_dir(&older, &alone_root.join("backup-alone"));
    let clean = clean_like(&rig).await;
    restore_into(&clean, &alone_root.join("backup-alone")).await;
    assert_eq!(
        login_status(&clean, &acct.creds, &old).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn new_recovery_words_after_a_backup_are_not_the_old_words_again() {
    // New words are a change of credentials too (the old recovery key is the one that may have leaked): after a restore the old words do
    // not work and the new ones do. The journal entry is the whole credential state, so the password is the one it was.
    let rig = rig().await;
    let acct = account(&rig).await;
    let (dir, _) = take(&rig).await;
    rig.clock.advance(DAY);
    let (new_rc, wrap) = (array32(), random_wrap());
    let (status, answer) = rig
        .http
        .post(
            "/v1/words/reissue",
            Some(&acct.token),
            &json!({"auth_key": b64(&acct.creds.auth_key), "new_rc_auth_key": b64(&new_rc), "wrap_rc": b64(&wrap)}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let report = restore_into(&rig, &dir).await;
    assert_eq!(report.credentials_applied, 1);
    let fetch = |key: [u8; 32]| {
        let body = json!({"login": acct.creds.login, "rc_auth_key": b64(&key)});
        let rig = &rig;
        async move { rig.http.post("/v1/recovery/fetch", None, &body).await.0 }
    };
    assert_eq!(
        fetch(acct.creds.rc_auth_key).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(fetch(new_rc).await, StatusCode::OK);
    assert_eq!(
        login_status(&rig, &acct.creds, &acct.creds.auth_key).await,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_branch_cut_from_an_older_backup_than_the_one_restored_now_is_placed_by_its_fork() {
    // The S4a scenario `review_round_3_a_branch_cut_from_an_older_backup_than_the_one_restored_now`, over HTTP with a real dump.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    let (older, older_manifest) = take(&rig).await;
    let root = older.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &acct, 1, 51, &purge).await;
    put(&rig, &acct, 2, 52, &msg(&maker, 2, 52, T0)).await;
    rig.clock.advance(181 * DAY);
    compact_now(&rig, &acct).await;
    rig.clock.advance(1);
    let (newer, newer_manifest) = take_into(&rig, &root).await;
    assert!(older_manifest.cut_ms < newer_manifest.cut_ms);

    // The installation is restored from the older backup: a branch, G2, cut at the older backup's time. A device enters it from 0.
    let installation = clean_like(&rig).await;
    restore_into(&installation, &older).await;
    let branch = generation(&installation).await;
    let device = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let (_, answer) = pull(&installation, &device, 0, 0, 0, 10).await;
    assert_eq!(answer["result"], "generation_changed");
    assert_eq!(answer["generation"], branch.to_string());
    let lineage = answer["lineage"].clone();
    assert_eq!(lineage[1]["cut_ms"], older_manifest.cut_ms);

    // Then the newer backup is restored over it: the lineage is [G1, G3] and G2 is abandoned.
    let report = restore_into(&installation, &newer).await;
    let current = report.generation;
    assert_ne!(current, branch);
    let server = rows(
        &installation,
        "SELECT generation::text AS g FROM lineage ORDER BY pos",
    )
    .await;
    assert_eq!(server.len(), 2, "{server:?}");
    assert!(
        !server.iter().any(|l| l.contains(&branch.to_string())),
        "the branch is not in the lineage"
    );
    let device = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };

    // The device on the branch is told it missed the drop: the drop was compacted after the branch's fork. Sending no lineage is answered
    // the same way (the safe side), and a branch forked after the drop has missed nothing.
    let ask = |lineage: Option<Value>, after: u64| {
        let mut body = json!({"generation": branch.to_string(), "after": after, "known_epoch": 0});
        if let Some(lineage) = lineage {
            body["lineage"] = lineage;
        }
        let (rig, device) = (&installation, &device);
        async move {
            rig.http
                .post("/v1/pull", Some(&device.token), &body)
                .await
                .1
        }
    };
    let answer = ask(Some(lineage.clone()), 1).await;
    assert_eq!(
        (answer["result"].as_str(), answer["stale"].as_bool()),
        (Some("generation_changed"), Some(true)),
        "{answer}"
    );
    assert_eq!(answer["generation"], current.to_string());
    assert_eq!(
        answer["lineage"][1]["cut_ms"], newer_manifest.cut_ms,
        "the new generation was cut from the newer backup"
    );
    assert_eq!(ask(None, 1).await["stale"], true);
    let after_the_drop = json!([
        {"generation": lineage[0]["generation"], "cut_ms": 0},
        {"generation": branch.to_string(), "cut_ms": newer_manifest.cut_ms},
    ]);
    assert_eq!(ask(Some(after_the_drop), 1).await["stale"], false);
    assert_eq!(
        ask(Some(lineage), 0).await["stale"],
        false,
        "a pull from 0 is never stale"
    );
    // A lineage that is too long or not made of numbers is a 400.
    let long: Vec<Value> = (0..65)
        .map(|n| json!({"generation": n.to_string(), "cut_ms": 0}))
        .collect();
    for bad in [
        Value::Array(long),
        json!([{"generation": "x", "cut_ms": 0}]),
        json!([{"generation": "1"}]),
        json!("nope"),
    ] {
        let (status, _) = installation
            .http
            .post(
                "/v1/pull",
                Some(&device.token),
                &json!({"generation": "1", "after": 1, "known_epoch": 0, "lineage": bad}),
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    // A lineage that does not end with the cursor's generation is malformed too (round 2 of the review): it is not the lineage that put
    // the device into that generation, and the server does not guess which of the two is meant.
    let ends_elsewhere = json!([{"generation": "7", "cut_ms": 0}]);
    let (status, answer) = installation
        .http
        .post(
            "/v1/pull",
            Some(&device.token),
            &json!({"generation": "8", "after": 1, "known_epoch": 0, "lineage": ends_elsewhere}),
        )
        .await;
    assert_eq!(
        (status, answer["error"].as_str()),
        (StatusCode::BAD_REQUEST, Some("lineage"))
    );

    // A push carries no lineage, its cursor has three parts, and a pusher whose generation is not the current one is stale whatever
    // it says: it pulls first (appendix §A.3).
    let item_op = id(70);
    let fresh = msg(&maker, 3, 70, T0 + 182 * DAY);
    let item = Item {
        entity: "message",
        id: &id(3),
        op: &item_op,
        bytes: &fresh,
    };
    let probe = |cursor: String| {
        let (rig, device, item) = (&installation, &device, &item);
        async move { push_with(rig, device, &cursor, Some("1"), item).await }
    };
    let (status, answer) = probe(format!("{branch}.1.0")).await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    assert_eq!(outcome(&answer["results"][0]), "stale");
    assert_eq!(
        probe(format!("{branch}.1.0.5")).await.0,
        StatusCode::BAD_REQUEST
    );
    let (_, answer) = probe(format!("{current}.3.1")).await;
    assert_eq!(
        outcome(&answer["results"][0]),
        "ok",
        "a pusher in the current generation, up to date"
    );
}

#[tokio::test]
async fn a_branch_that_was_itself_restored_is_placed_by_where_it_forked_and_not_by_its_own_cut() {
    // The round 1 critical of the guardian, on real dumps. B1 is cut; a message X is purged and the drop K is compacted; B2 is cut and holds
    // it. B1 is restored (generation G1), a backup B3 of that branch is cut after K and restored (G2): the device enters G2 from 0 and has X
    // alive. Then B2 is restored: the server's lineage is [G0, G3]. The device's own cut time (B3's) is after K; it forked from the server's
    // history at G1, cut at B1, before K, so it has missed K and X must not be pushed back.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    let (b1, b1_manifest) = take(&rig).await;
    let root = b1.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &acct, 1, 51, &purge).await;
    put(&rig, &acct, 2, 52, &msg(&maker, 2, 52, T0)).await;
    rig.clock.advance(181 * DAY);
    compact_now(&rig, &acct).await;
    let drop_time = field(
        &rows(&rig, "SELECT compacted_ms FROM drops").await[0],
        "compacted_ms",
    );
    rig.clock.advance(1);
    let (b2, _) = take_into(&rig, &root).await;

    let installation = clean_like(&rig).await;
    let g1 = restore_into(&installation, &b1).await.generation;
    let (b3, b3_manifest) = take_into(&installation, &root).await;
    assert!(b3_manifest.cut_ms > drop_time, "B3 is cut after K");
    let g2 = restore_into(&installation, &b3).await.generation;
    let session = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let (_, entered) = pull(&installation, &session, 0, 0, 0, 10).await;
    assert_eq!(entered["generation"], g2.to_string());
    let lineage = entered["lineage"].clone();
    assert_eq!(
        lineage.as_array().map(Vec::len),
        Some(3),
        "[G0, G1, G2]: {lineage}"
    );
    assert_eq!(lineage[1]["generation"], g1.to_string());
    assert_eq!(lineage[1]["cut_ms"], b1_manifest.cut_ms);
    assert_eq!(lineage[2]["cut_ms"], b3_manifest.cut_ms);

    restore_into(&installation, &b2).await;
    let session = Acct {
        token: login_token(&installation, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let ask = |lineage: Value| {
        let body =
            json!({"generation": g2.to_string(), "after": 1, "known_epoch": 0, "lineage": lineage});
        let (rig, session) = (&installation, &session);
        async move {
            rig.http
                .post("/v1/pull", Some(&session.token), &body)
                .await
                .1
        }
    };
    let answer = ask(lineage.clone()).await;
    assert_eq!(answer["result"], "generation_changed", "{answer}");
    assert_eq!(
        answer["stale"], true,
        "the fork is G1, cut before K: K is missed"
    );
    // What the first rule said, with only the device's own generation and its own cut time (after K): not stale. That was the hole.
    // A lineage cut down to the device's own entry shares nothing with the server's and names no fork (round 2 of the review): cut 0,
    // everything missed, the safe side.
    let own_only = json!([lineage[2].clone()]);
    assert_eq!(
        ask(own_only).await["stale"],
        true,
        "a lineage cut down to the device's own entry cannot be placed"
    );
    // The fork is found only past the first shared generation: the same lineage without its shared head is not placed late either.
    let without_head = json!([lineage[1].clone(), lineage[2].clone()]);
    assert_eq!(ask(without_head).await["stale"], true);
}

#[tokio::test]
async fn the_lineage_is_capped_and_a_device_that_fell_off_it_is_told_it_missed_everything() {
    // Every restore appends a generation; the server keeps the newest 64 (`rules::pull::MAX_LINEAGE`, ADR-0001 §6.3) and a device sends
    // its lineage with every pull. A device whose lineage shares nothing with the server's any more has missed every drop.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &acct, 1, 51, &purge).await;
    rig.clock.advance(181 * DAY);
    compact_now(&rig, &acct).await;
    // A device that got the first generation, with the lineage it was given, and a server that has seen 70 restores since (the rows a
    // backup of such a server holds; making 70 real restores would take a minute for the same state).
    let first = generation(&rig).await;
    // Its cut time is not zero and in the future of every drop: taken as the fork it would say "nothing missed". A lineage that shares
    // nothing with the server's names no fork at all (round 2 of the review), whatever its first entry's cut time is.
    let lineage = json!([{"generation": first.to_string(), "cut_ms": 4_000_000_000_000u64}]);
    let client = rig.state.pool.get().await.expect("connection");
    client
        .batch_execute(
            "INSERT INTO lineage (pos, generation, cut_ms) SELECT n, 1000000 + n, n FROM generate_series(2, 71) AS n",
        )
        .await
        .expect("seed the lineage");
    drop(client);
    let (dir, _) = take(&rig).await;

    restore_into(&rig, &dir).await;
    let kept = rows(&rig, "SELECT generation FROM lineage ORDER BY pos").await;
    assert_eq!(
        kept.len(),
        64,
        "the newest 64 generations of the 72: {}",
        kept.len()
    );
    assert!(
        !kept.iter().any(|g| g.contains(&first.to_string())),
        "and the first one has fallen off"
    );
    let session = Acct {
        token: login_token(&rig, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let (status, answer) = rig
        .http
        .post(
            "/v1/pull",
            Some(&session.token),
            &json!({"generation": first.to_string(), "after": 1, "known_epoch": 0, "lineage": lineage}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["result"], "generation_changed");
    assert_eq!(answer["stale"], true, "{answer}");
    assert_eq!(answer["lineage"].as_array().map(Vec::len), Some(64));
}

#[tokio::test]
async fn a_restore_refuses_to_run_under_a_server_and_runs_when_it_is_gone() {
    let rig = rig().await;
    let acct = account(&rig).await;
    seed(&rig, &acct, &Maker::new(), 2).await;
    let (dir, _) = take(&rig).await;
    let before = rows(&rig, "SELECT * FROM accounts").await;
    let held = ServerLock::acquire(&rig.db.pg_config())
        .await
        .expect("the server lock");
    // Another server on the same database is fine (the lock is shared).
    let second = ServerLock::acquire(&rig.db.pg_config())
        .await
        .expect("a second server");
    drop(second);

    let refused = backup::restore(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, &dir)
        .await
        .expect_err("a restore under a server");
    assert_eq!(refused, BackupError::ServerRunning);
    assert_eq!(rows(&rig, "SELECT * FROM accounts").await, before);
    assert_eq!(
        generation_count(&rig).await,
        1,
        "no generation was appended"
    );

    // The server stops: its connection ends and the lock with it.
    drop(held);
    let mut done = false;
    for _ in 0..50 {
        if backup::restore(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, &dir)
            .await
            .is_ok()
        {
            done = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(done, "the restore runs once the server is gone");
    assert_eq!(generation_count(&rig).await, 2);
}

async fn generation_count(rig: &Rig) -> usize {
    rows(rig, "SELECT 1 FROM lineage").await.len()
}

async fn accounts_left(rig: &Rig) -> Vec<String> {
    rows(rig, "SELECT id FROM accounts ORDER BY id")
        .await
        .iter()
        .map(|row| row.split('"').nth(3).expect("an id").to_string())
        .collect()
}

fn copy_dir(from: &Path, to: &Path) {
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
