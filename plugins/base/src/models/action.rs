use code_gen::{Model, erp_methods};
use erp::Result;
use erp::data;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds, SingleId};
use std::collections::HashMap;

/// What opening something in a client does: show a model's records.
///
/// `views` lists the kinds of views a client offers, the first one shown: `list,form`. `domain`
/// narrows the records, as JSON in the form a caller sends; all of them when left out.
#[derive(Model)]
#[erp(id = "action", methods)]
#[allow(dead_code)]
pub struct Action<Mode: IdMode> {
    pub id: Mode,
    name: String,
    model: String,
    #[erp(default = "list,form")]
    views: String,
    domain: Option<String>,
}

impl Action<SingleId> {
    /// What a client needs to open it: its external identifier — what a link names it by — its
    /// model, its kinds of views in order, and its domain.
    pub fn describe(&self, env: &mut Environment) -> Result<Value> {
        let xml_id = data::external_id_of(env, "action", self.get_id())?;
        self.describe_as(env, xml_id)
    }

    fn describe_as(&self, env: &mut Environment, xml_id: Option<String>) -> Result<Value> {
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
            "xml_id": xml_id,
            "name": self.get_name(env)?,
            "model": self.get_model(env)?,
            "views": views,
            "domain": domain,
        }))
    }
}

impl Action<MultipleIds> {
    /// Each action described as [`Action::describe`] does, by id: their external identifiers
    /// looked up together rather than one by one.
    pub fn describe_each(&self, env: &mut Environment) -> Result<HashMap<u32, Value>> {
        let mut xml_ids = data::external_ids_of(env, "action", &self.get_ids())?;
        let mut described = HashMap::new();
        for action in self {
            let xml_id = xml_ids.remove(&action.get_id());
            described.insert(action.get_id(), action.describe_as(env, xml_id)?);
        }
        Ok(described)
    }
}

#[erp_methods]
impl Action<MultipleIds> {
    /// An action by its external identifier, described the way a client opens it.
    ///
    /// As sudo: actions are read by administrators only, yet a button of anybody's form may open
    /// one. What it opens stays under the rights of its model.
    #[erp(rpc)]
    pub fn load(&self, env: &mut Environment, xml_id: String) -> Result<Value> {
        let _ = self;
        let env = &mut *env.sudo();
        let action: Action<SingleId> = env.named(&xml_id)?;
        action.describe(env)
    }
}
