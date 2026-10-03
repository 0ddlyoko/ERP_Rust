//! The operations every model answers to, without anyone declaring them.
//!
//! They are reserved names: a method exposed under one of them would be unreachable, so that is
//! refused when it is registered rather than discovered when a call goes somewhere unexpected.

use crate::access::Operation;
use crate::environment::Environment;
use crate::model::{RpcFn, Selections};
use erp_internal_types::FinalInternalField;
use erp_search::{OrderBy, RightTuple, SearchOperator, SearchOptions, SearchTuple, SearchType};
use erp_types::field::{
    FieldKind, FieldKinds, FieldReferenceType, IdMode, MapOfFieldsSeed, MultipleIds,
};
use erp_types::model::MapOfFields;
use serde::Deserialize;
use serde::de::DeserializeSeed;
use serde_json::{Value, json};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// An operation the protocol answers to on every model.
///
/// An enum rather than a list of names, so that the set of reserved names and the set the
/// dispatcher handles cannot drift apart: adding a variant without handling it fails to compile,
/// and there is nowhere left to handle one that does not exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Search,
    Read,
    ReadMatching,
    Count,
    Create,
    Write,
    Delete,
    FieldsGet,
    Names,
    NameSearch,
}

impl Verb {
    /// Every operation. Keeping a variant out of this list is the one mistake the compiler
    /// cannot catch, which is why nothing else enumerates them.
    pub const ALL: &'static [Verb] = &[
        Verb::Search,
        Verb::Read,
        Verb::ReadMatching,
        Verb::Count,
        Verb::Create,
        Verb::Write,
        Verb::Delete,
        Verb::FieldsGet,
        Verb::Names,
        Verb::NameSearch,
    ];

    /// The name a caller writes.
    pub fn name(self) -> &'static str {
        match self {
            Verb::Search => "search",
            Verb::Read => "read",
            Verb::ReadMatching => "read_matching",
            Verb::Count => "count",
            Verb::Create => "create",
            Verb::Write => "write",
            Verb::Delete => "delete",
            Verb::FieldsGet => "fields_get",
            Verb::Names => "names",
            Verb::NameSearch => "name_search",
        }
    }

    /// The operation a name refers to, if it refers to one at all.
    pub fn parse(name: &str) -> Option<Self> {
        Verb::ALL.iter().copied().find(|verb| verb.name() == name)
    }

    /// This operation, in the shape everything reachable by name has.
    ///
    /// Spelled out one by one rather than closed over `self`, because a function pointer carries
    /// nothing with it — and that is the price of the two kinds sharing one type.
    pub fn handler(self) -> RpcFn {
        match self {
            Verb::Search => |env, model, params| dispatch(env, model, Verb::Search, params),
            Verb::Read => |env, model, params| dispatch(env, model, Verb::Read, params),
            Verb::ReadMatching => {
                |env, model, params| dispatch(env, model, Verb::ReadMatching, params)
            }
            Verb::Count => |env, model, params| dispatch(env, model, Verb::Count, params),
            Verb::Create => |env, model, params| dispatch(env, model, Verb::Create, params),
            Verb::Write => |env, model, params| dispatch(env, model, Verb::Write, params),
            Verb::Delete => |env, model, params| dispatch(env, model, Verb::Delete, params),
            Verb::FieldsGet => |env, model, params| dispatch(env, model, Verb::FieldsGet, params),
            Verb::Names => |env, model, params| dispatch(env, model, Verb::Names, params),
            Verb::NameSearch => |env, model, params| dispatch(env, model, Verb::NameSearch, params),
        }
    }
}

/// Names no model method may take.
pub fn reserved_names() -> Vec<&'static str> {
    Verb::ALL.iter().map(|verb| verb.name()).collect()
}

/// Whether a field never leaves the process.
fn is_private(env: &Environment, model_name: &str, field_name: &str) -> Result<bool> {
    // The primary key is a real column that no model declares, so asking the registry about it
    // fails. It is never hidden: a caller that can read a record already holds its id.
    if field_name == "id" {
        return Ok(false);
    }
    let model = env.model_manager.try_get_model(model_name)?;
    Ok(model.try_get_internal_field(field_name)?.private)
}

