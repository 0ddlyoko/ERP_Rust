use crate::models::Template;
use crate::qweb::{Value, Values};
use base::models::{Group, Users};
use code_gen::{Controller, erp_routes};
use erp::Result;
use erp::assets::content_type;
use erp::data;
use erp::environment::Environment;
use erp::http::{HttpError, Request, Response};
use erp::serde_json::json;
use erp::types::field::{MultipleIds, SingleId};
use std::hash::{DefaultHasher, Hash, Hasher};

/// The web client, and what it loads.
#[derive(Controller)]
#[erp(id = "web")]
pub struct Web;

#[erp_routes]
impl Web {
    /// The back office: the page template `web.WebClientPage`, which loads its bundle.
    ///
    /// Only for somebody logged in; anybody else is sent to log in, and back here after.
    #[erp(route = "/web")]
    pub fn client(&self, env: &mut Environment, request: &Request) -> Result<Response> {
        if env.is_anonymous() {
            let back = request.path();
            return Ok(Response::redirect(&format!("/login?redirect={back}")));
        }
        let mut values = Values::new();
        if let Some(debug) = request.query("debug") {
            values.insert("debug".to_string(), Value::Text(debug.to_string()));
        }
        values.insert(
            "session_info".to_string(),
            Value::Markup(script_json(&session_info(env, request)?)),
        );
        let page = Template::<SingleId>::render_page(env, "web.WebClientPage".to_string(), values)?;
        Ok(Response::html(page))
    }

    /// A file an installed plugin serves: `/static/<plugin>/<path>`.
    ///
    /// Only at its exact path: a TypeScript import written without extension is compiled with it,
    /// and answering both would let a browser load one file as two modules.
    #[erp(route = "/static/<module>/<*path>", auth = "none")]
    pub fn static_file(
        &self,
        env: &mut Environment,
        request: &Request,
        module: String,
        path: String,
    ) -> Result<Response> {
        let asked = format!("{module}/static/{path}");
        let Some(content) = env.model_manager.assets.file(&asked) else {
            return Err(HttpError::not_found("No installed plugin serves this file").into());
        };
        Ok(cached(request, content.to_vec(), content_type(&asked)))
    }

    /// A bundle, as one file per kind: `/web/assets/<bundle>.js`, `.css` or `.xml`.
    ///
    /// What debugging wants, each browser asking whether it changed: the JavaScript is a module
    /// importing each file of the bundle from its own URL, so every file stays where it was
    /// written. The styles are the bundle's CSS files, one after the other. The templates are
    /// those served from the bundle's `.xml` files, every extension applied.
    #[erp(route = "/web/assets/<file>", auth = "none")]
    pub fn bundle(
        &self,
        env: &mut Environment,
        request: &Request,
        file: String,
    ) -> Result<Response> {
        let assets = &env.model_manager.assets;
        let (body, content_type) = match file.rsplit_once('.') {
            Some((name, "js")) => (
                assets.script_imports(name),
                "text/javascript; charset=utf-8",
            ),
            Some((name, "css")) => (
                assets.stylesheet(name).map(|built| built.content.clone()),
                "text/css; charset=utf-8",
            ),
            Some((name, "xml")) => (
                Template::<SingleId>::bundle_markup(env, name.to_string())?
                    .as_ref()
                    .clone(),
                "application/xml; charset=utf-8",
            ),
            _ => (None, ""),
        };
        let Some(body) = body else {
            return Err(bundle_not_found().into());
        };
        Ok(cached(request, body.into_bytes(), content_type))
    }

    /// A bundle built into one file, at the version its URL names:
    /// `/web/assets/<version>/<bundle>.js` or `.css`.
    ///
    /// Every module of the JavaScript is in it, those its files import included, so a page loads
    /// a bundle in one request. Kept for good by the browser, the version changing with the
    /// content; a page asking for another version — rendered before the plugins changed — is
    /// answered the current one, which it must ask about again.
    #[erp(route = "/web/assets/<version>/<file>", auth = "none")]
    pub fn built_bundle(
        &self,
        env: &mut Environment,
        request: &Request,
        version: String,
        file: String,
    ) -> Result<Response> {
        let assets = &env.model_manager.assets;
        let (built, content_type) = match file.rsplit_once('.') {
            Some((name, "js")) => (assets.script(name), "text/javascript; charset=utf-8"),
            Some((name, "css")) => (assets.stylesheet(name), "text/css; charset=utf-8"),
            _ => (None, ""),
        };
        let Some(built) = built else {
            return Err(bundle_not_found().into());
        };
        if built.version != version {
            return Ok(cached(
                request,
                built.content.clone().into_bytes(),
                content_type,
            ));
        }
        Ok(
            Response::file(built.content.clone().into_bytes(), content_type)
                .with_header("Cache-Control", "public, max-age=31536000, immutable"),
        )
    }
}

fn bundle_not_found() -> HttpError {
    HttpError::not_found("No installed plugin contributes to this bundle")
}

/// What the web client knows of its session from the start, so it begins without asking: who is
/// logged in, which groups they are in by external identifier, and the CSRF token its calls carry.
fn session_info(env: &mut Environment, request: &Request) -> Result<erp::serde_json::Value> {
    let uid = env.uid().ok_or("Only somebody logged in has a session")?;
    // Their own account, which they may not otherwise be allowed to read.
    let env = &mut *env.sudo();
    let user = Users::<SingleId>::from_id(uid, env);
    let group_ids = user.get_groups::<Group<MultipleIds>>(env)?.get_ids();
    let mut groups: Vec<String> = data::external_ids_of(env, "group", &group_ids)?
        .into_values()
        .collect();
    groups.sort();
    Ok(json!({
        "uid": uid,
        "name": user.get_name(env)?,
        "login": user.get_login(env)?,
        "groups": groups,
        "csrf_token": request.csrf_token(),
    }))
}

/// JSON to write inside a `<script>`: `<`, `>` and `&` escaped, so no value can close the element
/// or be read as markup.
fn script_json(value: &erp::serde_json::Value) -> String {
    value
        .to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

/// A response the browser may keep, and check again with the tag it was given.
///
/// Answered 304, with no body, when the browser already holds this exact content.
fn cached(request: &Request, body: Vec<u8>, content_type: &str) -> Response {
    let mut hasher = DefaultHasher::new();
    body.hash(&mut hasher);
    let tag = format!("\"{:016x}\"", hasher.finish());
    let unchanged = request
        .header("if-none-match")
        .is_some_and(|sent| sent.split(',').any(|candidate| candidate.trim() == tag));
    let response = if unchanged {
        Response::new(304, content_type, Vec::new())
    } else {
        Response::file(body, content_type)
    };
    response
        .with_header("ETag", &tag)
        .with_header("Cache-Control", "no-cache")
}
