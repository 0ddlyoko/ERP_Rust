use crate::models::{Action, BaseAction, BaseGroup, Group, Plugin};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::data;
use erp::environment::Environment;
use erp::search::SearchType;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp_search_code_gen::make_domain;
use std::collections::{HashMap, HashSet};

/// An entry of a client's menu: a title, the entries under it, and what choosing it opens.
///
/// Shown only to users in one of its `groups`, or to all of them when it has none. Entries under
/// one parent come by `sequence`, then in the order they were declared.
#[derive(Model)]
#[erp(id = "menu", methods)]
#[allow(dead_code)]
pub struct Menu<Mode: IdMode> {
    pub id: Mode,
    name: String,
    #[erp(label = "Full name", compute = "compute_complete_name", depends = ["name", "parent"])]
    complete_name: String,
    #[erp(ondelete = "cascade")]
    parent: Reference<BaseMenu, SingleId>,
    #[erp(inverse = "parent")]
    children: Reference<BaseMenu, MultipleIds>,
    #[erp(default = 10)]
    sequence: i32,
    action: Reference<BaseAction, SingleId>,
    icon: Option<String>,
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
    icon: Option<String>,
    groups: Vec<u32>,
}

#[erp_methods]
impl Menu<MultipleIds> {
    /// Its name after those of the entries above it: `Settings / Technical / Menus`.
    pub fn compute_complete_name(&self, env: &mut Environment) -> Result<()> {
        for menu in self {
            let mut names = vec![menu.get_name(env)?.clone()];
            let mut seen = HashSet::from([menu.get_id()]);
            let mut parent: Menu<SingleId> = menu.get_parent(env)?;
            while !parent.is_empty() && seen.insert(parent.get_id()) {
                names.push(parent.get_name(env)?.clone());
                parent = parent.get_parent(env)?;
            }
            names.reverse();
            menu.set_complete_name(names.join(" / "), env)?;
        }
        Ok(())
    }

    /// The menus the caller sees, as a tree: each with its name, its icon, its action described
    /// the way a client opens it, and the entries under it. A module — an entry at the top — also
    /// has the colour of the plugin declaring it.
    ///
    /// Read as sudo — menus are the client's layout, not anybody's data — then filtered by the
    /// caller's groups. An entry with no action and nothing visible under it is left out too: a
    /// title leading nowhere.
    #[erp(rpc)]
    pub fn tree(&self, env: &mut Environment) -> Result<Value> {
        let _ = self;
        let member_of: HashSet<u32> = match env.uid() {
            Some(uid) => env.groups_of(uid)?.into_iter().collect(),
            None => HashSet::new(),
        };
        let env = &mut *env.sudo();
        let all: Menu<MultipleIds> = env.search(&SearchType::Nothing)?;
        let mut entries = Vec::new();
        for menu in all {
            entries.push(Entry {
                id: menu.get_id(),
                name: menu.get_name(env)?.clone(),
                parent: menu.get_parent::<Menu<SingleId>>(env)?.get_optional_id(),
                order: (*menu.get_sequence(env)?, menu.get_id()),
                action: menu.get_action::<Action<SingleId>>(env)?.get_optional_id(),
                icon: menu.get_icon(env)?.cloned(),
                groups: menu.get_groups::<Group<MultipleIds>>(env)?.get_ids(),
            });
        }
        entries.sort_by_key(|entry| entry.order);
        let mut under: HashMap<Option<u32>, Vec<&Entry>> = HashMap::new();
        for entry in &entries {
            under.entry(entry.parent).or_default().push(entry);
        }
        let mut action_ids: Vec<u32> = entries.iter().filter_map(|entry| entry.action).collect();
        action_ids.sort_unstable();
        action_ids.dedup();
        let actions = Action::<MultipleIds>::from_ids(action_ids, env).describe_each(env)?;
        let modules: Vec<u32> = entries
            .iter()
            .filter(|entry| entry.parent.is_none())
            .map(|entry| entry.id)
            .collect();
        let colors = module_colors(env, &modules)?;
        Ok(branch(None, &under, &member_of, &actions, &colors))
    }
}

/// The colour of each module, by its menu: that of the plugin whose data declares it.
fn module_colors(env: &mut Environment, modules: &[u32]) -> Result<HashMap<u32, String>> {
    let declared_by: HashMap<u32, String> = data::external_ids_of(env, "menu", modules)?
        .into_iter()
        .filter_map(|(id, xml_id)| Some((id, xml_id.split_once('.')?.0.to_string())))
        .collect();
    let names: Vec<String> = declared_by.values().cloned().collect();
    let plugins: Plugin<MultipleIds> = env.search(&make_domain!([("name", "in", names)]))?;
    let mut color_of = HashMap::new();
    for plugin in plugins {
        if let Some(color) = plugin.get_color(env)?.cloned() {
            color_of.insert(plugin.get_name(env)?.clone(), color);
        }
    }
    Ok(declared_by
        .into_iter()
        .filter_map(|(id, plugin)| Some((id, color_of.get(&plugin)?.clone())))
        .collect())
}

/// The visible entries under one parent, each with its own branch.
fn branch(
    parent: Option<u32>,
    under: &HashMap<Option<u32>, Vec<&Entry>>,
    member_of: &HashSet<u32>,
    actions: &HashMap<u32, Value>,
    colors: &HashMap<u32, String>,
) -> Value {
    let mut shown = Vec::new();
    for entry in under.get(&parent).map(Vec::as_slice).unwrap_or_default() {
        let visible =
            entry.groups.is_empty() || entry.groups.iter().any(|group| member_of.contains(group));
        if !visible {
            continue;
        }
        let children = branch(Some(entry.id), under, member_of, actions, colors);
        let action = entry
            .action
            .and_then(|id| actions.get(&id).cloned())
            .unwrap_or(Value::Null);
        let leads_somewhere =
            !action.is_null() || children.as_array().is_some_and(|c| !c.is_empty());
        if leads_somewhere {
            shown.push(json!({
                "id": entry.id,
                "name": entry.name,
                "icon": entry.icon,
                "color": colors.get(&entry.id),
                "action": action,
                "children": children,
            }));
        }
    }
    Value::Array(shown)
}
