//! Recovery with the 24 words and reissuing them (ADR-0001 §8.6, appendix §A.5).

use super::{Client, ClientError, Pending, PendingReissue};
use crate::link::Link;
use crate::server::{RecoverError, Recovery};
use kinige_crypto::ids::normalize_login;
use kinige_crypto::keys::{self, Dek};
use kinige_crypto::secret::Password;
use kinige_crypto::words::Words;
use kinige_crypto::wrap::{unwrap_rc, wrap_rc};
use kinige_crypto::{AccountId, Profile};

/// A recovery that is prepared and saved but not yet confirmed by the server.
#[derive(Clone)]
pub struct PendingRecovery {
    pub account_id: AccountId,
    pub login: String,
    pub request: Recovery,
    pub dek: Dek,
    pub profile: Profile,
    pub saved: bool,
}

/// What a restart that finds an unfinished recovery learns by trying the new password (§A.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resumed {
    /// The login with the new password worked: the replacement went through.
    Applied,
    /// It did not: the old words have to be entered again.
    NotApplied,
}

impl Client {
    /// Steps 1-3 of §8.6. The words give `rc_auth_key`; the server returns `wrap_rc` (no session) and the DEK comes out. A new password
    /// (new salt, `auth_key`, `wrap_pw`, profile `max(seen, sent, default)`) and **new words** (new `rc_auth_key`, `wrap_rc`) are made,
    /// and the package is saved before the new words can be shown. Nothing has changed on the server yet: if the client dies now, the
    /// old words still work.
    pub fn begin_recovery(
        &mut self,
        link: &mut Link<'_>,
        login: &str,
        old_words: &str,
        new_password: &Password,
    ) -> Result<(), ClientError> {
        let login = normalize_login(login).ok_or(ClientError::Invalid)?;
        let old = Words::parse(old_words)?.rc_root()?;
        let old_rc_auth_key = keys::rc_auth_key(&old)?;
        let fetch = link
            .call(|server| server.recovery_fetch(&login, &old_rc_auth_key))?
            .map_err(|_| ClientError::Auth)?;
        let dek = unwrap_rc(
            &fetch.wrap_rc,
            &keys::rc_kek(&old)?,
            &fetch.account_id,
            fetch.key_id,
        )?;

        let profile = Profile::for_new_password(
            fetch.profile,
            self.store.kdf_profile_seen,
            self.default_profile,
        )?;
        let password = self.password_part(new_password, &dek, &fetch.account_id, profile)?;
        let new_words = Words::generate()?;
        let new_root = new_words.rc_root()?;
        let request = Recovery {
            password,
            rc_auth_key: keys::rc_auth_key(&new_root)?,
            wrap_rc: wrap_rc(&dek, &keys::rc_kek(&new_root)?, &fetch.account_id)?,
        };
        self.store.pending = Some(Pending::Recovery(PendingRecovery {
            account_id: fetch.account_id,
            login,
            request,
            dek,
            profile,
            saved: false,
        }));
        self.memory.words = Some(new_words);
        self.memory.old_rc_auth_key = Some(old_rc_auth_key);
        self.log.event("client recovery prepared");
        Ok(())
    }

    /// Step 4: the replacement is requested only after "Save". A lost answer leaves the package; sending it again is idempotent by the
    /// new `rc_auth_key` (§A.5). After a restart the old `rc_auth_key` is gone: use [`Client::resume_recovery_after_restart`].
    pub fn send_recovery(&mut self, link: &mut Link<'_>) -> Result<(), ClientError> {
        let Some(Pending::Recovery(pending)) = &self.store.pending else {
            return Err(ClientError::NothingPending);
        };
        if !pending.saved {
            self.log.event("client recovery held: words not saved");
            return Err(ClientError::SaveNotConfirmed);
        }
        let old = self
            .memory
            .old_rc_auth_key
            .clone()
            .ok_or(ClientError::Auth)?;
        let pending = pending.clone();
        let answer = link.call(|server| server.recover(&pending.login, &old, &pending.request))?;
        match answer {
            Ok(session) => {
                self.set_account(
                    pending.account_id,
                    pending.login,
                    pending.dek,
                    session.token,
                    pending.profile,
                );
                self.log.event("client recovered");
                Ok(())
            }
            Err(RecoverError::AuthFailed) => Err(ClientError::Auth),
            Err(RecoverError::PackageMismatch) => Err(ClientError::PackageMismatch),
            Err(RecoverError::Invalid) => Err(ClientError::Invalid),
        }
    }

