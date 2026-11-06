//! What surrounds a backup (ADR-0020): the backup lock does what `store::lock` says, the clocks cannot put a stamp below a cut time, rotation
//! keeps the newest and leaves what is not a backup alone, and nothing in the blob store or the backup directory is plain text.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use common::backup::*;
use common::blobs::*;
use common::leak::{Secret, find_leaks};
use common::*;
use kinige_server::Clock;
use kinige_server::backup::{self, Manifest};
use kinige_server::config::Config;
use kinige_server::store::{compact, lock};
use kinige_server::testing::{BlobBackend, Creds, blob_backends};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_backup_lock_waits_for_a_running_compaction_and_a_compaction_waits_for_a_backup() {
    let rig = Arc::new(rig().await);
    let acct = account(&rig).await;
    let maker = Maker::new();
    seed(&rig, &acct, &maker, 4).await;

    // A running compaction (it holds the shared lock): the backup cannot read its cut time until the compaction has committed.
    let mut holder = rig.state.pool.get().await.expect("connection");
    let running = holder.transaction().await.expect("tx");
    lock::shared(&running).await.expect("shared");
    let root = rig.db.scratch_dir("lock").expect("dir");
    let task = {
        let (rig, root) = (rig.clone(), root.clone());
        tokio::spawn(async move { take_into(&rig, &root).await })
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !task.is_finished(),
        "the backup waits for the compaction that is running"
    );
    rig.clock.advance(5000);
    running.commit().await.expect("commit");
    let (_, manifest) = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .expect("finished")
        .expect("joined");
    assert_eq!(
        manifest.cut_ms,
        rig.clock.now_ms(),
        "the cut time is read after the lock is held"
    );

    // The other way round: while the backup holds the lock a compaction does not start, so its stamp is read after the backup let go.
    let guard = rig.state.pool.get().await.expect("connection");
    lock::exclusive(&guard).await.expect("exclusive");
    let compaction = {
        let (rig, id) = (rig.clone(), acct.creds.account_id.clone());
        tokio::spawn(async move { compact::account(&rig.state.pool, &id, &*rig.clock).await })
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !compaction.is_finished(),
        "a compaction waits for a backup that holds the lock"
    );
    lock::release(&guard).await.expect("release");
    tokio::time::timeout(Duration::from_secs(10), compaction)
        .await
        .expect("finished")
        .expect("joined")
        .expect("compacted");
}

