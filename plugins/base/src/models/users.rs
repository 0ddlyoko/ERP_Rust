use crate::auth::{hash_password, verify_password};
use crate::models::BaseGroup;
use code_gen::Model;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp_search_code_gen::make_domain;
use std::error::Error;

/// Someone who can log in.
#[derive(Model)]
#[erp(id = "users")]
#[allow(dead_code)]
pub struct Users<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    login: String,
    /// Argon2 hash. The clear password is never stored, and never recoverable.
    #[erp(default = "")]
    #[erp(private)]
    password: String,
    #[erp(default = "")]
    name: String,
    #[erp(default = true)]
    active: bool,
    #[erp(relation = "user_group_rel")]
    groups: Reference<BaseGroup, MultipleIds>,
}

impl Users<SingleId> {
    /// Find the user these credentials identify.
    ///
    /// An unknown login and a wrong password are answered the same way, and both pay for a
    /// verification, so neither the answer nor the time it takes says whether the account exists.
    pub fn authenticate(
        env: &mut Environment,
        login: &str,
        password: &str,
    ) -> Result<Option<Users<SingleId>>, Box<dyn Error + Send + Sync>> {
        let found: Users<MultipleIds> = env.search(&make_domain!([("login", "=", login)]))?;
        let Some(id) = found.id.get_ids_ref().first().copied() else {
            // Deliberate: verifying against a throwaway hash keeps the cost of a missing account
            // close to that of a wrong password.
            let _ = verify_password(password, &hash_password("")?);
            return Ok(None);
        };

        let user: Users<SingleId> = env.get_record(id.into());
        if !user.check_password(env, password)? || !*user.get_active(env)? {
            return Ok(None);
        }
        Ok(Some(user))
    }

    /// Whether this password is the user's.
    pub fn check_password(
        &self,
        env: &mut Environment,
        password: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(verify_password(password, self.get_password(env)?))
    }

    /// Replace the user's password.
    ///
    /// Not named `set_password`: that is the generated setter for the field, which takes the hash
    /// rather than the clear password.
    pub fn change_password(
        &self,
        env: &mut Environment,
        password: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.set_password(hash_password(password)?, env)
    }

    /// Whether the user has a usable password yet.
    pub fn has_password(
        &self,
        env: &mut Environment,
    ) -> Result<bool, Box<dyn Error + Send + Sync>> {
        Ok(!self.get_password(env)?.is_empty())
    }
}
