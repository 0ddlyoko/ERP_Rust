use code_gen::{Controller, erp_routes};
use erp::Result;
use erp::environment::Environment;
use erp::http::{Request, Response};

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
