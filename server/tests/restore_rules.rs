//! The rules a restore applies (ADR-0001 §6.3, §8.9, ADR-0017, ADR-0020): an account deleted after a backup does not come back with it, the
//! cut time of a generation places a branch that a later restore abandoned (the `review_round_3_*` case of the S4a simulation, over HTTP),
//! and a backup directory that is cut, damaged or not ours is refused before anything is changed.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use axum::http::StatusCode;
use common::backup::*;
use common::blobs::*;
use common::*;
use kinige_server::Clock;
use kinige_server::backup::manifest::{Entry, hash_file};
use kinige_server::backup::{self, BackupError, Manifest};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

async fn delete_account(rig: &Rig, acct: &Acct, key: &[u8; 32]) -> (StatusCode, Value) {
    rig.http
        .post(
            "/v1/account/delete",
            Some(&acct.token),
            &json!({"auth_key": b64(key)}),
        )
        .await
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

async fn accounts_left(rig: &Rig) -> Vec<String> {
    rows(rig, "SELECT id FROM accounts ORDER BY id")
        .await
        .iter()
        .map(|row| row.split('"').nth(3).expect("an id").to_string())
        .collect()
}

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
    assert_eq!((code, answer["error"].as_str()), (StatusCode::UNAUTHORIZED, Some("auth")));
    assert_eq!(accounts_left(&rig).await.len(), 2);
    let (code, _) = rig
        .http
        .post("/v1/account/delete", None, &json!({"auth_key": b64(&one.creds.auth_key)}))
        .await;
    assert_eq!(code, StatusCode::UNAUTHORIZED, "a session is needed");
    let (code, _) = delete_account(&rig, &one, &one.creds.auth_key).await;
    assert_eq!(code, StatusCode::OK);
    assert_eq!(accounts_left(&rig).await, vec![two.creds.account_id.clone()]);
    for table in ["sessions", "envelopes", "ops", "entity_known", "purged_entities", "drops"] {
        let left = rows(&rig, &format!("SELECT 1 FROM {table} WHERE account_id = '{}'", one.creds.account_id)).await;
        assert!(left.is_empty(), "{table}");
    }
    assert_eq!(blob_row(&rig, &one, &seeded.blob_id).await.map(|r| r.1.is_some()), Some(true));
    assert!(rig.state.blobs.staging.find(&one.creds.account_id, &blob_uuid(9)).await.is_none());
    assert_eq!(download(&rig, &one, &seeded.blob_id, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(count(&rig, "deletions").await, 1);
    let hash = kinige_server::hashing::hex(&kinige_server::hashing::account_hash(&one.creds.account_id).expect("hash"));
    assert_eq!(rows(&rig, "SELECT encode(account_hash, 'hex') AS h FROM deletions").await, vec![format!("{{\"h\":\"{hash}\"}}")]);
    rig.clock.advance(DAY);
    let (newer, _) = take_into(&rig, &root).await;

    // In place: the journal of the installation is read before it is replaced.
    let report = restore_into(&rig, &older).await;
    assert_eq!(report.deletions_applied, 1);
    assert_eq!(accounts_left(&rig).await, vec![two.creds.account_id.clone()]);
    assert_eq!(count(&rig, "deletions").await, 1, "and the journal is kept");
    assert_eq!(blob_row(&rig, &one, &seeded.blob_id).await.map(|r| r.1.is_some()), Some(true), "the blob goes the delayed way");

    // Into a clean installation: no journal of its own; the newest backup next to the one restored has it.
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &older).await;
    assert_eq!(report.deletions_applied, 1);
    assert_eq!(accounts_left(&clean).await, vec![two.creds.account_id.clone()]);
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
    assert_eq!(accounts_left(&clean).await, vec![two.creds.account_id.clone()]);
    assert_eq!(count(&clean, "deletions").await, 1, "the journal travels with the backup");
    assert!(older_manifest.cut_ms < rig.clock.now_ms());
}

#[tokio::test]
async fn a_branch_cut_from_an_older_backup_than_the_one_restored_now_is_placed_by_its_cut_time() {
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
    let device = Acct { token: login_token(&installation, &acct.creds).await, creds: acct.creds.clone() };
    let (_, answer) = pull(&installation, &device, 0, 0, 0, 10).await;
    assert_eq!(answer["result"], "generation_changed");
    assert_eq!(answer["generation"], branch.to_string());
    let cut = answer["cut_ms"].as_u64().expect("the cut time of the branch");
    assert_eq!(cut, older_manifest.cut_ms);

    // Then the newer backup is restored over it: the lineage is [G1, G3] and G2 is abandoned.
    let report = restore_into(&installation, &newer).await;
    let current = report.generation;
    assert_ne!(current, branch);
    let lineage = rows(&installation, "SELECT generation::text AS g FROM lineage ORDER BY pos").await;
    assert_eq!(lineage.len(), 2, "{lineage:?}");
    assert!(!lineage.iter().any(|l| l.contains(&branch.to_string())), "the branch is not in the lineage");
    let device = Acct { token: login_token(&installation, &acct.creds).await, creds: acct.creds.clone() };

    // The device on the branch is told it missed the drop, because the drop was compacted after the branch was cut. Sending no cut time
    // is answered the same way (the safe side), and a branch cut after the drop has missed nothing.
    let ask = |cut: Option<u64>, after: u64| {
        let mut body = json!({"generation": branch.to_string(), "after": after, "known_epoch": 0});
        if let Some(cut) = cut {
            body["cut_ms"] = json!(cut);
        }
        let (rig, device) = (&installation, &device);
        async move { rig.http.post("/v1/pull", Some(&device.token), &body).await.1 }
    };
    let answer = ask(Some(cut), 1).await;
    assert_eq!((answer["result"].as_str(), answer["stale"].as_bool()), (Some("generation_changed"), Some(true)), "{answer}");
    assert_eq!(answer["generation"], current.to_string());
    assert_eq!(answer["cut_ms"], newer_manifest.cut_ms, "the new generation was cut from the newer backup");
    assert_eq!(ask(None, 1).await["stale"], true);
    assert_eq!(ask(Some(newer_manifest.cut_ms), 1).await["stale"], false);
    assert_eq!(ask(Some(cut), 0).await["stale"], false, "a pull from 0 is never stale");

    // A push carries the cut time in its cursor header too (a fourth part, which must parse). A pusher whose generation is not the
    // current one is stale whatever its cut time says: it pulls first (appendix §A.3).
    let item_op = id(70);
    let fresh = msg(&maker, 3, 70, T0 + 182 * DAY);
    let item = Item { entity: "message", id: &id(3), op: &item_op, bytes: &fresh };
    let probe = |cursor: String| {
        let (rig, device, item) = (&installation, &device, &item);
        async move {
            let (status, answer) = push_with(rig, device, &cursor, Some("1"), item).await;
            assert_eq!(status, StatusCode::OK, "{answer}");
            outcome(&answer["results"][0]).to_string()
        }
    };
    assert_eq!(probe(format!("{branch}.1.0.{cut}")).await, "stale");
    assert_eq!(probe(format!("{branch}.1.0")).await, "stale");
    assert_eq!(probe(format!("{branch}.1.0.{}", newer_manifest.cut_ms)).await, "stale");
    assert_eq!(probe(format!("{current}.3.1")).await, "ok", "a pusher in the current generation, up to date");
}

/// A target with data of its own, and a dump to restore into it.
struct Target {
    rig: Rig,
    acct: Acct,
}

async fn target() -> Target {
    let rig = rig().await;
    let acct = account(&rig).await;
    let maker = Maker::new();
    seed(&rig, &acct, &maker, 7).await;
    Target { rig, acct }
}

impl Target {
    /// What a failed restore must leave as it was.
    async fn state(&self) -> (Vec<String>, Vec<String>, usize, i64) {
        (
            rows(&self.rig, "SELECT * FROM accounts").await,
            rows(&self.rig, "SELECT seq, op_id FROM envelopes ORDER BY seq").await,
            self.rig.state.blobs.objects.list().await.expect("list").len(),
            count(&self.rig, "sessions").await,
        )
    }

    async fn restore_is_refused(&self, dir: &Path, why: &str) -> BackupError {
        let before = self.state().await;
        let error = backup::restore(&self.rig.state.pool, &self.rig.state.blobs.objects, &*self.rig.clock, dir)
            .await
            .expect_err(why);
        assert_eq!(self.state().await, before, "{why}: nothing changed");
        error
    }
}

/// Rewrites the manifest of `dir` with `change` applied, then writes it back.
async fn edit_manifest(dir: &Path, change: impl FnOnce(&mut Manifest)) {
    let mut manifest = Manifest::read(dir).await.expect("manifest");
    change(&mut manifest);
    std::fs::write(dir.join("manifest.json"), serde_json::to_vec_pretty(&manifest).expect("json")).expect("write");
}

/// Replaces the content of a dump file and makes the manifest agree, so that only the content is wrong.
async fn replace_and_rehash(dir: &Path, path: &str, bytes: &[u8]) {
    std::fs::write(dir.join(path), bytes).expect("write");
    let (len, hash) = hash_file(&dir.join(path)).await.expect("hash");
    edit_manifest(dir, |m| {
        for entry in m.files.iter_mut().chain(m.blobs.iter_mut()).filter(|e| e.path == path) {
            *entry = Entry { path: path.to_string(), bytes: len, hash: hash.clone() };
        }
    })
    .await;
}

#[tokio::test]
async fn a_damaged_backup_is_refused_before_anything_is_changed() {
    let source = rig().await;
    let acct = account(&source).await;
    let seeded = seed(&source, &acct, &Maker::new(), 3).await;
    let (good, manifest) = take(&source).await;
    let target = target().await;
    let blob_path = format!("blobs/{}/{}", acct.creds.account_id, seeded.blob_id);
    let work = source.db.scratch_dir("damaged").expect("dir");
    let variant = |name: &str| -> PathBuf {
        let dir = work.join(name);
        copy_dir(&good, &dir);
        dir
    };

    // The manifest: absent, empty, cut off at every tenth of its length, not JSON, JSON of another shape.
    let dir = variant("no-manifest");
    std::fs::remove_file(dir.join("manifest.json")).expect("remove");
    assert!(matches!(target.restore_is_refused(&dir, "no manifest").await, BackupError::Manifest(_)));
    let text = std::fs::read(good.join("manifest.json")).expect("manifest");
    for tenth in 0..10 {
        let dir = variant(&format!("cut-{tenth}"));
        std::fs::write(dir.join("manifest.json"), text.get(..text.len() * tenth / 10).expect("prefix")).expect("write");
        assert!(matches!(target.restore_is_refused(&dir, "a cut manifest").await, BackupError::Manifest(_)), "{tenth}");
    }
    for (name, content) in [("array", "[]"), ("object", "{}"), ("text", "hello"), ("null", "null"), ("numbers", "{\"format\":1,\"schema\":2,\"cut_ms\":-5}")] {
        let dir = variant(name);
        std::fs::write(dir.join("manifest.json"), content).expect("write");
        assert!(matches!(target.restore_is_refused(&dir, name).await, BackupError::Manifest(_)), "{name}");
    }

    // Edits of a well-formed manifest: another format, another schema, a file added, a file removed, a path that leaves the directory.
    let dir = variant("format");
    edit_manifest(&dir, |m| m.format = 9).await;
    assert!(matches!(target.restore_is_refused(&dir, "format").await, BackupError::Manifest(_)));
    let dir = variant("schema");
    edit_manifest(&dir, |m| m.schema = 1).await;
    assert_eq!(target.restore_is_refused(&dir, "schema").await, BackupError::Schema { backup: 1, server: 2 });
    let dir = variant("fewer-files");
    edit_manifest(&dir, |m| {
        m.files.pop();
    })
    .await;
    assert!(matches!(target.restore_is_refused(&dir, "a file less").await, BackupError::Manifest(_)));
    let dir = variant("traversal");
    edit_manifest(&dir, |m| {
        if let Some(entry) = m.blobs.first_mut() {
            entry.path = "blobs/../../../../etc/passwd".to_string();
        }
    })
    .await;
    assert!(matches!(target.restore_is_refused(&dir, "traversal").await, BackupError::Manifest(_)));
    let dir = variant("wrong-hash");
    edit_manifest(&dir, |m| {
        if let Some(entry) = m.files.first_mut() {
            entry.hash = "0".repeat(64);
        }
    })
    .await;
    assert!(matches!(target.restore_is_refused(&dir, "a wrong hash").await, BackupError::Hash(_)));

    // The files: missing, one byte flipped, cut, extended; a table and the blob alike.
    for path in ["tables/envelopes.copy", "tables/accounts.copy", blob_path.as_str(), "lineage.json"] {
        let dir = variant(&format!("missing-{}", path.replace('/', "_")));
        std::fs::remove_file(dir.join(path)).expect("remove");
        assert!(matches!(target.restore_is_refused(&dir, "a missing file").await, BackupError::Missing(p) if p == path), "{path}");

        let bytes = std::fs::read(good.join(path)).expect("file");
        let mut flipped = bytes.clone();
        let middle = flipped.len() / 2;
        flipped[middle] ^= 0x01;
        let mut longer = bytes.clone();
        longer.push(b'\n');
        let cut = bytes.get(..bytes.len() - 1).expect("shorter").to_vec();
        for (label, damaged) in [("flipped", flipped), ("longer", longer), ("cut", cut)] {
            let dir = variant(&format!("{label}-{}", path.replace('/', "_")));
            std::fs::write(dir.join(path), damaged).expect("write");
            assert!(matches!(target.restore_is_refused(&dir, "a damaged file").await, BackupError::Hash(p) if p == path), "{label} {path}");
        }
    }

    // Content that matches its hash and is still not a dump: refused all the same, and the transaction leaves the target as it was.
    let dir = variant("lineage-garbage");
    replace_and_rehash(&dir, "lineage.json", b"[{\"pos\":1,\"generation\":\"x\",\"cut_ms\":0}]").await;
    assert!(matches!(target.restore_is_refused(&dir, "a lineage that is not numbers").await, BackupError::Manifest(_)));
    let dir = variant("journal-garbage");
    replace_and_rehash(&dir, "deletions.json", b"[{\"account_hash\":\"zz\",\"deleted_ms\":1}]").await;
    assert!(matches!(target.restore_is_refused(&dir, "a journal entry that is not a hash").await, BackupError::Manifest(_)));
    let dir = variant("copy-garbage");
    replace_and_rehash(&dir, "tables/ops.copy", b"this is not a COPY stream\nat all\n").await;
    assert_eq!(target.restore_is_refused(&dir, "a table that does not load").await, BackupError::Database);

    // Not backups at all.
    let file = work.join("a-file");
    std::fs::write(&file, "x").expect("file");
    for path in [file, work.join("does-not-exist"), work.clone()] {
        assert!(matches!(target.restore_is_refused(&path, "not a backup").await, BackupError::Manifest(_)));
    }

    // And the good one still restores into that target, which proves the variants differed from it in the one thing each said.
    let report = restore_into(&target.rig, &good).await;
    assert_eq!(report.cut_ms, manifest.cut_ms);
    assert_eq!(rows(&target.rig, "SELECT id FROM accounts").await, rows(&source, "SELECT id FROM accounts").await);
    let _ = &target.acct;
}

#[tokio::test]
async fn a_backup_that_cannot_be_completed_leaves_nothing_behind() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let seeded = seed(&rig, &acct, &Maker::new(), 4).await;
    let root = rig.db.scratch_dir("failing").expect("dir");

    // A published blob whose object has gone missing from the store: the backup fails and no directory is left, finished or not.
    let key = kinige_server::blobs::object_key(&acct.creds.account_id, &seeded.blob_id);
    rig.state.blobs.objects.delete(&key).await.expect("delete");
    let error = backup::create(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, &root)
        .await
        .expect_err("a backup with a missing blob");
    assert_eq!(error, BackupError::Store);
    assert!(std::fs::read_dir(&root).expect("dir").next().is_none(), "nothing left under the root");

    // A root that cannot be a directory.
    let file = root.join("file");
    std::fs::write(&file, "x").expect("file");
    let error = backup::create(&rig.state.pool, &rig.state.blobs.objects, &*rig.clock, &file)
        .await
        .expect_err("a root that is a file");
    assert!(matches!(error, BackupError::Io(_)), "{error}");
}