    /// If the outcome is uncertain after a restart (the old `rc_auth_key` is no longer in memory), try to log in with the new password:
    /// success means the replacement went through, refusal means the old words have to be entered again (§A.5). The unfinished
    /// package is dropped in that case: it never reached the server.
    pub fn resume_recovery_after_restart(
        &mut self,
        link: &mut Link<'_>,
        new_password: &Password,
    ) -> Result<Resumed, ClientError> {
        let Some(Pending::Recovery(pending)) = &self.store.pending else {
            return Err(ClientError::NothingPending);
        };
        let login = pending.login.clone();
        match self.login(link, &login, new_password) {
            Ok(()) => Ok(Resumed::Applied),
            // A refusal by the profile rule is the same answer: a replacement made on this device would have given the new password a
            // profile of at least `seen`, so a server that still sends a lower one has not applied it (§A.5: "refusal = enter the
            // old words again").
            Err(ClientError::Auth | ClientError::Profile(_)) => {
                self.store.pending = None;
                self.log.event("client recovery dropped: not applied");
                Ok(Resumed::NotApplied)
            }
            Err(other) => Err(other),
        }
    }

    /// "Reissuing the words" (§8.6): with the session; new words, a new `rc_auth_key` and `wrap_rc`, shown and saved as at registration.
    /// The package is saved before the words are shown.
    pub fn begin_reissue(&mut self) -> Result<(), ClientError> {
        let account = self.store.account.clone().ok_or(ClientError::NotLoggedIn)?;
        let words = Words::generate()?;
        let root = words.rc_root()?;
        self.store.pending = Some(Pending::Reissue(PendingReissue {
            new_rc_auth_key: keys::rc_auth_key(&root)?,
            wrap_rc: wrap_rc(&account.dek, &keys::rc_kek(&root)?, &account.account_id)?,
            saved: false,
        }));
        self.memory.words = Some(words);
        self.log.event("client reissue prepared");
        Ok(())
    }

    /// Sends the new words after "Save". The password proves itself again with its `auth_key`.
    pub fn send_reissue(
        &mut self,
        link: &mut Link<'_>,
        password: &Password,
    ) -> Result<(), ClientError> {
        let Some(Pending::Reissue(pending)) = &self.store.pending else {
            return Err(ClientError::NothingPending);
        };
        if !pending.saved {
            self.log.event("client reissue held: words not saved");
            return Err(ClientError::SaveNotConfirmed);
        }
        let pending = pending.clone();
        let account = self.store.account.clone().ok_or(ClientError::NotLoggedIn)?;
        let token = account.token.clone().ok_or(ClientError::NotLoggedIn)?;
        let params = link.call(|server| server.login_params(&account.login))?;
        let profile = Profile::from_server(params.profile, self.store.kdf_profile_seen)
            .map_err(ClientError::Profile)?;
        let auth_key = keys::pw_auth_key(&self.derive_root(password, &params.salt, profile)?)?;
        link.call(|server| {
            server.reissue_words(
                &token,
                &auth_key,
                &pending.new_rc_auth_key,
                &pending.wrap_rc,
            )
        })?
        .map_err(|_| ClientError::Auth)?;
        self.store.pending = None;
        self.memory = Default::default();
        self.log.event("client words reissued");
        Ok(())
    }
}
