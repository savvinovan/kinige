//! Backup and restore (PRD §10.1, ADR-0001 §6.3, ADR-0020): a backup restores into a clean installation and in place, a blob is never lost
//! while a backup refers to it, a backup taken during writes restores and its cut time separates the drops it holds from those it does not,
//! the backup lock does what `store::lock` says, rotation keeps the newest, and nothing in the blob store or the backup directory is plain
//! text. Every scenario that touches blobs runs against the disk store and, with `KINIGE_TEST_S3_URL`, an S3-compatible store.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::blobs::*;
use common::leak::{Secret, find_leaks};
use common::*;
use kinige_server::Clock;
use kinige_server::backup::{self, Manifest};
use kinige_server::blobs::sweep;
use kinige_server::config::Config;
use kinige_server::store::{compact, lock};
use kinige_server::testing::{BlobBackend, blob_backends};
use std::sync::Arc;
use std::time::Duration;

/// The blobs the writer of a scenario uploaded: `(blob id, bytes)`.
type Uploaded = Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;

fn backends() -> Vec<BlobBackend> {
    blob_backends().expect("the blob backends")
}

#[tokio::test]
async fn a_backup_restores_into_a_clean_installation_and_in_place() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        let maker = Maker::new();
        let seeded = seed(&rig, &acct, &maker, 1).await;
        let (dir, manifest) = take(&rig).await;

        // The directory holds what the manifest says and nothing else: a dump per table, the journal, the lineage, the blob, the manifest.
        let names = files_under(&dir);
        assert!(names.contains(&"manifest.json".to_string()), "{names:?}");
        assert!(names.contains(&"tables/envelopes.copy".to_string()));
        assert!(
            names.contains(&"lineage.json".to_string())
                && names.contains(&"deletions.json".to_string())
        );
        let blob_file = format!("blobs/{}/{}", acct.creds.account_id, seeded.blob_id);
        assert!(names.contains(&blob_file), "{names:?}");
        assert_eq!(
            names.len(),
            manifest.files.len() + manifest.blobs.len() + 1,
            "no stray file"
        );
        assert_eq!(manifest.cut_ms, kinige_server::Clock::now_ms(&*rig.clock));
        assert_eq!(
            std::fs::read(dir.join(&blob_file)).expect("blob"),
            seeded.blob
        );
        manifest
            .verify(&dir)
            .await
            .expect("the files match the manifest");
        assert!(
            !files_under(dir.parent().expect("root"))
                .iter()
                .any(|f| f.starts_with(".tmp-")),
            "no unfinished directory"
        );

        // A clean installation: a new database, an empty store, nobody logged in.
        let clean = clean_like(&rig).await;
        let report = restore_into(&clean, &dir).await;
        assert_eq!(
            (report.cut_ms, report.deletions_applied, report.orphans),
            (manifest.cut_ms, 0, 0)
        );
        for sql in [
            "SELECT * FROM accounts ORDER BY id",
            "SELECT * FROM envelopes ORDER BY account_id, seq",
            "SELECT * FROM ops ORDER BY account_id, op_id",
            "SELECT * FROM entity_known ORDER BY 1, 2, 3, 4",
            "SELECT * FROM purged_entities ORDER BY 1, 2, 3",
            "SELECT * FROM drops ORDER BY 1, 2, 3",
            "SELECT * FROM blobs ORDER BY 1, 2",
        ] {
            assert_eq!(
                rows(&clean, sql).await,
                rows(&rig, sql).await,
                "{sql} ({backend:?})"
            );
        }
        assert_eq!(
            count(&clean, "sessions").await,
            0,
            "a restore brings back no session"
        );
        assert_eq!(
            rows(&clean, "SELECT pos, cut_ms FROM lineage ORDER BY pos")
                .await
                .len(),
            2,
            "the lineage got a generation"
        );
        let (old_gen, new_gen) = (generation(&rig).await, generation(&clean).await);
        assert_ne!(old_gen, new_gen);
        let new_row = rows(&clean, "SELECT cut_ms FROM lineage WHERE pos = 2").await;
        assert_eq!(new_row, vec![format!("{{\"cut_ms\":{}}}", manifest.cut_ms)]);

        // The old session is gone, a login works, the blob downloads byte for byte, and an old cursor is told the generation changed.
        let old = Acct {
            creds: acct.creds.clone(),
            token: acct.token.clone(),
        };
        assert_eq!(
            download(&clean, &old, &seeded.blob_id, None).await.0,
            StatusCode::UNAUTHORIZED
        );
        let fresh = Acct {
            token: login_token(&clean, &acct.creds).await,
            creds: acct.creds.clone(),
        };
        assert_eq!(
            download(&clean, &fresh, &seeded.blob_id, None).await.2,
            seeded.blob
        );
        let (code, answer) = pull(&clean, &fresh, old_gen, 2, 0, 10).await;
        assert_eq!(code, StatusCode::OK);
        assert_eq!(answer["result"], "generation_changed", "{answer}");
        assert_eq!(answer["generation"], new_gen.to_string());
        assert_eq!(answer["cut_ms"], manifest.cut_ms);

        // In place: what came after the backup is gone, a blob published after it becomes a deleted blob that goes the delayed way, and the
        // sequence numbers are issued again from the backup's head.
        let late = blob_uuid(7);
        let late_blob = sealed(3000, 7);
        upload_all(&rig, &acct, &late, &late_blob).await;
        put(&rig, &acct, 9, 190, &msg(&maker, 9, 190, T0 + 181 * DAY)).await;
        let head_before = head_of(&rig).await;
        let report = restore_into(&rig, &dir).await;
        assert_eq!(report.orphans, 1);
        assert_eq!(count(&rig, "sessions").await, 0);
        assert_eq!(
            download(&rig, &acct, &seeded.blob_id, None).await.0,
            StatusCode::UNAUTHORIZED
        );
        let acct = Acct {
            token: login_token(&rig, &acct.creds).await,
            creds: acct.creds.clone(),
        };
        assert_eq!(
            head_of(&rig).await,
            head_of(&clean).await,
            "the head is the backup's"
        );
        assert_eq!(
            head_of(&rig).await + 1,
            head_before,
            "and the lost envelope's seq is free to be issued again"
        );
        assert_eq!(
            download(&rig, &acct, &seeded.blob_id, None).await.2,
            seeded.blob,
            "{backend:?}"
        );
        assert_eq!(
            download(&rig, &acct, &late, None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(status(&rig, &acct, &late).await["state"], "deleted");
        assert!(
            object_size(&rig, &acct, &late).await.is_some(),
            "the object waits out the retention"
        );
        let result = put(&rig, &acct, 9, 190, &msg(&maker, 9, 190, T0 + 181 * DAY)).await;
        assert_eq!(
            outcome(&result),
            "ok",
            "the push of a lost envelope is accepted again"
        );
        rig.clock.advance(31 * DAY);
        let config = rig.state.config.clone();
        sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
            .await
            .expect("sweep");
        assert!(object_size(&rig, &acct, &late).await.is_none());
        assert!(
            object_size(&rig, &acct, &seeded.blob_id).await.is_some(),
            "the backed-up blob stays"
        );
    }
}

