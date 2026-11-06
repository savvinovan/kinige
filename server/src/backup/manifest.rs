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

    /// Checks every file of the directory against its entry: it exists, has the length and the hash. Nothing is changed.
    pub async fn verify(&self, dir: &Path) -> Result<(), BackupError> {
        for entry in self.files.iter().chain(&self.blobs) {
            let path = dir.join(&entry.path);
            let (len, hash) = hash_file(&path)
                .await
                .map_err(|_| BackupError::Missing(entry.path.clone()))?;
            if len != entry.bytes || hash != entry.hash {
                return Err(BackupError::Hash(entry.path.clone()));
            }
        }
        Ok(())
    }
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
        for name in [LINEAGE_FILE, DELETIONS_FILE] {
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
