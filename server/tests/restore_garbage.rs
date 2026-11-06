//! Garbage on `backup` and `restore` (CLAUDE.md rule 12): a backup directory that is cut, damaged or not ours is refused before anything is
//! changed; one damaged **blob** file does not make a backup unrestorable and does not spread through the hard links of later backups; a
//! neighbour's journals are not trusted unless its manifest vouches for them; a backup that cannot be completed leaves nothing behind.
#![cfg(test)]
#![cfg(feature = "pg-tests")]

mod common;

use common::backup::*;
use common::blobs::*;
use common::*;
use kinige_server::backup::manifest::{Entry, hash_file};
use kinige_server::backup::{self, BackupError, Manifest};
use std::path::{Path, PathBuf};

#[tokio::test]
async fn a_damaged_backup_is_refused_before_anything_is_changed() {
    let source = rig().await;
    let acct = account(&source).await;
    seed(&source, &acct, &Maker::new(), 3).await;
    let (good, manifest) = take(&source).await;
    let target = target().await;
    let work = source.db.scratch_dir("damaged").expect("dir");
    let variant = |name: &str| -> PathBuf {
        let dir = work.join(name);
        copy_dir(&good, &dir);
        dir
    };

    // The manifest: absent, empty, cut off at every tenth of its length, not JSON, JSON of another shape.
    let dir = variant("no-manifest");
    std::fs::remove_file(dir.join("manifest.json")).expect("remove");
    assert!(matches!(
        target.restore_is_refused(&dir, "no manifest").await,
        BackupError::Manifest(_)
    ));
    let text = std::fs::read(good.join("manifest.json")).expect("manifest");
    for tenth in 0..10 {
        let dir = variant(&format!("cut-{tenth}"));
        std::fs::write(
            dir.join("manifest.json"),
            text.get(..text.len() * tenth / 10).expect("prefix"),
        )
        .expect("write");
        assert!(
            matches!(
                target.restore_is_refused(&dir, "a cut manifest").await,
                BackupError::Manifest(_)
            ),
            "{tenth}"
        );
    }
    for (name, content) in [
        ("array", "[]"),
        ("object", "{}"),
        ("text", "hello"),
        ("null", "null"),
        ("numbers", "{\"format\":1,\"schema\":3,\"cut_ms\":-5}"),
    ] {
        let dir = variant(name);
        std::fs::write(dir.join("manifest.json"), content).expect("write");
        assert!(
            matches!(
                target.restore_is_refused(&dir, name).await,
                BackupError::Manifest(_)
            ),
            "{name}"
        );
    }

    // Edits of a well-formed manifest: another format, another schema, a file added, a file removed, a path that leaves the directory.
    let dir = variant("format");
    edit_manifest(&dir, |m| m.format = 9).await;
    assert!(matches!(
        target.restore_is_refused(&dir, "format").await,
        BackupError::Manifest(_)
    ));
    let dir = variant("schema");
    edit_manifest(&dir, |m| m.schema = 1).await;
    assert_eq!(
        target.restore_is_refused(&dir, "schema").await,
        BackupError::Schema {
            backup: 1,
            server: 3
        }
    );
    let dir = variant("fewer-files");
    edit_manifest(&dir, |m| {
        m.files.pop();
    })
    .await;
    assert!(matches!(
        target.restore_is_refused(&dir, "a file less").await,
        BackupError::Manifest(_)
    ));
    let dir = variant("traversal");
    edit_manifest(&dir, |m| {
        if let Some(entry) = m.blobs.first_mut() {
            entry.path = "blobs/../../../../etc/passwd".to_string();
        }
    })
    .await;
    assert!(matches!(
        target.restore_is_refused(&dir, "traversal").await,
        BackupError::Manifest(_)
    ));
    let dir = variant("wrong-hash");
    edit_manifest(&dir, |m| {
        if let Some(entry) = m.files.first_mut() {
            entry.hash = "0".repeat(64);
        }
    })
    .await;
    assert!(matches!(
        target.restore_is_refused(&dir, "a wrong hash").await,
        BackupError::Hash(_)
    ));

    // The files of the dump: missing, one byte flipped, cut, extended. Without any of them the database cannot be restored.
    for path in [
        "tables/envelopes.copy",
        "tables/accounts.copy",
        "lineage.json",
        "credentials.json",
    ] {
        let dir = variant(&format!("missing-{}", path.replace('/', "_")));
        std::fs::remove_file(dir.join(path)).expect("remove");
        assert!(
            matches!(target.restore_is_refused(&dir, "a missing file").await, BackupError::Missing(p) if p == path),
            "{path}"
        );

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
            assert!(
                matches!(target.restore_is_refused(&dir, "a damaged file").await, BackupError::Hash(p) if p == path),
                "{label} {path}"
            );
        }
    }

    // Content that matches its hash and is still not a dump: refused all the same, and the transaction leaves the target as it was.
    let dir = variant("lineage-garbage");
    replace_and_rehash(
        &dir,
        "lineage.json",
        b"[{\"pos\":1,\"generation\":\"x\",\"cut_ms\":0}]",
    )
    .await;
    assert!(matches!(
        target
            .restore_is_refused(&dir, "a lineage that is not numbers")
            .await,
        BackupError::Manifest(_)
    ));
    for (name, file, content) in [
        (
            "deletions-garbage",
            "deletions.json",
            "[{\"account_hash\":\"zz\",\"deleted_ms\":1}]",
        ),
        (
            "credentials-garbage",
            "credentials.json",
            "[{\"account_hash\":\"00\",\"cred_version\":1,\"salt\":\"\",\"profile\":1,\"auth_hash\":\"\",\"rc_hash\":\"\",\"key_id\":1,\"wrap_pw\":\"\",\"wrap_rc\":\"\",\"changed_ms\":1}]",
        ),
    ] {
        let dir = variant(name);
        replace_and_rehash(&dir, file, content.as_bytes()).await;
        assert!(
            matches!(
                target
                    .restore_is_refused(&dir, "a journal entry that is not a hash")
                    .await,
                BackupError::Manifest(_)
            ),
            "{name}"
        );
    }
    let dir = variant("copy-garbage");
    replace_and_rehash(
        &dir,
        "tables/ops.copy",
        b"this is not a COPY stream\nat all\n",
    )
    .await;
    assert_eq!(
        target
            .restore_is_refused(&dir, "a table that does not load")
            .await,
        BackupError::Database
    );

    // Not backups at all.
    let file = work.join("a-file");
    std::fs::write(&file, "x").expect("file");
    for path in [file, work.join("does-not-exist"), work.clone()] {
        assert!(matches!(
            target.restore_is_refused(&path, "not a backup").await,
            BackupError::Manifest(_)
        ));
    }

    // And the good one still restores into that target, which proves the variants differed from it in the one thing each said.
    let report = restore_into(&target.rig, &good).await;
    assert_eq!(report.cut_ms, manifest.cut_ms);
    assert_eq!(
        rows(&target.rig, "SELECT id FROM accounts").await,
        rows(&source, "SELECT id FROM accounts").await
    );
}

