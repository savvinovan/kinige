//! Reconciling the file and the database (ADR-0010): reading, applying an external edit, the "hash matched -- the database wins" rule,
//! writing the file from the state.

use super::apply::{ApplyInput, KEY_FILE_HASH};
use super::engine::{Engine, ShortcutRefusal};
use super::io::{self, ReadOutcome};
use super::parse::{Parsed, parse};
use crate::Now;
use crate::agent_settings::put;
use crate::error::write_err;
use kinige_domain::{
    FileProblem, FileProblemKind, KeyIssue, KeyIssueKind, SettingsFileState, ShortcutKind,
    UnsafeReason,
};
use sha2::{Digest, Sha256};
use std::sync::PoisonError;
use std::time::{Duration, Instant};

impl Engine {
    pub(super) fn tick(&mut self) {
        // A deferred write means something only while the file can be rewritten: a broken, unsafe or unreadable file
        // is re-read only when it changed (the signature), not on every poll.
        let due = self.retry_at.is_none_or(|at| Instant::now() >= at);
        let retry_write = self.pending_write && self.writable && due;
        if io::signature(&self.path) == self.last_sig && !retry_write {
            return;
        }
        self.reconcile();
    }

    /// Writing the file failed: the next attempt is later (2, 4, … 60 seconds), not on every poll.
    fn write_failed(&mut self) {
        self.pending_write = true;
        self.write_failures = self.write_failures.saturating_add(1);
        let wait = 2u64.saturating_pow(self.write_failures.min(6)).min(60);
        self.retry_at = Some(Instant::now() + Duration::from_secs(wait));
    }

    /// A warning to the process log once, until the same kind of error has succeeded (`forget`): repeated passes
    /// stay silent (otherwise a broken file or a busy database would give a line on every poll). Kinds are independent: alternating two
    /// errors adds no lines. Categories only, no data.
    fn warn_once(&mut self, key: String, text: &str) {
        if self.logged.insert(key.clone()) {
            log::warn!("{text}: {key}");
        }
    }

    /// A kind of error (the key prefix) has succeeded: the next failure will get into the log again.
    fn forget(&mut self, prefix: &str) {
        self.logged.retain(|key| !key.starts_with(prefix));
    }

    /// Reads the file and brings it and the database into agreement. The hash matches the last synchronized one -- no external edit, the database
    /// wins (on every read, not only at startup); it does not match -- the file is applied as an external edit.
    pub(super) fn reconcile(&mut self) {
        match io::read_file(&self.path, &self.dir) {
            ReadOutcome::Missing => {
                self.forget("read");
                self.forget("parse");
                self.forget("unsafe");
                self.last_sig = None;
                self.base = None;
                self.writable = true;
                let directory = io::unsafe_directory(&self.dir);
                self.update(|state| {
                    state.problem = None;
                    state.unsafe_reason = directory;
                    state.issues.clear();
                    state.unknown_keys.clear();
                    state.unknown_more = 0;
                });
                self.write_from_state();
            }
            ReadOutcome::Failed(error) => {
                self.warn_once(
                    format!("read {}", error.category()),
                    "settings file not read",
                );
                self.last_sig = io::signature(&self.path);
                self.writable = false;
                self.base = None;
                self.update(|state| {
                    state.problem = Some(error.problem());
                    state.unsafe_reason = None;
                });
            }
            ReadOutcome::NotRegular => {
                self.warn_once("unsafe".into(), "settings file is not a regular file");
                self.last_sig = io::signature(&self.path);
                self.writable = false;
                self.base = None;
                self.update(|state| {
                    state.problem = None;
                    state.unsafe_reason = Some(UnsafeReason::NotRegular);
                });
            }
            ReadOutcome::Content {
                bytes,
                unsafe_reason,
                sig,
            } => {
                self.last_sig = sig;
                match parse(&bytes) {
                    Err(error) => {
                        self.warn_once(
                            format!("parse {}", error.category()),
                            "settings file not parsed",
                        );
                        self.writable = false;
                        self.base = None;
                        self.update(|state| {
                            state.problem = Some(error.problem());
                            state.unsafe_reason = unsafe_reason;
                            state.issues.clear();
                            state.unknown_keys.clear();
                            state.unknown_more = 0;
                        });
                    }
                    Ok(parsed) => {
                        self.forget("read");
                        self.forget("parse");
                        self.forget("unsafe");
                        self.writable = unsafe_reason.is_none();
                        self.base = Some(parsed.raw.clone());
                        let hash = sha256_hex(&bytes);
                        if self.stored_hash.as_deref() == Some(hash.as_str()) {
                            self.settle(parsed, unsafe_reason);
                        } else {
                            self.apply_external(parsed, hash, unsafe_reason);
                        }
                    }
                }
            }
        }
    }

