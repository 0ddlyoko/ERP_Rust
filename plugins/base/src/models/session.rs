use crate::models::{BaseUsers, Users};
use code_gen::Model;
use erp::environment::Environment;
use erp::errors::MissingRecords;
use erp::types::field::{
    IdMode, Password, Reference, SingleId, TimeDelta, Timestamp, Utc, generate_secret,
};
use erp::types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Proof that someone authenticated, and how long it stays good for.
#[derive(Model)]
#[erp(id = "session")]
#[allow(dead_code)]
pub struct Session<Mode: IdMode> {
    pub id: Mode,
    #[erp(ondelete = "cascade")]
    user: Reference<BaseUsers, SingleId>,
    secret: Password,
    #[erp(default = true)]
    active: bool,
    expires_at: Timestamp,
    created_at: Timestamp,
}

/// A session, and the one copy of its token.
///
/// The record alone would not do. Only the token's hash is stored, and nothing reads a hash back
/// out of a [`Password`] — so the clear token exists at this moment and never again. Everything
/// else a caller might want is on the record, which is why it is handed over rather than copied
/// out field by field.
pub struct OpenedSession {
    pub session: Session<SingleId>,
    pub token: String,
}

/// The sessions, by id, as resolving a token reads them.
pub const SESSIONS_CACHE: &str = "base.sessions";

struct Held {
    secret: Password,
    active: bool,
    expires_at: Timestamp,
    user: Option<u32>,
    user_active: bool,
}

impl Session<SingleId> {
    /// Open a session for a user.
    ///
    /// The token is `<id>.<secret>`: the first half says which record to look at, the second is
    /// what proves it. Splitting them is what lets a lookup find one row instead of verifying
    /// every live session in turn.
    pub fn open(env: &mut Environment, user: u32) -> Result<OpenedSession> {
        let secret = generate_secret();
        let now = Utc::now();
        let duration = TimeDelta::try_seconds(i64::try_from(env.server_config().session_duration)?)
            .ok_or("The configured session duration is not a length of time")?;
        let expires_at = now
            .checked_add_signed(duration)
            .ok_or("The configured session duration runs past the end of time")?;

        let mut values = MapOfFields::default();
        values.insert("user", user);
        values.insert("secret", Password::from_random(&secret)?);
        values.insert("expires_at", expires_at);
        values.insert("created_at", now);
        let session: Session<SingleId> = env.create_new_record_from_map(values)?;

        Ok(OpenedSession {
            token: format!("{}.{secret}", session.get_id()),
            session,
        })
    }

    /// Who this token identifies.
    ///
    /// One answer for every way of failing — malformed, unknown, revoked, expired, wrong secret.
    /// Saying which would tell whoever is guessing how far they got. What a session holds is kept
    /// across requests until a session or a user changes, so most requests resolve without the
    /// database.
    pub fn resolve(env: &mut Environment, token: &str) -> Result<Option<u32>> {
        let Some((selector, secret)) = token.split_once('.') else {
            return Ok(None);
        };
        let Ok(id) = selector.parse::<u32>() else {
            return Ok(None);
        };
        let held = env.cached(SESSIONS_CACHE, selector, |env| Self::held(env, id))?;
        let Some(held) = held.as_ref() else {
            return Ok(None);
        };
        // The account counts, not only the session: closing an account must log out whoever
        // holds a token of it, rather than leave them in for as long as their session had left.
        let live = held.active && held.user_active && held.expires_at > Utc::now();
        if !live || !held.secret.is_same_password(secret) {
            return Ok(None);
        }
        Ok(held.user)
    }

    /// What resolving a token needs of a session; `None` when there is no such session.
    fn held(env: &mut Environment, id: u32) -> Result<Option<Held>> {
        match env.existing("session", vec![id]) {
            Ok(_) => {}
            Err(error) if error.is::<MissingRecords>() => return Ok(None),
            Err(error) => return Err(error),
        }
        let session = Session::<SingleId>::from_id(id, env);
        let user: Users<SingleId> = session.get_user(env)?;
        let user_active = !user.is_empty() && *user.get_active(env)?;
        Ok(Some(Held {
            secret: session.get_secret(env)?.clone(),
            active: *session.get_active(env)?,
            expires_at: *session.get_expires_at(env)?,
            user: user.get_optional_id(),
            user_active,
        }))
    }

    /// End the session a token opens, if it is a live session of `uid`.
    ///
    /// Returns whether it ended one. Checking the owner is what keeps a token picked up somewhere
    /// from being used to log somebody else out.
    pub fn revoke(env: &mut Environment, token: &str, uid: u32) -> Result<bool> {
        let env = &mut *env.sudo();
        if Self::resolve(env, token)? != Some(uid) {
            return Ok(false);
        }
        let Some(id) = token
            .split_once('.')
            .and_then(|(selector, _)| selector.parse::<u32>().ok())
        else {
            return Ok(false);
        };
        let mut values = MapOfFields::default();
        values.insert("active", false);
        env.write("session", &SingleId::from(id), values)?;
        Ok(true)
    }
}
