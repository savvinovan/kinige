//! The credential journal across restores (ADR-0001 §8.9, ADR-0020): a password or recovery words changed after a backup are not the old
//! ones again, two changes of one account in the same millisecond are two entries and the later one wins, and retired credentials do not
//! stay in the journal for longer than a deleted blob stays in the store. The deletion journal is in `restore_rules.rs`.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::*;
use kinige_server::blobs::sweep;
use kinige_server::testing::{array32, random_wrap};
use serde_json::json;

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
async fn changes_in_one_millisecond_are_all_kept_and_the_latest_wins_after_a_restore() {
    // The clock does not move at all: every stamp is the same base. Before round 2 the second change had the first one's key and
    // `ON CONFLICT DO NOTHING` kept the first, so a restore put back superseded words.
    let rig = rig().await;
    let acct = account(&rig).await;
    let original = acct.creds.auth_key;
    let (dir, _) = take(&rig).await;

    let (rc1, rc2, rc3) = (array32(), array32(), array32());
    reissue(&rig, &acct, &original, &rc1).await;
    reissue(&rig, &acct, &original, &rc2).await;
    // A password change, then new words, inside the same window: the entry after the change must carry the new password and the
    // newest words together.
    let renewed = change_password(&rig, &acct, 1, &original).await;
    reissue(&rig, &acct, &renewed, &rc3).await;

    let stamps = rows(
        &rig,
        "SELECT changed_ms FROM credential_changes ORDER BY changed_ms",
    )
    .await;
    assert_eq!(stamps.len(), 4, "four changes, four entries");
    let times: Vec<u64> = stamps.iter().map(|row| field(row, "changed_ms")).collect();
    assert!(
        times.windows(2).all(|pair| pair[0] < pair[1]),
        "strictly increasing: {times:?}"
    );

    let report = restore_into(&rig, &dir).await;
    assert_eq!(
        report.credentials_applied, 1,
        "one account, its newest state"
    );
    assert_eq!(
        login_status(&rig, &acct.creds, &original).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login_status(&rig, &acct.creds, &renewed).await,
        StatusCode::OK
    );
    assert_eq!(recovery_status(&rig, &acct, &rc3).await, StatusCode::OK);
    for superseded in [acct.creds.rc_auth_key, rc1, rc2] {
        assert_eq!(
            recovery_status(&rig, &acct, &superseded).await,
            StatusCode::UNAUTHORIZED
        );
    }
}

#[tokio::test]
async fn retired_credentials_leave_the_journal_after_the_window_and_the_newest_state_stays() {
    // A password from a year ago must not unwrap a DEK from a copy of the database: the retired entries go with the same delay as a
    // deleted blob (retention plus one interval). A restore of a backup inside the window still applies its entries.
    let rig = rig().await;
    let acct = account(&rig).await;
    let original = acct.creds.auth_key;
    let (backup, _) = take(&rig).await;
    rig.clock.advance(DAY);
    let first = change_password(&rig, &acct, 1, &original).await;
    assert_eq!(count(&rig, "credential_changes").await, 1);

    // Inside the window nothing is removed, and the backup still gets its entry.
    rig.clock.advance(20 * DAY);
    let swept = run_sweep(&rig).await;
    assert_eq!(swept.journal, 0);
    let report = restore_into(&rig, &backup).await;
    assert_eq!(report.credentials_applied, 1);
    assert_eq!(
        login_status(&rig, &acct.creds, &first).await,
        StatusCode::OK
    );
    assert_eq!(
        login_status(&rig, &acct.creds, &original).await,
        StatusCode::UNAUTHORIZED
    );

    // Past the window a newer change retires the first entry; the sweep removes it. The newest entry stays: it holds what the account
    // holds now, and the newest backup is kept whatever its age.
    rig.clock.advance(40 * DAY);
    let (status, answer) = rig
        .http
        .post(
            "/v1/login",
            None,
            &json!({"login": acct.creds.login, "auth_key": b64(&first)}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
    let session = Acct {
        token: answer["token"].as_str().expect("a token").to_string(),
        creds: acct.creds.clone(),
    };
    let second = change_password(&rig, &session, 2, &first).await;
    assert_eq!(count(&rig, "credential_changes").await, 2);
    let first_wrap = rows(
        &rig,
        "SELECT encode(wrap_pw, 'hex') AS wrap FROM credential_changes ORDER BY changed_ms LIMIT 1",
    )
    .await;
    rig.clock.advance(40 * DAY);
    let swept = run_sweep(&rig).await;
    assert_eq!(swept.journal, 1, "the retired entry is gone");
    let left = rows(
        &rig,
        "SELECT cred_version, encode(wrap_pw, 'hex') AS wrap FROM credential_changes",
    )
    .await;
    assert_eq!(left.len(), 1);
    assert_eq!(field(&left[0], "cred_version"), 3);
    assert!(
        !left[0].contains(first_wrap[0].split('"').nth(3).expect("a wrap")),
        "the retired wrap is not in the table"
    );
    let swept = run_sweep(&rig).await;
    assert_eq!(
        swept.journal, 0,
        "and the newest entry is not retired by age"
    );

    // The backup is older than the window now, as the newest backup of a server whose cron stopped: its restore still lands on the
    // newest password.
    let report = restore_into(&rig, &backup).await;
    assert_eq!(report.credentials_applied, 1);
    assert_eq!(
        login_status(&rig, &acct.creds, &second).await,
        StatusCode::OK
    );
    assert_eq!(
        login_status(&rig, &acct.creds, &first).await,
        StatusCode::UNAUTHORIZED
    );
}

async fn run_sweep(rig: &Rig) -> sweep::Swept {
    sweep::run(
        &rig.state.pool,
        &rig.state.blobs,
        &rig.state.config,
        &*rig.clock,
    )
    .await
    .expect("a sweep")
}

/// New words `new_rc`, authorised by the password `auth_key`.
async fn reissue(rig: &Rig, acct: &Acct, auth_key: &[u8; 32], new_rc: &[u8; 32]) {
    let (status, answer) = rig
        .http
        .post(
            "/v1/words/reissue",
            Some(&acct.token),
            &json!({"auth_key": b64(auth_key), "new_rc_auth_key": b64(new_rc), "wrap_rc": b64(&random_wrap())}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{answer}");
}

/// The word check with `key`: 200 if it is the account's words, 401 if not.
async fn recovery_status(rig: &Rig, acct: &Acct, key: &[u8; 32]) -> StatusCode {
    rig.http
        .post(
            "/v1/recovery/fetch",
            None,
            &json!({"login": acct.creds.login, "rc_auth_key": b64(key)}),
        )
        .await
        .0
}