    /// An external edit: everything valid is applied. An unsafe file is applied without `agent_access.*` and `mirror.folder`.
    fn apply_external(
        &mut self,
        parsed: Parsed,
        hash: String,
        unsafe_reason: Option<UnsafeReason>,
    ) {
        let unknown = parsed.unknown.clone();
        let unknown_more = parsed.unknown_more as u32;
        let input = ApplyInput {
            parsed,
            trusted: unsafe_reason.is_none(),
            data_dir: self.dir.clone(),
            hash: hash.clone(),
            at_ms: Now::system().ms,
        };
        let outcome = match self.store.apply_settings_file(input) {
            Ok(outcome) => outcome,
            Err(error) => {
                // Nothing was applied: the hash did not move, the next poll will try again.
                self.warn_once(
                    format!("apply {}", error.category()),
                    "settings file not applied",
                );
                self.last_sig = None;
                return;
            }
        };
        self.stored_hash = Some(hash);
        self.forget("apply");
        if outcome.mirror {
            // The copy folder changed: the copy writer wakes up and writes the days to the new folder at once.
            self.store.mirror_dir_changed();
        }
        // An edit from the file is newer than the window's deferred edit: the latest one wins.
        self.pending_write = false;
        let mut issues = outcome.issues;
        let mut shortcuts_changed = false;
        for (kind, accelerator) in outcome.shortcuts {
            match self.apply_shortcut(kind, &accelerator) {
                Ok(()) => shortcuts_changed = true,
                Err(refusal) => issues.push(KeyIssue {
                    key: format!("shortcuts.{}", shortcut_name(kind)),
                    kind: match refusal {
                        ShortcutRefusal::Invalid => KeyIssueKind::ShortcutInvalid,
                        ShortcutRefusal::Duplicate => KeyIssueKind::ShortcutDuplicate,
                        ShortcutRefusal::Taken => KeyIssueKind::ShortcutTaken,
                    },
                }),
            }
        }
        let now = Now::system().ms;
        self.update(|state| {
            state.problem = None;
            state.unsafe_reason = unsafe_reason;
            state.issues = issues;
            state.unknown_keys = unknown;
            state.unknown_more = unknown_more;
            let stamp = |flag: bool, slot: &mut Option<i64>| {
                if flag {
                    *slot = Some(now);
                }
            };
            stamp(outcome.appearance, &mut state.changed.appearance);
            stamp(shortcuts_changed, &mut state.changed.shortcuts);
            stamp(outcome.mirror, &mut state.changed.mirror);
            stamp(outcome.access, &mut state.changed.access);
        });
        self.notify();
    }

    fn apply_shortcut(&self, kind: ShortcutKind, accelerator: &str) -> Result<(), ShortcutRefusal> {
        use crate::StoreError;
        match (&self.options.shortcuts, self.initial) {
            (Some(applier), false) => applier.apply(&self.store, kind, accelerator),
            // Startup: shortcuts are registered later (`register_saved`); for now only the check and saving.
            _ => self
                .store
                .save_shortcut(kind, accelerator)
                .map_err(|error| match error {
                    StoreError::ShortcutDuplicate => ShortcutRefusal::Duplicate,
                    _ => ShortcutRefusal::Invalid,
                }),
        }
    }

