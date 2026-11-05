//! Registration, login on a new device and password change (ADR-0001 §8.3-§8.5, appendix §A.5).

use super::{Client, ClientError, Pending, PendingRegistration, Resumed, random_uuid_v4};
use crate::link::Link;
use crate::server::{Package, RegisterError};
use kinige_crypto::ids::normalize_login;
use kinige_crypto::keys::{self, Dek};
use kinige_crypto::secret::Password;
use kinige_crypto::words::Words;
use kinige_crypto::wrap::{unwrap_pw, wrap_rc};
use kinige_crypto::{AccountId, Profile};

impl Client {
    /// §8.3: makes `account_id`, `salt`, `DEK[1]` and 24 words, derives the keys, and saves the **prepared package** in the store
    /// before the words can be shown (§A.5). Nothing is sent. The words are in [`Client::shown_words`].
    pub fn begin_registration(
        &mut self,
        login: &str,
        password: &Password,
    ) -> Result<(), ClientError> {
        let login = normalize_login(login).ok_or(ClientError::Invalid)?;
        let account = random_uuid_v4();
        let dek = Dek::generate(1)?;
        let words = Words::generate()?;
        let package = self.build_package(
            &account,
            &login,
            password,
            &dek,
            &words,
            self.default_profile,
        )?;
        self.store.pending = Some(Pending::Registration(PendingRegistration {
            package,
            dek,
            saved: false,
        }));
        self.memory.words = Some(words);
        self.log.event("client registration prepared");
        Ok(())
    }

    /// Sends the prepared package. Before "Save" is confirmed nothing goes out, not even a request that would fail (§8.3).
    /// A lost answer leaves the package in place; sending again repeats the same one (§A.5).
    pub fn send_registration(&mut self, link: &mut Link<'_>) -> Result<(), ClientError> {
        let Some(Pending::Registration(pending)) = &self.store.pending else {
            return Err(ClientError::NothingPending);
        };
        if !pending.saved {
            self.log.event("client registration held: words not saved");
            return Err(ClientError::SaveNotConfirmed);
        }
        let (package, dek) = (pending.package.clone(), pending.dek.clone());
        let answer = link.call(|server| server.register(&package))?;
        match answer {
            Ok(session) => {
                let profile = Profile::from_number(package.profile)?;
                self.set_account(
                    package.account_id,
                    package.login,
                    dek,
                    session.token,
                    profile,
                );
                self.log.event("client registered");
                Ok(())
            }
            Err(RegisterError::LoginTaken) => Err(ClientError::LoginTaken),
            Err(RegisterError::AccountIdTaken) => Err(ClientError::AccountIdTaken),
            Err(RegisterError::Invalid) => Err(ClientError::Invalid),
        }
    }

    /// "Login taken": the user picks another login; `account_id`, the keys and **the words do not change** (`rc_root` does not depend
    /// on the login, §A.5). The package is the same except for the login.
    pub fn change_pending_login(&mut self, login: &str) -> Result<(), ClientError> {
        let login = normalize_login(login).ok_or(ClientError::Invalid)?;
        let Some(Pending::Registration(pending)) = &mut self.store.pending else {
            return Err(ClientError::NothingPending);
        };
        pending.package.login = login;
        Ok(())
    }

    /// "Account id taken by another package": a new `account_id` and the wraps rebuilt with the same DEK, password and words (§A.5).
    /// The KEKs are not kept in the package, so this asks for the password and the words again; the words stay the same.
    pub fn rebuild_after_account_id_taken(
        &mut self,
        password: &Password,
        words: &str,
    ) -> Result<(), ClientError> {
        let Some(Pending::Registration(pending)) = &self.store.pending else {
            return Err(ClientError::NothingPending);
        };
        let (login, dek, saved, profile) = (
            pending.package.login.clone(),
            pending.dek.clone(),
            pending.saved,
            Profile::from_number(pending.package.profile)?,
        );
        let words = Words::parse(words)?;
        let package =
            self.build_package(&random_uuid_v4(), &login, password, &dek, &words, profile)?;
        self.store.pending = Some(Pending::Registration(PendingRegistration {
            package,
            dek,
            saved,
        }));
        Ok(())
    }

