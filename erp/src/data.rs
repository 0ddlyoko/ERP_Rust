//! Loading the data a plugin ships with.
//!
//! Records are addressed by a stable external identifier, `module.name`, rather than by the
//! technical id the database hands out. That is what lets one plugin reference another's data,
//! and what lets a file be loaded again without duplicating anything.
use crate::environment::Environment;
use crate::model::Model;
use erp_search::SearchType;
use erp_search_code_gen::make_domain;
use erp_types::field::{FieldType, IdMode, MultipleIds, SingleId};
use erp_types::model::{CommonModel, MapOfFields};
use std::error::Error;
use thiserror::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Model holding the external identifier registry.
const MODEL_DATA: &str = "model_data";

#[derive(Debug, Error)]
pub enum DataError {
    #[error("Cannot read the data file of plugin {module}: {source}")]
    Xml {
        module: String,
        source: roxmltree::Error,
    },
    #[error("A <record> of plugin {module} has no {attribute}")]
    MissingAttribute { module: String, attribute: String },
    #[error("Record {reference}, referenced by {record}, does not exist")]
    UnknownReference { reference: String, record: String },
    #[error("No record is named {external_id}")]
    UnknownExternalId { external_id: String },
    #[error("{external_id} names a record of {actual}, not one of {expected}")]
    WrongModel {
        external_id: String,
        expected: String,
        actual: String,
    },
}

/// Load one XML document on behalf of `module`.
pub fn load(env: &mut Environment, module: &str, xml: &str) -> Result<()> {
    let document = roxmltree::Document::parse(xml).map_err(|source| DataError::Xml {
        module: module.to_string(),
        source,
    })?;
    let root = document.root_element();
    let root_noupdate = read_noupdate(root);

    for node in root.children().filter(roxmltree::Node::is_element) {
        load_record(env, module, node, root_noupdate)?;
    }
    Ok(())
}

/// Model a record element declares, by the same rule fields follow.
///
/// A record is named by its own tag — `<group id="group_user">`. `<record model="group">` is the
/// long form, and the two only meet on `<record>`: with a `model`, it is the long form; without
/// one, it is the short form for a model actually called `record`.
///
/// Unlike the field rule, whose reserved set is permanently `{record}`, this one's will grow as
/// the loader gains directives — `<delete>`, `<function>`, `<menuitem>`. Each will make one more
/// model name reachable only through the long form.
fn model_of<'a>(node: roxmltree::Node<'a, 'a>) -> &'a str {
    match node.attribute("model") {
        Some(model) if node.has_tag_name("record") => model,
        _ => node.tag_name().name(),
    }
}

fn read_noupdate(node: roxmltree::Node) -> bool {
    matches!(node.attribute("noupdate"), Some("1") | Some("true"))
}

fn load_record(
    env: &mut Environment,
    module: &str,
    node: roxmltree::Node,
    inherited_noupdate: bool,
) -> Result<()> {
    let name = node
        .attribute("id")
        .ok_or_else(|| DataError::MissingAttribute {
            module: module.to_string(),
            attribute: "id".to_string(),
        })?;
    let model_name = model_of(node);
    let noupdate = inherited_noupdate || read_noupdate(node);
    let external_id = qualify(module, name);

    let mut values = MapOfFields::default();
    for field in node.children().filter(roxmltree::Node::is_element) {
        // A field is named by its own tag — `<price>7</price>`. `<field name="price">` is the
        // long form, kept because it reads better when generating files and because it makes the
        // intent explicit.
        //
        // The two only meet on `<field>`: with a `name`, it is the long form; without one, it is
        // the short form for a field actually called `field`. That is what lets every name be
        // written, including the ones that collide with the structural elements.
        let field_name = match field.attribute("name") {
            Some(name) if field.has_tag_name("field") => name,
            _ => field.tag_name().name(),
        };

        let value = match field.attribute("ref") {
            Some(reference) => {
                let reference = qualify(module, reference);
                let target =
                    resolve(env, &reference)?.ok_or_else(|| DataError::UnknownReference {
                        reference: reference.clone(),
                        record: external_id.clone(),
                    })?;
                Some(FieldType::Ref(target))
            }
            None => {
                let raw = field.text().unwrap_or_default();
                // The declared kind is what says whether "4" is a number or a reference.
                let kind = env
                    .model_manager
                    .try_get_model(model_name)?
                    .try_get_internal_field(field_name)?
                    .kind;
                Some(kind.parse(raw)?)
            }
        };
        values.insert_option(field_name, value);
    }

    match resolve(env, &external_id)? {
        Some(existing) => {
            if !is_protected(env, &external_id)? {
                env.write(model_name, &SingleId::from(existing), values)?;
            }
        }
        None => {
            let created: MultipleIds = env.create_records(model_name, vec![values])?;
            let id = *created
                .get_ids_ref()
                .first()
                .ok_or("Creating a record returned no id")?;
            remember(env, module, name, model_name, id, noupdate)?;
        }
    }
    Ok(())
}

