//! Turning a credential a caller presented into the user it identifies.
//!
//! The core knows that something can answer this, and nothing about how. A session is a record of
//! a model, and models belong to plugins — so whichever plugin defines them registers the answer
//! here, and `erp` never names it.

use crate::Result;
use crate::environment::Environment;
use std::sync::OnceLock;

/// Who a bearer token belongs to.
///
/// `None` for a token that names nobody: unknown, revoked, expired, or malformed. One answer for
/// all four on purpose — telling them apart tells whoever is guessing which half they got right.
///
/// Takes an environment because answering means reading the database, and the environment is
/// already open by the time anyone asks.
pub type ResolveToken = fn(&mut Environment, &str) -> Result<Option<u32>>;

/// Who answers "whose token is this".
///
/// Empty until a plugin fills it, which is what an application serving nothing but public data
/// looks like: a caller presenting a token is then told their token names nobody, rather than
/// quietly served as the system.
#[derive(Default)]
pub struct Identities {
    resolve: Option<ResolveToken>,
    /// Who a caller is when nobody authenticated.
    default_user: OnceLock<u32>,
    /// Who every rule will let through.
    root_user: OnceLock<u32>,
    user_model: Option<&'static str>,
}

impl Identities {
    /// Name the model whose records are the users: who creates and who last changes a record
    /// point to it.
    pub fn register_user_model(&mut self, model: &'static str) {
        self.user_model = Some(model);
    }

    pub fn user_model(&self) -> Option<&'static str> {
        self.user_model
    }

    /// Name the function that resolves a token.
    ///
    /// Panics on a second one. Two plugins each believing they own authentication would leave one
    /// of them silently unused, and which one would depend on the order they loaded in.
    pub fn register(&mut self, resolve: ResolveToken) {
        if self.resolve.is_some() {
            panic!(
                "Two plugins both register how a token identifies its caller. Only one can \
                 answer, and which one would depend on load order."
            );
        }
        self.resolve = Some(resolve);
    }

    /// The resolver, if there is one.
    ///
    /// Handed out rather than called here: the caller holds the environment this needs, and it
    /// borrows the registry this lives in.
    pub fn resolver(&self) -> Option<ResolveToken> {
        self.resolve
    }

    /// Say which record is the caller nobody authenticated as.
    ///
    /// Set through a shared reference because the answer is only known once the plugin's data is
    /// loaded, which happens with the registry already borrowed. Settable once: the identity a
    /// request starts from cannot change under a running application.
    pub fn set_default_user(&self, uid: u32) -> Result<()> {
        set_once(&self.default_user, uid, "default")
    }

    /// Say which record is the one every rule lets through.
    pub fn set_root_user(&self, uid: u32) -> Result<()> {
        set_once(&self.root_user, uid, "root")
    }

    /// Who a caller is when nobody authenticated, if a plugin said.
    ///
    /// `None` before the plugin owning users has finished loading, and for an application that
    /// has no users at all — booting is not somebody.
    pub fn default_user(&self) -> Option<u32> {
        self.default_user.get().copied()
    }

    /// Who every rule lets through, if a plugin said.
    pub fn root_user(&self) -> Option<u32> {
        self.root_user.get().copied()
    }
}

/// Fill a slot that is written once.
///
/// Saying the same thing twice is loading the same plugin twice, which is ordinary. Saying two
/// different things is two plugins disagreeing about who somebody is, which is not.
fn set_once(slot: &OnceLock<u32>, uid: u32, which: &str) -> Result<()> {
    match slot.set(uid) {
        Ok(()) => Ok(()),
        Err(_) if slot.get() == Some(&uid) => Ok(()),
        Err(_) => Err(format!(
            "The {which} user is already {:?} and cannot become {uid}",
            slot.get()
        )
        .into()),
    }
}
