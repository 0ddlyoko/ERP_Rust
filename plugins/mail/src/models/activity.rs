use crate::models::follower::{contact_of, follow};
use crate::models::{Message, MessageKind};
use base::models::{BaseUsers, Users};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::access::Operation;
use erp::environment::Environment;
use erp::search::SearchType;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds, NaiveDate, Reference, SingleId, Utc};
use erp::types::model::MapOfFields;
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;

/// A kind of thing to do about a record — a call, a meeting — with the icon it is shown with and
/// how many days ahead one is planned by default. Declared in data, as subtypes are.
#[derive(Model)]
#[erp(id = "activity_type", order = "sequence, id")]
#[allow(dead_code)]
pub struct ActivityType<Mode: IdMode> {
    pub id: Mode,
    name: String,
    icon: Option<String>,
    #[erp(default = 0)]
    delay: i32,
    #[erp(default = 10)]
    sequence: i32,
}

/// Something someone has to do about a record by a day: call the customer, send the quote.
///
/// Tied to the record by its model and id, as its thread is. Done, it becomes a note of the
/// thread saying so, and is gone.
#[derive(Model)]
#[erp(id = "activity", order = "deadline, id", methods)]
#[allow(dead_code)]
pub struct Activity<Mode: IdMode> {
    pub id: Mode,
    #[erp(index)]
    model: String,
    #[erp(default = 0, index)]
    record: i32,
    #[erp(label = "Type", ondelete = "restrict")]
    kind: Reference<BaseActivityType, SingleId>,
    summary: Option<String>,
    note: Option<String>,
    #[erp(required, ondelete = "cascade", index)]
    assignee: Reference<BaseUsers, SingleId>,
    #[erp(index)]
    deadline: NaiveDate,
}

#[erp_methods]
impl Activity<MultipleIds> {
    /// The kinds of activities that may be planned.
    #[erp(rpc)]
    pub fn kinds(&self, env: &mut Environment) -> Result<Value> {
        let _ = self;
        let env = &mut *env.sudo();
        let kinds: ActivityType<MultipleIds> = env.search(&SearchType::Nothing)?;
        let mut found = Vec::new();
        for kind in kinds {
            found.push(json!({
                "id": kind.get_id(),
                "name": kind.get_name(env)?,
                "icon": kind.get_icon(env)?,
                "delay": kind.get_delay(env)?,
            }));
        }
        Ok(Value::Array(found))
    }

