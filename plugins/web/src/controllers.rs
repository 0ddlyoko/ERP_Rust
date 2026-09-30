use code_gen::{Controller, erp_routes};
use erp::assets::content_type;
use erp::environment::Environment;
use erp::http::{HttpError, Request, Response};
use std::error::Error;
use std::hash::{DefaultHasher, Hash, Hasher};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// The root of the site sends the browser to the web client.
#[derive(Controller)]
#[erp(id = "home")]
pub struct Home;

#[erp_routes]
impl Home {
    pub fn index(&self, env: &mut Environment, request: &Request) -> Result<Response> {
        let _ = (env, request);
        Ok(Response::redirect("/web"))
    }
}

/// The web client, and what it loads.
#[derive(Controller)]
#[erp(id = "web")]
pub struct Web;

#[erp_routes]
impl Web {
    /// A placeholder until the web client is rendered from its page template.
    #[erp(route = "/web")]
    pub fn client(&self, env: &mut Environment, request: &Request) -> Result<Response> {
        let _ = (env, request);
        Ok(Response::html(
            "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>ERP</title></head>\
             <body><h1>ERP</h1><p>The web client is on its way.</p></body></html>\n",
        ))
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

    /// The JavaScript of a bundle: `/web/assets/<bundle>.js`.
    ///
    /// A module importing each JavaScript file of the bundle, in the bundle's order. The files stay
    /// separate modules, each at its own URL, so their relative imports resolve as written.
    #[erp(route = "/web/assets/<file>")]
    pub fn bundle(
        &self,
        env: &mut Environment,
        request: &Request,
        file: String,
    ) -> Result<Response> {
        let not_found = || HttpError::not_found("No installed plugin contributes to this bundle");
        let Some(name) = file.strip_suffix(".js") else {
            return Err(not_found().into());
        };
        let assets = &env.model_manager.assets;
        if !assets.bundles().contains(&name) {
            return Err(not_found().into());
        }
        let mut module = format!("// Bundle {name}\n");
        for path in assets.bundle(name) {
            if !path.ends_with(".js") {
                continue;
            }
            if let Some((plugin, rest)) = path.split_once("/static/") {
                module.push_str(&format!("import \"/static/{plugin}/{rest}\";\n"));
            }
        }
        Ok(cached(
            request,
            module.into_bytes(),
            "text/javascript; charset=utf-8",
        ))
    }
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
