//! The leftovers of the old settings file (slice 5d, ADR-0022): the temporary files `.settings.json.<32 hex>.tmp` that the
//! atomic writer of slice 4b could leave next to `kinige.db` after a crash. Kinige no longer writes or reads `settings.json`;
//! it removes only these files of its own and never the file itself.

use crate::Store;
use std::fs;
use std::path::Path;

const TMP_PREFIX: &str = ".settings.json.";
const TMP_SUFFIX: &str = ".tmp";
/// `Uuid::new_v4().simple()`: 32 lowercase hex digits.
const TMP_HEX_LEN: usize = 32;

impl Store {
    /// Removes the old temporary files from the data directory (the one that holds the database). Idempotent. Only regular
    /// files of exactly that name shape go: a hyphenated look-alike, a symlink and a directory stay, no link is followed, an
    /// error on one file is skipped. Returns how many were removed; the log gets that count and nothing else.
    pub fn remove_settings_file_leftovers(&self) -> usize {
        let Some(dir) = self.inner.db_path.parent() else {
            return 0;
        };
        let removed = remove_leftovers(dir);
        if removed > 0 {
            log::info!("removed {removed} leftover temporary files of the old settings file");
        }
        removed
    }
}

fn remove_leftovers(dir: &Path) -> usize {
    let Ok(entries) = fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .filter(|entry| {
            // `DirEntry::file_type` does not follow a symlink: a link reports as a link, not as a regular file.
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && entry.file_name().to_str().is_some_and(is_leftover_name)
        })
        .filter(|entry| fs::remove_file(entry.path()).is_ok())
        .count()
}

fn is_leftover_name(name: &str) -> bool {
    name.strip_prefix(TMP_PREFIX)
        .and_then(|rest| rest.strip_suffix(TMP_SUFFIX))
        .is_some_and(|hex| {
            hex.len() == TMP_HEX_LEN && hex.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_name_shape_matches() {
        let hex = "0123456789abcdef0123456789abcdef";
        let short = "0123456789abcdef0123456789abcde";
        assert!(is_leftover_name(&format!(".settings.json.{hex}.tmp")));
        for wrong in [
            "settings.json".to_string(),
            ".settings.json".to_string(),
            format!(".settings.json.{}.tmp", hex.to_uppercase()),
            format!(".settings.json.{short}.tmp"),
            format!(".settings.json.{hex}0.tmp"),
            format!(".settings.json.{hex}.tmp.bak"),
            format!("x.settings.json.{hex}.tmp"),
            ".settings.json.01234567-89ab-cdef-0123-456789abcdef.tmp".to_string(),
            format!(".settings.json.{short}g.tmp"),
            String::new(),
        ] {
            assert!(!is_leftover_name(&wrong), "{wrong}");
        }
    }
}
