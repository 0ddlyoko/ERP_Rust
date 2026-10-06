//! The operations every model answers to, without anyone declaring them.
//!
//! They are reserved names: a method exposed under one of them would be unreachable, so that is
//! refused when it is registered rather than discovered when a call goes somewhere unexpected.

use crate::access::Operation;
use crate::database::{FieldType as StoredValue, Group, GroupBy};
use crate::environment::{Environment, LineKey, Onchange};
use crate::model::{RegisteredKinds, RpcFn, Selections};
use erp_internal_types::FinalInternalField;
use erp_search::{OrderBy, RightTuple, SearchOperator, SearchOptions, SearchTuple, SearchType};
use erp_types::field::{
    FieldKind, FieldKinds, FieldReferenceType, IdMode, MapOfFieldsSeed, MultipleIds,
};
use erp_types::model::MapOfFields;
use serde::Deserialize;
use serde::de::DeserializeSeed;
use serde_json::{Value, json};
use std::collections::HashMap;
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
    ReadGroup,
    Create,
    Write,
    Delete,
    FieldsGet,
    Names,
    NameSearch,
    NameCreate,
    Onchange,
}

impl Verb {
    /// Every operation. Keeping a variant out of this list is the one mistake the compiler
    /// cannot catch, which is why nothing else enumerates them.
    pub const ALL: &'static [Verb] = &[
        Verb::Search,
        Verb::Read,
        Verb::ReadMatching,
        Verb::Count,
        Verb::ReadGroup,
        Verb::Create,
        Verb::Write,
        Verb::Delete,
        Verb::FieldsGet,
        Verb::Names,
        Verb::NameSearch,
        Verb::NameCreate,
        Verb::Onchange,
    ];

    /// The name a caller writes.
    pub fn name(self) -> &'static str {
        match self {
            Verb::Search => "search",
            Verb::Read => "read",
            Verb::ReadMatching => "read_matching",
            Verb::Count => "count",
            Verb::ReadGroup => "read_group",
            Verb::Create => "create",
            Verb::Write => "write",
            Verb::Delete => "delete",
            Verb::FieldsGet => "fields_get",
            Verb::Names => "names",
            Verb::NameSearch => "name_search",
            Verb::NameCreate => "name_create",
            Verb::Onchange => "onchange",
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
            Verb::ReadGroup => |env, model, params| dispatch(env, model, Verb::ReadGroup, params),
            Verb::Create => |env, model, params| dispatch(env, model, Verb::Create, params),
            Verb::Write => |env, model, params| dispatch(env, model, Verb::Write, params),
            Verb::Delete => |env, model, params| dispatch(env, model, Verb::Delete, params),
            Verb::FieldsGet => |env, model, params| dispatch(env, model, Verb::FieldsGet, params),
            Verb::Names => |env, model, params| dispatch(env, model, Verb::Names, params),
            Verb::NameSearch => |env, model, params| dispatch(env, model, Verb::NameSearch, params),
            Verb::NameCreate => |env, model, params| dispatch(env, model, Verb::NameCreate, params),
            Verb::Onchange => |env, model, params| dispatch(env, model, Verb::Onchange, params),
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
            let tuple = by_name(env, model_name, tuple)?;
            SearchType::Tuple(typed_dates(env, model_name, tuple)?)
        }
    })
}

/// A date or a moment compared with text, as JSON has to write it, compared with the date or
/// moment the text writes: `"2026-01-31"`, `"2026-01-31T08:00:00Z"`, `"2026-01-31 08:00:00"`.
fn typed_dates(env: &Environment, model_name: &str, mut tuple: SearchTuple) -> Result<SearchTuple> {
    let mut model = env.model_manager.try_get_model(model_name)?;
    let mut kind = None;
    for segment in &tuple.left.path {
        if segment == "id" {
            return Ok(tuple);
        }
        let field = model.try_get_internal_field(segment)?;
        kind = Some(field.kind);
        if let Some(reference) = &field.inverse {
            model = env.model_manager.try_get_model(reference.target_model)?;
        }
    }
    let typed = |right: &RightTuple| -> Result<RightTuple> {
        let RightTuple::String(text) = right else {
            return Ok(right.clone());
        };
        let path = tuple.left.path.join(".");
        Ok(match kind {
            Some(FieldKind::Date) => RightTuple::Date(
                chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
                    .map_err(|_| format!("{path}: \"{text}\" is not a date, YYYY-MM-DD"))?,
            ),
            Some(FieldKind::DateTime) => RightTuple::DateTime(moment(text).ok_or_else(|| {
                format!("{path}: \"{text}\" is not a moment, YYYY-MM-DDTHH:MM:SSZ")
            })?),
            _ => right.clone(),
        })
    };
    tuple.right = match &tuple.right {
        RightTuple::Array(items) => {
            RightTuple::Array(items.iter().map(typed).collect::<Result<Vec<_>>>()?)
        }
        right => typed(right)?,
    };
    Ok(tuple)
}

