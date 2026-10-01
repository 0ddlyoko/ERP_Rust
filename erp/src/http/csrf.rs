//! Tokens proving a request was sent by a page of this site, not by another site using the
//! browser's cookies.
//!
//! As Odoo's: `<hmac>o<expiry>`, the HMAC of what the browser holds — its session cookie, or for
//! somebody not logged in a random `csrf_id` cookie — and the expiry, signed with the server's
//! secret. Another site can make the browser send the cookies, but cannot read them, nor compute
//! the HMAC without the secret.

use super::{Request, SESSION_COOKIE};
use crate::app::Application;
use erp_types::field::generate_secret;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// The cookie a token is bound to when the browser has no session.
pub const CSRF_COOKIE: &str = "csrf_id";

/// The form field carrying the token.
pub const CSRF_PARAM: &str = "csrf_token";

/// The header carrying the token, for a request a script sends.
pub const CSRF_HEADER: &str = "X-CSRF-Token";

/// Methods that change nothing, and so need no token.
const SAFE_METHODS: &[&str] = &["GET", "HEAD", "OPTIONS", "TRACE"];

/// How long a token stays valid: a year, as Odoo's. Bound to the session, it dies with it
/// anyway; the expiry is mostly a salt, so two tokens of one session differ.
const LIFETIME_SECONDS: u64 = 365 * 24 * 3600;

/// What the tokens of a request are bound to.
pub(crate) struct Binding {
    secret: String,
    value: String,
    fresh: bool,
    used: AtomicBool,
}

impl std::fmt::Debug for Binding {
    /// Without the secret, nor the cookie it is bound to: a request is logged, these are not.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Binding")
            .field("fresh", &self.fresh)
            .finish()
    }
}

impl Binding {
    /// The session cookie, the `csrf_id` cookie, or a new `csrf_id` when there is neither.
    pub(crate) fn of(secret: &str, request: &Request) -> Arc<Binding> {
        let held = request
            .cookie(SESSION_COOKIE)
            .filter(|value| !value.is_empty())
            .map(|value| format!("session:{value}"))
            .or_else(|| {
                request
                    .cookie(CSRF_COOKIE)
                    .filter(|value| !value.is_empty())
                    .map(|value| format!("anonymous:{value}"))
            });
        let (value, fresh) = match held {
            Some(value) => (value, false),
            None => (format!("anonymous:{}", generate_secret()), true),
        };
        Arc::new(Binding {
            secret: secret.to_string(),
            value,
            fresh,
            used: AtomicBool::new(false),
        })
    }

    pub(crate) fn token(&self) -> String {
        self.used.store(true, Ordering::Relaxed);
        let expiry = now() + LIFETIME_SECONDS;
        let signature: String = sign(&self.secret, &self.value, expiry)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("{signature}o{expiry}")
    }

    fn accepts(&self, token: &str) -> bool {
        let Some((signature, expiry)) = token.rsplit_once('o') else {
            return false;
        };
        let Ok(expiry) = expiry.parse::<u64>() else {
            return false;
        };
        if expiry < now() {
            return false;
        }
        let Some(signature) = decode_hex(signature) else {
            return false;
        };
        mac(&self.secret, &self.value, expiry)
            .verify_slice(&signature)
            .is_ok()
    }

    /// The cookie to set so the browser presents this binding again, when it is a new one that a
    /// token was made for.
    pub(crate) fn cookie_to_set(&self) -> Option<String> {
        if !self.fresh || !self.used.load(Ordering::Relaxed) {
            return None;
        }
        let value = self.value.strip_prefix("anonymous:")?;
        Some(format!(
            "{CSRF_COOKIE}={value}; Path=/; HttpOnly; SameSite=Lax"
        ))
    }
}

/// Whether a request may go on: it changes nothing, or it carries a token of this browser.
pub(crate) fn check(binding: &Binding, request: &Request) -> bool {
    if SAFE_METHODS.contains(&request.method()) {
        return true;
    }
    let token = request
        .header(CSRF_HEADER)
        .map(str::to_string)
        .or_else(|| request.param(CSRF_PARAM));
    match token {
        Some(token) => binding.accepts(&token),
        None => false,
    }
}

/// A token for a request, as a controller would get it, for whatever builds requests outside
/// [`super::handle`] — a test submitting a form.
pub fn token_for(app: &Application, request: &Request) -> String {
    Binding::of(app.signing_secret(), request).token()
}

fn mac(secret: &str, value: &str, expiry: u64) -> Hmac<Sha256> {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes a key of any size");
    mac.update(value.as_bytes());
    mac.update(b"|");
    mac.update(expiry.to_string().as_bytes());
    mac
}

fn sign(secret: &str, value: &str, expiry: u64) -> Vec<u8> {
    mac(secret, value, expiry).finalize().into_bytes().to_vec()
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(text.get(index..index + 2)?, 16).ok())
        .collect()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}
