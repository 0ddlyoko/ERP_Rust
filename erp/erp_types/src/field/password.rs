//! A secret that is only ever held hashed.

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use std::fmt::{Debug, Display, Formatter};

/// Raised when a clear password cannot be hashed.
#[derive(Debug, Clone, thiserror::Error)]
#[error("Cannot hash a password: {reason}")]
pub struct HashError {
    pub reason: String,
}

/// A password, held as its hash and nothing else.
///
/// Built from a clear password, which is hashed on the spot and then dropped: a `Password` that
/// exists has already forgotten what it was made from. Storing one stores the hash, reading one
/// back gives the hash, and neither step ever sees the clear password again.
///
/// Nothing a `Password` offers reads the hash. What it offers is answers:
/// [`Password::is_same_password`] against a clear password, [`Password::is_same_hash`] against a
/// hash the caller already holds. Getting the hash out at all means naming [`StoredHash`], which
/// exists for the one layer that has to write it to a column.
#[derive(Clone, Default)]
pub struct Password(String);

impl Password {
    /// Hash a clear password.
    ///
    /// Salted per call, so two accounts sharing a password do not share a hash.
    pub fn new(clear: &str) -> Result<Self, HashError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(clear.as_bytes(), &salt)
            .map_err(|error| HashError {
                reason: error.to_string(),
            })?;
        Ok(Self(hash.to_string()))
    }

    /// Wrap a hash that already exists.
    ///
    /// The way back in from storage, and the only way to build a `Password` without knowing the
    /// clear password — which is why it asks for the hash rather than producing one.
    pub fn from_hash(hash: impl Into<String>) -> Self {
        Self(hash.into())
    }

    /// Whether this is the password.
    ///
    /// False for a password that was never set, and for a hash that cannot be read — a value that
    /// verifies nothing is the right answer to "is this it?", not an error.
    pub fn is_same_password(&self, clear: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(&self.0) else {
            return false;
        };
        Argon2::default()
            .verify_password(clear.as_bytes(), &parsed)
            .is_ok()
    }

    /// Whether this is the hash, compared without leaking where two hashes start to differ.
    pub fn is_same_hash(&self, hash: &str) -> bool {
        same_bytes(self.0.as_bytes(), hash.as_bytes())
    }

    /// Whether a password was ever set.
    ///
    /// An account seeded without one holds the default, which no clear password matches.
    pub fn is_set(&self) -> bool {
        !self.0.is_empty()
    }
}

/// The hash, for whoever has to store it.
///
/// A trait rather than a method on [`Password`], and sealed so that nothing else can implement
/// it: the hash is not part of what a password offers, it is what persistence needs. Asking for
/// it means naming this trait on purpose, which is as far as a language without cross-crate
/// privacy can go — a determined caller can still import it, but not reach the hash by accident.
pub trait StoredHash: sealed::Sealed {
    /// The hash as it goes into a column.
    fn into_hash(self) -> String;
}

impl StoredHash for Password {
    fn into_hash(self) -> String {
        self.0
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for super::Password {}
}

/// Compare without stopping at the first difference.
///
/// Lengths are compared openly: a hash's length says nothing about the password behind it.
fn same_bytes(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right)
        .fold(0u8, |differences, (l, r)| differences | (l ^ r))
        == 0
}

impl PartialEq for Password {
    fn eq(&self, other: &Self) -> bool {
        same_bytes(self.0.as_bytes(), other.0.as_bytes())
    }
}

impl Eq for Password {}

/// Prints as a mask, in both forms.
///
/// A hash is not a secret the way a password is, but it is an offline attack waiting for a log
/// file, and a value that never prints itself cannot be printed by accident.
impl Debug for Password {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(****)")
    }
}

impl Display for Password {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("****")
    }
}
