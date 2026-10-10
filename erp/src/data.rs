//! Loading the data a plugin ships with.
//!
//! Records are addressed by a stable external identifier, `module.name`, rather than by the
//! technical id the database hands out. That is what lets one plugin reference another's data,
//! and what lets a file be loaded again without duplicating anything.
use crate::Result;
use crate::environment::Environment;
use crate::model::Model;
use erp_search::SearchType;
use erp_search_code_gen::make_domain;
use erp_types::field::{FieldKind, FieldType, IdMode, MultipleIds, SingleId};
use erp_types::model::{CommonModel, MapOfFields};
use std::collections::HashMap;
use thiserror::Error;

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
    #[error("Field {field} of {record} holds one record, but is given several references")]
    SeveralReferences { field: String, record: String },
    #[error("No record is named {external_id}")]
    UnknownExternalId { external_id: String },
    #[error("{external_id} names a record of {actual}, not one of {expected}")]
    WrongModel {
        external_id: String,
        expected: String,
        actual: String,
    },
}

/// Load one XML document on behalf of `module`, as one piece of work: the checks its records
/// concern run once all of them are loaded, a payment term with the lines that follow it.
pub fn load(env: &mut Environment, module: &str, xml: &str) -> Result<()> {
    let document = roxmltree::Document::parse(xml).map_err(|source| DataError::Xml {
        module: module.to_string(),
        source,
    })?;
    let root = document.root_element();
    let root_noupdate = read_noupdate(root);

    let owned = env.external_ids.is_none();
    if owned {
        env.external_ids = Some(ExternalIds::default());
    }
    let loaded = env.checked(
        |env| {
            prefetch(env, module, root)?;
            for node in root.children().filter(roxmltree::Node::is_element) {
                if node.has_tag_name("function") {
                    call_function(env, module, node)?;
                } else {
                    load_record(env, module, node, root_noupdate, None)?;
                }
            }
            Ok(())
        },
        |_, ()| Ok(()),
    );
    if owned {
        env.external_ids = None;
    }
    loaded
}

/// The external identifiers modules gave, by module then name, each module read at once the
/// first time one of its names is looked up.
#[derive(Default)]
pub(crate) struct ExternalIds {
    modules: HashMap<String, HashMap<String, Named>>,
}

/// What an external identifier designates.
#[derive(Clone)]
struct Named {
    model: String,
    res_id: u32,
    noupdate: bool,
}

/// What the registry says of an external identifier, from the names of its module read at once
/// while a data file loads, else from the registry itself.
fn named_by(env: &mut Environment, external_id: &str) -> Result<Option<Named>> {
    let (module, name) = split(external_id);
    let known = env
        .external_ids
        .as_ref()
        .map(|registry| registry.modules.contains_key(module));
    match known {
        None => Ok(None),
        Some(true) => Ok(lookup(env, module, name)),
        Some(false) => {
            let names = names_given_by(env, module)?;
            if let Some(registry) = env.external_ids.as_mut() {
                registry.modules.insert(module.to_string(), names);
            }
            Ok(lookup(env, module, name))
        }
    }
}

fn lookup(env: &Environment, module: &str, name: &str) -> Option<Named> {
    env.external_ids
        .as_ref()?
        .modules
        .get(module)?
        .get(name)
        .cloned()
}

/// Every name a module gave, with what it designates: two queries.
fn names_given_by(env: &mut Environment, module: &str) -> Result<HashMap<String, Named>> {
    let env = &mut *env.sudo();
    let ids = env.search_ids(MODEL_DATA, &make_domain!([("module", "=", module)]))?;
    let rows = env.read(
        MODEL_DATA,
        &MultipleIds::from(ids),
        &["name", "model", "res_id", "noupdate"],
    )?;
    let mut names = HashMap::with_capacity(rows.len());
    for row in &rows {
        if let (Some(name), Some(model), Some(res_id)) = (
            row.get_option::<&String>("name"),
            row.get_option::<&String>("model"),
            row.get_option::<&i32>("res_id"),
        ) && let Ok(res_id) = u32::try_from(*res_id)
        {
            let noupdate = row
                .get_option::<&bool>("noupdate")
                .copied()
                .unwrap_or(false);
            names.insert(
                name.clone(),
                Named {
                    model: model.clone(),
                    res_id,
                    noupdate,
                },
            );
        }
    }
    Ok(names)
}

