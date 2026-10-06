use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;

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

impl Parameter<MultipleIds> {
    /// The value kept under a key, if any; read as sudo, parameters being the database's.
    pub fn value_of(env: &mut Environment, key: &str) -> Result<Option<String>> {
        let env = &mut *env.sudo();
        let found: Parameter<MultipleIds> = env.search(&make_domain!([("key", "=", key)]))?;
        match found.into_iter().next() {
            Some(parameter) => Ok(parameter.get_value(env)?.cloned()),
            None => Ok(None),
        }
    }

    /// Keep a value under a key: the parameter holding it changes, or one is made.
    pub fn keep(env: &mut Environment, key: &str, value: &str) -> Result<()> {
        let env = &mut *env.sudo();
        let found: Parameter<MultipleIds> = env.search(&make_domain!([("key", "=", key)]))?;
        match found.into_iter().next() {
            Some(parameter) => parameter.set_value(Some(value.to_string()), env),
            None => {
                let mut values = MapOfFields::default();
                values.insert("key", key);
                values.insert("value", value);
                env.create_records("parameter", vec![values])?;
                Ok(())
            }
        }
    }
}

#[erp_methods]
impl Parameter<MultipleIds> {
    /// A key is held by one parameter only, so that reading it finds one value.
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

    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let ids: MultipleIds = sup.call_with(values, env)?;
        Parameter::<MultipleIds>::from_ids(ids.get_ids_ref().clone(), env).check_keys(env)?;
        Ok(ids)
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        sup.call_with(values, env)?;
        self.check_keys(env)
    }
}
