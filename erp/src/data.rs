//! Loading the data a plugin ships with.
//!
//! Records are addressed by a stable external identifier, `module.name`, rather than by the
//! technical id the database hands out. That is what lets one plugin reference another's data,
//! and what lets a file be loaded again without duplicating anything.
use crate::environment::Environment;
use erp_search::{LeftTuple, SearchOperator, SearchTuple, SearchType};
use erp_types::field::{FieldType, IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
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
    #[error("A <field> of record {record} has no name")]
    UnnamedField { record: String },
    #[error("Record {reference}, referenced by {record}, does not exist")]
    UnknownReference { reference: String, record: String },
}

/// Load one XML document on behalf of `module`.
pub fn load(env: &mut Environment, module: &str, xml: &str) -> Result<()> {
    let document = roxmltree::Document::parse(xml).map_err(|source| DataError::Xml {
        module: module.to_string(),
        source,
    })?;
    let root = document.root_element();
    let root_noupdate = read_noupdate(root);

    for node in root.children().filter(|node| node.has_tag_name("record")) {
        load_record(env, module, node, root_noupdate)?;
    }
    Ok(())
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
    let model_name = node
        .attribute("model")
        .ok_or_else(|| DataError::MissingAttribute {
            module: module.to_string(),
            attribute: "model".to_string(),
        })?;
    let noupdate = inherited_noupdate || read_noupdate(node);
    let external_id = qualify(module, name);

    let mut values = MapOfFields::default();
    for field in node.children().filter(|node| node.has_tag_name("field")) {
        let field_name = field
            .attribute("name")
            .ok_or_else(|| DataError::UnnamedField {
                record: external_id.clone(),
            })?;

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
    SearchType::And(
        Box::new(SearchType::Tuple(SearchTuple {
            left: LeftTuple::from("module"),
            operator: SearchOperator::Equal,
            right: module.into(),
        })),
        Box::new(SearchType::Tuple(SearchTuple {
            left: LeftTuple::from("name"),
            operator: SearchOperator::Equal,
            right: name.into(),
        })),
    )
}

/// Technical id behind an external identifier, if it has one yet.
pub fn resolve(env: &mut Environment, external_id: &str) -> Result<Option<u32>> {
    let ids = env.search_ids(MODEL_DATA, &domain_for(external_id))?;
    let Some(id) = ids.first() else {
        return Ok(None);
    };
    let rows = env.read(MODEL_DATA, &SingleId::from(*id), &["res_id"])?;
    let res_id: i32 = *rows[0].get::<&i32>("res_id");
    Ok(Some(res_id as u32))
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
