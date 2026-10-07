use crate::models::ModelData;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::inheritance::{Arch as Markup, Archs};
use erp::internal_types::FinalInternalModel;
use erp::search::SearchType;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::xml::{Element, Node, to_markup};
use std::collections::{HashMap, HashSet};

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
    name: Option<String>,
    #[erp(index)]
    model: Option<String>,
    arch: String,
    #[erp(ondelete = "cascade")]
    inherit: Reference<BaseView, SingleId>,
    #[erp(default = "extension")]
    mode: String,
    #[erp(default = 16)]
    priority: i32,
}

/// The shared cache of the views shown, by model and kind.
pub const VIEWS_CACHE: &str = "web.views";

/// The kinds of views a model has without anybody declaring one.
const GENERATED_KINDS: &[&str] = &["list", "form", "search"];

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
        Self::load_without(env, &HashSet::new())
    }

    /// Same, leaving out the views of these ids.
    fn load_without(env: &mut Environment, left_out: &HashSet<u32>) -> Result<Self> {
        let env = &mut *env.sudo();
        let all: View<MultipleIds> = env.search(&SearchType::Nothing)?;
        let mut rows = Vec::new();
        for view in all {
            if left_out.contains(&view.get_id()) {
                continue;
            }
            let inherit = view.get_inherit::<View<SingleId>>(env)?.get_optional_id();
            let label = match view.get_name(env)? {
                Some(name) => format!("View {name}"),
                None => format!("View #{}", view.get_id()),
            };
            rows.push(Markup {
                id: view.get_id(),
                label,
                arch: view.get_arch(env)?.clone(),
                primary: inherit.is_none() || view.get_mode(env)? == "primary",
                inherit,
                order: (0, String::new(), i64::from(*view.get_priority(env)?)),
                data: ViewData {
                    model: view.get_model(env)?.cloned().unwrap_or_default(),
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
    "block", "field", "h1", "h2", "h3", "h4", "h5", "h6", "pages", "totals",
];
const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];
/// What lays out a record as a card — the list's `<compact>`, `<folded>` and `<preview>`.
const CARD: &[&str] = &[
    "row", "column", "title", "subtitle", "figure", "muted", "spacer", "field",
];
/// What in a card may hold text between its fields: `<muted><field name="name"/> · …</muted>`.
const CARD_TEXT: &[&str] = &["title", "subtitle", "figure", "muted"];
/// What a field of a form's `<leader>` may stand for; one with none is a tile.
const LEADER_ROLES: &[&str] = &["status", "avatar", "title", "subtitle", "figure", "note"];
/// The colours a `decoration-*` attribute may name.
const DECORATIONS: &[&str] = &["success", "info", "warning", "danger", "muted"];

impl Arch<'_> {
    fn check(&self, root: &Element) -> std::result::Result<(), String> {
        if root.name == "list" {
            self.decorations(root)?;
        }
        if root.name == "kanban"
            && let Some(field) = root.attribute("default_group_by")
        {
            self.known(field)?;
        }
        let allowed: &[&str] = match root.name.as_str() {
            "list" => &["field", "buttons", "compact", "folded", "preview"],
            "kanban" => &["field"],
            "search" => &["field", "filter"],
            "form" => &[
                "block", "field", "h1", "h2", "h3", "h4", "h5", "h6", "pages", "buttons", "side",
                "chatter", "totals", "leader", "related",
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
                    if !text.trim().is_empty()
                        && !HEADINGS.contains(&parent.name.as_str())
                        && !CARD_TEXT.contains(&parent.name.as_str()) =>
                {
                    return Err(self.error(format!(
                        "<{}> holds text, which only a heading or a card's text may",
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
        let conditions: &[&str] = match element.name.as_str() {
            "field" => &["invisible", "readonly", "required"],
            "block" | "page" | "pages" | "button" | "totals" | "related" | "link" | "row"
            | "column" | "title" | "subtitle" | "figure" | "muted" => &["invisible"],
            heading if HEADINGS.contains(&heading) => &["invisible"],
            _ => &[],
        };
        for (attribute, expression) in &element.attributes {
            if conditions.contains(&attribute.as_str()) {
                self.condition(attribute, expression)?;
            } else if ["invisible", "readonly", "required"].contains(&attribute.as_str()) {
                return Err(self.error(format!("<{}> cannot be {attribute}", element.name)));
            }
        }
        if let Some(domain) = element.attribute("domain") {
            self.domain(domain)?;
        }
        if element.name == "field" {
            self.decorations(element)?;
        }
        if let Some(nolabel) = element.attribute("nolabel")
            && !matches!(nolabel, "0" | "1")
        {
            return Err(self.error(format!("nolabel is \"{nolabel}\": 0 or 1")));
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
            "leader" => {
                for field in element.children.iter().filter_map(as_element) {
                    if let Some(role) = field.attribute("role")
                        && !LEADER_ROLES.contains(&role)
                    {
                        return Err(self.error(format!(
                            "role \"{role}\" is none a leader knows: {}",
                            LEADER_ROLES.join(", ")
                        )));
                    }
                }
                self.children(element, &["actions", "field"])
            }
            "related" => self.children(element, &["link"]),
            "compact" | "folded" | "preview" | "row" | "column" | "title" | "subtitle"
            | "figure" | "muted" => self.children(element, CARD),
            "spacer" => self.children(element, &[]),
            "link" => {
                self.required(element, "action")?;
                self.field(element)?;
                self.children(element, &[])
            }
            "buttons" | "actions" => {
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
            "totals" => self.children(element, &["field"]),
            "filter" => {
                self.required(element, "name")?;
                match element.attribute("group_by") {
                    Some(group_by) => {
                        let field = group_by
                            .split_once(':')
                            .map_or(group_by, |(field, _)| field);
                        self.known(field)?;
                    }
                    None => {
                        self.required(element, "domain")?;
                    }
                }
                self.children(element, &[])
            }
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

    /// `decoration-success="state === 'done'"` and its kin: a colour the client knows, and a
    /// condition reading fields of the model.
    fn decorations(&self, element: &Element) -> std::result::Result<(), String> {
        for (attribute, expression) in &element.attributes {
            let Some(kind) = attribute.strip_prefix("decoration-") else {
                continue;
            };
            if !DECORATIONS.contains(&kind) {
                return Err(self.error(format!(
                    "{attribute} is no decoration: {}",
                    DECORATIONS.join(", ")
                )));
            }
            self.condition(attribute, expression)?;
        }
        Ok(())
    }

    /// A condition the client evaluates as written: every name it reads is a field of the model.
    fn condition(&self, attribute: &str, expression: &str) -> std::result::Result<(), String> {
        if expression.trim().is_empty() {
            return Err(self.error(format!("{attribute} is empty")));
        }
        for name in names_read(expression) {
            if name != "id" && !self.model.fields.contains_key(&name) {
                return Err(self.error(format!(
                    "{attribute}=\"{expression}\" reads \"{name}\", which model \"{}\" does not have",
                    self.model_name
                )));
            }
        }
        Ok(())
    }

    /// A domain as JSON, the form a caller sends: every path starts with a field of the model.
    fn domain(&self, domain: &str) -> std::result::Result<(), String> {
        let parsed: SearchType = erp::serde_json::from_str(domain)
            .map_err(|error| self.error(format!("domain {domain} is not one: {error}")))?;
        let mut paths = Vec::new();
        collect_paths(&parsed, &mut paths);
        for path in paths {
            if let Some(first) = path.first() {
                self.known(first)?;
            }
        }
        Ok(())
    }

    fn error(&self, message: String) -> String {
        format!("{}: {message}", self.label)
    }
}

/// Names an expression may read without being fields: the language's own, and a few globals.
const EXPRESSION_WORDS: &[&str] = &[
    "true",
    "false",
    "null",
    "undefined",
    "typeof",
    "instanceof",
    "in",
    "of",
    "new",
    "void",
    "NaN",
    "Infinity",
    "Math",
    "Number",
    "String",
    "Boolean",
    "Array",
    "Date",
    "JSON",
    "Object",
];

/// The names an expression reads: what is left once strings, numbers, properties (`.includes`),
/// the language's words and the parameters of arrow functions (`x => x.id`) are set aside.
pub fn names_read(expression: &str) -> Vec<String> {
    let chars: Vec<char> = expression.chars().collect();
    let mut tokens: Vec<(String, bool)> = Vec::new();
    let mut index = 0;
    let mut after_dot = false;
    while index < chars.len() {
        let c = chars[index];
        if c == '\'' || c == '"' || c == '`' {
            index += 1;
            while index < chars.len() && chars[index] != c {
                index += if chars[index] == '\\' { 2 } else { 1 };
            }
            index += 1;
            after_dot = false;
        } else if c.is_ascii_digit() {
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '.')
            {
                index += 1;
            }
            after_dot = false;
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let start = index;
            while index < chars.len()
                && (chars[index].is_alphanumeric() || chars[index] == '_' || chars[index] == '$')
            {
                index += 1;
            }
            tokens.push((chars[start..index].iter().collect(), after_dot));
            after_dot = false;
        } else {
            after_dot = c == '.';
            index += 1;
        }
    }
    let parameters = arrow_parameters(expression);
    let mut names: Vec<String> = Vec::new();
    for (name, is_property) in tokens {
        if !is_property
            && !EXPRESSION_WORDS.contains(&name.as_str())
            && !parameters.contains(&name)
            && !names.contains(&name)
        {
            names.push(name);
        }
    }
    names
}

/// The parameters of the arrow functions of an expression: `x` of `x => …`, `a, b` of `(a, b) => …`.
fn arrow_parameters(expression: &str) -> Vec<String> {
    let mut parameters = Vec::new();
    let mut rest = expression;
    while let Some(arrow) = rest.find("=>") {
        let before = rest[..arrow].trim_end();
        let declared = match before.strip_suffix(')') {
            Some(inside) => inside.rsplit_once('(').map_or("", |(_, list)| list),
            None => before
                .rsplit(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$'))
                .next()
                .unwrap_or(""),
        };
        parameters.extend(
            declared
                .split(',')
                .map(|parameter| parameter.trim().to_string())
                .filter(|parameter| !parameter.is_empty()),
        );
        rest = &rest[arrow + 2..];
    }
    parameters
}

fn collect_paths<'d>(domain: &'d SearchType, paths: &mut Vec<&'d [String]>) {
    match domain {
        SearchType::And(left, right) | SearchType::Or(left, right) => {
            collect_paths(left, paths);
            collect_paths(right, paths);
        }
        SearchType::Tuple(tuple) => paths.push(&tuple.left.path),
        SearchType::Nothing | SearchType::Never => {}
    }
}

fn as_element(node: &Node) -> Option<&Element> {
    match node {
        Node::Element(element) => Some(element),
        _ => None,
    }
}

/// A view of every field a model shows, for a model nobody declared one of this kind for —
/// those the ORM fills in left out; a search by its name only.
fn generated(env: &Environment, model: &str, kind: &str) -> Result<String> {
    let model = env.model_manager.try_get_model(model)?;
    if kind == "search" {
        let fields = model
            .name_field()
            .map(|name| format!("<field name=\"{name}\"/>"))
            .unwrap_or_default();
        return Ok(format!("<search>{fields}</search>"));
    }
    let mut names: Vec<&String> = model
        .fields
        .iter()
        .filter(|(_, field)| !field.private && !field.automatic)
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
    /// lowest priority, or, for a list or a form nobody declared, one of every field — for a
    /// search, of its name.
    ///
    /// Kept in a shared cache until a view changes or a plugin loads. The fields it shows are
    /// described by `fields_get`, which the client keeps per model.
    #[erp(rpc)]
    pub fn load(&self, env: &mut Environment, model: String, kind: String) -> erp::Result<String> {
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

#[erp_methods]
impl View<SingleId> {
    /// Refuse views that cannot be shown: markup that does not parse, an extension whose path
    /// matches nothing, a view inheriting from itself, an element where it may not stand, or a
    /// field — shown, or named in a label's `{{ field }}` — its model lacks.
    ///
    /// Run once any plugin has loaded, so a view shipped by a later plugin is checked too. A view
    /// of a model not registered yet is left for when its plugin loads, and so is a view of an
    /// installed plugin that has not loaded yet: it may show a field that plugin adds.
    pub fn on_plugin_loaded(env: &mut Environment, _plugin: String) -> Result<()> {
        let not_loaded = ModelData::of_plugins_not_loaded(env, "view".to_string())?;
        for view in Views::load_without(env, &not_loaded)?.resolved()? {
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
