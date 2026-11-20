//! Where the Telegram bot token lives (slice 5c, ADR-0021, PRD §9.2): the OS secret store, nothing else.
//!   macOS: a generic-password item, service `me.savvinov.kinige` (`.dev` in a debug build), account `telegram-bot-token`, not synchronizable;
//!   Windows and Linux: no store in this slice, every call answers `Unsupported` (Credential Manager is not shipped, ADR-0021; Secret Service
//!   comes with the Linux build, slice 10).
//! There is no file fallback. A call may block on a system prompt (macOS asks once after an update, because every ad-hoc build is a new
//! program for the keychain), so it never runs on the main thread, at startup, or on opening Settings or the panel: the commands call it
//! through `spawn_blocking`, and only a pack download reads the token.

use std::sync::Arc;

/// Why the store did not answer. A category only: the system's own message may name the item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretError {
    /// The system asked for access and it was dismissed or denied, or the item belongs to another build and cannot be changed.
    Denied,
    /// A locked or missing keychain, or any other failure.
    Unavailable,
    /// This system has no store yet (Windows, Linux): nothing is saved, nothing is read, there is nothing to delete.
    Unsupported,
}

/// The token store. `get` of a missing item is `Ok(None)`; `delete` of a missing item is `Ok(())`.
pub trait SecretStore: Send + Sync {
    fn get(&self) -> Result<Option<String>, SecretError>;
    fn set(&self, token: &str) -> Result<(), SecretError>;
    fn delete(&self) -> Result<(), SecretError>;
}

/// The names of the item.
#[cfg(target_os = "macos")]
const ACCOUNT: &str = "telegram-bot-token";
#[cfg(target_os = "macos")]
fn service() -> &'static str {
    if cfg!(debug_assertions) {
        "me.savvinov.kinige.dev"
    } else {
        "me.savvinov.kinige"
    }
}

/// The store of this platform; with the `test-hooks` feature and `KINIGE_TEST_TOKEN_STORE=memory` (the screens runner) an in-memory one.
pub fn production() -> Arc<dyn SecretStore> {
    #[cfg(feature = "test-hooks")]
    if std::env::var("KINIGE_TEST_TOKEN_STORE").is_ok_and(|value| value == "memory") {
        log::warn!("kinige-bot-test-hooks-5c: the bot token is kept in memory");
        return Arc::new(MemorySecrets::default());
    }
    Arc::new(OsSecrets)
}

/// The OS store.
pub struct OsSecrets;

#[cfg(target_os = "macos")]
impl SecretStore for OsSecrets {
    fn get(&self) -> Result<Option<String>, SecretError> {
        match generic_password(options()) {
            Ok(bytes) => Ok(String::from_utf8(bytes).ok()),
            Err(error) => classify(error.code()).map(|()| None),
        }
    }

    fn set(&self, token: &str) -> Result<(), SecretError> {
        set_generic_password_options(token.as_bytes(), options())
            .or_else(|error| classify(error.code()))
    }

    fn delete(&self) -> Result<(), SecretError> {
        delete_generic_password_options(options()).or_else(|error| classify(error.code()))
    }
}

#[cfg(target_os = "macos")]
use security_framework::passwords::{
    PasswordOptions, delete_generic_password_options, generic_password,
    set_generic_password_options,
};

#[cfg(target_os = "macos")]
fn options() -> PasswordOptions {
    let mut options = PasswordOptions::new_generic_password(service(), ACCOUNT);
    options.set_access_synchronized(Some(false));
    options
}

/// `Ok(())` for "no such item", `Denied` for a prompt that was dismissed or could not be shown and for an item another build owns,
/// `Unavailable` for the rest. The codes are the ones the check in `spikes/5c-keychain` saw or the headers name.
#[cfg(target_os = "macos")]
fn classify(code: i32) -> Result<(), SecretError> {
    const ITEM_NOT_FOUND: i32 = -25300;
    const USER_CANCELED: i32 = -128;
    const AUTH_FAILED: i32 = -25293;
    const INTERACTION_NOT_ALLOWED: i32 = -25308;
    const INVALID_OWNER_EDIT: i32 = -25244;
    match code {
        ITEM_NOT_FOUND => Ok(()),
        USER_CANCELED | AUTH_FAILED | INTERACTION_NOT_ALLOWED | INVALID_OWNER_EDIT => {
            Err(SecretError::Denied)
        }
        _ => Err(SecretError::Unavailable),
    }
}

/// Windows and Linux: no store in this slice (ADR-0021). The Windows Credential Manager entry of the `keyring` crate persists as
/// "enterprise" (it can roam with a domain account) and no job has ever run that code, so it is not shipped; Linux has no Secret Service
/// yet (slice 10). The token cannot be saved there, and a pack cannot be added by its link; the stickers of a Telegram export work
/// everywhere, because they need no token.
#[cfg(not(target_os = "macos"))]
impl SecretStore for OsSecrets {
    fn get(&self) -> Result<Option<String>, SecretError> {
        Err(SecretError::Unsupported)
    }

    fn set(&self, _token: &str) -> Result<(), SecretError> {
        Err(SecretError::Unsupported)
    }

    fn delete(&self) -> Result<(), SecretError> {
        Err(SecretError::Unsupported)
    }
}

/// A store in memory that counts its calls: for the tests and the screens runner (`test-hooks` only, never in a build for people).
#[cfg(feature = "test-hooks")]
#[doc(hidden)]
#[derive(Default)]
pub struct MemorySecrets {
    token: std::sync::Mutex<Option<String>>,
    fail: std::sync::Mutex<Option<SecretError>>,
    calls: std::sync::atomic::AtomicUsize,
}

#[cfg(feature = "test-hooks")]
impl MemorySecrets {
    /// How many times any method was called: a test proves "nothing at startup, on opening Settings or the panel" with it.
    pub fn calls(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The token as saved, for a test that checks where it is not.
    pub fn peek(&self) -> Option<String> {
        self.token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Every following call fails with `error` (`None` stops it).
    pub fn fail_with(&self, error: Option<SecretError>) {
        *self
            .fail
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = error;
    }

    fn enter(&self) -> Result<(), SecretError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match *self
            .fail
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

#[cfg(feature = "test-hooks")]
impl SecretStore for MemorySecrets {
    fn get(&self) -> Result<Option<String>, SecretError> {
        self.enter()?;
        Ok(self.peek())
    }

    fn set(&self, token: &str) -> Result<(), SecretError> {
        self.enter()?;
        *self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(token.to_string());
        Ok(())
    }

    fn delete(&self) -> Result<(), SecretError> {
        self.enter()?;
        *self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }
}
