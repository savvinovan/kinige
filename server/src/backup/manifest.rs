//! The manifest of a backup: the cut time, the format version, the schema version and the hash of every file in the directory. It is the
//! last file written, so a directory without one is a backup that did not finish. A restore checks the whole directory against it
//! before it touches the database or the store.

use super::BackupError;
use crate::blobs::parse_key;
use crate::hashing::Hasher;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

/// The version of this layout. A restore refuses another one.
pub const FORMAT: u32 = 1;
pub const MANIFEST_FILE: &str = "manifest.json";
pub const LINEAGE_FILE: &str = "lineage.json";
pub const DELETIONS_FILE: &str = "deletions.json";
pub const CREDENTIALS_FILE: &str = "credentials.json";
/// The tables copied as text with `COPY`, in the order a restore loads them (`accounts` first: the others refer to it).
pub const TABLES: [&str; 7] = [
    "accounts",
    "envelopes",
    "ops",
    "entity_known",
    "purged_entities",
    "drops",
    "blobs",
];
/// A manifest above this size is not one of ours.
const MAX_MANIFEST: u64 = 256 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    /// The highest migration the dump was taken from; a restore needs the same.
    pub schema: i32,
    /// When the snapshot was cut, ms since the Unix epoch, on the server's clock (ADR-0020, `store::lock`).
    pub cut_ms: u64,
    pub server_version: String,
    /// The dump of the database: `tables/<name>.copy` for each of [`TABLES`], `lineage.json` and `deletions.json`.
    pub files: Vec<Entry>,
    /// The objects of every blob that was published and not deleted at the cut: `blobs/<account_id>/<blob_id>`.
    pub blobs: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub path: String,
    pub bytes: u64,
    /// `crypto_generichash`, 32 bytes, lower-case hex.
    pub hash: String,
}

impl Manifest {
    /// Reads and parses `manifest.json` of `dir` and judges its shape; the files are not looked at.
    pub async fn read(dir: &Path) -> Result<Manifest, BackupError> {
        let path = dir.join(MANIFEST_FILE);
        let mut file = File::open(&path)
            .await
            .map_err(|_| BackupError::Manifest("there is no manifest.json"))?;
        let len = file
            .metadata()
            .await
            .map_err(|_| BackupError::Manifest("manifest.json cannot be read"))?
            .len();
        if len > MAX_MANIFEST {
            return Err(BackupError::Manifest("manifest.json is too large"));
        }
        let mut text = Vec::new();
        file.read_to_end(&mut text)
            .await
            .map_err(|_| BackupError::Manifest("manifest.json cannot be read"))?;
        let manifest: Manifest = serde_json::from_slice(&text)
            .map_err(|_| BackupError::Manifest("manifest.json is not a manifest"))?;
        manifest.judge()?;
        Ok(manifest)
    }

    /// The shape of the manifest: the version, the file list a dump must have, and paths that cannot leave the directory.
    fn judge(&self) -> Result<(), BackupError> {
        if self.format != FORMAT {
            return Err(BackupError::Manifest(
                "the backup format is not this server's",
            ));
        }
        let mut expected: Vec<String> = TABLES.iter().map(|t| table_path(t)).collect();
        expected.push(LINEAGE_FILE.to_string());
        expected.push(DELETIONS_FILE.to_string());
        expected.push(CREDENTIALS_FILE.to_string());
        let mut have: Vec<&str> = self.files.iter().map(|e| e.path.as_str()).collect();
        have.sort_unstable();
        expected.sort_unstable();
        if have != expected {
            return Err(BackupError::Manifest("the list of files is not a dump"));
        }
        for entry in &self.blobs {
            let blob_path_ok = entry
                .path
                .strip_prefix("blobs/")
                .is_some_and(|key| parse_key(key).is_some());
            if !blob_path_ok {
                return Err(BackupError::Manifest("a blob path is not one of ours"));
            }
        }
        if self
            .files
            .iter()
            .chain(&self.blobs)
            .any(|e| e.hash.len() != 64)
        {
            return Err(BackupError::Manifest("a hash is not a hash"));
        }
        Ok(())
    }

    /// Checks every file of the directory against its entry, blobs included: it exists, has the length and the hash. Nothing is changed.
    pub async fn verify(&self, dir: &Path) -> Result<(), BackupError> {
        self.verify_dump(dir).await?;
        match self.damaged_blobs(dir).await.first() {
            Some(path) => Err(BackupError::Hash(path.clone())),
            None => Ok(()),
        }
    }