/// A moment written in RFC 3339, or as a date and a time of UTC, or as a day starting at midnight
/// UTC.
fn moment(text: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    if let Ok(moment) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(moment.with_timezone(&chrono::Utc));
    }
    if let Ok(moment) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S") {
        return Some(moment.and_utc());
    }
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .and_then(|day| day.and_hms_opt(0, 0, 0))
        .map(|start| start.and_utc())
}

/// A pattern on a relation matches the names of the records it points to: `("groups", "ilike",
/// "adm")` reads `("groups.name", "ilike", "adm")`, whatever field names the target's records.
fn by_name(env: &Environment, model_name: &str, tuple: &SearchTuple) -> Result<SearchTuple> {
    if !matches!(tuple.operator, SearchOperator::Like | SearchOperator::ILike)
        || !matches!(tuple.right, RightTuple::String(_))
    {
        return Ok(tuple.clone());
    }
    let mut current = model_name.to_string();
    for segment in &tuple.left.path {
        let model = env.model_manager.try_get_model(&current)?;
        match &model.try_get_internal_field(segment)?.inverse {
            Some(reference) => current = reference.target_model.to_string(),
            None => return Ok(tuple.clone()),
        }
    }
    let target = env.model_manager.try_get_model(&current)?;
    let mut named = tuple.clone();
    named.left.path.push(
        target
            .name_field()
            .ok_or_else(|| format!("Model \"{current}\" has no name to search by"))?
            .to_string(),
    );
    Ok(named)
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

/// One domain to count, or several at once: `domains` answers a count per domain, in order.
#[derive(Debug, Deserialize)]
struct CountParams {
    #[serde(default = "everything")]
    domain: SearchType,
    domains: Option<Vec<SearchType>>,
}

#[derive(Debug, Deserialize)]
struct ReadGroupParams {
    #[serde(default = "everything")]
    domain: SearchType,
    /// `state`, or `date_order:month`; all of them in one group when left out.
    #[serde(default)]
    group_by: Option<String>,
    #[serde(default)]
    sums: Vec<String>,
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
    /// Only among the records matching it: those the field searched from may point to.
    #[serde(default = "everything")]
    domain: SearchType,
    #[serde(default = "name_search_limit")]
    limit: usize,
}

#[derive(Deserialize)]
struct OnchangeParams {
    #[serde(default)]
    id: Option<u32>,
}

#[derive(Deserialize)]
struct NameCreateParams {
    text: String,
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
            let CountParams { domain, domains } = parse(params)?;
            match domains {
                None => {
                    let domain = blind_domain(env, model_name, &domain)?;
                    Ok(json!(env.count(model_name, &domain)?))
                }
                Some(domains) => {
                    let mut counts = Vec::with_capacity(domains.len());
                    for domain in &domains {
                        let domain = blind_domain(env, model_name, domain)?;
                        counts.push(env.count(model_name, &domain)?);
                    }
                    Ok(json!(counts))
                }
            }
        }
        Verb::ReadGroup => {
            let ReadGroupParams {
                domain,
                group_by,
                sums,
            } = parse(params)?;
            let domain = blind_domain(env, model_name, &domain)?;
            let group_by = group_by.as_deref().map(GroupBy::parse).transpose()?;
            let asked = group_by.iter().map(|group_by| &group_by.field).chain(&sums);
            for name in asked {
                if is_private(env, model_name, name)? {
                    return Err(format!("Field {model_name}.{name} is private").into());
                }
            }
            let sums: Vec<&str> = sums.iter().map(String::as_str).collect();
            let groups = env.read_group(model_name, &domain, group_by.as_ref(), &sums)?;
            groups_as_json(env, model_name, group_by.as_ref(), groups)
        }
        Verb::Read => {
            let ReadParams {
                ids,
                fields,
                names: with_names,
            } = parse(params)?;
            let (readable, hidden) = split_private(env, model_name, &fields)?;
            let names: Vec<&str> = readable.iter().map(String::as_str).collect();
            let ids = env.existing(model_name, ids)?;
            let mut rows = env.read(model_name, &ids, &names)?;
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
            let ids = env.existing(model_name, ids)?;
            env.write(model_name, &ids, values)?;
            Ok(json!(true))
        }
        Verb::Delete => {
            let IdsParams { ids } = parse(params)?;
            let ids = env.existing(model_name, ids)?;
            Ok(json!(env.delete(model_name, &ids)?))
        }
        Verb::FieldsGet => {
            let FieldsGetParams { fields } = parse(params)?;
            fields_get(env, model_name, &fields)
        }
        Verb::NameSearch => {
            let NameSearchParams {
                text,
                domain,
                limit,
            } = parse(params)?;
            let domain = blind_domain(env, model_name, &domain)?;
            let found = env.name_search_within(model_name, &text, &domain, limit)?;
            Ok(json!(
                found
                    .into_iter()
                    .map(|(id, name)| json!([id, name]))
                    .collect::<Vec<_>>()
            ))
        }
        Verb::Onchange => {
            let OnchangeParams { id } = parse(params)?;
            let mut params = params.clone();
            let drafts = take_drafts(&mut params["values"]);
            let mut values = records_of(env, model_name, &params, "values")?;
            refuse_private_writes(env, model_name, &values)?;
            let values = values.pop().unwrap_or_default();
            let onchange = env.onchange(model_name, id, values, &drafts)?;
            onchange_answer(env, model_name, onchange)
        }
        Verb::NameCreate => {
            let NameCreateParams { text } = parse(params)?;
            let (id, name) = env.name_create(model_name, &text)?;
            Ok(json!([id, name]))
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
        let mut description = describe(field, &env.model_manager.selections)?;
        if model.name_field() == Some(name.as_str()) {
            description["name_field"] = json!(true);
        }
        described.insert(name, description);
    }
    Ok(Value::Object(described))
}

