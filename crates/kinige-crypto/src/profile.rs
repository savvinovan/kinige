//! Argon2id profiles and the rule that an account's profile only grows (ADR-0001 §8.2, decision 8, appendix §D).
//!
//! The server sends a profile number; it is untrusted. A `Profile` can be made from a number only here, so key derivation
//! (`keys::derive_pw_root`) cannot be reached with a number nobody checked.

use crate::error::{CryptoError, ProfileError};
use libsodium_rs::crypto_pwhash::argon2id;

/// A closed set (§8.2): profile 1 is `MODERATE` (3 passes, 256 MiB), profile 2 is `SENSITIVE` (4 passes, 1 GiB). The order is the
/// number, and the lower bound is profile 1: it can be lowered only by a new client version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Profile {
    P1 = 1,
    P2 = 2,
}

impl Profile {
    /// What a new account gets (§8.3).
    pub const DEFAULT: Profile = Profile::P1;

    pub fn number(self) -> u8 {
        self as u8
    }

    /// `opslimit` of `crypto_pwhash` (appendix §D).
    pub fn ops_limit(self) -> u64 {
        match self {
            Profile::P1 => argon2id::OPSLIMIT_MODERATE,
            Profile::P2 => argon2id::OPSLIMIT_SENSITIVE,
        }
    }

    /// `memlimit` of `crypto_pwhash` in bytes (appendix §D).
    pub fn mem_limit(self) -> usize {
        match self {
            Profile::P1 => argon2id::MEMLIMIT_MODERATE,
            Profile::P2 => argon2id::MEMLIMIT_SENSITIVE,
        }
    }

    /// A number from outside. Unknown is a refusal: this is the upper bound of the memory a malicious server can ask for (§8.2).
    pub fn from_number(number: u8) -> Result<Profile, CryptoError> {
        match number {
            1 => Ok(Profile::P1),
            2 => Ok(Profile::P2),
            other => Err(CryptoError::Profile(ProfileError::Unknown(other))),
        }
    }

    /// The profile to derive with when the server sent `sent` (a login, step 1 of §8.4): unknown is refused, and so is a profile
    /// lower than `seen`, the highest this device has seen for the account. The check is made **before** anything is derived;
    /// the way out of a refusal is recovery with the words, which does not depend on Argon2 (§8.2).
    pub fn from_server(sent: u8, seen: Option<Profile>) -> Result<Profile, CryptoError> {
        let sent_profile = Profile::from_number(sent)?;
        match seen {
            Some(seen) if sent_profile < seen => {
                Err(CryptoError::Profile(ProfileError::LowerThanSeen {
                    sent,
                    seen: seen.number(),
                }))
            }
            _ => Ok(sent_profile),
        }
    }

    /// The profile of a new password, on a password change and on recovery (§8.2, §8.5, §8.6):
    /// `max(seen, sent, default)`. A sent profile lower than `seen` is not refused here (nothing is derived from it) but does not
    /// lower the result: a recovery on a device that has seen profile 2 gets profile 2 even if the server says 1 (B item 14).
    /// An unknown number is still a refusal.
    pub fn for_new_password(sent: u8, seen: Option<Profile>) -> Result<Profile, CryptoError> {
        let sent_profile = Profile::from_number(sent)?;
        Ok(sent_profile
            .max(seen.unwrap_or(Profile::DEFAULT))
            .max(Profile::DEFAULT))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_profiles_are_the_constants_of_appendix_d() {
        assert_eq!(
            (Profile::P1.ops_limit(), Profile::P1.mem_limit()),
            (3, 268_435_456)
        );
        assert_eq!(
            (Profile::P2.ops_limit(), Profile::P2.mem_limit()),
            (4, 1_073_741_824)
        );
    }

    #[test]
    fn an_unknown_number_is_refused() {
        for number in [0u8, 3, 4, 7, 255] {
            assert_eq!(
                Profile::from_number(number),
                Err(CryptoError::Profile(ProfileError::Unknown(number)))
            );
            assert!(Profile::from_server(number, None).is_err());
            assert!(Profile::for_new_password(number, Some(Profile::P2)).is_err());
        }
    }

    #[test]
    fn a_lower_profile_than_seen_is_refused_for_a_login() {
        assert_eq!(
            Profile::from_server(1, Some(Profile::P2)),
            Err(CryptoError::Profile(ProfileError::LowerThanSeen {
                sent: 1,
                seen: 2
            }))
        );
        assert_eq!(Profile::from_server(2, Some(Profile::P2)), Ok(Profile::P2));
        assert_eq!(Profile::from_server(2, Some(Profile::P1)), Ok(Profile::P2));
        assert_eq!(Profile::from_server(1, None), Ok(Profile::P1));
    }

    #[test]
    fn a_new_password_never_gets_a_lower_profile() {
        assert_eq!(
            Profile::for_new_password(1, Some(Profile::P2)),
            Ok(Profile::P2)
        );
        assert_eq!(
            Profile::for_new_password(2, Some(Profile::P1)),
            Ok(Profile::P2)
        );
        assert_eq!(Profile::for_new_password(2, None), Ok(Profile::P2));
        assert_eq!(Profile::for_new_password(1, None), Ok(Profile::P1));
    }
}