/// The fields worth asking the ORM for, and the ones to answer empty.
///
/// A private field is answered as empty rather than refused, so a caller gets the shape it asked
/// for. It is dropped before the read rather than blanked after: a value nobody is going to send
/// has no reason to be fetched.
fn split_private(
    env: &Environment,
    model_name: &str,
    fields: &[String],
) -> Result<(Vec<String>, Vec<String>)> {
    let mut readable = Vec::with_capacity(fields.len());
    let mut hidden = Vec::new();
    for field in fields {
        if is_private(env, model_name, field)? {
            hidden.push(field.clone());
        } else {
            readable.push(field.clone());
        }
    }
    Ok((readable, hidden))
}

/// Refuse a write that touches a hidden field.
///
/// Reading one is answered empty, so a caller gets back the shape it asked for. Writing cannot be
/// treated the same way: dropping the value in silence would report a change that never happened,
/// and nothing the caller can read afterwards would say so. Until access rights exist, a hidden
/// field is written from inside the process or not at all.
fn refuse_private_writes(
    env: &Environment,
    model_name: &str,
    values: &[MapOfFields],
) -> Result<()> {
    for record in values {
        for field in record.get_keys() {
            if is_private(env, model_name, field)? {
                return Err(format!(
                    "Field \"{model_name}\".\"{field}\" cannot be written from outside the \
                     process"
                )
                .into());
            }
        }
    }
    Ok(())
}

/// Put the hidden fields back, empty, so the answer has the shape that was asked for.
fn blank_out(rows: &mut [MapOfFields], hidden: &[String]) {
    for row in rows.iter_mut() {
        for field in hidden {
            row.insert_none(field);
        }
    }
}

/// Rewrite a domain so that conditions on hidden fields select nothing.
///
/// Every segment of a path, not only the last: crossing a private relation says which records it
/// links, which is the thing being hidden.
fn blind_domain(env: &Environment, model_name: &str, domain: &SearchType) -> Result<SearchType> {
    Ok(match domain {
        SearchType::And(left, right) => SearchType::And(
            Box::new(blind_domain(env, model_name, left)?),
            Box::new(blind_domain(env, model_name, right)?),
        ),
        SearchType::Or(left, right) => SearchType::Or(
            Box::new(blind_domain(env, model_name, left)?),
            Box::new(blind_domain(env, model_name, right)?),
        ),
        SearchType::Nothing => SearchType::Nothing,
        SearchType::Never => SearchType::Never,
        SearchType::Tuple(tuple) => {
            let mut current = model_name.to_string();
            for segment in &tuple.left.path {
                if is_private(env, &current, segment)? {
                    return Ok(SearchType::Never);
                }
                if segment == "id" {
                    break;
                }
                let model = env.model_manager.try_get_model(&current)?;
                match &model.try_get_internal_field(segment)?.inverse {
                    Some(reference) => current = reference.target_model.to_string(),
                    None => break,
                }
            }
            refuse_unknown_keys(env, model_name, tuple)?;
            SearchType::Tuple(tuple.clone())
        }
    })
}