/// Write every record these rows point to as `[id, name]`, the name `null` when the caller may
/// not read it: a many2one as one pair, a one2many or a many2many as a list of them. One search
/// per relation for all the rows, rather than one per row.
/// Take the draft numbers out of the lines a form creates, by field, in the order its commands
/// create them: they name lines for the answer, and are no field of theirs.
fn take_drafts(values: &mut Value) -> HashMap<String, Vec<Option<u32>>> {
    let mut drafts: HashMap<String, Vec<Option<u32>>> = HashMap::new();
    let Some(values) = values.as_object_mut() else {
        return drafts;
    };
    for (field, value) in values.iter_mut() {
        let objects: Vec<&mut Value> = match value {
            Value::Object(_) => vec![value],
            Value::Array(items) => items.iter_mut().filter(|item| item.is_object()).collect(),
            _ => continue,
        };
        for object in objects {
            let Some(Value::Array(created)) = object.get_mut("create") else {
                continue;
            };
            for line in created {
                let draft = line
                    .as_object_mut()
                    .and_then(|line| line.remove("draft"))
                    .and_then(|draft| draft.as_u64())
                    .and_then(|draft| u32::try_from(draft).ok());
                drafts.entry(field.clone()).or_default().push(draft);
            }
        }
    }
    drafts
}

