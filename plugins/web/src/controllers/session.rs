use crate::models::Template;
use crate::qweb::{Value, Values};
use base::models::Users;
use code_gen::{Controller, erp_routes};
use erp::Result;
use erp::environment::Environment;
use erp::http::{Request, Response, SESSION_COOKIE};
use erp::types::field::SingleId;

/// Logging in and out of the web client, with a session cookie.
#[derive(Controller)]
#[erp(id = "web_session")]
pub struct Session;

#[erp_routes]
impl Session {
    /// The login form; somebody already logged in goes straight where they were heading.
    #[erp(route = "/login")]
    pub fn login_form(&self, env: &mut Environment, request: &Request) -> Result<Response> {
        let redirect = destination(request.param("redirect"));
        if env.is_anonymous() {
            login_page(env, request, &redirect, "", None)
        } else {
            Ok(Response::redirect(&redirect))
        }
    }

    /// Exchange the submitted credentials for a session, held by the browser as a cookie.
    ///
    /// The cookie is `HttpOnly`, so no script reads the token, and `SameSite=Lax`, so another
    /// site cannot submit requests carrying it. The form's CSRF token is checked before this
    /// runs: without it, another site could log the browser in as an account of its own.
    #[erp(route = "/login", methods = ["POST"])]
    pub fn login(&self, env: &mut Environment, request: &Request) -> Result<Response> {
        let redirect = destination(request.param("redirect"));
        let login = request.param("login").unwrap_or_default();
        let password = request.param("password").unwrap_or_default();
        let token =
            match env
                .get_empty_record::<Users<_>>()
                .authenticate(env, login.clone(), password)
            {
                Ok(answer) => Some(answer.token),
                Err(error) => {
                    tracing::info!(%login, %error, "A login was refused");
                    None
                }
            };
        let Some(token) = token else {
            let page = login_page(
                env,
                request,
                &redirect,
                &login,
                Some("Wrong login or password."),
            )?;
            return Ok(page.with_status(401));
        };
        let max_age = env.server_config().session_duration;
        Ok(Response::redirect(&redirect).with_header(
            "Set-Cookie",
            &format!("{SESSION_COOKIE}={token}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax"),
        ))
    }

    /// End this browser's session, and forget its cookie.
    ///
    /// A GET, as Odoo's `/web/session/logout`: a link logs out.
    #[erp(route = "/logout")]
    pub fn logout(&self, env: &mut Environment, request: &Request) -> Result<Response> {
        if let Some(token) = request.cookie(SESSION_COOKIE)
            && !env.is_anonymous()
        {
            env.get_empty_record::<Users<_>>()
                .log_out(env, token.to_string())?;
        }
        Ok(Response::redirect("/login").with_header(
            "Set-Cookie",
            &format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax"),
        ))
    }
}

fn login_page(
    env: &mut Environment,
    request: &Request,
    redirect: &str,
    login: &str,
    error: Option<&str>,
) -> Result<Response> {
    let mut values = Values::new();
    if let Some(debug) = request.query("debug") {
        values.insert("debug".to_string(), Value::Text(debug.to_string()));
    }
    values.insert("csrf_token".to_string(), Value::Text(request.csrf_token()));
    values.insert("redirect".to_string(), Value::Text(redirect.to_string()));
    values.insert("login".to_string(), Value::Text(login.to_string()));
    if let Some(error) = error {
        values.insert("error".to_string(), Value::Text(error.to_string()));
    }
    let page = Template::<SingleId>::render_page(env, "web.Login".to_string(), values)?;
    Ok(Response::html(page))
}

/// Where to go once logged in: a path of this site, `/web` otherwise.
///
/// Anything else — `//elsewhere.example`, `https://…` — would make the login form a way to send
/// somebody to another site right after they trusted this one with their password.
fn destination(asked: Option<String>) -> String {
    match asked {
        Some(path) if path.starts_with('/') && !path.starts_with("//") && !path.contains('\\') => {
            path
        }
        _ => "/web".to_string(),
    }
}
