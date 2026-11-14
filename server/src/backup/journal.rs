//! The two journals a restore re-applies (ADR-0001 §8.9): the deletion journal (an account deleted after the backup is deleted again) and
//! the credential journal (a password, recovery words and wraps changed after the backup are put back). Both are read from three
//! sources and merged: the backup itself, the installation being replaced (read before it is emptied) and the newest other backup next
//! to the restored one. A neighbour's files are used only if they are what *its* manifest says they are.

use super::BackupError;
use super::manifest::{
    CREDENTIALS_FILE, CredentialRow, DELETIONS_FILE, Deletion, Manifest, hash_file,
};
use crate::hashing::{account_hash, hex, unhex};
use crate::store::{blobs, to_i64, to_u64};
use deadpool_postgres::Transaction;
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

/// Everything the three sources hold.
#[derive(Debug, Default, Clone)]
pub struct Journals {
    pub deletions: Vec<Deletion>,
    pub credentials: Vec<CredentialRow>,
}

/// What a restore did with the journals.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Applied {
    /// Accounts deleted again.
    pub deletions: usize,
    /// Accounts whose credentials were put back to what they were after the backup.
    pub credentials: usize,
}

impl Journals {
    /// The journals of a backup directory, which must be the one `manifest` describes: the files are hashed against it.
    pub async fn of_backup(dir: &Path, manifest: &Manifest) -> Result<Journals, BackupError> {
        Ok(Journals {
            deletions: checked_deletions(read_verified(dir, manifest, DELETIONS_FILE).await?)?,
            credentials: checked_credentials(
                read_verified(dir, manifest, CREDENTIALS_FILE).await?,
            )?,
        })
    }

    /// The journals of the installation in the target database (empty for a clean one).
    pub async fn of_installation(tx: &Transaction<'_>) -> Result<Journals, BackupError> {
        let deletions = tx
            .query("SELECT account_hash, deleted_ms FROM deletions", &[])
            .await?;
        let credentials = tx
            .query(
                "SELECT account_hash, cred_version, salt, profile, auth_hash, rc_hash, key_id, wrap_pw, wrap_rc, changed_ms \
                 FROM credential_changes",
                &[],
            )
            .await?;
        Ok(Journals {
            deletions: deletions
                .iter()
                .map(|row| {
                    Ok(Deletion {
                        account_hash: hex(&row.try_get::<_, Vec<u8>>("account_hash")?),
                        deleted_ms: to_u64(row.try_get("deleted_ms")?),
                    })
                })
                .collect::<Result<_, tokio_postgres::Error>>()?,
            credentials: credentials
                .iter()
                .map(|row| {
                    Ok(CredentialRow {
                        account_hash: hex(&row.try_get::<_, Vec<u8>>("account_hash")?),
                        cred_version: to_u64(row.try_get("cred_version")?),
                        salt: hex(&row.try_get::<_, Vec<u8>>("salt")?),
                        profile: row.try_get::<_, i16>("profile")?,
                        auth_hash: hex(&row.try_get::<_, Vec<u8>>("auth_hash")?),
                        rc_hash: hex(&row.try_get::<_, Vec<u8>>("rc_hash")?),
                        key_id: row.try_get("key_id")?,
                        wrap_pw: hex(&row.try_get::<_, Vec<u8>>("wrap_pw")?),
                        wrap_rc: hex(&row.try_get::<_, Vec<u8>>("wrap_rc")?),
                        changed_ms: to_u64(row.try_get("changed_ms")?),
                    })
                })
                .collect::<Result<_, tokio_postgres::Error>>()?,
        })
    }