/// Read at once what the records a data file declares hold already, a query per model, so that
/// comparing each with what the file says costs none.
fn prefetch(env: &mut Environment, module: &str, root: roxmltree::Node) -> Result<()> {
    let mut existing: HashMap<String, Vec<u32>> = HashMap::new();
    let mut pending: Vec<roxmltree::Node> = root
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    while let Some(node) = pending.pop() {
        if node.has_tag_name("function") {
            continue;
        }
        let model_name = model_of(node);
        if env.model_manager.data_children(model_name).is_some() {
            pending.extend(
                node.children()
                    .filter(|child| child.has_tag_name(model_name)),
            );
        }
        let Some(name) = node.attribute("id") else {
            continue;
        };
        if let Some(named) = named_by(env, &qualify(module, name))? {
            existing.entry(named.model).or_default().push(named.res_id);
        }
    }
    let env = &mut *env.sudo();
    for (model_name, ids) in existing {
        let Ok(model) = env.model_manager.try_get_model(&model_name) else {
            continue;
        };
        let fields: Vec<String> = model
            .fields
            .values()
            .filter(|field| field.is_stored() || field.is_many2many())
            .map(|field| field.name.clone())
            .collect();
        let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
        env.read(&model_name, &MultipleIds::from(ids), &fields)?;
    }
    Ok(())
}

/// Run a method of a model on records the file names:
/// `<function model="sale_order" name="action_confirm" ref="order_1,order_2"/>`.
///
/// As a caller would run it, through the methods a model offers callers, so the records end up
/// as they would by hand — an order confirmed, its delivery made. It runs each time the document
/// is loaded: for documents loaded once, such as demo data.
fn call_function(env: &mut Environment, module: &str, node: roxmltree::Node) -> Result<()> {
    let attribute = |name: &str| {
        node.attribute(name)
            .ok_or_else(|| DataError::MissingAttribute {
                module: module.to_string(),
                attribute: name.to_string(),
            })
    };
    let model_name = attribute("model")?;
    let method = attribute("name")?;
    let mut ids = Vec::new();
    for reference in attribute("ref")?.split(',').map(str::trim) {
        let reference = qualify(module, reference);
        ids.push(
            resolve(env, &reference)?.ok_or_else(|| DataError::UnknownReference {
                reference: reference.clone(),
                record: format!("<function name=\"{method}\">"),
            })?,
        );
    }
    env.call_rpc(model_name, method, &serde_json::json!({ "ids": ids }))?;
    Ok(())
}

/// Model a record element declares, by the same rule fields follow.
///
/// A record is named by its own tag — `<group id="group_user">`. `<record model="group">` is the
/// long form, and the two only meet on `<record>`: with a `model`, it is the long form; without
/// one, it is the short form for a model actually called `record`.
///
/// Unlike the field rule, whose reserved set is permanently `{record}`, this one's grows as the
/// loader gains directives — `<function>` already, `<delete>` and `<menuitem>` to come. Each makes
/// one more model name reachable only through the long form.
fn model_of<'a>(node: roxmltree::Node<'a, 'a>) -> &'a str {
    match node.attribute("model") {
        Some(model) if node.has_tag_name("record") => model,
        _ => node.tag_name().name(),
    }
}

fn read_noupdate(node: roxmltree::Node) -> bool {
    matches!(node.attribute("noupdate"), Some("1") | Some("true"))
}