    /// §8.4: login on a new device by password only. The profile the server sends is checked against `kdf_profile_seen` **before**
    /// the password is derived (§8.2); a refusal leaves `derive_runs` where it was.
    pub fn login(
        &mut self,
        link: &mut Link<'_>,
        login: &str,
        password: &Password,
    ) -> Result<(), ClientError> {
        let login = normalize_login(login).ok_or(ClientError::Invalid)?;
        let params = link.call(|server| server.login_params(&login))?;
        let profile =
            Profile::from_server(params.profile, self.store.kdf_profile_seen).map_err(|error| {
                self.log.event("client refused the server's profile");
                ClientError::Profile(error)
            })?;
        let root = self.derive_root(password, &params.salt, profile)?;
        let auth_key = keys::pw_auth_key(&root)?;
        let kek = keys::pw_kek(&root)?;
        let ok = link
            .call(|server| server.login(&login, &auth_key))?
            .map_err(|_| ClientError::Auth)?;
        let dek = unwrap_pw(
            &ok.wrap_pw,
            &kek,
            &ok.account_id,
            ok.key_id,
            &params.salt,
            profile,
        )?;
        self.set_account(ok.account_id, login, dek, ok.token, profile);
        self.log.event("client logged in");
        Ok(())
    }

    /// §8.5: the old password proves itself with its `auth_key`; the new one gets a new salt and the profile
    /// `max(seen, sent, default)`; `wrap_pw` is rebuilt around the same DEK. Nothing is re-encrypted.
    pub fn change_password(
        &mut self,
        link: &mut Link<'_>,
        old: &Password,
        new: &Password,
        revoke_others: bool,
    ) -> Result<(), ClientError> {
        let account = self.store.account.clone().ok_or(ClientError::NotLoggedIn)?;
        let token = account.token.clone().ok_or(ClientError::NotLoggedIn)?;
        let params = link.call(|server| server.login_params(&account.login))?;
        let old_profile = Profile::from_server(params.profile, self.store.kdf_profile_seen)
            .map_err(ClientError::Profile)?;
        let old_root = self.derive_root(old, &params.salt, old_profile)?;
        let old_auth = keys::pw_auth_key(&old_root)?;
        let new_profile = Profile::for_new_password(
            params.profile,
            self.store.kdf_profile_seen,
            self.default_profile,
        )?;
        let part = self.password_part(new, &account.dek, &account.account_id, new_profile)?;
        let answer =
            link.call(|server| server.change_password(&token, &old_auth, &part, revoke_others));
        let Ok(answer) = answer else {
            // The server may or may not have applied it. A retry with the old password cannot tell: if it was applied, the old
            // password is already wrong. Mark it and settle by logging in with the new one.
            self.store.pending = Some(Pending::PasswordChange);
            self.log.event("client password change: no answer");
            return Err(ClientError::Lost);
        };
        answer.map_err(|_| ClientError::Auth)?;
        self.store.kdf_profile_seen =
            Some(new_profile.max(self.store.kdf_profile_seen.unwrap_or(new_profile)));
        self.store.pending = None;
        self.log.event("client password changed");
        Ok(())
    }

    /// After a lost answer to a password change (§8.5): logs in with the **new** password. Success means the server applied the change
    /// (the device has a fresh session and the profile it was given); a refusal means it did not, the old password still works and the
    /// change can be sent again. The same rule as for a recovery (§A.5).
    pub fn settle_password_change(
        &mut self,
        link: &mut Link<'_>,
        new: &Password,
    ) -> Result<Resumed, ClientError> {
        if !matches!(self.store.pending, Some(Pending::PasswordChange)) {
            return Err(ClientError::NothingPending);
        }
        let login = self
            .store
            .account
            .as_ref()
            .map(|account| account.login.clone())
            .ok_or(ClientError::NotLoggedIn)?;
        match self.login(link, &login, new) {
            Ok(()) => Ok(Resumed::Applied),
            Err(ClientError::Auth | ClientError::Profile(_)) => {
                self.store.pending = None;
                self.log.event("client password change not applied");
                Ok(Resumed::NotApplied)
            }
            Err(other) => Err(other),
        }
    }

    fn build_package(
        &mut self,
        account: &AccountId,
        login: &str,
        password: &Password,
        dek: &Dek,
        words: &Words,
        profile: Profile,
    ) -> Result<Package, ClientError> {
        let part = self.password_part(password, dek, account, profile)?;
        let rc_root = words.rc_root()?;
        let rc_auth_key = keys::rc_auth_key(&rc_root)?;
        let wrap = wrap_rc(dek, &keys::rc_kek(&rc_root)?, account)?;
        Ok(Package {
            account_id: *account,
            login: login.to_string(),
            salt: part.salt,
            profile: part.profile,
            auth_key: part.auth_key,
            rc_auth_key,
            wrap_pw: part.wrap_pw,
            wrap_rc: wrap,
            key_id: dek.key_id(),
        })
    }
}