/// What an onchange answers: the record's fields computed again, and those of its lines, by id
/// Groups as a client reads them: the value — a record as `[id, name]`, a period by its first
/// day — how many records, their sums, and the domain finding them, to open the group with.
fn groups_as_json(
    env: &mut Environment,
    model_name: &str,
    group_by: Option<&GroupBy>,
    groups: Vec<Group>,
) -> Result<Value> {
    let target = match group_by {
        Some(group_by) => {
            let model = env.model_manager.try_get_model(model_name)?;
            let field = model.try_get_internal_field(&group_by.field)?;
            match (&field.kind, &field.inverse) {
                (FieldKind::Ref, Some(reference)) => Some(reference.target_model),
                _ => None,
            }
        }
        None => None,
    };
    let names = match target {
        Some(target) => {
            let ids: Vec<u32> = groups
                .iter()
                .filter_map(|group| match group.key {
                    Some(StoredValue::UInteger(id)) => Some(id),
                    _ => None,
                })
                .collect();
            env.names(target, &ids)?
        }
        None => HashMap::new(),
    };
    let answer = groups
        .into_iter()
        .map(|group| {
            let raw = group.key.as_ref().map_or(Value::Null, stored_as_json);
            let value = match (&group.key, target) {
                (Some(StoredValue::UInteger(id)), Some(_)) => json!([id, names.get(id)]),
                _ => raw.clone(),
            };
            let domain = match group_by {
                None => json!([]),
                Some(GroupBy {
                    field,
                    period: Some(period),
                }) => match group.key {
                    Some(StoredValue::Date(start)) => json!([
                        [field, ">=", start.to_string()],
                        [field, "<", period.next(start).to_string()]
                    ]),
                    _ => json!([[field, "=", null]]),
                },
                Some(GroupBy { field, .. }) => json!([[field, "=", raw]]),
            };
            let sums: serde_json::Map<String, Value> = group
                .sums
                .into_iter()
                .map(|(name, total)| (name, json!(total.to_string())))
                .collect();
            json!({"value": value, "count": group.count, "sums": sums, "domain": domain})
        })
        .collect();
    Ok(Value::Array(answer))
}

/// A value of a column as the protocol writes it: a decimal as text, to keep every digit.
fn stored_as_json(value: &StoredValue) -> Value {
    match value {
        StoredValue::String(text) | StoredValue::Password(text) => json!(text),
        StoredValue::Integer(number) => json!(number),
        StoredValue::UInteger(number) => json!(number),
        StoredValue::Decimal(number) => json!(number.to_string()),
        StoredValue::Boolean(flag) => json!(flag),
        StoredValue::Date(date) => json!(date.to_string()),
        StoredValue::DateTime(moment) => json!(moment.to_rfc3339()),
    }
}

/// for a line that exists and by draft number for one being created; references with names.
fn onchange_answer(env: &mut Environment, model_name: &str, onchange: Onchange) -> Result<Value> {
    let named = |env: &mut Environment, model: &str, values: MapOfFields| -> Result<Value> {
        let fields: Vec<String> = values.fields.keys().cloned().collect();
        let mut rows = json!([values]);
        name_references(env, model, &fields, &mut rows)?;
        Ok(rows[0].clone())
    };
    let values = named(env, model_name, onchange.values)?;
    let errors: Vec<Value> = onchange
        .errors
        .into_iter()
        .map(|(field, message)| json!({"field": field, "message": message}))
        .collect();
    let mut lines = serde_json::Map::new();
    for field in onchange.lines {
        let mut updated = Vec::with_capacity(field.updated.len());
        for (id, values) in field.updated {
            updated.push(json!({"id": id, "values": named(env, &field.model, values)?}));
        }
        let mut created = Vec::with_capacity(field.created.len());
        for (draft, values) in field.created {
            created.push(json!({"draft": draft, "values": named(env, &field.model, values)?}));
        }
        let line_errors: Vec<Value> = field
            .errors
            .into_iter()
            .map(|(line, name, message)| {
                let mut error = json!({"field": name, "message": message});
                match line {
                    LineKey::Id(id) => error["id"] = json!(id),
                    LineKey::Draft(draft) => error["draft"] = json!(draft),
                }
                error
            })
            .collect();
        lines.insert(
            field.field,
            json!({"updated": updated, "created": created, "errors": line_errors}),
        );
    }
    Ok(json!({"values": values, "errors": errors, "lines": lines}))
}

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
        "readonly": (field.compute.is_some() && !field.editable) || field.automatic,
        "stored": field.is_stored(),
    });
    if let Some(reference) = &field.inverse {
        described["relation"] = json!(reference.target_model);
        described["relation_kind"] = json!(match reference.inverse_field {
            FieldReferenceType::M2O { .. } => "many2one",
            FieldReferenceType::O2M { .. } => "one2many",
            FieldReferenceType::M2M { .. } => "many2many",
        });
        if let FieldReferenceType::O2M { inverse_field } = &reference.inverse_field {
            described["inverse"] = json!(inverse_field);
        }
        if let Some(domain) = field.domain {
            described["domain"] = serde_json::from_str(domain)?;
        }
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
    let registered = RegisteredKinds {
        manager: env.model_manager,
        model,
    };
    let kinds = &registered as &dyn FieldKinds;

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
