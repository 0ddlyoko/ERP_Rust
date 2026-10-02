use crate::models::{Action, BaseAction, BaseGroup, Group, Users};
use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::search::SearchType;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use std::collections::{HashMap, HashSet};
use std::error::Error;

/// An entry of a client's menu: a title, the entries under it, and what choosing it opens.
///
/// Shown only to users in one of its `groups`, or to all of them when it has none. Entries under
/// one parent come by `sequence`, then in the order they were declared.
#[derive(Model)]
#[erp(id = "menu", methods)]
#[allow(dead_code)]
pub struct Menu<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    name: String,
    parent: Reference<BaseMenu, SingleId>,
    #[erp(inverse = "parent")]
    children: Reference<BaseMenu, MultipleIds>,
    #[erp(default = 10)]
    sequence: i32,
    action: Reference<BaseAction, SingleId>,
    #[erp(relation = "menu_group_rel")]
    groups: Reference<BaseGroup, MultipleIds>,
}

/// One menu, the way building the tree needs it.
struct Entry {
    id: u32,
    name: String,
    parent: Option<u32>,
    order: (i32, u32),
    action: Option<u32>,
    groups: Vec<u32>,
}

#[erp_methods]
impl Menu<MultipleIds> {
    /// The menus the caller sees, as a tree: each with its name, its action described the way a
    /// client opens it, and the entries under it.
    ///
    /// Read as sudo — menus are the client's layout, not anybody's data — then filtered by the
    /// caller's groups. An entry with no action and nothing visible under it is left out too: a
    /// title leading nowhere.
    #[erp(rpc)]
    pub fn tree(&self, env: &mut Environment) -> Result<Value, Box<dyn Error + Send + Sync>> {
        let _ = self;
        let member_of: HashSet<u32> = match env.uid() {
            Some(uid) => Users::<SingleId>::from_id(uid, env)
                .get_groups::<Group<MultipleIds>>(&mut env.sudo())?
                .get_ids()
                .into_iter()
                .collect(),
            None => HashSet::new(),
        };
        let env = &mut *env.sudo();
        let all: Menu<MultipleIds> = env.search(&SearchType::Nothing)?;
        let mut entries = Vec::new();
        for menu in all {
            entries.push(Entry {
                id: menu.get_id(),
                name: menu.get_name(env)?.clone(),
                parent: menu
                    .get_parent::<Menu<SingleId>>(env)?
                    .map(|parent| parent.get_id()),
                order: (*menu.get_sequence(env)?, menu.get_id()),
                action: menu
                    .get_action::<Action<SingleId>>(env)?
                    .map(|action| action.get_id()),
                groups: menu.get_groups::<Group<MultipleIds>>(env)?.get_ids(),
            });
        }
        entries.sort_by_key(|entry| entry.order);
        let mut under: HashMap<Option<u32>, Vec<&Entry>> = HashMap::new();
        for entry in &entries {
            under.entry(entry.parent).or_default().push(entry);
        }
        let mut actions = HashMap::new();
        branch(env, None, &under, &member_of, &mut actions)
    }
}

/// The visible entries under one parent, each with its own branch.
fn branch(
    env: &mut Environment,
    parent: Option<u32>,
    under: &HashMap<Option<u32>, Vec<&Entry>>,
    member_of: &HashSet<u32>,
    actions: &mut HashMap<u32, Value>,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    let mut shown = Vec::new();
    for entry in under.get(&parent).map(Vec::as_slice).unwrap_or_default() {
        let visible =
            entry.groups.is_empty() || entry.groups.iter().any(|group| member_of.contains(group));
        if !visible {
            continue;
        }
        let children = branch(env, Some(entry.id), under, member_of, actions)?;
        let action = match entry.action {
            Some(id) => match actions.get(&id) {
                Some(described) => described.clone(),
                None => {
                    let described = Action::<SingleId>::from_id(id, env).describe(env)?;
                    actions.insert(id, described.clone());
                    described
                }
            },
            None => Value::Null,
        };
        let leads_somewhere =
            !action.is_null() || children.as_array().is_some_and(|c| !c.is_empty());
        if leads_somewhere {
            shown.push(json!({
                "id": entry.id,
                "name": entry.name,
                "action": action,
                "children": children,
            }));
        }
    }
    Ok(Value::Array(shown))
}
