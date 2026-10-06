use base::models::BaseUsers;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::model::ModelVerbs;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;

/// A search a user saved, to find it again on the same list: its facets as the client writes
/// them, and whether the list opens with it.
///
/// Nobody reads or writes these records directly — no access rule opens the model — but each
/// user through the methods below, which only ever touch the caller's own.
#[derive(Model)]
#[erp(id = "saved_filter", methods)]
#[allow(dead_code)]
pub struct SavedFilter<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(required, ondelete = "cascade")]
    user: Reference<BaseUsers, SingleId>,
    action: String,
    facets: String,
    #[erp(label = "Opened with", default = false)]
    is_default: bool,
}

#[erp_methods]
impl SavedFilter<MultipleIds> {
    /// The searches the caller saved on a list, by name: `[{id, name, facets, is_default}]`.
    #[erp(rpc)]
    pub fn mine(&self, env: &mut Environment, action: String) -> Result<Value> {
        let _ = self;
        let Some(uid) = env.uid() else {
            return Ok(json!([]));
        };
        let env = &mut *env.sudo();
        let options = SearchOptions {
            order: vec![OrderBy::asc("name")],
            ..SearchOptions::new()
        };
        let filters: SavedFilter<MultipleIds> = env.search_with(
            &make_domain!([("user", "=", uid), ("action", "=", action)]),
            &options,
        )?;
        let mut answer = Vec::new();
        for filter in filters {
            let facets: Value = erp::serde_json::from_str(filter.get_facets(env)?)?;
            answer.push(json!({
                "id": filter.get_id(),
                "name": filter.get_name(env)?,
                "facets": facets,
                "is_default": *filter.get_is_default(env)?,
            }));
        }
        Ok(Value::Array(answer))
    }

    /// Save the search the caller made on a list under a name; opened with by default, the
    /// list no longer opens with any other of theirs. Answers the id of the search saved.
    #[erp(rpc)]
    pub fn save(
        &self,
        env: &mut Environment,
        action: String,
        name: String,
        facets: Value,
        is_default: bool,
    ) -> Result<u32> {
        let _ = self;
        let uid = env.uid().ok_or("Only a user saves searches")?;
        if name.trim().is_empty() {
            return Err("A saved search has a name".into());
        }
        let env = &mut *env.sudo();
        if is_default {
            let others: SavedFilter<MultipleIds> = env.search(&make_domain!([
                ("user", "=", uid),
                ("action", "=", action.clone()),
                ("is_default", "=", true)
            ]))?;
            for other in others {
                other.set_is_default(false, env)?;
            }
        }
        let mut values = MapOfFields::default();
        values.insert("name", name.trim());
        values.insert("user", uid);
        values.insert("action", action);
        values.insert("facets", erp::serde_json::to_string(&facets)?);
        values.insert("is_default", is_default);
        let created = SavedFilter::<MultipleIds>::create(vec![values], env)?;
        Ok(created.get_ids_ref()[0])
    }

    /// Forget a search the caller saved; someone else's is not theirs to forget.
    #[erp(rpc)]
    pub fn forget(&self, env: &mut Environment, id: u32) -> Result<bool> {
        let _ = self;
        let uid = env.uid().ok_or("Only a user forgets searches")?;
        let env = &mut *env.sudo();
        let mine: SavedFilter<MultipleIds> =
            env.search(&make_domain!([("id", "=", id), ("user", "=", uid)]))?;
        if mine.get_ids_ref().is_empty() {
            return Err(format!("Saved search #{id} is not one of yours").into());
        }
        mine.delete(env)?;
        Ok(true)
    }
}