/// Create a record, or update it unless it is protected.
///
/// A field is given as a child element, or as an attribute of the record element —
/// `<template id="x" key="web.Client">`. A `ref` names another record by its external identifier,
/// and so does an attribute naming a relational field. A many2many takes several, separated by
/// commas — `<groups ref="group_user,group_admin"/>` — and replaces what the field held.
///
/// For a model with a body field ([`ModelManager::set_data_body`]), the content of the record
/// element is that field's value, rather than more fields. For a model nesting records
/// ([`ModelManager::set_data_children`]), an element of the model's own tag is a child record,
/// loaded after this one with `parent` naming it.
///
/// [`ModelManager::set_data_body`]: crate::model::ModelManager::set_data_body
/// [`ModelManager::set_data_children`]: crate::model::ModelManager::set_data_children
fn load_record(
    env: &mut Environment,
    module: &str,
    node: roxmltree::Node,
    inherited_noupdate: bool,
    parent: Option<(&str, u32)>,
) -> Result<u32> {
    let name = node
        .attribute("id")
        .ok_or_else(|| DataError::MissingAttribute {
            module: module.to_string(),
            attribute: "id".to_string(),
        })?;
    let model_name = model_of(node);
    let noupdate = inherited_noupdate || read_noupdate(node);
    let external_id = qualify(module, name);
    let record = Declared {
        module,
        model_name,
        external_id: &external_id,
    };

    let children_field = env
        .model_manager
        .data_children(model_name)
        .map(str::to_string);
    let mut children = Vec::new();
    let mut values = MapOfFields::default();
    if let Some((field, id)) = parent {
        values.insert_option(field, Some(FieldType::Ref(id)));
    }
    for attribute in node.attributes() {
        let field_name = attribute.name();
        let is_directive = matches!(field_name, "id" | "noupdate")
            || (field_name == "model" && node.has_tag_name("record"));
        if is_directive {
            continue;
        }
        let value = if record.is_relational(env, field_name)? {
            record.references(env, field_name, attribute.value())?
        } else {
            record.parse(env, field_name, attribute.value())?
        };
        values.insert_option(field_name, Some(value));
    }
    if let Some(body_field) = env.model_manager.data_body(model_name) {
        let value = record.parse(env, body_field, content_of(node).trim())?;
        values.insert_option(body_field, Some(value));
    } else {
        for field in node.children().filter(roxmltree::Node::is_element) {
            if children_field.is_some() && field.has_tag_name(model_name) {
                children.push(field);
                continue;
            }
            // A field is named by its own tag — `<price>7</price>`. `<field name="price">` is the
            // long form, kept because it reads better when generating files and because it makes
            // the intent explicit.
            //
            // The two only meet on `<field>`: with a `name`, it is the long form; without one, it
            // is the short form for a field actually called `field`. That is what lets every name
            // be written, including the ones that collide with the structural elements.
            let field_name = match field.attribute("name") {
                Some(name) if field.has_tag_name("field") => name,
                _ => field.tag_name().name(),
            };
            let value = match field.attribute("ref") {
                Some(references) => record.references(env, field_name, references)?,
                None => record.parse(env, field_name, content_of(field))?,
            };
            values.insert_option(field_name, Some(value));
        }
    }
    let id = save_record(env, module, name, model_name, values, noupdate)?;
    if let Some(children_field) = &children_field {
        for child in children {
            load_record(env, module, child, noupdate, Some((children_field, id)))?;
        }
    }
    Ok(id)
}

/// The record a data file declares, for reading its values.
struct Declared<'a> {
    module: &'a str,
    model_name: &'a str,
    external_id: &'a str,
}

impl Declared<'_> {
    /// The declared kind is what says whether "4" is a number or a reference.
    fn kind(&self, env: &Environment, field_name: &str) -> Result<FieldKind> {
        Ok(env
            .model_manager
            .try_get_model(self.model_name)?
            .try_get_internal_field(field_name)?
            .kind)
    }

    fn is_relational(&self, env: &Environment, field_name: &str) -> Result<bool> {
        Ok(matches!(
            self.kind(env, field_name)?,
            FieldKind::Ref | FieldKind::Refs
        ))
    }

    fn parse(&self, env: &Environment, field_name: &str, text: &str) -> Result<FieldType> {
        Ok(self.kind(env, field_name)?.parse(text)?)
    }

    fn references(
        &self,
        env: &mut Environment,
        field_name: &str,
        references: &str,
    ) -> Result<FieldType> {
        let mut targets = Vec::new();
        for reference in references.split(',').map(str::trim) {
            let reference = qualify(self.module, reference);
            targets.push(
                resolve(env, &reference)?.ok_or_else(|| DataError::UnknownReference {
                    reference: reference.clone(),
                    record: self.external_id.to_string(),
                })?,
            );
        }
        match (self.kind(env, field_name)?, targets.as_slice()) {
            (FieldKind::Refs, _) => Ok(FieldType::Refs(targets)),
            (_, [target]) => Ok(FieldType::Ref(*target)),
            _ => Err(DataError::SeveralReferences {
                field: field_name.to_string(),
                record: self.external_id.to_string(),
            }
            .into()),
        }
    }
}

/// Create the record `module.name`, or update it unless it is protected, and return its id.
///
/// What loading a data file does for each record, for a plugin declaring records from anything
/// else than a data file.
pub fn save_record(
    env: &mut Environment,
    module: &str,
    name: &str,
    model_name: &str,
    values: MapOfFields,
    noupdate: bool,
) -> Result<u32> {
    let external_id = qualify(module, name);
    match resolve(env, &external_id)? {
        Some(existing) => {
            if !is_protected(env, &external_id)? {
                env.write_changes(model_name, existing, values)?;
            }
            Ok(existing)
        }
        None => {
            let created: MultipleIds = env.create_records(model_name, vec![values])?;
            let id = *created
                .get_ids_ref()
                .first()
                .ok_or("Creating a record returned no id")?;
            remember(env, module, name, model_name, id, noupdate)?;
            Ok(id)
        }
    }
}