async fn head_of(rig: &Rig) -> i64 {
    let rows = rows(rig, "SELECT seq_head FROM accounts").await;
    let text = rows.first().expect("a row");
    text.trim_matches(|c| !char::is_ascii_digit(&c))
        .parse()
        .expect("a number")
}

#[tokio::test]
async fn a_blob_deleted_after_a_backup_is_still_there_for_that_backup_until_it_is_rotated() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        let maker = Maker::new();
        let seeded = seed(&rig, &acct, &maker, 2).await;
        let (dir, _) = take(&rig).await;
        let config = rig.state.config.clone();

        // The blob is deleted an hour after the backup. The backup is rotated after 30 days; the object stays 30 days and one interval.
        rig.clock.advance(3_600_000);
        assert_eq!(delete(&rig, &acct, &seeded.blob_id).await, StatusCode::OK);
        rig.clock.advance(30 * DAY - 3_600_000);
        let swept = sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
            .await
            .expect("sweep");
        assert_eq!(swept.removed, 0, "the backup is still within its retention");
        let report = restore_into(&rig, &dir).await;
        assert_eq!(report.orphans, 0);
        let acct = Acct {
            token: login_token(&rig, &acct.creds).await,
            creds: acct.creds.clone(),
        };
        assert_eq!(
            download(&rig, &acct, &seeded.blob_id, None).await.2,
            seeded.blob,
            "{backend:?}"
        );

        // Deleted again, it goes once the retention and one backup interval are over.
        assert_eq!(delete(&rig, &acct, &seeded.blob_id).await, StatusCode::OK);
        rig.clock.advance(31 * DAY);
        let swept = sweep::run(&rig.state.pool, &rig.state.blobs, &config, &*rig.clock)
            .await
            .expect("sweep");
        assert_eq!(swept.removed, 1);
    }
}

