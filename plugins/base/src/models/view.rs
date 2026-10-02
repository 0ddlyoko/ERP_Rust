use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::inheritance::{Arch as Markup, Archs};
use erp::internal_types::FinalInternalModel;
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
    #[erp(default = "")]
    name: String,
    #[erp(default = "")]
    model: String,
    #[erp(default = "")]
    arch: String,
    inherit: Reference<BaseView, SingleId>,
    #[erp(default = "extension")]
    mode: String,
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
            rows.push(Markup {
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
    fn model_of<'a>(&'a self, row: &'a Markup<ViewData>) -> &'a str {
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

/// What may stand where in a view, and what a `<field>` or a `{{ field }}` may name.
struct Arch<'a> {
    label: &'a str,
    model_name: &'a str,
    model: &'a FinalInternalModel,
}

/// Elements a block or a page holds: its contents, laid out on two columns.
const CONTENTS: &[&str] = &[
    "block", "field", "h1", "h2", "h3", "h4", "h5", "h6", "pages",
];
const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

impl Arch<'_> {
    fn check(&self, root: &Element) -> std::result::Result<(), String> {
        let allowed: &[&str] = match root.name.as_str() {
            "list" => &["field"],
            "form" => &[
                "block", "field", "h1", "h2", "h3", "h4", "h5", "h6", "pages", "buttons", "side",
                "chatter",
            ],
            _ => return Ok(()),
        };
        self.children(root, allowed)
    }

    fn children(&self, parent: &Element, allowed: &[&str]) -> std::result::Result<(), String> {
        for node in &parent.children {
            match node {
                Node::Element(child) if allowed.contains(&child.name.as_str()) => {
                    self.element(child)?
                }
                Node::Element(child) => {
                    return Err(self.error(format!(
                        "<{}> cannot stand in <{}>: {} may",
                        child.name,
                        parent.name,
                        allowed.join(", ")
                    )));
                }
                Node::Text(text)
                    if !text.trim().is_empty() && !HEADINGS.contains(&parent.name.as_str()) =>
                {
                    return Err(self.error(format!(
                        "<{}> holds text, which only a heading may",
                        parent.name
                    )));
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn element(&self, element: &Element) -> std::result::Result<(), String> {
        if let Some(string) = element.attribute("string") {
            self.placeholders(string)?;
        }
        match element.name.as_str() {
            "field" => self.field(element),
            "block" | "page" => self.children(element, CONTENTS),
            "side" => self.children(element, &["block", "pages", "chatter"]),
            "pages" => {
                for page in element.children.iter().filter_map(as_element) {
                    if page.name != "page" {
                        return Err(
                            self.error(format!("<pages> holds <{}>: only <page> may", page.name))
                        );
                    }
                    self.required(page, "name")?;
                }
                self.children(element, &["page"])
            }
            "buttons" => {
                for button in element.children.iter().filter_map(as_element) {
                    self.required(button, "name")?;
                    let kind = button.attribute("type").unwrap_or_default();
                    if kind != "method" && kind != "action" {
                        return Err(self.error(format!(
                            "button \"{}\" has type \"{kind}\": method or action",
                            button.attribute("name").unwrap_or_default()
                        )));
                    }
                }
                self.children(element, &["button"])
            }
            "chatter" => self.children(element, &[]),
            heading if HEADINGS.contains(&heading) => self.children(element, &["field"]),
            _ => Ok(()),
        }
    }

    /// A field the model has; what it holds — a view of its own, later — is not checked here.
    fn field(&self, element: &Element) -> std::result::Result<(), String> {
        let name = self.required(element, "name")?;
        self.known(name)
    }

    fn known(&self, name: &str) -> std::result::Result<(), String> {
        if name == "id" || self.model.fields.contains_key(name) {
            return Ok(());
        }
        Err(self.error(format!(
            "shows field \"{name}\", which model \"{}\" does not have",
            self.model_name
        )))
    }

    fn required<'e>(
        &self,
        element: &'e Element,
        attribute: &str,
    ) -> std::result::Result<&'e str, String> {
        element
            .attribute(attribute)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| self.error(format!("<{}> has no {attribute}", element.name)))
    }

    /// Every `{{ field }}` of a label names a field of the model; `\{{` is a brace as written.
    fn placeholders(&self, string: &str) -> std::result::Result<(), String> {
        let mut rest = string;
        while let Some(start) = rest.find("{{") {
            if rest[..start].ends_with('\\') {
                rest = &rest[start + 2..];
                continue;
            }
            let after = &rest[start + 2..];
            let end = after
                .find("}}")
                .ok_or_else(|| self.error(format!("\"{string}\" opens {{{{ without closing it")))?;
            let name = after[..end].trim();
            let is_name =
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
            if !is_name {
                return Err(self.error(format!(
                    "\"{string}\" holds {{{{ {name} }}}}: only a field's name may stand there"
                )));
            }
            self.known(name)?;
            rest = &after[end + 2..];
        }
        Ok(())
    }

    fn error(&self, message: String) -> String {
        format!("{}: {message}", self.label)
    }
}

fn as_element(node: &Node) -> Option<&Element> {
    match node {
        Node::Element(element) => Some(element),
        _ => None,
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
    /// matches nothing, a view inheriting from itself, an element where it may not stand, or a
    /// field — shown, or named in a label's `{{ field }}` — its model lacks.
    ///
    /// Run once any plugin has loaded, so a view shipped by a later plugin is checked too. A view
    /// of a model not registered yet is left for when its plugin loads.
    pub fn on_plugin_loaded(env: &mut Environment, _plugin: &str) -> Result<()> {
        for view in Views::load(env)?.resolved()? {
            let Ok(model) = env.model_manager.try_get_model(&view.model) else {
                continue;
            };
            let arch = Arch {
                label: &view.label,
                model_name: &view.model,
                model,
            };
            arch.check(&view.root)?;
        }
        Ok(())
    }
}
