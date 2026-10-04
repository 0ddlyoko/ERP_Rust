use code_gen::{Controller, erp_routes};
use erp::Result;
use erp::environment::Environment;
use erp::http::{Request, Response};

/// What the root of the site answers.
///
/// A placeholder page: the plugin that gives the application a face overrides `index`, to show
/// itself or to send the browser where it lives.
#[derive(Controller)]
#[erp(id = "home")]
pub struct Home;

#[erp_routes]
impl Home {
    #[erp(route = "/")]
    pub fn index(&self, env: &mut Environment, request: &Request) -> Result<Response> {
        let _ = (env, request);
        Ok(Response::html(
            "<!doctype html>\n<html><head><meta charset=\"utf-8\"><title>ERP</title></head>\
             <body><p>The server is running.</p></body></html>\n",
        ))
    }
}
