use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use std::collections::HashMap;

/// A value the database keeps under a key, for plugins and administrators: `demo_data` is `1`
/// once the database wants demo data. One parameter per key.
#[derive(Model)]
#[erp(id = "parameter", methods)]
#[allow(dead_code)]
pub struct Parameter<Mode: IdMode> {
    pub id: Mode,
    #[erp(index)]
    key: String,
    value: Option<String>,
}

#[erp_methods]
impl Parameter<MultipleIds> {
    /// A key is held by one parameter only, so that reading it finds one value.
    #[erp(check)]
    pub fn check_keys(&self, env: &mut Environment) -> Result<()> {
        for parameter in self {
            let key = parameter.get_key(env)?.clone();
            let holding: Parameter<MultipleIds> =
                env.search(&make_domain!([("key", "=", key.as_str())]))?;
            if holding.get_ids_ref().len() > 1 {
                return Err(format!("Parameter \"{key}\" is already kept").into());
            }
        }
        Ok(())
    }

    /// The value kept under a key, if any; read as sudo, parameters being the database's —
    /// all of them at once while the application loads.
    pub fn value_of(env: &mut Environment, key: String) -> Result<Option<String>> {
        erp::plugin::parameter(env, &key)
    }

    /// The values kept under these keys, by key: those with none are left out.
    pub fn values_of(env: &mut Environment, keys: Vec<String>) -> Result<HashMap<String, String>> {
        let mut values = HashMap::new();
        for key in keys {
            if let Some(value) = erp::plugin::parameter(env, &key)? {
                values.insert(key, value);
            }
        }
        Ok(values)
    }

    /// Keep a value under a key: the parameter holding it changes, or one is made.
    pub fn keep(env: &mut Environment, key: String, value: String) -> Result<()> {
        let env = &mut *env.sudo();
        let found: Parameter<MultipleIds> =
            env.search(&make_domain!([("key", "=", key.as_str())]))?;
        match found.into_iter().next() {
            Some(parameter) => parameter.set_value(Some(value.clone()), env)?,
            None => {
                let mut values = MapOfFields::default();
                values.insert("key", key.as_str());
                values.insert("value", value.as_str());
                env.create_records("parameter", vec![values])?;
            }
        }
        erp::plugin::note_parameter(env, &key, Some(value));
        Ok(())
    }
}
