//! The operations every model answers to, without anyone declaring them.
//!
//! They are reserved names: a method exposed under one of them would be unreachable, so that is
//! refused when it is registered rather than discovered when a call goes somewhere unexpected.

use crate::environment::Environment;
use crate::model::RpcFn;
use erp_search::{OrderBy, SearchOptions, SearchType};
use erp_types::field::{FieldKinds, IdMode, MapOfFieldsSeed, MultipleIds};
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
        }
    }
}

/// Names no model method may take.
pub fn reserved_names() -> Vec<&'static str> {
    Verb::ALL.iter().map(|verb| verb.name()).collect()
}

/// Whether a field never leaves the process.
fn is_private(env: &Environment, model_name: &str, field_name: &str) -> Result<bool> {
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
                let model = env.model_manager.try_get_model(&current)?;
                match &model.try_get_internal_field(segment)?.inverse {
                    Some(reference) => current = reference.target_model.clone(),
                    None => break,
                }
            }
            SearchType::Tuple(tuple.clone())
        }
    })
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
}

#[derive(Debug, Deserialize)]
struct ReadMatchingParams {
    #[serde(default = "everything")]
    domain: SearchType,
    fields: Vec<String>,
    #[serde(flatten)]
    paging: Paging,
}

#[derive(Debug, Deserialize)]
struct CountParams {
    #[serde(default = "everything")]
    domain: SearchType,
}

#[derive(Debug, Deserialize)]
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
            let ReadParams { ids, fields } = parse(params)?;
            let (readable, hidden) = split_private(env, model_name, &fields)?;
            let names: Vec<&str> = readable.iter().map(String::as_str).collect();
            let mut rows = env.read(model_name, &MultipleIds::from(ids), &names)?;
            blank_out(&mut rows, &hidden);
            Ok(serde_json::to_value(rows)?)
        }
        Verb::ReadMatching => {
            let ReadMatchingParams {
                domain,
                fields,
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
            Ok(serde_json::to_value(rows)?)
        }
        Verb::Create => {
            let values = records_of(env, model_name, params, "values")?;
            let created: MultipleIds = env.create_records(model_name, values)?;
            Ok(json!(created.get_ids_ref()))
        }
        Verb::Write => {
            let IdsParams { ids } = parse(params)?;
            let mut values = records_of(env, model_name, params, "values")?;
            let values = values.pop().unwrap_or_default();
            env.write(model_name, &MultipleIds::from(ids), values)?;
            Ok(json!(true))
        }
        Verb::Delete => {
            let IdsParams { ids } = parse(params)?;
            Ok(json!(env.delete(model_name, &MultipleIds::from(ids))?))
        }
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