#[tokio::test]
async fn one_damaged_or_missing_blob_file_does_not_make_a_backup_unrestorable() {
    // Round 1 of the review: a blob that cannot be restored becomes a deleted row (a device finds `missing` and uploads it again), the
    // rest of the backup is restored, and the report says how many were lost (the command exits non-zero on it).
    let source = rig().await;
    let acct = account(&source).await;
    let first = seed(&source, &acct, &Maker::new(), 3).await;
    let second = (blob_uuid(20), sealed(5000, 20));
    let third = (blob_uuid(21), sealed(6000, 21));
    upload_all(&source, &acct, &second.0, &second.1).await;
    upload_all(&source, &acct, &third.0, &third.1).await;
    let (good, _) = take(&source).await;

    // The first blob's file has a byte flipped; the second one's file is gone; the third is intact.
    let first_file = good.join(format!("blobs/{}/{}", acct.creds.account_id, first.blob_id));
    let mut damaged = std::fs::read(&first_file).expect("file");
    let middle = damaged.len() / 2;
    damaged[middle] ^= 0x01;
    std::fs::write(&first_file, damaged).expect("write");
    std::fs::remove_file(good.join(format!("blobs/{}/{}", acct.creds.account_id, second.0)))
        .expect("remove");

    let clean = clean_like(&source).await;
    let report = restore_into(&clean, &good).await;
    assert_eq!(report.damaged_blobs, 2);
    assert_eq!(
        rows(&clean, "SELECT id FROM accounts").await,
        rows(&source, "SELECT id FROM accounts").await,
        "the database is restored"
    );
    let session = Acct {
        token: login_token(&clean, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    assert_eq!(
        status(&clean, &session, &first.blob_id).await["state"],
        "deleted"
    );
    assert_eq!(
        status(&clean, &session, &second.0).await["state"],
        "deleted"
    );
    assert_eq!(download(&clean, &session, &third.0, None).await.2, third.1);
    assert!(
        object_size(&clean, &session, &first.blob_id)
            .await
            .is_none(),
        "the damaged bytes were not put into the store"
    );

    // What the operator reads names no id.
    let message =
        BackupError::Hash(format!("blobs/{}/{}", acct.creds.account_id, first.blob_id)).to_string();
    assert!(!message.contains(&acct.creds.account_id) && !message.contains(&first.blob_id));
}

#[tokio::test]
async fn in_place_a_damaged_backup_file_does_not_cost_a_blob_the_store_still_holds_intact() {
    // Round 2 of the review: restoring in place, the store usually still has the object the damaged file was a copy of. Marking its row
    // deleted would throw away a blob that is there; only a blob that exists nowhere is lost.
    let rig = rig().await;
    let acct = account(&rig).await;
    let kept = seed(&rig, &acct, &Maker::new(), 3).await;
    let removed = (blob_uuid(20), sealed(5000, 20));
    let lost = (blob_uuid(21), sealed(6000, 21));
    upload_all(&rig, &acct, &removed.0, &removed.1).await;
    upload_all(&rig, &acct, &lost.0, &lost.1).await;
    let (good, _) = take(&rig).await;

    // The backup's file of `kept` is flipped and the one of `removed` is gone; the store has both intact. `lost` is damaged in the
    // backup and also gone from the store (deleted by hand): nothing holds it.
    let file = |blob: &str| good.join(format!("blobs/{}/{}", acct.creds.account_id, blob));
    let mut damaged = std::fs::read(file(&kept.blob_id)).expect("file");
    damaged[0] ^= 0x01;
    std::fs::write(file(&kept.blob_id), damaged).expect("write");
    std::fs::remove_file(file(&removed.0)).expect("remove");
    std::fs::remove_file(file(&lost.0)).expect("remove");
    rig.state
        .blobs
        .objects
        .delete(&kinige_server::blobs::object_key(
            &acct.creds.account_id,
            &lost.0,
        ))
        .await
        .expect("delete from the store");

    let report = restore_into(&rig, &good).await;
    assert_eq!(report.damaged_blobs, 1, "only the blob that exists nowhere");
    let session = Acct {
        token: login_token(&rig, &acct.creds).await,
        creds: acct.creds.clone(),
    };
    for (blob, bytes) in [(&kept.blob_id, &kept.blob), (&removed.0, &removed.1)] {
        assert_eq!(status(&rig, &session, blob).await["state"], "published");
        assert_eq!(&download(&rig, &session, blob, None).await.2, bytes);
    }
    assert_eq!(status(&rig, &session, &lost.0).await["state"], "deleted");
}

#[tokio::test]
async fn a_damaged_blob_file_of_an_earlier_backup_is_not_linked_into_the_next_one() {
    let rig = rig().await;
    let acct = account(&rig).await;
    let seeded = seed(&rig, &acct, &Maker::new(), 3).await;
    let (first, _) = take(&rig).await;
    let root = first.parent().expect("root").to_path_buf();
    let file = first.join(format!(
        "blobs/{}/{}",
        acct.creds.account_id, seeded.blob_id
    ));
    let mut damaged = std::fs::read(&file).expect("file");
    damaged[0] ^= 0x01;
    std::fs::write(&file, damaged).expect("write");
    rig.clock.advance(DAY);

    let (second, manifest) = take_into(&rig, &root).await;
    manifest
        .verify(&second)
        .await
        .expect("the new backup is whole");
    let copied = std::fs::read(second.join(format!(
        "blobs/{}/{}",
        acct.creds.account_id, seeded.blob_id
    )))
    .expect("file");
    assert_eq!(
        copied, seeded.blob,
        "the file came from the store, not from the damaged link"
    );
    assert!(
        Manifest::read(&first)
            .await
            .expect("manifest")
            .verify(&first)
            .await
            .is_err(),
        "the earlier backup is still damaged, and only it"
    );
}

#[tokio::test]
async fn the_journals_of_a_neighbour_are_used_only_if_its_manifest_vouches_for_them() {
    let rig = rig().await;
    let (one, two) = (account(&rig).await, account(&rig).await);
    seed(&rig, &one, &Maker::new(), 3).await;
    let (older, _) = take(&rig).await;
    let root = older.parent().expect("root").to_path_buf();
    rig.clock.advance(DAY);
    let (code, _) = delete_account(&rig, &one, &one.creds.auth_key).await;
    assert_eq!(code, axum::http::StatusCode::OK);
    rig.clock.advance(DAY);
    let (newer, _) = take_into(&rig, &root).await;

    // Untouched: the newer backup next to the older one carries the deletion into a clean installation.
    let clean = clean_like(&rig).await;
    assert_eq!(restore_into(&clean, &older).await.deletions_applied, 1);

    // The journal of the neighbour is emptied by hand, its manifest left as it was: the files do not match, it is passed over.
    std::fs::write(newer.join("deletions.json"), "[]").expect("write");
    let clean = clean_like(&rig).await;
    let report = restore_into(&clean, &older).await;
    assert_eq!(
        report.deletions_applied, 0,
        "a neighbour that is not what it says is not believed"
    );
    assert_eq!(
        rows(&clean, "SELECT id FROM accounts").await.len(),
        2,
        "and the account is back, which is the limit of a backup kept with nothing that vouches for later deletions"
    );
    let _ = two;
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
    let error = backup::create(
        &rig.state.pool,
        &rig.state.blobs.objects,
        &*rig.clock,
        &root,
    )
    .await
    .expect_err("a backup with a missing blob");
    assert_eq!(error, BackupError::Store);
    assert!(
        std::fs::read_dir(&root).expect("dir").next().is_none(),
        "nothing left under the root"
    );

    // A root that cannot be a directory.
    let file = root.join("file");
    std::fs::write(&file, "x").expect("file");
    let error = backup::create(
        &rig.state.pool,
        &rig.state.blobs.objects,
        &*rig.clock,
        &file,
    )
    .await
    .expect_err("a root that is a file");
    assert!(matches!(error, BackupError::Io(_)), "{error}");
}

/// A target with data of its own, and a restore that must leave it as it was.
struct Target {
    rig: Rig,
}

async fn target() -> Target {
    let rig = rig().await;
    let acct = account(&rig).await;
    seed(&rig, &acct, &Maker::new(), 7).await;
    Target { rig }
}

impl Target {
    /// What a failed restore must leave as it was.
    async fn state(&self) -> (Vec<String>, Vec<String>, usize, i64) {
        (
            rows(&self.rig, "SELECT * FROM accounts").await,
            rows(&self.rig, "SELECT seq, op_id FROM envelopes ORDER BY seq").await,
            self.rig
                .state
                .blobs
                .objects
                .list()
                .await
                .expect("list")
                .len(),
            count(&self.rig, "sessions").await,
        )
    }

    async fn restore_is_refused(&self, dir: &Path, why: &str) -> BackupError {
        let before = self.state().await;
        let error = backup::restore(
            &self.rig.state.pool,
            &self.rig.state.blobs.objects,
            &*self.rig.clock,
            dir,
        )
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
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).expect("json"),
    )
    .expect("write");
}

/// Replaces the content of a dump file and makes the manifest agree, so that only the content is wrong.
async fn replace_and_rehash(dir: &Path, path: &str, bytes: &[u8]) {
    std::fs::write(dir.join(path), bytes).expect("write");
    let (len, hash) = hash_file(&dir.join(path)).await.expect("hash");
    edit_manifest(dir, |m| {
        for entry in m
            .files
            .iter_mut()
            .chain(m.blobs.iter_mut())
            .filter(|e| e.path == path)
        {
            *entry = Entry {
                path: path.to_string(),
                bytes: len,
                hash: hash.clone(),
            };
        }
    })
    .await;
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
