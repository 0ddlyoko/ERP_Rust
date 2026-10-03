use crate::models::{BaseMessageChange, MessageChange};
use base::models::{BaseUsers, Users};
use code_gen::{Model, erp_methods, selection};
use erp::access::Operation;
use erp::environment::Environment;
use erp::internal_types::FinalInternalField;
use erp::model::TrackedChange;
use erp::serde_json::{Value, json};
use erp::types::field::{
    FieldType, IdMode, MultipleIds, Reference, Selection, SingleId, Timestamp, Utc,
};
use erp::types::model::MapOfFields;
use erp_search::{OrderBy, SearchOptions, SearchType};
use erp_search_code_gen::make_domain;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[selection]
pub enum MessageKind {
    #[default]
    Comment,
    Note,
    Tracking,
    Creation,
}

/// Something said about a record, or noted on it: a message of the record's thread.
///
/// Tied to the record by its model and id rather than by a relation, so that any record has a
/// thread without its model declaring anything.
#[derive(Model)]
#[erp(id = "message", methods)]
#[allow(dead_code)]
pub struct Message<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    model: String,
    #[erp(default = 0)]
    record: i32,
    author: Reference<BaseUsers, SingleId>,
    date: Timestamp,
    kind: MessageKind,
    body: Option<String>,
    #[erp(inverse = "message")]
    changes: Reference<BaseMessageChange, MultipleIds>,
}

/// The models a thread is made of, whose own changes are not noted.
const THREAD_MODELS: [&str; 2] = ["message", "message_change"];

#[erp_methods]
impl Message<MultipleIds> {
    /// The thread of a record, newest first, for whoever may read the record; given a `field`,
    /// only the messages noting a change of it.
    ///
    /// As sudo once the record is checked: a message is readable by whoever reads its record, a
    /// right no access rule on `message` could express.
    #[erp(rpc)]
    pub fn thread(
        &self,
        env: &mut Environment,
        model: String,
        record: u32,
        field: Option<String>,
    ) -> Result<Value> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let env = &mut *env.sudo();
        let options = SearchOptions {
            order: vec![OrderBy::desc("date"), OrderBy::desc("id")],
            ..SearchOptions::new()
        };
        let mut domain = make_domain!([
            ("model", "=", model),
            ("record", "=", i32::try_from(record)?)
        ]);
        if let Some(field) = field {
            domain = SearchType::And(
                Box::new(domain),
                Box::new(make_domain!([("changes.field", "=", field)])),
            );
        }
        let messages: Message<MultipleIds> = env.search_with(&domain, &options)?;
        let mut thread = Vec::new();
        for message in messages {
            thread.push(message.describe(env)?);
        }
        Ok(Value::Array(thread))
    }
}

impl Message<SingleId> {
    /// What a client shows of the message: who, when, what kind, and what it says or changed.
    fn describe(&self, env: &mut Environment) -> Result<Value> {
        let author = match self.get_author::<Users<SingleId>>(env)? {
            Some(user) => json!([user.get_id(), user.get_name(env)?]),
            None => Value::Null,
        };
        let changes: MessageChange<MultipleIds> = self.get_changes(env)?;
        let mut lines = Vec::new();
        for change in changes {
            lines.push(json!({
                "field": change.get_field(env)?,
                "label": change.get_label(env)?,
                "old": change.get_old(env)?,
                "new": change.get_new(env)?,
                "old_value": change.get_old_value(env)?,
                "new_value": change.get_new_value(env)?,
            }));
        }
        Ok(json!({
            "id": self.get_id(),
            "kind": self.get_kind(env)?.key().as_str(),
            "date": self.get_date(env)?.to_rfc3339(),
            "author": author,
            "body": self.get_body(env)?,
            "changes": lines,
        }))
    }
}

/// Note the changes of a record's tracked fields as a message of its thread, by whoever made
/// them. Values are kept as they read now — a label, a record's name — and as they are stored.
pub fn note_changes(
    env: &mut Environment,
    model_name: &str,
    record: u32,
    author: Option<u32>,
    changes: &[TrackedChange],
) -> Result<()> {
    if THREAD_MODELS.contains(&model_name) {
        return Ok(());
    }
    let model = env.model_manager.try_get_model(model_name)?;
    let mut lines = Vec::with_capacity(changes.len());
    for change in changes {
        let field = model.try_get_internal_field(&change.field)?;
        if !field.private {
            lines.push(line(env, field, &change.old, &change.new)?);
        }
    }
    if lines.is_empty() {
        return Ok(());
    }
    post(
        env,
        model_name,
        record,
        author,
        MessageKind::Tracking,
        lines,
    )
}

/// Note the creation of records whose model tracks fields, with what those fields start as: the
/// start of what their thread will note the changes of.
pub fn note_creation(env: &mut Environment, model_name: &str, ids: &[u32]) -> Result<()> {
    if THREAD_MODELS.contains(&model_name) {
        return Ok(());
    }
    let model = env.model_manager.try_get_model(model_name)?;
    let tracked: Vec<&FinalInternalField> = model
        .fields
        .values()
        .filter(|field| field.tracking && !field.private)
        .collect();
    if tracked.is_empty() {
        return Ok(());
    }
    let names: Vec<&str> = tracked.iter().map(|field| field.name.as_str()).collect();
    let author = env.uid();
    let rows = env
        .sudo()
        .read(model_name, &MultipleIds::from(ids.to_vec()), &names)?;
    for row in rows {
        let Some(id) = row.get_option::<&u32>("id").copied() else {
            continue;
        };
        let mut lines = Vec::new();
        for field in &tracked {
            let value = row.fields.get(&field.name).cloned().flatten();
            if stored(&value).is_some() {
                lines.push(line(env, field, &None, &value)?);
            }
        }
        post(env, model_name, id, author, MessageKind::Creation, lines)?;
    }
    Ok(())
}

