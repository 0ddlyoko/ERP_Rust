use crate::models::Template;
use crate::qweb::{Value, Values};
use base::models::{Group, Users};
use code_gen::{Controller, erp_routes};
use erp::assets::content_type;
use erp::data;
use erp::environment::Environment;
use erp::http::{HttpError, Request, Response};
use erp::serde_json::json;
use erp::types::field::{MultipleIds, SingleId};
use std::error::Error;
use std::hash::{DefaultHasher, Hash, Hasher};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

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
        values.insert(
            "session_info".to_string(),
            Value::Markup(script_json(&session_info(env, request)?)),
        );
        let page = Template::<SingleId>::render_page(env, "web.WebClientPage", values)?;
        Ok(Response::html(page))
    }

    /// A file an installed plugin serves: `/static/<plugin>/<path>`.
    ///
    /// Only at its exact path: a TypeScript import written without extension is compiled with it,
    /// and answering both would let a browser load one file as two modules.
    #[erp(route = "/static/<module>/<*path>")]
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
    /// The JavaScript is a module importing each file of the bundle in order: the files stay
    /// separate modules, each at its own URL, so their relative imports resolve as written. The
    /// styles are the bundle's CSS files, one after the other. The templates are those served from
    /// the bundle's `.xml` files, every extension applied.
    #[erp(route = "/web/assets/<file>")]
    pub fn bundle(
        &self,
        env: &mut Environment,
        request: &Request,
        file: String,
    ) -> Result<Response> {
        let not_found = || HttpError::not_found("No installed plugin contributes to this bundle");
        let Some((name, kind)) = file.rsplit_once('.') else {
            return Err(not_found().into());
        };
        let (body, content_type) = match kind {
            "js" => (scripts(env, name), "text/javascript; charset=utf-8"),
            "css" => (styles(env, name), "text/css; charset=utf-8"),
            "xml" => (
                Template::<SingleId>::bundle_markup(env, name)?
                    .as_ref()
                    .clone(),
                "application/xml; charset=utf-8",
            ),
            _ => (None, ""),
        };
        let Some(body) = body else {
            return Err(not_found().into());
        };
        Ok(cached(request, body.into_bytes(), content_type))
    }
}

fn scripts(env: &Environment, bundle: &str) -> Option<String> {
    let assets = &env.model_manager.assets;
    if !assets.bundles().contains(&bundle) {
        return None;
    }
    let mut module = format!("// Bundle {bundle}\n");
    for path in assets.bundle(bundle) {
        if !path.ends_with(".js") {
            continue;
        }
        if let Some((plugin, rest)) = path.split_once("/static/") {
            module.push_str(&format!("import \"/static/{plugin}/{rest}\";\n"));
        }
    }
    Some(module)
}

/// The CSS files of a bundle, each preceded by its path, so the browser shows where a rule is from.
fn styles(env: &Environment, bundle: &str) -> Option<String> {
    let assets = &env.model_manager.assets;
    if !assets.bundles().contains(&bundle) {
        return None;
    }
    let mut sheet = String::new();
    for path in assets.bundle(bundle) {
        if !path.ends_with(".css") {
            continue;
        }
        if let Some(content) = assets.file(&path) {
            sheet.push_str(&format!("/* {path} */\n"));
            sheet.push_str(&String::from_utf8_lossy(content));
            if !sheet.ends_with('\n') {
                sheet.push('\n');
            }
        }
    }
    Some(sheet)
}

/// What the web client knows of its session from the start, so it begins without asking: who is
/// logged in, which groups they are in by external identifier, and the CSRF token its calls carry.
fn session_info(env: &mut Environment, request: &Request) -> Result<erp::serde_json::Value> {
    let uid = env.uid().ok_or("Only somebody logged in has a session")?;
    // Their own account, which they may not otherwise be allowed to read.
    let env = &mut *env.sudo();
    let user = Users::<SingleId>::from_id(uid, env);
    let mut groups = Vec::new();
    for group in user.get_groups::<Group<MultipleIds>>(env)? {
        if let Some(external_id) = data::external_id_of(env, "group", group.get_id())? {
            groups.push(external_id);
        }
    }
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
