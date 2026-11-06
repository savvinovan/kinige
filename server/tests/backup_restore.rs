//! Backup and restore (PRD §10.1, ADR-0001 §6.3, ADR-0020): a backup restores into a clean installation and in place, a blob is never lost
//! while a backup refers to it, an object of the right size that is not the backed-up one is repaired, and a backup taken during writes
//! restores with its cut time separating the drops it holds from those it does not. The lock, rotation, the clocks and the plain-text scan
//! are in `backup_housekeeping.rs`; the rules a restore applies are in `restore_rules.rs`; damaged backups are in `restore_garbage.rs`.
//! Every scenario that touches blobs runs against the disk store and, with `KINIGE_TEST_S3_URL`, an S3-compatible store.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::blobs::*;
use common::*;
use kinige_server::blobs::sweep;
use kinige_server::config::Config;
use kinige_server::testing::{BlobBackend, blob_backends};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test]
async fn a_backup_restores_into_a_clean_installation_and_in_place() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        let maker = Maker::new();
        let seeded = seed(&rig, &acct, &maker, 1).await;
        let (dir, manifest) = take(&rig).await;

        // The directory holds what the manifest says and nothing else: a dump per table, the journals, the lineage, the blob, the manifest.
        let names = files_under(&dir);
        for expected in [
            "manifest.json",
            "tables/envelopes.copy",
            "lineage.json",
            "deletions.json",
            "credentials.json",
        ] {
            assert!(
                names.contains(&expected.to_string()),
                "{expected}: {names:?}"
            );
        }
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
            (
                report.cut_ms,
                report.deletions_applied,
                report.orphans,
                report.damaged_blobs
            ),
            (manifest.cut_ms, 0, 0, 0)
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

        // The old session is gone, a login works, the blob downloads byte for byte, and an old cursor is told the generation changed,
        // with the lineage (the generations and the cut time of each) that it keeps.
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
        let lineage = answer["lineage"].as_array().expect("a lineage");
        assert_eq!(lineage.len(), 2, "{answer}");
        assert_eq!(lineage[0]["generation"], old_gen.to_string());
        assert_eq!(lineage[0]["cut_ms"], 0);
        assert_eq!(lineage[1]["generation"], new_gen.to_string());
        assert_eq!(lineage[1]["cut_ms"], manifest.cut_ms);

        // In place: what came after the backup is gone, a blob published after it becomes a deleted blob that goes the delayed way, and the
        // sequence numbers are issued again from the backup's head.
        let late = blob_uuid(7);
        let late_blob = sealed(3000, 7);
        upload_all(&rig, &acct, &late, &late_blob).await;
        put(&rig, &acct, 9, 190, &msg(&maker, 9, 190, T0 + 181 * DAY)).await;
        let head_before = head_of(&rig).await;
        let report = restore_into(&rig, &dir).await;
        assert_eq!(report.orphans, 1);
        assert!(!report.orphan_scan_failed);
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
        // The restore stamped the orphan one millisecond above the backup's cut time (the floor), so a day and a millisecond.
        rig.clock.advance(31 * DAY + 1);
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

#[tokio::test]
async fn an_in_place_restore_repairs_an_object_of_the_right_size_that_is_not_the_backed_up_one() {
    for backend in backends() {
        let rig = rig_on(Config::default(), backend).await;
        let acct = account(&rig).await;
        let seeded = seed(&rig, &acct, &Maker::new(), 6).await;
        let (dir, _) = take(&rig).await;

        // The object in the store is damaged and keeps its size (a flipped byte): a restore that compares sizes would keep it.
        let key = kinige_server::blobs::object_key(&acct.creds.account_id, &seeded.blob_id);
        let mut damaged = seeded.blob.clone();
        let middle = damaged.len() / 2;
        damaged[middle] ^= 0xff;
        let file = rig.db.scratch_dir("damaged-object").expect("dir").join("f");
        std::fs::write(&file, &damaged).expect("file");
        rig.state
            .blobs
            .objects
            .put_file(&key, &file)
            .await
            .expect("put");
        assert_eq!(
            object_size(&rig, &acct, &seeded.blob_id).await,
            Some(seeded.blob.len() as u64),
            "the damaged object has the right size"
        );

        restore_into(&rig, &dir).await;
        let acct = Acct {
            token: login_token(&rig, &acct.creds).await,
            creds: acct.creds.clone(),
        };
        assert_eq!(
            download(&rig, &acct, &seeded.blob_id, None).await.2,
            seeded.blob,
            "the restore put the backed-up bytes back ({backend:?})"
        );
    }
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

/// The blobs the writer of a scenario uploaded: `(blob id, bytes)`.
type Uploaded = Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;

fn backends() -> Vec<BlobBackend> {
    blob_backends().expect("the blob backends")
}

async fn head_of(rig: &Rig) -> i64 {
    let rows = rows(rig, "SELECT seq_head FROM accounts").await;
    let text = rows.first().expect("a row");
    text.trim_matches(|c| !char::is_ascii_digit(&c))
        .parse()
        .expect("a number")
}
