use crate::models::Template;
use crate::qweb::Values;
use code_gen::{Controller, erp_routes};
use erp::assets::content_type;
use erp::environment::Environment;
use erp::http::{HttpError, Request, Response};
use erp::types::field::SingleId;
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
        let page = Template::<SingleId>::render_page(env, "web.WebClientPage", Values::new())?;
        Ok(Response::html(page))
    }

    /// A file an installed plugin serves: `/static/<plugin>/<path>`.
    ///
    /// A path without an extension also finds the `.js` of that name, because a TypeScript import
    /// is written without one (`./util/format`) and a browser asks for exactly what it reads.
    #[erp(route = "/static/<module>/<*path>")]
    pub fn static_file(
        &self,
        env: &mut Environment,
        request: &Request,
        module: String,
        path: String,
    ) -> Result<Response> {
        let assets = &env.model_manager.assets;
        let asked = format!("{module}/static/{path}");
        let has_extension = path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.contains('.'));
        let found = assets.file(&asked).map(|content| (asked.clone(), content));
        let found = match found {
            Some(found) => Some(found),
            None if !has_extension => {
                let with_js = format!("{asked}.js");
                assets.file(&with_js).map(|content| (with_js, content))
            }
            None => None,
        };
        let Some((served, content)) = found else {
            return Err(HttpError::not_found("No installed plugin serves this file").into());
        };
        Ok(cached(request, content.to_vec(), content_type(&served)))
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