/// Qualify a bare name with the module declaring it.
fn qualify(module: &str, name: &str) -> String {
    if name.contains('.') {
        name.to_string()
    } else {
        format!("{module}.{name}")
    }
}

fn split(external_id: &str) -> (&str, &str) {
    external_id.split_once('.').unwrap_or(("", external_id))
}

fn domain_for(external_id: &str) -> SearchType {
    let (module, name) = split(external_id);
    make_domain!([("module", "=", module), ("name", "=", name)])
}

/// Technical id behind an external identifier, if it has one yet.
pub fn resolve(env: &mut Environment, external_id: &str) -> Result<Option<u32>> {
    Ok(designated(env, external_id)?.map(|(_, res_id)| res_id))
}

/// The model and technical id behind an external identifier.
fn designated(env: &mut Environment, external_id: &str) -> Result<Option<(String, u32)>> {
    let ids = env.search_ids(MODEL_DATA, &domain_for(external_id))?;
    let Some(id) = ids.first() else {
        return Ok(None);
    };
    let rows = env.read(MODEL_DATA, &SingleId::from(*id), &["model", "res_id"])?;
    let model: String = rows[0].get::<&String>("model").clone();
    let res_id: i32 = *rows[0].get::<&i32>("res_id");
    Ok(Some((model, res_id as u32)))
}

impl Environment<'_> {
    /// The record a data file declared under this name.
    ///
    /// Typed, so what comes back is the record rather than a number, and so the model it belongs
    /// to can be checked against the one asked for. That check is the reason to prefer this over
    /// [`resolve`]: the registry knows what `base.group_user` designates, and nothing else would
    /// notice a caller reading it as a user.
    ///
    /// Not named `ref` — that is a Rust keyword, and every call would have to be written
    /// `env.r#ref::<…>(…)`.
    pub fn named<M>(&mut self, external_id: &str) -> Result<M>
    where
        M: Model<SingleId>,
    {
        let Some((model, res_id)) = designated(self, external_id)? else {
            return Err(DataError::UnknownExternalId {
                external_id: external_id.to_string(),
            }
            .into());
        };
        let expected = <M as CommonModel<SingleId>>::_get_model_name();
        if model != expected {
            return Err(DataError::WrongModel {
                external_id: external_id.to_string(),
                expected: expected.to_string(),
                actual: model,
            }
            .into());
        }
        Ok(M::create_instance(SingleId::from(res_id)))
    }
}

/// Whether later loads must leave the record alone.
fn is_protected(env: &mut Environment, external_id: &str) -> Result<bool> {
    let ids = env.search_ids(MODEL_DATA, &domain_for(external_id))?;
    let Some(id) = ids.first() else {
        return Ok(false);
    };
    let rows = env.read(MODEL_DATA, &SingleId::from(*id), &["noupdate"])?;
    Ok(*rows[0].get::<&bool>("noupdate"))
}

fn remember(
    env: &mut Environment,
    module: &str,
    name: &str,
    model_name: &str,
    res_id: u32,
    noupdate: bool,
) -> Result<()> {
    let mut values = MapOfFields::default();
    values.insert("module", module);
    values.insert("name", name);
    values.insert("model", model_name);
    values.insert("res_id", res_id as i32);
    values.insert("noupdate", noupdate);
    env.create_records(MODEL_DATA, vec![values])?;
    Ok(())
}