    /// The hash matched: no external edit. If the safe and parsed file differs from the state (a window edit did not reach
    /// the file: an interruption, an unsafe state), it is rewritten from the database.
    fn settle(&mut self, parsed: Parsed, unsafe_reason: Option<UnsafeReason>) {
        let in_sync = match self.store.effective_state() {
            Ok(state) => {
                self.forget("state");
                state.matches(&parsed)
            }
            Err(error) => {
                self.warn_once(
                    format!("state {}", error.category()),
                    "settings state not read",
                );
                true
            }
        };
        let rewrite = unsafe_reason.is_none() && (self.pending_write || !in_sync);
        if !rewrite {
            self.update(|state| {
                state.problem = None;
                state.unsafe_reason = unsafe_reason;
                // Parse errors are recomputed, state errors (folder, chats, system) stay until the next edit.
                state.issues.retain(|issue| !is_parse_level(issue.kind));
                for issue in parsed.issues {
                    if !state.issues.contains(&issue) {
                        state.issues.push(issue);
                    }
                }
                state.unknown_keys = parsed.unknown;
                state.unknown_more = parsed.unknown_more as u32;
            });
            return;
        }
        self.write_from_state();
    }

    /// After a window edit: the file is rewritten from the database if it can be rewritten, otherwise the write is deferred.
    pub(super) fn after_edit(&mut self) {
        if self.writable {
            self.write_from_state();
        } else {
            self.pending_write = true;
        }
        self.notify();
    }

    /// The file from the current state: a temporary file, `fsync`, `rename`; the hash of the written bytes -- after `rename`.
    fn write_from_state(&mut self) {
        let state = match self.store.effective_state() {
            Ok(state) => state,
            Err(error) => {
                self.warn_once(
                    format!("write-state {}", error.category()),
                    "settings file not written",
                );
                self.write_failed();
                return;
            }
        };
        self.forget("write-state");
        let bytes = state.render(self.base.as_ref());
        let hash = sha256_hex(&bytes);
        if let Err(error) = io::write_atomic(&self.path, &self.dir, &bytes) {
            self.warn_once(
                format!("write-io {:?}", error.kind()),
                "settings file not written",
            );
            self.write_failed();
            self.update(|state| {
                state.problem = Some(FileProblem {
                    kind: FileProblemKind::Io,
                    line: None,
                    column: None,
                });
            });
            return;
        }
        self.forget("write-io");
        // The signature is taken right after `rename`, before the hash step: if the file is replaced later, the signature changes and the poll notices it.
        self.last_sig = io::signature(&self.path);
        if let Some(step) = &self.options.after_rename {
            step();
        }
        let stored = hash.clone();
        let saved = self
            .store
            .write(move |conn| put(conn, KEY_FILE_HASH, &stored).map_err(write_err));
        if let Err(error) = saved {
            self.warn_once(
                format!("hash {}", error.category()),
                "settings file hash not written",
            );
        } else {
            self.stored_hash = Some(hash.clone());
            self.forget("hash");
        }
        self.pending_write = false;
        self.write_failures = 0;
        self.retry_at = None;
        // If the disk no longer holds what was written (an agent managed to replace the file), it will be read and applied by the next poll.
        // It is read with the same safe read (no following links, pipes and devices are not read).
        let same = matches!(
            io::read_file(&self.path, &self.dir),
            ReadOutcome::Content { bytes, .. } if sha256_hex(&bytes) == hash
        );
        if !same {
            self.last_sig = None;
        }
        let (unknown, unknown_more) = parse(&bytes)
            .map(|parsed| (parsed.unknown, parsed.unknown_more as u32))
            .unwrap_or_default();
        // The data directory could turn out unsafe while the file itself is fine: the reason stays visible.
        let directory = io::unsafe_directory(&self.dir);
        self.update(|state| {
            state.problem = None;
            state.unsafe_reason = directory;
            state.issues.clear();
            state.unknown_keys = unknown;
            state.unknown_more = unknown_more;
        });
    }

    fn update(&self, change: impl FnOnce(&mut SettingsFileState)) {
        let mut state = self.status.lock().unwrap_or_else(PoisonError::into_inner);
        change(&mut state);
    }

    fn notify(&self) {
        if !self.initial {
            (self.options.on_change)();
        }
    }
}

fn is_parse_level(kind: KeyIssueKind) -> bool {
    matches!(
        kind,
        KeyIssueKind::Type | KeyIssueKind::Value | KeyIssueKind::ShortcutInvalid
    )
}

fn shortcut_name(kind: ShortcutKind) -> &'static str {
    match kind {
        ShortcutKind::Capture => "capture",
        ShortcutKind::Clipboard => "clipboard",
    }
}

/// SHA-256 in hex.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