#[tokio::test]
async fn a_restore_drops_objects_no_restored_row_refers_to_through_the_same_delayed_path() {
    // Covered in the first scenario for one orphan; here a clean installation restored over a store that already holds objects of another
    // installation (a shared bucket reused by mistake): they are adopted as deleted blobs, not served and not removed at once.
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    seed(&rig, &acct, &maker, 3).await;
    let (dir, _) = take(&rig).await;
    let clean = clean_like(&rig).await;
    let stranger = "00000009-0000-4000-8000-000000000009/00000009-0000-4000-8000-00000000000a";
    let file = clean.db.scratch_dir("stranger").expect("dir").join("f");
    std::fs::write(&file, sealed(2000, 1)).expect("file");
    clean
        .state
        .blobs
        .objects
        .put_file(stranger, &file)
        .await
        .expect("put");
    let report = restore_into(&clean, &dir).await;
    assert_eq!(report.orphans, 1);
    assert!(
        clean
            .state
            .blobs
            .objects
            .size(stranger)
            .await
            .expect("size")
            .is_some()
    );
    let marked = rows(&clean, "SELECT account_id, deleted_ms IS NOT NULL AS deleted FROM blobs WHERE account_id LIKE '00000009%'").await;
    assert_eq!(marked.len(), 1);
    assert!(marked[0].contains("true"), "{marked:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_backup_taken_during_writes_restores_and_its_cut_time_separates_the_drops() {
    for backend in backends() {
        let rig = Arc::new(rig_on(Config::default(), backend).await);
        let acct = Arc::new(account(&rig).await);
        let maker = Arc::new(Maker::new());
        let root = rig.db.scratch_dir("during").expect("dir");
        let known: Uploaded = Arc::default();

        // Writers: envelopes, blobs published and deleted, purges, and compactions with the clock jumping, all while backups are taken.
        let writer = {
            let (rig, acct, maker, known) =
                (rig.clone(), acct.clone(), maker.clone(), known.clone());
            tokio::spawn(async move {
                for k in 0u8..36 {
                    let n = 10 + k;
                    put(&rig, &acct, n, n + 100, &msg(&maker, n, n + 100, T0)).await;
                    let blob = sealed(3000 + usize::from(k), k);
                    let blob_id = blob_uuid(k);
                    upload_all(&rig, &acct, &blob_id, &blob).await;
                    known.lock().expect("lock").push((blob_id.clone(), blob));
                    if k % 3 == 2 {
                        let purge = maker.purged(
                            "message",
                            &id(n - 1),
                            &id(n + 101),
                            &[("body", hlc(T0, 0))],
                        );
                        put(&rig, &acct, n - 1, n + 101, &purge).await;
                        delete(&rig, &acct, &blob_uuid(k - 1)).await;
                    }
                    if k % 5 == 4 {
                        rig.clock.advance(181 * DAY);
                        compact_now(&rig, &acct).await;
                    }
                    tokio::time::sleep(Duration::from_millis(3)).await;
                }
            })
        };
        let backups = {
            let (rig, root) = (rig.clone(), root.clone());
            tokio::spawn(async move {
                let mut taken = Vec::new();
                for _ in 0..5 {
                    taken.push(take_into(&rig, &root).await);
                    tokio::time::sleep(Duration::from_millis(15)).await;
                }
                taken
            })
        };
        writer.await.expect("writer");
        let taken = backups.await.expect("backups");

        let live_drops = rows(
            &rig,
            "SELECT generation, epoch, compacted_ms FROM drops ORDER BY epoch",
        )
        .await;
        let expected: std::collections::HashMap<String, Vec<u8>> =
            known.lock().expect("lock").iter().cloned().collect();
        assert!(!live_drops.is_empty());
        for (dir, manifest) in taken {
            manifest.verify(&dir).await.expect("verifies");
            let clean = clean_like(&rig).await;
            restore_into(&clean, &dir).await;

            // One snapshot: the head, the log and the idempotency index agree.
            let consistent = rows(&clean, "SELECT (SELECT seq_head FROM accounts) = COALESCE((SELECT max(seq) FROM ops), 0) AS head_is_last, (SELECT count(*) FROM envelopes e WHERE NOT EXISTS (SELECT 1 FROM ops o WHERE o.account_id = e.account_id AND o.seq = e.seq)) = 0 AS every_envelope_has_its_op").await;
            assert_eq!(
                consistent,
                vec!["{\"head_is_last\":true,\"every_envelope_has_its_op\":true}".to_string()],
                "{backend:?}"
            );

            // The cut time separates the drops: those in the dump were stamped at or before it, those not in it at or after (a tie is
            // possible, and the rule counts a drop stamped at the cut time as missed: the safe side).
            let held = rows(
                &clean,
                "SELECT generation, epoch, compacted_ms FROM drops ORDER BY epoch",
            )
            .await;
            for drop in &live_drops {
                let stamp = field(drop, "compacted_ms");
                if held.contains(drop) {
                    assert!(
                        stamp <= manifest.cut_ms,
                        "a drop in the backup is stamped at or before the cut: {drop} vs {}",
                        manifest.cut_ms
                    );
                } else {
                    assert!(
                        stamp >= manifest.cut_ms,
                        "a drop not in the backup is stamped at or after the cut: {drop} vs {}",
                        manifest.cut_ms
                    );
                }
            }

            // Every live blob of the dump is in the restored store, and downloads as it was uploaded.
            let session = Acct {
                token: login_token(&clean, &acct.creds).await,
                creds: acct.creds.clone(),
            };
            let live = rows(&clean, "SELECT blob_id FROM blobs WHERE deleted_ms IS NULL").await;
            assert_eq!(live.len(), manifest.blobs.len());
            for row in live {
                let blob_id = row.split('"').nth(3).expect("an id").to_string();
                let (code, _, bytes) = download(&clean, &session, &blob_id, None).await;
                assert_eq!(code, StatusCode::OK, "{blob_id} ({backend:?})");
                assert_eq!(Some(&bytes), expected.get(&blob_id));
            }
        }
    }
}

fn field(row: &str, name: &str) -> u64 {
    let after = row.split(&format!("\"{name}\":")).nth(1).expect("a field");
    after
        .trim_end_matches(['}', ' '])
        .split(',')
        .next()
        .expect("a value")
        .parse()
        .expect("a number")
}

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

    // Far in the future, everything is older than the retention, but the newest backup stays.
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
        let (dir, _) = take(&rig).await;

        let secrets = [
            Secret::text("entry text", text),
            Secret::new("plaintext of the blob", plaintext.get(..64).expect("bytes")),
            Secret::new("blob key", &[9u8; 32]),
            Secret::new("auth_key", &acct.creds.auth_key),
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
