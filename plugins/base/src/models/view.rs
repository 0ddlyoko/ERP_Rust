use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::inheritance::{Arch, Archs};
use erp::search::SearchType;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::xml::{Element, Node, to_markup};
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// How a model's records are shown — a list, a form — described in XML in a plugin's `views/`.
///
/// Its kind is the root element of its final `arch`: `<list>`, `<form>`. A view without `inherit`
/// is a base one, of its `model`. One with `inherit` in `extension` mode changes that view in place
/// with `<xpath>` specifications, lowest `priority` first; one in `primary` mode is a view of its
/// own, its parent's final arch changed by its specifications. Of the primary views of a model and
/// a kind, the one with the lowest `priority` is the one shown.
#[derive(Model)]
#[erp(id = "view", methods)]
#[allow(dead_code)]
pub struct View<Mode: IdMode> {
    pub id: Mode,
    #[erp(description = "Name")]
    #[erp(default = "")]
    name: String,
    #[erp(description = "Model")]
    #[erp(default = "")]
    model: String,
    #[erp(default = "")]
    arch: String,
    #[erp(description = "Inherits")]
    inherit: Reference<BaseView, SingleId>,
    #[erp(default = "extension")]
    mode: String,
    #[erp(description = "Priority")]
    #[erp(default = 16)]
    priority: i32,
}

/// The shared cache of the views shown, by model and kind.
pub const VIEWS_CACHE: &str = "web.views";

/// The kinds of views a model has without anybody declaring one.
const GENERATED_KINDS: &[&str] = &["list", "form"];

/// What a view keeps beside its markup: the model it is of, unless it inherits it.
struct ViewData {
    model: String,
}

/// Every view, to resolve any of them without going back to the database.
struct Views {
    archs: Archs<ViewData>,
}

impl Views {
    /// Read every view, as sudo: views are the client's code, not anybody's data.
    fn load(env: &mut Environment) -> Result<Self> {
        let env = &mut *env.sudo();
        let all: View<MultipleIds> = env.search(&SearchType::Nothing)?;
        let mut rows = Vec::new();
        for view in all {
            let inherit = view
                .get_inherit::<View<SingleId>>(env)?
                .map(|parent| parent.get_id());
            let name = view.get_name(env)?.clone();
            let label = if name.is_empty() {
                format!("View #{}", view.get_id())
            } else {
                format!("View {name}")
            };
            rows.push(Arch {
                id: view.get_id(),
                label,
                arch: view.get_arch(env)?.clone(),
                primary: inherit.is_none() || view.get_mode(env)? == "primary",
                inherit,
                order: (0, String::new(), i64::from(*view.get_priority(env)?)),
                data: ViewData {
                    model: view.get_model(env)?.clone(),
                },
            });
        }
        Ok(Views {
            archs: Archs::new(rows),
        })
    }

    /// The model a view is of: its own, or that of the view it derives from.
    fn model_of<'a>(&'a self, row: &'a Arch<ViewData>) -> &'a str {
        let mut row = row;
        while row.data.model.is_empty() {
            match row.inherit.and_then(|parent| self.archs.get(parent)) {
                Some(parent) => row = parent,
                None => break,
            }
        }
        &row.data.model
    }

    /// Every primary view, resolved, with its model and kind; lowest priority first.
    fn resolved(&self) -> Result<Vec<Resolved>> {
        let mut resolved = Vec::new();
        for row in self.archs.primaries() {
            let nodes = self.archs.resolve(row.id)?;
            let root = root_element(&nodes)
                .ok_or_else(|| format!("{} holds no element to be the view", row.label))?;
            resolved.push(Resolved {
                label: row.label.clone(),
                model: self.model_of(row).to_string(),
                kind: root.name.clone(),
                arch: to_markup(&nodes).trim().to_string(),
                root: root.clone(),
            });
        }
        Ok(resolved)
    }
}

/// A primary view, as it is shown.
struct Resolved {
    label: String,
    model: String,
    kind: String,
    arch: String,
    root: Element,
}

/// The one element of a view's markup, blanks and comments around it aside.
fn root_element(nodes: &[Node]) -> Option<&Element> {
    let mut elements = nodes.iter().filter_map(|node| match node {
        Node::Element(element) => Some(element),
        _ => None,
    });
    let root = elements.next()?;
    elements.next().is_none().then_some(root)
}

/// Every field a view shows, at any depth: the `name` of each `<field>`.
fn fields_of(element: &Element, out: &mut Vec<String>) {
    if element.name == "field"
        && let Some(name) = element.attribute("name")
    {
        out.push(name.to_string());
    }
    for child in &element.children {
        if let Node::Element(child) = child {
            fields_of(child, out);
        }
    }
}

/// A view of every field a model shows, for a model nobody declared one of this kind for.
fn generated(env: &Environment, model: &str, kind: &str) -> Result<String> {
    let model = env.model_manager.try_get_model(model)?;
    let mut names: Vec<&String> = model
        .fields
        .iter()
        .filter(|(_, field)| !field.private)
        .map(|(name, _)| name)
        .collect();
    names.sort();
    let fields: String = names
        .iter()
        .map(|name| format!("<field name=\"{name}\"/>"))
        .collect();
    Ok(format!("<{kind}>{fields}</{kind}>"))
}

#[erp_methods]
impl View<MultipleIds> {
    /// The view shown for a model's records of one kind, as its final XML: the primary one of
    /// lowest priority, or, for a list or a form nobody declared, one of every field.
    ///
    /// Kept in a shared cache until a view changes or a plugin loads. The fields it shows are
    /// described by `fields_get`, which the client keeps per model.
    #[erp(rpc)]
    pub fn load(
        &self,
        env: &mut Environment,
        model: String,
        kind: String,
    ) -> std::result::Result<String, Box<dyn Error + Send + Sync>> {
        let _ = self;
        let shown = env.cached(VIEWS_CACHE, "all", |env| {
            let mut shown: HashMap<(String, String), String> = HashMap::new();
            for view in Views::load(env)?.resolved()? {
                shown.entry((view.model, view.kind)).or_insert(view.arch);
            }
            Ok(shown)
        })?;
        if let Some(arch) = shown.get(&(model.clone(), kind.clone())) {
            return Ok(arch.clone());
        }
        if GENERATED_KINDS.contains(&kind.as_str()) {
            return generated(env, &model, &kind);
        }
        Err(format!("Model \"{model}\" has no {kind} view").into())
    }
}

impl View<SingleId> {
    /// Refuse views that cannot be shown: markup that does not parse, an extension whose path
    /// matches nothing, a view inheriting from itself, or one showing a field its model lacks.
    ///
    /// Run once any plugin has loaded, so a view shipped by a later plugin is checked too. A view
    /// of a model not registered yet is left for when its plugin loads.
    pub fn on_plugin_loaded(env: &mut Environment, _plugin: &str) -> Result<()> {
        for view in Views::load(env)?.resolved()? {
            let Ok(model) = env.model_manager.try_get_model(&view.model) else {
                continue;
            };
            let mut fields = Vec::new();
            fields_of(&view.root, &mut fields);
            for field in fields {
                if field != "id" && !model.fields.contains_key(&field) {
                    return Err(format!(
                        "{} shows field \"{field}\", which model \"{}\" does not have",
                        view.label, view.model
                    )
                    .into());
                }
            }
        }
        Ok(())
    }
}
