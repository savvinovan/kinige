//! Unfinished uploads (ADR-0001 §7, "Upload"). A blob is not in the store until it is complete and its framing has been checked, so what is
//! half uploaded is a file in the staging directory: `<account_id>/<blob_id>.<declared size>.part`. The declared size is in the name, so
//! the database holds nothing about an unfinished upload and a backup never has to copy a file that is still growing.
//!
//! One upload per id: a request holds the id's busy flag (see [`Busy`]) while it writes.

use crate::rules::blob::Staged;
use std::collections::HashSet;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime};
use tokio::fs::{self, File, OpenOptions};
use tokio::io::AsyncSeekExt;

const SUFFIX: &str = ".part";

#[derive(Clone)]
pub struct Staging {
    dir: PathBuf,
    busy: Arc<Mutex<HashSet<String>>>,
}

/// The flag that says a request is writing to an id. Dropping it clears the flag.
pub struct Busy {
    busy: Arc<Mutex<HashSet<String>>>,
    key: String,
}

/// An unfinished upload on disk.
#[derive(Debug, Clone)]
pub struct Found {
    pub path: PathBuf,
    pub state: Staged,
}

impl Staging {
    pub fn new(dir: PathBuf) -> Staging {
        Staging {
            dir,
            busy: Arc::default(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Takes the id's flag, or `None` if another request holds it (the caller answers 409: one writer per upload).
    pub fn try_busy(&self, account: &str, blob: &str) -> Option<Busy> {
        let key = format!("{account}/{blob}");
        let mut held = self.busy.lock().unwrap_or_else(PoisonError::into_inner);
        held.insert(key.clone()).then(|| Busy {
            busy: Arc::clone(&self.busy),
            key,
        })
    }

    /// The unfinished upload of `blob`, if there is one.
    pub async fn find(&self, account: &str, blob: &str) -> Option<Found> {
        self.entries(account)
            .await
            .into_iter()
            .find(|entry| entry.blob == blob)
            .map(|entry| entry.found)
    }

    /// Opens the file for an upload of `size` bytes in total, with `keep` bytes of an earlier attempt kept (0 starts over). The file is
    /// positioned at `keep`. Anything else staged under this id is removed first.
    pub async fn open(
        &self,
        account: &str,
        blob: &str,
        size: u64,
        keep: u64,
    ) -> std::io::Result<(File, PathBuf)> {
        let path = self.path(account, blob, size);
        for entry in self.entries(account).await {
            if entry.blob == blob && (keep == 0 || entry.found.path != path) {
                let _ = fs::remove_file(&entry.found.path).await;
            }
        }
        fs::create_dir_all(self.dir.join(account)).await?;
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(keep == 0)
            .open(&path)
            .await?;
        if keep > 0 {
            file.set_len(keep).await?;
        }
        file.seek(SeekFrom::Start(keep)).await?;
        Ok((file, path))
    }

    pub async fn remove(&self, account: &str, blob: &str) {
        for entry in self.entries(account).await {
            if entry.blob == blob {
                let _ = fs::remove_file(&entry.found.path).await;
            }
        }
    }

    /// Removes everything staged for an account (its deletion).
    pub async fn remove_account(&self, account: &str) {
        let _ = fs::remove_dir_all(self.dir.join(account)).await;
    }

    /// What the account's unfinished uploads declare in total, `except` one id (the one being written counts by its new size).
    pub async fn declared_total(&self, account: &str, except: &str) -> u64 {
        self.entries(account)
            .await
            .iter()
            .filter(|entry| entry.blob != except)
            .fold(0u64, |sum, entry| {
                sum.saturating_add(entry.found.state.declared)
            })
    }

    /// Removes the unfinished uploads nobody has written to for `older_than`; returns how many.
    pub async fn remove_older_than(&self, older_than: Duration) -> usize {
        let mut removed = 0;
        let Ok(mut accounts) = fs::read_dir(&self.dir).await else {
            return 0;
        };
        while let Ok(Some(account)) = accounts.next_entry().await {
            let name = account.file_name().to_string_lossy().into_owned();
            for entry in self.entries(&name).await {
                let age = fs::metadata(&entry.found.path)
                    .await
                    .and_then(|meta| meta.modified())
                    .ok()
                    .and_then(|modified| SystemTime::now().duration_since(modified).ok());
                if age.is_some_and(|age| age >= older_than)
                    && fs::remove_file(&entry.found.path).await.is_ok()
                {
                    removed += 1;
                }
            }
            // An account's directory with nothing left in it goes too.
            let _ = fs::remove_dir(account.path()).await;
        }
        removed
    }

    fn path(&self, account: &str, blob: &str, size: u64) -> PathBuf {
        self.dir
            .join(account)
            .join(format!("{blob}.{size}{SUFFIX}"))
    }

    async fn entries(&self, account: &str) -> Vec<Entry> {
        let mut found = Vec::new();
        let Ok(mut dir) = fs::read_dir(self.dir.join(account)).await else {
            return found;
        };
        while let Ok(Some(item)) = dir.next_entry().await {
            let name = item.file_name().to_string_lossy().into_owned();
            let Some((blob, declared)) = parse_name(&name) else {
                continue;
            };
            let received = item.metadata().await.map_or(0, |meta| meta.len());
            found.push(Entry {
                blob,
                found: Found {
                    path: item.path(),
                    state: Staged { declared, received },
                },
            });
        }
        found
    }
}

struct Entry {
    blob: String,
    found: Found,
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.busy
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.key);
    }
}

/// `<blob_id>.<size>.part` -> (`blob_id`, size).
fn parse_name(name: &str) -> Option<(String, u64)> {
    let stem = name.strip_suffix(SUFFIX)?;
    let (blob, size) = stem.rsplit_once('.')?;
    Some((blob.to_string(), size.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_staged_name_carries_the_blob_id_and_the_declared_size() {
        let id = "00000001-0000-4000-8000-000000000001";
        assert_eq!(
            parse_name(&format!("{id}.1065.part")),
            Some((id.to_string(), 1065))
        );
        for bad in ["", "x.part", "x.1.partial", &format!("{id}.big.part")] {
            assert_eq!(parse_name(bad), None, "{bad}");
        }
    }

    #[test]
    fn one_request_at_a_time_writes_to_an_id() {
        let staging = Staging::new(PathBuf::from("/nonexistent/kinige-test"));
        let first = staging.try_busy("a", "b");
        assert!(first.is_some());
        assert!(staging.try_busy("a", "b").is_none());
        assert!(staging.try_busy("a", "c").is_some());
        drop(first);
        assert!(staging.try_busy("a", "b").is_some());
    }
}