/// One change as a message keeps it: the field, its label, and its values before and after, as
/// shown and as stored.
struct Line {
    field: String,
    label: String,
    old: (Option<String>, Option<String>),
    new: (Option<String>, Option<String>),
}

fn line(
    env: &mut Environment,
    field: &FinalInternalField,
    old: &Option<FieldType>,
    new: &Option<FieldType>,
) -> Result<Line> {
    Ok(Line {
        field: field.name.clone(),
        label: field.label.clone(),
        old: (shown(env, field, old)?, stored(old)),
        new: (shown(env, field, new)?, stored(new)),
    })
}

/// Add a message to a record's thread, with the changes it notes, as its author: the message is
/// theirs, whoever this unit of work runs as by now. Without rights checked, since whoever may
/// change a record may leave the trace of it.
fn post(
    env: &mut Environment,
    model_name: &str,
    record: u32,
    author: Option<u32>,
    kind: MessageKind,
    lines: Vec<Line>,
) -> Result<()> {
    let mut as_author = match author {
        Some(author) => env.as_user(author),
        None => env.sudo(),
    };
    let env = &mut *as_author.sudo();
    let mut values = MapOfFields::default();
    values.insert("model", model_name.to_string());
    values.insert("record", i32::try_from(record)?);
    values.insert("date", Utc::now());
    values.insert("kind", kind);
    if let Some(author) = author {
        values.insert("author", author);
    }
    let message: Message<SingleId> = env.create_new_record_from_map(values)?;
    for line in lines {
        let mut values = MapOfFields::default();
        values.insert("message", message.get_id());
        values.insert("field", line.field);
        values.insert("label", line.label);
        for (name, value) in [
            ("old", line.old.0),
            ("old_value", line.old.1),
            ("new", line.new.0),
            ("new_value", line.new.1),
        ] {
            if let Some(value) = value {
                values.insert(name, value);
            }
        }
        env.create_records("message_change", vec![values])?;
    }
    Ok(())
}

/// A value as it is stored, as text: a selection's key, a record's id, ids separated by commas.
fn stored(value: &Option<FieldType>) -> Option<String> {
    Some(match value.as_ref()? {
        FieldType::Refs(ids) if ids.is_empty() => return None,
        FieldType::Refs(ids) => ids.iter().map(u32::to_string).collect::<Vec<_>>().join(","),
        FieldType::Password(_) => return None,
        other => other.to_string(),
    })
}

/// A value as a person reads it: a selection's label, the names of the records a relation points
/// to, yes or no.
fn shown(
    env: &mut Environment,
    field: &FinalInternalField,
    value: &Option<FieldType>,
) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    Ok(Some(match value {
        FieldType::String(key) if field.selection.is_some() => env
            .model_manager
            .selections
            .choices(field.selection.map_or("", |family| family.family))
            .iter()
            .find(|choice| &choice.key == key)
            .map_or_else(|| key.clone(), |choice| choice.label.clone()),
        FieldType::Ref(id) => names(env, field, &[*id])?,
        FieldType::Refs(ids) if ids.is_empty() => return Ok(None),
        FieldType::Refs(ids) => names(env, field, ids)?,
        FieldType::Bool(true) => "Yes".to_string(),
        FieldType::Bool(false) => "No".to_string(),
        other => other.to_string(),
    }))
}

/// The names of records a relation points to, in order, separated by commas.
fn names(env: &mut Environment, field: &FinalInternalField, ids: &[u32]) -> Result<String> {
    let target = field
        .inverse
        .as_ref()
        .map(|reference| reference.target_model)
        .ok_or("a relation without a target")?;
    let mut names = env.sudo().names(target, ids)?;
    Ok(ids
        .iter()
        .map(|id| names.remove(id).unwrap_or_else(|| format!("#{id}")))
        .collect::<Vec<_>>()
        .join(", "))
}

/// Delete the thread of records that are gone: nothing else points at it.
pub fn forget_deleted(env: &mut Environment, model_name: &str, ids: &[u32]) -> Result<()> {
    if THREAD_MODELS.contains(&model_name) {
        return Ok(());
    }
    let records: Vec<i32> = ids
        .iter()
        .map(|id| i32::try_from(*id))
        .collect::<std::result::Result<_, _>>()?;
    let env = &mut *env.sudo();
    let messages = env.search_ids(
        "message",
        &make_domain!([
            ("model", "=", model_name.to_string()),
            ("record", "in", records)
        ]),
    )?;
    if messages.is_empty() {
        return Ok(());
    }
    let changes = env.search_ids(
        "message_change",
        &make_domain!([("message", "in", messages.clone())]),
    )?;
    env.delete("message_change", &MultipleIds::from(changes))?;
    env.delete("message", &MultipleIds::from(messages))?;
    Ok(())
}