#[tokio::test]
async fn nothing_is_stamped_below_the_newest_cut_time_of_the_lineage_whatever_the_clock_says() {
    // Round 1 of the review: the stamp of a drop, a deletion or a credential change, and the cut time of a backup, come from the
    // database's clock or the injected one, and are raised above the newest cut time of the lineage. Here a clean installation is
    // restored on a host whose clock is 400 days behind the backup's.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    seed(&rig, &acct, &maker, 8).await;
    let (dir, manifest) = take(&rig).await;
    let cut = manifest.cut_ms;

    let behind = clean_like(&rig).await;
    behind.clock.set(cut - 400 * DAY);
    restore_into(&behind, &dir).await;
    assert!(behind.clock.now_ms() < cut, "the clock is behind the cut");

    // A backup taken there is cut above the lineage's newest cut.
    let (_, later) = take(&behind).await;
    assert_eq!(
        later.cut_ms,
        cut + 1,
        "the floor: one more than the newest cut"
    );

    // A compaction there: the purge was pushed at the clock that is behind, long before the cut, and the drop is stamped above the cut.
    let session = Acct {
        token: login_token(&behind, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    let wall = behind.clock.now_ms();
    put(&behind, &session, 40, 140, &msg(&maker, 40, 140, wall)).await;
    let purge = maker.purged("message", &id(40), &id(141), &[("body", hlc(wall, 0))]);
    put(&behind, &session, 40, 141, &purge).await;
    // 181 days of that host's own time pass (it is still 219 days behind the cut): the purge is old enough by the clock, and the drop is
    // stamped by the floor, above the cut (round 2: the age is the clock's, the stamp is the floor's).
    behind.clock.advance(181 * DAY);
    assert!(
        behind.clock.now_ms() < cut,
        "the clock is still behind the cut"
    );
    compact_now(&behind, &session).await;
    let drops = rows(&behind, "SELECT compacted_ms FROM drops ORDER BY epoch").await;
    assert_eq!(drops.len(), 2, "the drop of the seed and the new one");
    assert!(field(&drops[1], "compacted_ms") > cut, "{drops:?} vs {cut}");

    // A password change there, on another account.
    let other = Creds::random();
    let token = other.register(&behind.http).await.expect("registration");
    let other = Acct {
        creds: other,
        token,
    };
    let key = other.creds.auth_key;
    change_password(&behind, &other, 1, &key).await;
    let changes = rows(&behind, "SELECT changed_ms FROM credential_changes").await;
    assert_eq!(changes.len(), 1);
    assert!(field(&changes[0], "changed_ms") > cut);

    // A deletion there.
    let (status, _) = delete_account(&behind, &session, &acct.creds.auth_key).await;
    assert_eq!(status, axum::http::StatusCode::OK);
    let deleted = rows(&behind, "SELECT deleted_ms FROM deletions").await;
    assert!(field(&deleted[0], "deleted_ms") > cut, "{deleted:?}");
}

#[tokio::test]
async fn a_restore_from_the_future_does_not_make_everything_old_at_once() {
    // Round 2 of the review: the floor under the stamps is for the three things compared with a cut time. If it also fed the age of a
    // `purged`, a restore from a backup cut 1000 days ahead of this host's clock would make every purge 180 days old at once, and
    // compaction would drop what devices still need to be told about.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    put(&rig, &acct, 1, 50, &msg(&maker, 1, 50, T0)).await;
    let purge = maker.purged("message", &id(1), &id(51), &[("body", hlc(T0, 0))]);
    put(&rig, &acct, 1, 51, &purge).await;
    rig.clock.set(T0 + 1000 * DAY);
    let (dir, manifest) = take(&rig).await;
    assert!(manifest.cut_ms >= T0 + 1000 * DAY);

    let behind = clean_like(&rig).await;
    behind.clock.set(T0 + 10 * DAY);
    restore_into(&behind, &dir).await;
    let session = Acct {
        token: login_token(&behind, &acct.creds).await,
        creds: acct.creds.clone(),
    };

    // 11 days after the purge, by this host's clock: nothing to drop. The floor sits at the cut, 1000 days ahead.
    behind.clock.set(T0 + 11 * DAY);
    let stats = compact_now(&behind, &session).await;
    assert!(!stats.dropped, "a purge 11 days old is not dropped");
    assert_eq!(count(&behind, "drops").await, 0);
    assert_eq!(log_seqs(&behind, &session).await.len(), 2);

    // After 180 days of real time it is dropped, and the drop is stamped by the floor, above the cut.
    behind.clock.set(T0 + 182 * DAY);
    let stats = compact_now(&behind, &session).await;
    assert!(stats.dropped);
    let drops = rows(&behind, "SELECT compacted_ms FROM drops").await;
    assert!(field(&drops[0], "compacted_ms") > manifest.cut_ms);
}

#[tokio::test]
async fn rotation_removes_backups_older_than_the_retention_and_keeps_the_newest_and_what_is_not_a_backup()
 {
    let rig = rig().await;
    let acct = account(&rig).await;
    seed(&rig, &acct, &Maker::new(), 5).await;
    let root = rig.db.scratch_dir("rotation").expect("dir");
    let mut made = Vec::new();
    for _ in 0..3 {
        made.push(take_into(&rig, &root).await);
        rig.clock.advance(20 * DAY);
    }
    std::fs::create_dir(root.join("notes")).expect("dir");
    std::fs::write(root.join("notes/readme.txt"), "not a backup").expect("file");
    std::fs::create_dir(root.join("backup-looks-like-one")).expect("dir");
    std::fs::create_dir(root.join(".tmp-abc")).expect("dir");
    let now = rig.clock.now_ms();
    let retention = 30 * DAY;
    // The backups were cut 60, 40 and 20 days ago.
    let pruned = backup::prune(&root, now, retention).await.expect("prune");
    assert_eq!(pruned.backups.len(), 2, "{pruned:?}");
    let left: Vec<String> = std::fs::read_dir(&root)
        .expect("dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert!(
        left.contains(
            &made[2]
                .0
                .file_name()
                .expect("name")
                .to_string_lossy()
                .into_owned()
        )
    );
    assert!(
        left.contains(&"notes".to_string()) && left.contains(&"backup-looks-like-one".to_string())
    );
    assert!(
        left.contains(&".tmp-abc".to_string()),
        "a fresh half-written directory is left alone"
    );

    // Far in the future, everything is older than the retention, but the newest backup stays: it is kept until the next one is taken.
    let pruned = backup::prune(&root, now + 400 * DAY, retention)
        .await
        .expect("prune");
    assert_eq!(pruned.backups.len(), 0, "{pruned:?}");
    assert!(made[2].0.exists());
    assert_eq!(
        Manifest::read(&made[2].0).await.expect("manifest").cut_ms,
        made[2].1.cut_ms
    );
}

#[tokio::test]
async fn nothing_in_the_blob_store_or_the_backup_directory_is_plain_text() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        let maker = Maker::new();
        // Entries sealed with the synthetic key carry a text; the blob is a real sealed file of a known plaintext.
        let text = "fake entry about a fake backup journey";
        let envelope = maker.upsert("message", &id(1), &id(50), &[("body", hlc(T0, 0))], text);
        put(&rig, &acct, 1, 50, &envelope).await;
        let plaintext = plain(6000, 9);
        let (blob_id, blob) = (blob_uuid(1), sealed(6000, 9));
        upload_all(&rig, &acct, &blob_id, &blob).await;
        // The credential journal is in the backup: a changed password writes a row, and no key is in it, only hashes and wraps.
        let old_key = acct.creds.auth_key;
        let new_key = change_password(&rig, &acct, 1, &old_key).await;
        let (dir, _) = take(&rig).await;

        let secrets = [
            Secret::text("entry text", text),
            Secret::new("plaintext of the blob", plaintext.get(..64).expect("bytes")),
            Secret::new("blob key", &[9u8; 32]),
            Secret::new("auth_key", &old_key),
            Secret::new("new auth_key", &new_key),
            Secret::new("rc_auth_key", &acct.creds.rc_auth_key),
            Secret::new("dek", maker.dek().key().as_bytes()),
        ];
        // Three places: the backup directory, the object store (read through its interface, so an S3 store is scanned too) and the
        // staging area (an unfinished upload is a file).
        upload(&rig, &acct, &blob_uuid(2), &sealed(6000, 9), 0, 700).await;
        let mut texts: Vec<(String, String)> = Vec::new();
        let mut scan = |name: String, bytes: Vec<u8>| {
            // Binary files are scanned as text with every non-UTF-8 byte replaced: the encodings searched for are all ASCII.
            texts.push((name, String::from_utf8_lossy(&bytes).into_owned()));
        };
        for relative in files_under(&dir) {
            scan(
                format!("backup/{relative}"),
                std::fs::read(dir.join(&relative)).expect("file"),
            );
        }
        for relative in files_under(rig.state.blobs.staging.dir()) {
            scan(
                format!("staging/{relative}"),
                std::fs::read(rig.state.blobs.staging.dir().join(&relative)).expect("file"),
            );
        }
        let scratch = rig.db.scratch_dir("objects").expect("dir");
        let listed = rig.state.blobs.objects.list().await.expect("list");
        assert_eq!(listed.len(), 1);
        for object in listed {
            let file = scratch.join("object");
            rig.state
                .blobs
                .objects
                .download(&object.key, &file)
                .await
                .expect("download");
            scan(
                format!("store/{}", object.key),
                std::fs::read(&file).expect("file"),
            );
        }
        assert!(
            texts.len() > 10,
            "the scan looked at the files: {}",
            texts.len()
        );
        for (name, text) in &texts {
            assert!(
                find_leaks(text, &secrets).is_empty(),
                "{name}: {:?} ({backend:?})",
                find_leaks(text, &secrets)
            );
        }
        // The log of the run (uploads, a backup) names no blob, no account and no token: events and error categories only.
        let log = logged_here().join("\n");
        assert!(
            log.contains("blob published") && log.contains("backup written"),
            "the run logged its events"
        );
        for (name, value) in [
            ("blob id", blob_uuid(1)),
            ("account id", acct.creds.account_id.clone()),
            ("token", acct.token.clone()),
        ] {
            assert!(!log.contains(&value), "the log holds a {name}");
        }
        // Positive control: the blob's plaintext, were it there, is found.
        let planted = String::from_utf8_lossy(&plaintext).into_owned();
        assert_eq!(
            find_leaks(&planted, &secrets),
            vec!["plaintext of the blob"]
        );
    }
}

fn backends() -> Vec<BlobBackend> {
    blob_backends().expect("the blob backends")
}
