use code_gen::Model;
use erp::data;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, SingleId};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// What opening something in a client does: show a model's records.
///
/// `views` lists the kinds of views a client offers, the first one shown: `list,form`. `domain`
/// narrows the records, as JSON in the form a caller sends; all of them when left out.
#[derive(Model)]
#[erp(id = "action")]
#[allow(dead_code)]
pub struct Action<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    name: String,
    #[erp(default = "")]
    model: String,
    #[erp(default = "list,form")]
    views: String,
    domain: Option<String>,
}

impl Action<SingleId> {
    /// What a client needs to open it: its external identifier — what a link names it by — its
    /// model, its kinds of views in order, and its domain.
    pub fn describe(&self, env: &mut Environment) -> Result<Value> {
        let domain: Value = match self.get_domain(env)? {
            Some(domain) => erp::serde_json::from_str(domain).map_err(|error| {
                format!(
                    "Action {} has a domain that is not one: {error}",
                    self.get_id()
                )
            })?,
            None => json!([]),
        };
        let views: Vec<String> = self
            .get_views(env)?
            .split(',')
            .map(str::trim)
            .filter(|kind| !kind.is_empty())
            .map(str::to_string)
            .collect();
        Ok(json!({
            "id": self.get_id(),
            "xml_id": data::external_id_of(env, "action", self.get_id())?,
            "name": self.get_name(env)?,
            "model": self.get_model(env)?,
            "views": views,
            "domain": domain,
        }))
    }
}
