//! Passwords and logging in.
use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use erp::environment::Environment;
use erp::search::{LeftTuple, SearchOperator, SearchTuple, SearchType};
use erp::types::field::SingleId;
use erp::types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const USERS: &str = "users";

/// Hash a password. Salted per call, so two accounts sharing a password do not share a hash.
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|err| err.to_string())?
        .to_string())
}

/// Check a password against a stored hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn login_domain(login: &str) -> SearchType {
    SearchType::Tuple(SearchTuple {
        left: LeftTuple::from("login"),
        operator: SearchOperator::Equal,
        right: login.into(),
    })
}

/// Identify a user by login and password.
///
/// An unknown login and a wrong password are answered the same way, and both still pay for a
/// verification, so neither the timing nor the result says whether the account exists.
pub fn authenticate(env: &mut Environment, login: &str, password: &str) -> Result<Option<u32>> {
    let ids = env.search_ids(USERS, &login_domain(login))?;
    let Some(uid) = ids.first().copied() else {
        // Deliberate: verifying against a throwaway hash keeps the cost of a missing account
        // close to that of a wrong password.
        let _ = verify_password(password, &hash_password("")?);
        return Ok(None);
    };

    let rows = env.read(USERS, &SingleId::from(uid), &["password", "active"])?;
    let stored = rows[0].get::<&String>("password").clone();
    let active = *rows[0].get::<&bool>("active");
    if !verify_password(password, &stored) || !active {
        return Ok(None);
    }
    Ok(Some(uid))
}

/// Replace a user's password.
pub fn set_password(env: &mut Environment, uid: u32, password: &str) -> Result<()> {
    let mut values = MapOfFields::default();
    values.insert("password", hash_password(password)?);
    env.write(USERS, &SingleId::from(uid), values)
}

/// Whether a user has a usable password yet.
pub fn has_password(env: &mut Environment, uid: u32) -> Result<bool> {
    let rows = env.read(USERS, &SingleId::from(uid), &["password"])?;
    Ok(!rows[0].get::<&String>("password").is_empty())
}
