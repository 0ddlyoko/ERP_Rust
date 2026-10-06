use crate::models::contact::BaseContact;
use crate::models::{BaseGroup, Contact, Session};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::model::ModelVerbs;
use erp::types::field::{IdMode, MultipleIds, Password, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;

/// Someone who can log in, and the contact they are.
#[derive(Model)]
#[erp(id = "users", methods)]
#[allow(dead_code)]
pub struct Users<Mode: IdMode> {
    pub id: Mode,
    #[erp(tracking, index)]
    login: String,
    password: Password,
    name: String,
    #[erp(default = true, tracking)]
    active: bool,
    #[erp(relation = "user_group_rel", tracking)]
    groups: Reference<BaseGroup, MultipleIds>,
    #[erp(required, ondelete = "restrict")]
    contact: Reference<BaseContact, SingleId>,
}

impl Users<SingleId> {
    /// Find the user these credentials identify.
    ///
    /// An unknown login and a wrong password are answered the same way, and both pay for a
    /// verification, so neither the answer nor the time it takes says whether the account exists.
    ///
    /// Not called `authenticate`: that is what a caller asks for over the wire, and it does more
    /// than this — it opens a session. This only says whose credentials these are.
    pub fn identified_by(
        env: &mut Environment,
        login: &str,
        password: &str,
    ) -> Result<Option<Users<SingleId>>> {
        // Checking credentials decides who the caller is, so it cannot wait on their rights.
        let env = &mut *env.sudo();
        let found: Users<MultipleIds> = env.search(&make_domain!([
            ("login", "=", login),
            ("active", "=", true)
        ]))?;
        let Some(id) = found.id.get_ids_ref().first().copied() else {
            // Deliberate: verifying against a throwaway hash keeps the cost of a missing account
            // close to that of a wrong password.
            let _ = Password::new("")?.is_same_password(password);
            return Ok(None);
        };

        let user: Users<SingleId> = env.get_record(id.into());
        if !user.check_password(env, password)? || !*user.get_active(env)? {
            return Ok(None);
        }
        Ok(Some(user))
    }

    /// Whether this password is the user's.
    pub fn check_password(&self, env: &mut Environment, password: &str) -> Result<bool> {
        Ok(self.get_password(env)?.is_same_password(password))
    }

    /// Replace the user's password.
    ///
    /// Not named `set_password`: that is the generated setter for the field, which takes a
    /// [`Password`] — already hashed — rather than the clear password.
    pub fn change_password(&self, env: &mut Environment, password: &str) -> Result<()> {
        self.set_password(Password::new(password)?, env)
    }

    /// Whether the user has a usable password yet.
    pub fn has_password(&self, env: &mut Environment) -> Result<bool> {
        Ok(self.get_password(env)?.is_set())
    }
}

/// What a caller gets back for its credentials.
///
/// The shape of an answer rather than of a record: the token, who it speaks for, and until when.
/// Built here, where the wire is, instead of by the session itself.
#[derive(erp::serde::Serialize)]
#[serde(crate = "erp::serde")]
pub struct Authenticated {
    pub token: String,
    pub uid: u32,
    pub expires_at: erp::types::field::Timestamp,
}

impl Users<MultipleIds> {
    /// New contacts for users with these values, one each and in their order, named as the user.
    /// Created together, as sudo: who may create a user may give them a contact.
    fn contacts_for(env: &mut Environment, users: &[&MapOfFields]) -> Result<Contact<MultipleIds>> {
        let contacts = users
            .iter()
            .map(|user| {
                let mut contact = MapOfFields::default();
                contact.insert_option("name", user.get_option::<&String>("name").cloned());
                contact
            })
            .collect();
        Contact::<MultipleIds>::create(contacts, &mut env.sudo())
    }
}

#[erp_methods]
impl Users<MultipleIds> {
    /// Each user created without a contact gets one, named as they are; all created together.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        let without: Vec<usize> = values
            .iter()
            .enumerate()
            .filter(|(_, user)| {
                user.get_option::<&u32>("contact")
                    .is_none_or(|contact| *contact == 0)
            })
            .map(|(index, _)| index)
            .collect();
        let users: Vec<&MapOfFields> = without.iter().map(|index| &values[*index]).collect();
        let contacts = Self::contacts_for(env, &users)?;
        for (index, contact) in without.into_iter().zip(contacts) {
            values[index].insert("contact", contact.get_id());
        }
        sup.call_with(values, env)
    }

    /// Exchange credentials for a session.
    ///
    /// The one door in, and the one place the token is ever handed out. When API keys arrive they
    /// become a second shape of credential here rather than a second way to get a session.
    pub fn authenticate(
        &self,
        env: &mut Environment,
        login: String,
        password: String,
    ) -> Result<Authenticated> {
        let _ = self;
        let Some(user) = Users::<SingleId>::identified_by(env, &login, &password)? else {
            return Err("These credentials identify nobody".into());
        };
        let uid = user.get_id();
        let env = &mut *env.sudo();
        let opened = Session::open(env, uid)?;
        Ok(Authenticated {
            token: opened.token,
            uid,
            expires_at: *opened.session.get_expires_at(env)?,
        })
    }

    /// End the caller's session this token opens.
    ///
    /// The token rather than the caller alone: logging out of one browser leaves the others
    /// logged in. Returns whether a session ended.
    pub fn log_out(&self, env: &mut Environment, token: String) -> Result<bool> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Ok(false);
        };
        Session::revoke(env, &token, uid)
    }

    /// Change the caller's own password.
    ///
    /// The only way a password is set from outside the process: writing the field is refused, and
    /// this asks for the current one. Runs as sudo, so nobody can change anybody else's only
    /// because the record it touches is the caller's own and no argument names another.
    ///
    /// Knowing the current password is also what keeps the accounts the framework acts as out of
    /// reach. Neither has one, and an account with no password holds an empty hash, which nothing
    /// verifies against — so a caller nobody authenticated, who is the portal user, cannot give
    /// the portal user a password and then log in as it.
    #[erp(rpc)]
    pub fn change_own_password(
        &self,
        env: &mut Environment,
        current: String,
        new: String,
    ) -> Result<bool> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Err("Only somebody can change their password".into());
        };
        // The current password is the authorisation here, not the caller's rights on `users`.
        let env = &mut *env.sudo();
        let user = Users::<SingleId>::from_id(uid, env);
        if !user.check_password(env, &current)? {
            return Err("That is not the current password".into());
        }
        user.change_password(env, &new)?;
        Ok(true)
    }

    /// Who the server takes this caller to be.
    ///
    /// `None` for a caller that presented no token, which is how a client tells "my token was not
    /// read" from "my token was read and I am nobody in particular".
    #[erp(rpc)]
    pub fn me(&self, env: &mut Environment) -> Result<Option<u32>> {
        let _ = self;
        Ok(env.uid())
    }

    /// Archive the users: they can no longer log in.
    #[erp(rpc)]
    pub fn archive(&self, env: &mut Environment) -> Result<bool> {
        self.set_active(false, env)?;
        Ok(true)
    }

    /// Bring archived users back: they can log in again.
    #[erp(rpc)]
    pub fn unarchive(&self, env: &mut Environment) -> Result<bool> {
        self.set_active(true, env)?;
        Ok(true)
    }
}