/// The external identifier of a record, `module.name`, if a data file or a plugin gave it one.
pub fn external_id_of(env: &mut Environment, model_name: &str, id: u32) -> Result<Option<String>> {
    Ok(external_ids_of(env, model_name, &[id])?.remove(&id))
}

/// The external identifiers of records, by id, for those a data file or a plugin named: two
/// queries whatever their number.
pub fn external_ids_of(
    env: &mut Environment,
    model_name: &str,
    ids: &[u32],
) -> Result<HashMap<u32, String>> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let env = &mut *env.sudo();
    let res_ids: Vec<i32> = ids.iter().map(|id| *id as i32).collect();
    let found = env.search_ids(
        MODEL_DATA,
        &make_domain!([("model", "=", model_name), ("res_id", "in", res_ids)]),
    )?;
    let rows = env.read(
        MODEL_DATA,
        &MultipleIds::from(found),
        &["module", "name", "res_id"],
    )?;
    let mut named = HashMap::with_capacity(rows.len());
    for row in &rows {
        if let (Some(module), Some(name), Some(res_id)) = (
            row.get_option::<&String>("module"),
            row.get_option::<&String>("name"),
            row.get_option::<&i32>("res_id"),
        ) {
            named
                .entry(*res_id as u32)
                .or_insert_with(|| format!("{module}.{name}"));
        }
    }
    Ok(named)
}

/// The names a module gave to records of a model, `name` in `module.name`.
pub fn names_of(env: &mut Environment, module: &str, model_name: &str) -> Result<Vec<String>> {
    let env = &mut *env.sudo();
    let ids = env.search_ids(
        MODEL_DATA,
        &make_domain!([("module", "=", module), ("model", "=", model_name)]),
    )?;
    let rows = env.read(MODEL_DATA, &MultipleIds::from(ids), &["name"])?;
    Ok(rows
        .iter()
        .map(|row| row.get::<&String>("name").clone())
        .collect())
}

/// Delete the record an external identifier names, and the identifier with it.
///
/// Returns whether there was one.
pub fn delete_record(env: &mut Environment, external_id: &str) -> Result<bool> {
    let Some((model, res_id)) = designated(env, external_id)? else {
        return Ok(false);
    };
    env.delete(&model, &SingleId::from(res_id))?;
    let (module, name) = split(external_id);
    if let Some(names) = env
        .external_ids
        .as_mut()
        .and_then(|registry| registry.modules.get_mut(module))
    {
        names.remove(name);
    }
    let env = &mut *env.sudo();
    let ids = env.search_ids(MODEL_DATA, &domain_for(external_id))?;
    env.delete(MODEL_DATA, &MultipleIds::from(ids))?;
    Ok(true)
}

/// What an element holds: its text, or — when it holds elements, as a template's markup does —
/// its content exactly as written.
fn content_of<'a>(field: roxmltree::Node<'a, 'a>) -> &'a str {
    if !field.children().any(|child| child.is_element()) {
        return field.text().unwrap_or_default();
    }
    match (field.first_child(), field.last_child()) {
        (Some(first), Some(last)) => {
            &field.document().input_text()[first.range().start..last.range().end]
        }
        _ => "",
    }
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
///
/// As sudo: the registry is how code names records, and naming one is not reading it.
fn designated(env: &mut Environment, external_id: &str) -> Result<Option<(String, u32)>> {
    if env.external_ids.is_some() {
        return Ok(named_by(env, external_id)?.map(|named| (named.model, named.res_id)));
    }
    let env = &mut *env.sudo();
    let ids = env.search_ids(MODEL_DATA, &domain_for(external_id))?;
    let Some(id) = ids.first() else {
        return Ok(None);
    };
    let rows = env.read(MODEL_DATA, &SingleId::from(*id), &["model", "res_id"])?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let (Some(model), Some(res_id)) = (
        row.get_option::<&String>("model"),
        row.get_option::<&i32>("res_id"),
    ) else {
        return Ok(None);
    };
    let Ok(res_id) = u32::try_from(*res_id) else {
        return Ok(None);
    };
    Ok(Some((model.clone(), res_id)))
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
    if env.external_ids.is_some() {
        return Ok(named_by(env, external_id)?.is_some_and(|named| named.noupdate));
    }
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
    if let Some(names) = env
        .external_ids
        .as_mut()
        .and_then(|registry| registry.modules.get_mut(module))
    {
        names.insert(
            name.to_string(),
            Named {
                model: model_name.to_string(),
                res_id,
                noupdate,
            },
        );
    }
    Ok(())
}