    /// What is planned about a record, soonest first, for whoever may read it.
    #[erp(rpc)]
    pub fn of(&self, env: &mut Environment, model: String, record: u32) -> Result<Value> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let env = &mut *env.sudo();
        let planned: Activity<MultipleIds> = env.search(&make_domain!([
            ("model", "=", model),
            ("record", "=", i32::try_from(record)?)
        ]))?;
        describe(env, planned)
    }

    /// What the user has to do, soonest first, with the record each is about.
    #[erp(rpc)]
    pub fn mine(&self, env: &mut Environment, limit: Option<usize>) -> Result<Value> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Ok(json!([]));
        };
        let env = &mut *env.sudo();
        let options = SearchOptions::new()
            .order_by(OrderBy::asc("deadline"))
            .with_limit(limit.unwrap_or(30));
        let planned: Activity<MultipleIds> =
            env.search_with(&make_domain!([("assignee", "=", uid)]), &options)?;
        describe(env, planned)
    }

    /// How many of the user's activities are due today or late.
    #[erp(rpc)]
    pub fn due(&self, env: &mut Environment) -> Result<u64> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Ok(0);
        };
        let today = Utc::now().date_naive();
        let domain = make_domain!([("assignee", "=", uid), ("deadline", "<=", today)]);
        Ok(env.sudo().count("activity", &domain)? as u64)
    }

    /// Plan something to do about a record the user may read, for someone — the user by default.
    #[erp(rpc)]
    #[allow(clippy::too_many_arguments)]
    pub fn schedule(
        &self,
        env: &mut Environment,
        model: String,
        record: u32,
        kind: u32,
        summary: Option<String>,
        note: Option<String>,
        assignee: Option<u32>,
        deadline: NaiveDate,
    ) -> Result<u32> {
        let _ = self;
        env.check_access(&model, Operation::Read, &[record], &[])?;
        let assignee = assignee.or(env.uid()).ok_or("Someone has to do it")?;
        let env = &mut *env.sudo();
        let mut values = MapOfFields::default();
        values.insert("model", model.clone());
        values.insert("record", i32::try_from(record)?);
        values.insert("kind", kind);
        values.insert("assignee", assignee);
        values.insert("deadline", deadline);
        if let Some(summary) = summary.filter(|text| !text.trim().is_empty()) {
            values.insert("summary", summary);
        }
        if let Some(note) = note.filter(|text| !text.trim().is_empty()) {
            values.insert("note", note);
        }
        let created: Activity<MultipleIds> = env.create_new_records_from_maps(vec![values])?;
        if let Some(contact) = contact_of(env, assignee)? {
            follow(env, &model, record, &[contact])?;
        }
        Ok(created.get_ids_ref()[0])
    }

    /// Mark an activity done: a note of its record's thread says so, with what came of it, and the
    /// activity is gone.
    #[erp(rpc)]
    pub fn done(
        &self,
        env: &mut Environment,
        activity: u32,
        feedback: Option<String>,
    ) -> Result<()> {
        let _ = self;
        let planned = Self::readable(env, activity)?;
        let author = env.uid();
        let env = &mut *env.sudo();
        let kind: ActivityType<SingleId> = planned.get_kind(env)?;
        let mut body = format!("{} done", kind.get_name(env)?);
        if let Some(summary) = planned.get_summary(env)?.cloned() {
            body.push_str(&format!(": {summary}"));
        }
        if let Some(feedback) = feedback.filter(|text| !text.trim().is_empty()) {
            body.push_str(&format!("\n{feedback}"));
        }
        let mut values = MapOfFields::default();
        values.insert("model", planned.get_model(env)?.clone());
        values.insert("record", *planned.get_record(env)?);
        values.insert("date", Utc::now());
        values.insert("kind", MessageKind::Note);
        values.insert("body", body);
        if let Some(author) = author {
            values.insert("author", author);
        }
        let _: Message<MultipleIds> = env.create_new_records_from_maps(vec![values])?;
        planned.delete(env)?;
        Ok(())
    }

    /// Drop an activity without doing it.
    #[erp(rpc)]
    pub fn cancel(&self, env: &mut Environment, activity: u32) -> Result<()> {
        let _ = self;
        let planned = Self::readable(env, activity)?;
        planned.delete(&mut *env.sudo())?;
        Ok(())
    }
}

impl Activity<MultipleIds> {
    /// An activity of a record the user may read.
    fn readable(env: &mut Environment, activity: u32) -> Result<Activity<SingleId>> {
        let found: Activity<SingleId> = env.get_record(SingleId::from(activity));
        let (model, record) = {
            let env = &mut *env.sudo();
            env.existing("activity", vec![activity])?;
            (
                found.get_model(env)?.clone(),
                u32::try_from(*found.get_record(env)?)?,
            )
        };
        env.check_access(&model, Operation::Read, &[record], &[])?;
        Ok(found)
    }
}

/// Activities as a client shows them: what, about which record, for whom, and by when.
fn describe(env: &mut Environment, planned: Activity<MultipleIds>) -> Result<Value> {
    let mut found = Vec::new();
    for activity in planned {
        let kind: ActivityType<SingleId> = activity.get_kind(env)?;
        let assignee: Users<SingleId> = activity.get_assignee(env)?;
        let model = activity.get_model(env)?.clone();
        let record = u32::try_from(*activity.get_record(env)?)?;
        let name = env.names(&model, &[record])?.remove(&record);
        found.push(json!({
            "id": activity.get_id(),
            "kind": {"id": kind.get_id(), "name": kind.get_name(env)?, "icon": kind.get_icon(env)?},
            "summary": activity.get_summary(env)?,
            "note": activity.get_note(env)?,
            "assignee": [assignee.get_id(), assignee.get_name(env)?],
            "deadline": activity.get_deadline(env)?.to_string(),
            "model": model,
            "record": record,
            "record_name": name,
        }));
    }
    Ok(Value::Array(found))
}