    /// The journals of the newest backup next to `dir` (the backup being restored is not counted), for an older backup restored into a
    /// clean installation. A neighbour that is not a valid backup, or whose journal files are not what its manifest says, is passed
    /// over (it must not block the restore of this backup, and must not feed it).
    pub async fn of_newest_neighbour(dir: &Path) -> Journals {
        let Some(parent) = dir.parent() else {
            return Journals::default();
        };
        let Ok(mut entries) = fs::read_dir(parent).await else {
            return Journals::default();
        };
        let mut best: Option<(u64, Journals)> = None;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path == dir {
                continue;
            }
            let Ok(manifest) = Manifest::read(&path).await else {
                continue;
            };
            if best
                .as_ref()
                .is_some_and(|(cut, _)| *cut >= manifest.cut_ms)
            {
                continue;
            }
            if let Ok(journals) = Journals::of_backup(&path, &manifest).await {
                best = Some((manifest.cut_ms, journals));
            }
        }
        best.map(|(_, journals)| journals).unwrap_or_default()
    }

    pub fn extend(&mut self, other: Journals) {
        self.deletions.extend(other.deletions);
        self.credentials.extend(other.credentials);
    }
}

/// Re-applies the journals to the restored data, and keeps every entry in the restored journals (the union).
///
/// - **Deletions:** an entry at or after the cut time (`>=`: a deletion stamped at the cut time may not be in the dump) whose account is
///   in the restored data deletes it again, with the delayed deletion of its blobs. A deleted account id is never registered again, so
///   one entry per account is all there can be.
/// - **Credentials:** for each surviving account, the newest entry (by version, then time) at or after the cut time replaces its salt,
///   profile, hashes, key id, wraps and credential version.
pub async fn apply(
    tx: &Transaction<'_>,
    journals: &Journals,
    cut_ms: u64,
    now_ms: u64,
) -> Result<Applied, BackupError> {
    for entry in &journals.deletions {
        let hash = unhex(&entry.account_hash).ok_or(BackupError::Manifest("a journal entry"))?;
        tx.execute(
            "INSERT INTO deletions (account_hash, deleted_ms) VALUES ($1, $2) \
             ON CONFLICT (account_hash) DO UPDATE SET deleted_ms = LEAST(deletions.deleted_ms, EXCLUDED.deleted_ms)",
            &[&hash, &to_i64(entry.deleted_ms, "deleted")?],
        )
        .await?;
    }
    for entry in &journals.credentials {
        insert_credentials(tx, entry).await?;
    }
    let deleted_at: HashMap<&str, u64> = journals
        .deletions
        .iter()
        .map(|d| (d.account_hash.as_str(), d.deleted_ms))
        .fold(HashMap::new(), |mut earliest, (hash, ms)| {
            let slot = earliest.entry(hash).or_insert(u64::MAX);
            *slot = (*slot).min(ms);
            earliest
        });
    let mut newest: HashMap<&str, &CredentialRow> = HashMap::new();
    for entry in journals
        .credentials
        .iter()
        .filter(|c| c.changed_ms >= cut_ms)
    {
        let slot = newest.entry(entry.account_hash.as_str()).or_insert(entry);
        if (entry.cred_version, entry.changed_ms) > (slot.cred_version, slot.changed_ms) {
            *slot = entry;
        }
    }
    let mut applied = Applied::default();
    for row in tx.query("SELECT id FROM accounts", &[]).await? {
        let id: String = row.try_get("id")?;
        let Some(hash) = account_hash(&id).map(|h| hex(&h)) else {
            continue;
        };
        if deleted_at
            .get(hash.as_str())
            .is_some_and(|ms| *ms >= cut_ms)
        {
            blobs::mark_account_deleted(tx, &id, now_ms).await?;
            tx.execute("DELETE FROM accounts WHERE id = $1", &[&id])
                .await?;
            applied.deletions += 1;
        } else if let Some(entry) = newest.get(hash.as_str()) {
            put_back(tx, &id, entry).await?;
            applied.credentials += 1;
        }
    }
    // The credentials of a deleted account are data that must go; the hash in the deletion journal stays.
    tx.execute(
        "DELETE FROM credential_changes WHERE account_hash IN (SELECT account_hash FROM deletions)",
        &[],
    )
    .await?;
    Ok(applied)
}

/// Marks the rows of blobs whose backed-up file was damaged or missing as deleted: a client finds `missing` and uploads again.
pub async fn mark_damaged(
    tx: &Transaction<'_>,
    damaged: &[(String, String)],
    now_ms: u64,
) -> Result<(), BackupError> {
    for (account, blob) in damaged {
        tx.execute(
            "UPDATE blobs SET deleted_ms = $3 WHERE account_id = $1 AND blob_id = $2 AND deleted_ms IS NULL",
            &[account, blob, &to_i64(now_ms, "clock")?],
        )
        .await?;
    }
    Ok(())
}