    /// Checks the files of the dump (the tables and the small files): without every one of them the database cannot be restored, and
    /// a restore is refused.
    pub async fn verify_dump(&self, dir: &Path) -> Result<(), BackupError> {
        for entry in &self.files {
            if !matches_entry(dir, entry).await? {
                return Err(BackupError::Hash(entry.path.clone()));
            }
        }
        Ok(())
    }

    /// The blob files of the directory that are missing or are not what the manifest says. One damaged blob must not make a backup
    /// unrestorable (round 1 of the review of S4c-2): the restore marks such a blob deleted, and its device finds `missing`.
    pub async fn damaged_blobs(&self, dir: &Path) -> Vec<String> {
        let mut damaged = Vec::new();
        for entry in &self.blobs {
            if !matches!(matches_entry(dir, entry).await, Ok(true)) {
                damaged.push(entry.path.clone());
            }
        }
        damaged
    }
}

/// Whether the file of `entry` is in `dir` with the entry's length and hash. A file that is not there is `Missing`.
async fn matches_entry(dir: &Path, entry: &Entry) -> Result<bool, BackupError> {
    let (len, hash) = hash_file(&dir.join(&entry.path))
        .await
        .map_err(|_| BackupError::Missing(entry.path.clone()))?;
    Ok(len == entry.bytes && hash == entry.hash)
}

/// A row of `lineage.json`. The generation is a decimal string: a random 63-bit number is not safe in a JSON number.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageRow {
    pub pos: i32,
    pub generation: String,
    pub cut_ms: u64,
}

/// A row of `deletions.json` (ADR-0001 §8.9): the hash of a deleted account's id and when it was deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deletion {
    /// Lower-case hex of `crypto_generichash(account_id)`.
    pub account_hash: String,
    pub deleted_ms: u64,
}

/// A row of `credentials.json` (ADR-0001 §8.9): the credentials an account had after a change, with the time. Bytes are lower-case hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRow {
    /// Lower-case hex of `crypto_generichash(account_id)`.
    pub account_hash: String,
    pub cred_version: u64,
    pub salt: String,
    pub profile: i16,
    pub auth_hash: String,
    pub rc_hash: String,
    pub key_id: i32,
    pub wrap_pw: String,
    pub wrap_rc: String,
    pub changed_ms: u64,
}

pub fn table_path(table: &str) -> String {
    format!("tables/{table}.copy")
}

/// The length and the hash of a file, read in pieces.
pub async fn hash_file(path: &Path) -> std::io::Result<(u64, String)> {
    let mut file = File::open(path).await?;
    let mut hasher = Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut len = 0u64;
    loop {
        let read = file.read(&mut buffer).await?;
        let Some(filled) = buffer.get(..read).filter(|filled| !filled.is_empty()) else {
            break;
        };
        hasher.update(filled);
        len = len.saturating_add(u64::try_from(read).unwrap_or(0));
    }
    Ok((len, hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Manifest {
        let mut files: Vec<Entry> = TABLES
            .iter()
            .map(|t| Entry {
                path: table_path(t),
                bytes: 0,
                hash: "0".repeat(64),
            })
            .collect();
        for name in [LINEAGE_FILE, DELETIONS_FILE, CREDENTIALS_FILE] {
            files.push(Entry {
                path: name.to_string(),
                bytes: 0,
                hash: "0".repeat(64),
            });
        }
        Manifest {
            format: FORMAT,
            schema: 2,
            cut_ms: 1,
            server_version: "0.0.0".to_string(),
            files,
            blobs: Vec::new(),
        }
    }

    #[test]
    fn a_manifest_of_the_right_shape_is_accepted_and_each_deviation_is_refused() {
        assert!(sample().judge().is_ok());
        let mut other_format = sample();
        other_format.format = 9;
        let mut missing_file = sample();
        missing_file.files.pop();
        let mut unknown_file = sample();
        unknown_file.files.push(Entry {
            path: "../escape".to_string(),
            bytes: 0,
            hash: "0".repeat(64),
        });
        let mut traversal = sample();
        traversal.blobs.push(Entry {
            path: "blobs/../../etc/passwd".to_string(),
            bytes: 0,
            hash: "0".repeat(64),
        });
        let mut short_hash = sample();
        if let Some(entry) = short_hash.files.first_mut() {
            entry.hash = "abc".to_string();
        }
        for (name, bad) in [
            ("format", other_format),
            ("missing", missing_file),
            ("unknown", unknown_file),
            ("traversal", traversal),
            ("hash", short_hash),
        ] {
            assert!(bad.judge().is_err(), "{name}");
        }
    }
}