/// Refuse comparing a field holding an enum with a key it does not have: a misspelt key would
/// quietly select nothing. Patterns (`like`) are left alone.
fn refuse_unknown_keys(env: &Environment, model_name: &str, tuple: &SearchTuple) -> Result<()> {
    if !matches!(
        tuple.operator,
        SearchOperator::Equal
            | SearchOperator::NotEqual
            | SearchOperator::In
            | SearchOperator::NotIn
    ) {
        return Ok(());
    }
    let Some((last, through)) = tuple.left.path.split_last() else {
        return Ok(());
    };
    let mut current = model_name.to_string();
    for segment in through {
        let model = env.model_manager.try_get_model(&current)?;
        match &model.try_get_internal_field(segment)?.inverse {
            Some(reference) => current = reference.target_model.to_string(),
            None => return Ok(()),
        }
    }
    let model = env.model_manager.try_get_model(&current)?;
    let Some(family) = model.fields.get(last).and_then(|field| field.selection) else {
        return Ok(());
    };
    let keys = match &tuple.right {
        RightTuple::String(key) => vec![key],
        RightTuple::Array(values) => values
            .iter()
            .filter_map(|value| match value {
                RightTuple::String(key) => Some(key),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    let selections = &env.model_manager.selections;
    if let Some(unknown) = keys
        .into_iter()
        .find(|key| !selections.contains(family.family, key))
    {
        let known: Vec<&str> = selections
            .choices(family.family)
            .iter()
            .map(|choice| choice.key.as_str())
            .collect();
        return Err(format!(
            "\"{unknown}\" is not a value of field \"{last}\" of model \"{current}\": {}",
            known.join(", ")
        )
        .into());
    }
    Ok(())
}

/// Drop sort keys naming a hidden field.
///
/// Sorting by one would order the records by a value the caller cannot see, which hands over the
/// comparison it was denied.
fn visible_order(env: &Environment, model_name: &str, order: &[String]) -> Result<Vec<String>> {
    let mut kept = Vec::with_capacity(order.len());
    for key in order {
        let field = key
            .rsplit_once(' ')
            .map_or(key.as_str(), |(field, _)| field);
        if !is_private(env, model_name, field.trim())? {
            kept.push(key.clone());
        }
    }
    Ok(kept)
}

/// Whether a name belongs to the protocol rather than to a model.
pub fn is_reserved(name: &str) -> bool {
    Verb::parse(name).is_some()
}

/// What a search takes, beyond the domain.
#[derive(Debug, Default, Deserialize)]
struct Paging {
    limit: Option<usize>,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    order: Vec<String>,
}

impl Paging {
    fn into_options(self) -> Result<SearchOptions> {
        let mut options = SearchOptions::new().with_offset(self.offset);
        if let Some(limit) = self.limit {
            options = options.with_limit(limit);
        }
        for key in self.order {
            // "name desc" reads as one key and a direction, which is how a client writes it.
            let (field, descending) = match key.rsplit_once(' ') {
                Some((field, "desc")) => (field.trim(), true),
                Some((field, "asc")) => (field.trim(), false),
                _ => (key.trim(), false),
            };
            options = options.order_by(if descending {
                OrderBy::desc(field)
            } else {
                OrderBy::asc(field)
            });
        }
        Ok(options)
    }
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    #[serde(default = "everything")]
    domain: SearchType,
    #[serde(flatten)]
    paging: Paging,
}

#[derive(Debug, Deserialize)]
struct ReadParams {
    ids: Vec<u32>,
    fields: Vec<String>,
    /// Each record a relation points to as `[id, name]` rather than its id alone.
    #[serde(default)]
    names: bool,
}

#[derive(Debug, Deserialize)]
struct ReadMatchingParams {
    #[serde(default = "everything")]
    domain: SearchType,
    fields: Vec<String>,
    #[serde(default)]
    names: bool,
    #[serde(flatten)]
    paging: Paging,
}

#[derive(Debug, Deserialize)]
struct CountParams {
    #[serde(default = "everything")]
    domain: SearchType,
}

#[derive(Debug, Deserialize)]
struct FieldsGetParams {
    #[serde(default)]
    fields: Vec<String>,
}

#[derive(Deserialize)]
struct NameSearchParams {
    #[serde(default)]
    text: String,
    #[serde(default = "name_search_limit")]
    limit: usize,
}

/// How many records a name search finds unless told otherwise: what a drop-down shows.
fn name_search_limit() -> usize {
    8
}

#[derive(Deserialize)]
struct IdsParams {
    ids: Vec<u32>,
}

fn everything() -> SearchType {
    SearchType::Nothing
}

fn dispatch(env: &mut Environment, model_name: &str, verb: Verb, params: &Value) -> Result<Value> {
    match verb {
        Verb::Search => {
            let SearchParams { domain, paging } = parse(params)?;
            let domain = blind_domain(env, model_name, &domain)?;
            let mut paging = paging;
            paging.order = visible_order(env, model_name, &paging.order)?;
            let ids = env.search_ids_with(model_name, &domain, &paging.into_options()?)?;
            Ok(json!(ids))
        }
        Verb::Count => {
            let CountParams { domain } = parse(params)?;
            let domain = blind_domain(env, model_name, &domain)?;
            Ok(json!(env.count(model_name, &domain)?))
        }
        Verb::Read => {
            let ReadParams {
                ids,
                fields,
                names: with_names,
            } = parse(params)?;
            let (readable, hidden) = split_private(env, model_name, &fields)?;
            let names: Vec<&str> = readable.iter().map(String::as_str).collect();
            let mut rows = env.read(model_name, &MultipleIds::from(ids), &names)?;
            blank_out(&mut rows, &hidden);
            let mut rows = serde_json::to_value(rows)?;
            if with_names {
                name_references(env, model_name, &readable, &mut rows)?;
            }
            Ok(rows)
        }
        Verb::ReadMatching => {
            let ReadMatchingParams {
                domain,
                fields,
                names: with_names,
                paging,
            } = parse(params)?;
            let (readable, hidden) = split_private(env, model_name, &fields)?;
            let domain = blind_domain(env, model_name, &domain)?;
            let mut paging = paging;
            paging.order = visible_order(env, model_name, &paging.order)?;
            let names: Vec<&str> = readable.iter().map(String::as_str).collect();
            let mut rows =
                env.read_matching(model_name, &names, &domain, &paging.into_options()?)?;
            blank_out(&mut rows, &hidden);
            let mut rows = serde_json::to_value(rows)?;
            if with_names {
                name_references(env, model_name, &readable, &mut rows)?;
            }
            Ok(rows)
        }
        Verb::Create => {
            let values = records_of(env, model_name, params, "values")?;
            refuse_private_writes(env, model_name, &values)?;
            let created: MultipleIds = env.create_records(model_name, values)?;
            Ok(json!(created.get_ids_ref()))
        }
        Verb::Write => {
            let IdsParams { ids } = parse(params)?;
            let mut values = records_of(env, model_name, params, "values")?;
            refuse_private_writes(env, model_name, &values)?;
            let values = values.pop().unwrap_or_default();
            env.write(model_name, &MultipleIds::from(ids), values)?;
            Ok(json!(true))
        }
        Verb::Delete => {
            let IdsParams { ids } = parse(params)?;
            Ok(json!(env.delete(model_name, &MultipleIds::from(ids))?))
        }
        Verb::FieldsGet => {
            let FieldsGetParams { fields } = parse(params)?;
            fields_get(env, model_name, &fields)
        }
        Verb::NameSearch => {
            let NameSearchParams { text, limit } = parse(params)?;
            let found = env.name_search(model_name, &text, limit)?;
            Ok(json!(
                found
                    .into_iter()
                    .map(|(id, name)| json!([id, name]))
                    .collect::<Vec<_>>()
            ))
        }
        Verb::Names => {
            let IdsParams { ids } = parse(params)?;
            let names = env.names(model_name, &ids)?;
            Ok(json!(
                ids.iter()
                    .map(|id| json!([id, names.get(id)]))
                    .collect::<Vec<_>>()
            ))
        }
    }
}

/// What a client needs to show and edit a model's fields, by name: all of them, `id` included,
/// or those asked for.
///
/// Only for a caller who may read some record of the model. A private field is left out, as if it
/// did not exist; asking for it by name is answered the same as asking for one that does not.
fn fields_get(env: &mut Environment, model_name: &str, asked: &[String]) -> Result<Value> {
    env.check_model_access(model_name, Operation::Read)?;
    let model = env.model_manager.try_get_model(model_name)?;
    let visible = |name: &str| model.fields.get(name).filter(|field| !field.private);
    let names: Vec<String> = if asked.is_empty() {
        let mut names: Vec<String> = model
            .fields
            .keys()
            .filter(|name| visible(name).is_some())
            .cloned()
            .collect();
        names.push("id".to_string());
        names.sort();
        names
    } else {
        asked.to_vec()
    };
    let mut described = serde_json::Map::new();
    for name in names {
        if name == "id" {
            described.insert(name, describe_id());
            continue;
        }
        let Some(field) = visible(&name) else {
            return Err(format!("Model \"{model_name}\" has no field \"{name}\"").into());
        };
        described.insert(name, describe(field, &env.model_manager.selections)?);
    }
    Ok(Value::Object(described))
}

/// Write every record these rows point to as `[id, name]`, the name `null` when the caller may
/// not read it: a many2one as one pair, a one2many or a many2many as a list of them. One search
/// per relation for all the rows, rather than one per row.
fn name_references(
    env: &mut Environment,
    model_name: &str,
    fields: &[String],
    rows: &mut Value,
) -> Result<()> {
    let Some(rows) = rows.as_array_mut() else {
        return Ok(());
    };
    let model = env.model_manager.try_get_model(model_name)?;
    let relations: Vec<(String, &'static str)> = fields
        .iter()
        .filter_map(|name| {
            let field = model.fields.get(name)?;
            match (&field.kind, &field.inverse) {
                (FieldKind::Ref | FieldKind::Refs, Some(reference)) => {
                    Some((name.clone(), reference.target_model))
                }
                _ => None,
            }
        })
        .collect();
    let id_of = |value: &Value| value.as_u64().map(|id| id as u32);
    for (field, target) in relations {
        let ids: Vec<u32> = rows
            .iter()
            .flat_map(|row| match &row[&field] {
                Value::Array(ids) => ids.iter().filter_map(id_of).collect(),
                value => id_of(value).into_iter().collect::<Vec<_>>(),
            })
            .collect();
        let names = env.names(target, &ids)?;
        let named = |id: u32| json!([id, names.get(&id)]);
        for row in rows.iter_mut() {
            row[&field] = match &row[&field] {
                Value::Array(ids) => ids.iter().filter_map(id_of).map(named).collect(),
                value => match id_of(value) {
                    Some(id) => named(id),
                    None => continue,
                },
            };
        }
    }
    Ok(())
}

/// The id every record has, which no struct declares as a field.
fn describe_id() -> Value {
    json!({
        "type": "integer",
        "label": "ID",
        "required": false,
        "readonly": true,
        "stored": true,
    })
}

/// A field as the client reads it; one holding an enum is a `selection`, with its values in
/// order as `[[key, label], ...]`.
fn describe(field: &FinalInternalField, selections: &Selections) -> Result<Value> {
    let mut described = json!({
        "type": if field.selection.is_some() { "selection" } else { kind_name(field.kind) },
        "label": field.label,
        "required": field.required,
        "readonly": field.compute.is_some(),
        "stored": field.is_stored(),
    });
    if let Some(reference) = &field.inverse {
        described["relation"] = json!(reference.target_model);
        described["relation_kind"] = json!(match reference.inverse_field {
            FieldReferenceType::M2O { .. } => "many2one",
            FieldReferenceType::O2M { .. } => "one2many",
            FieldReferenceType::M2M { .. } => "many2many",
        });
    }
    if let Some(family) = field.selection {
        described["values"] = selections
            .choices(family.family)
            .iter()
            .map(|choice| json!([choice.key, choice.label]))
            .collect();
    }
    if let Some(default) = &field.default_value {
        described["default"] = serde_json::to_value(default)?;
    }
    if let Some(description) = &field.description {
        described["description"] = json!(description);
    }
    Ok(described)
}

fn kind_name(kind: FieldKind) -> &'static str {
    match kind {
        FieldKind::String => "string",
        FieldKind::Integer => "integer",
        FieldKind::Decimal => "decimal",
        FieldKind::Bool => "bool",
        FieldKind::Date => "date",
        FieldKind::DateTime => "datetime",
        FieldKind::Ref => "ref",
        FieldKind::Refs => "refs",
        FieldKind::Password => "password",
    }
}

fn parse<T: for<'de> Deserialize<'de>>(params: &Value) -> Result<T> {
    Ok(serde_json::from_value(params.clone())?)
}

/// Read one or several records' values, against the kinds the model declares.
///
/// A single object is taken as one record: writing to several ids and creating one are the common
/// cases, and both read better without a list of one.
fn records_of(
    env: &Environment,
    model_name: &str,
    params: &Value,
    key: &str,
) -> Result<Vec<MapOfFields>> {
    let raw = params
        .get(key)
        .ok_or_else(|| format!("missing \"{key}\""))?;
    let model = env.model_manager.try_get_model(model_name)?;
    let kinds = model as &dyn FieldKinds;

    match raw {
        Value::Array(entries) => entries
            .iter()
            .map(|entry| {
                MapOfFieldsSeed(kinds)
                    .deserialize(entry)
                    .map_err(Into::into)
            })
            .collect(),
        entry => Ok(vec![MapOfFieldsSeed(kinds).deserialize(entry)?]),
    }
}
