//! Failures injected by tests (ENOSPC, a source that changes while it is copied, a crash between rename and commit, a failed
//! deletion). Off by default; the checks are single atomic loads. The pattern of `mirror::inject_panic`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// A step a test runs inside the pipeline.
pub(crate) type Hook = Arc<dyn Fn() + Send + Sync>;

/// A failure a test asks for. Only with the `test-hooks` feature: a build for people cannot ask for one.
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
#[derive(Clone)]
pub enum AttachFault {
    /// The write fails with "no space left" once this many bytes of a file have been written.
    DiskFullAfter(u64),
    /// A step after the first chunk of every copy (the test changes the source in it).
    MidCopy(Hook),
    /// A step right after the stored file is renamed into place and before the row is committed (the test panics in it:
    /// the writer job is lost like a killed process, the transaction rolls back).
    AfterRename(Hook),
    /// Deleting files after a purge fails.
    DeleteFails,
    /// The next thumbnail decodes into a panic (debug builds only).
    DecoderPanics,
    /// Switches every injected failure off.
    Clear,
}

pub(crate) struct Faults {
    disk_full_after: AtomicU64,
    delete_fails: AtomicBool,
    decoder_panics: AtomicBool,
    mid_copy: Mutex<Option<Hook>>,
    after_rename: Mutex<Option<Hook>>,
}

impl Default for Faults {
    fn default() -> Self {
        Faults {
            disk_full_after: AtomicU64::new(u64::MAX),
            delete_fails: AtomicBool::new(false),
            decoder_panics: AtomicBool::new(false),
            mid_copy: Mutex::new(None),
            after_rename: Mutex::new(None),
        }
    }
}

impl Faults {
    #[cfg(feature = "test-hooks")]
    pub(crate) fn set(&self, fault: AttachFault) {
        match fault {
            AttachFault::DiskFullAfter(bytes) => {
                self.disk_full_after.store(bytes, Ordering::SeqCst);
            }
            AttachFault::MidCopy(hook) => *lock(&self.mid_copy) = Some(hook),
            AttachFault::AfterRename(hook) => *lock(&self.after_rename) = Some(hook),
            AttachFault::DeleteFails => self.delete_fails.store(true, Ordering::SeqCst),
            AttachFault::DecoderPanics => self.decoder_panics.store(true, Ordering::SeqCst),
            AttachFault::Clear => {
                self.disk_full_after.store(u64::MAX, Ordering::SeqCst);
                self.delete_fails.store(false, Ordering::SeqCst);
                self.decoder_panics.store(false, Ordering::SeqCst);
                *lock(&self.mid_copy) = None;
                *lock(&self.after_rename) = None;
            }
        }
    }

    /// Whether a write that brings a file to `written` bytes must fail with "no space left".
    pub(crate) fn disk_full(&self, written: u64) -> bool {
        written > self.disk_full_after.load(Ordering::SeqCst)
    }

    pub(crate) fn delete_fails(&self) -> bool {
        self.delete_fails.load(Ordering::SeqCst)
    }

    pub(crate) fn decoder_panics(&self) -> bool {
        self.decoder_panics.load(Ordering::SeqCst)
    }

    pub(crate) fn run_mid_copy(&self) {
        run(&self.mid_copy);
    }

    pub(crate) fn run_after_rename(&self) {
        run(&self.after_rename);
    }
}

fn run(slot: &Mutex<Option<Hook>>) {
    let hook = lock(slot).clone();
    if let Some(hook) = hook {
        hook();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