async fn insert_credentials(tx: &Transaction<'_>, c: &CredentialRow) -> Result<(), BackupError> {
    let bytes = |text: &str| unhex(text).ok_or(BackupError::Manifest("a journal entry"));
    tx.execute(
        "INSERT INTO credential_changes \
             (account_hash, cred_version, salt, profile, auth_hash, rc_hash, key_id, wrap_pw, wrap_rc, changed_ms) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT DO NOTHING",
        &[
            &bytes(&c.account_hash)?,
            &to_i64(c.cred_version, "version")?,
            &bytes(&c.salt)?,
            &c.profile,
            &bytes(&c.auth_hash)?,
            &bytes(&c.rc_hash)?,
            &c.key_id,
            &bytes(&c.wrap_pw)?,
            &bytes(&c.wrap_rc)?,
            &to_i64(c.changed_ms, "changed")?,
        ],
    )
    .await?;
    Ok(())
}

async fn put_back(tx: &Transaction<'_>, id: &str, c: &CredentialRow) -> Result<(), BackupError> {
    let bytes = |text: &str| unhex(text).ok_or(BackupError::Manifest("a journal entry"));
    tx.execute(
        "UPDATE accounts SET salt = $2, profile = $3, auth_hash = $4, rc_hash = $5, key_id = $6, wrap_pw = $7, wrap_rc = $8, \
         cred_version = $9 WHERE id = $1",
        &[
            &id,
            &bytes(&c.salt)?,
            &c.profile,
            &bytes(&c.auth_hash)?,
            &bytes(&c.rc_hash)?,
            &c.key_id,
            &bytes(&c.wrap_pw)?,
            &bytes(&c.wrap_rc)?,
            &to_i64(c.cred_version, "version")?,
        ],
    )
    .await?;
    Ok(())
}

/// A journal file of a backup, read only if its length and hash are the manifest's.
async fn read_verified<T: serde::de::DeserializeOwned>(
    dir: &Path,
    manifest: &Manifest,
    name: &str,
) -> Result<Vec<T>, BackupError> {
    let entry = manifest
        .files
        .iter()
        .find(|e| e.path == name)
        .ok_or(BackupError::Manifest("a journal file is not listed"))?;
    let path = dir.join(name);
    let (len, hash) = hash_file(&path)
        .await
        .map_err(|_| BackupError::Missing(name.to_string()))?;
    if len != entry.bytes || hash != entry.hash {
        return Err(BackupError::Hash(name.to_string()));
    }
    let bytes = fs::read(&path)
        .await
        .map_err(|_| BackupError::Missing(name.to_string()))?;
    serde_json::from_slice(&bytes).map_err(|_| BackupError::Manifest("a dump file is not valid"))
}

/// A journal entry whose hash is a hash and whose time is a time; any other entry makes the whole journal unusable.
fn checked_deletions(journal: Vec<Deletion>) -> Result<Vec<Deletion>, BackupError> {
    let fine = journal.iter().all(|e| {
        e.account_hash.len() == 64
            && unhex(&e.account_hash).is_some()
            && i64::try_from(e.deleted_ms).is_ok()
    });
    if fine {
        Ok(journal)
    } else {
        Err(BackupError::Manifest("a journal entry is not a hash"))
    }
}

fn checked_credentials(journal: Vec<CredentialRow>) -> Result<Vec<CredentialRow>, BackupError> {
    let hex_ok = |text: &str| unhex(text).is_some();
    let fine = journal.iter().all(|c| {
        c.account_hash.len() == 64
            && hex_ok(&c.account_hash)
            && [&c.salt, &c.auth_hash, &c.rc_hash, &c.wrap_pw, &c.wrap_rc]
                .iter()
                .all(|text| hex_ok(text))
            && i64::try_from(c.cred_version).is_ok()
            && i64::try_from(c.changed_ms).is_ok()
    });
    if fine {
        Ok(journal)
    } else {
        Err(BackupError::Manifest("a journal entry is not a hash"))
    }
}
