use crate::qweb::{Renderer, Values};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::data;
use erp::environment::Environment;
use erp::inheritance::{Arch, Archs, failed};
use erp::search::SearchType;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp::xml::{Element, Node, XmlError, parse_document, to_markup};
use erp_search_code_gen::make_domain;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// A template of the web client's components, read from the `.xml` next to their `.ts`.
///
/// Written as Trame reads them. `<t t-name="…">` is a base template, served under `key` in every
/// bundle holding its `file`. `<t t-name="…" t-inherit="…">` is a `primary` one: its parent's
/// final markup changed by its `<xpath>` specifications, served under its own key wherever its
/// parent is. `<t t-inherit="…">` is an `extension`, changing its parent in place; extensions
/// apply in the order a bundle loads them: by plugin, then file, then `sequence` in the file.
#[derive(Model)]
#[erp(id = "template", methods)]
#[allow(dead_code)]
pub struct Template<Mode: IdMode> {
    pub id: Mode,
    #[erp(index)]
    key: Option<String>,
    arch: String,
    file: Option<String>,
    #[erp(ondelete = "cascade")]
    inherit: Reference<BaseTemplate, SingleId>,
    #[erp(default = "extension")]
    mode: String,
    #[erp(default = 0)]
    sequence: i32,
}

/// The shared cache of the resolved templates, by bundle.
pub const BUNDLES_CACHE: &str = "web.template_bundles";

/// The shared cache of every resolved template, by key, for rendering pages.
pub const RESOLVED_CACHE: &str = "web.resolved_templates";

/// What a template keeps beside its markup.
struct TemplateData {
    key: String,
    file: Option<String>,
}

/// Every template, to resolve any of them without going back to the database.
struct Templates {
    archs: Archs<TemplateData>,
}

impl Templates {
    /// Read every template, as sudo: templates are the client's code, not anybody's data.
    ///
    /// Extensions apply in the order a bundle loads them: by plugin, then file, then place in it.
    fn load(env: &mut Environment) -> Result<Self> {
        let env = &mut *env.sudo();
        let plugins = env.model_manager.loaded_plugins().to_vec();
        let all: Template<MultipleIds> = env.search(&SearchType::Nothing)?;
        let mut rows = Vec::new();
        for template in all {
            let inherit = template
                .get_inherit::<Template<SingleId>>(env)?
                .get_optional_id();
            let file = template.get_file(env)?.cloned();
            let plugin = file
                .as_deref()
                .and_then(|file| file.split_once('/'))
                .and_then(|(plugin, _)| plugins.iter().position(|loaded| loaded == plugin))
                .unwrap_or(usize::MAX);
            let key = template.get_key(env)?.cloned().unwrap_or_default();
            let label = match (&key, &file) {
                (key, _) if !key.is_empty() => format!("Template {key}"),
                (_, Some(file)) => format!("Template extending in {file}"),
                _ => format!("Template #{}", template.get_id()),
            };
            rows.push(Arch {
                id: template.get_id(),
                label,
                arch: template.get_arch(env)?.clone(),
                primary: inherit.is_none() || template.get_mode(env)? == "primary",
                inherit,
                order: (
                    plugin,
                    file.clone().unwrap_or_default(),
                    i64::from(*template.get_sequence(env)?),
                ),
                data: TemplateData { key, file },
            });
        }
        Ok(Templates {
            archs: Archs::new(rows),
        })
    }

    /// The templates served under a key: base ones and primary ones, in the order a bundle loads
    /// them.
    fn served(&self) -> Vec<&Arch<TemplateData>> {
        self.archs.primaries()
    }

    /// The file a template is served from: its own, or that of the base template it derives from.
    fn file_of<'a>(&'a self, row: &'a Arch<TemplateData>) -> Option<&'a str> {
        self.archs.root_of(row)?.data.file.as_deref()
    }

    fn resolve(&self, id: u32) -> Result<Vec<Node>> {
        self.archs.resolve(id)
    }
}

/// A template a plugin ships in one of its static `.xml` files.
struct Shipped {
    name: String,
    key: String,
    arch: String,
    inherit: Option<String>,
    sequence: i32,
}

/// The templates of a static file: the `<t>` of a `<templates>` document.
///
/// A file whose root is anything else is no template file, and yields nothing. A template is
/// named `<plugin>.<Name>`, and `<Name>` is its external identifier, so it reads the same as its
/// key. An extension has no name: it is identified by its file and its place in it.
fn shipped_in(plugin: &str, path: &str, content: &[u8]) -> Result<Vec<Shipped>> {
    let text = std::str::from_utf8(content).map_err(|error| format!("{path}: {error}"))?;
    let root = parse_document(text).map_err(|error| format!("{path}: {error}"))?;
    if root.name != "templates" {
        return Ok(Vec::new());
    }
    let mut shipped = Vec::new();
    for (sequence, element) in (1..).zip(elements_of(path, &root)?) {
        let inherit = element.attribute("t-inherit").map(str::to_string);
        let (name, key) = match element.attribute("t-name") {
            Some(key) => {
                let name = key
                    .strip_prefix(plugin)
                    .and_then(|rest| rest.strip_prefix('.'))
                    .filter(|name| !name.is_empty() && !name.contains('.'))
                    .ok_or_else(|| {
                        format!("{path}: template {key} must be named {plugin}.<Name>")
                    })?;
                (name.to_string(), key.to_string())
            }
            None if inherit.is_some() => {
                let stem: String = path
                    .strip_prefix(&format!("{plugin}/"))
                    .unwrap_or(path)
                    .chars()
                    .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
                    .collect();
                (format!("{stem}_{sequence}"), String::new())
            }
            None => return Err(format!("{path}: a <t> has neither t-name nor t-inherit").into()),
        };
        shipped.push(Shipped {
            name,
            key,
            arch: to_markup(&element.children).trim().to_string(),
            inherit,
            sequence,
        });
    }
    Ok(shipped)
}

/// The `<t>` of a `<templates>` document, refusing anything else but blanks and comments.
fn elements_of<'a>(path: &str, root: &'a Element) -> Result<Vec<&'a Element>> {
    let mut elements = Vec::new();
    for node in &root.children {
        match node {
            Node::Element(element) if element.name == "t" => elements.push(element),
            Node::Element(element) => {
                return Err(format!("{path}: <{}> where a <t> was expected", element.name).into());
            }
            Node::Text(text) if !text.trim().is_empty() => {
                return Err(format!("{path}: text outside of a <t>").into());
            }
            _ => {}
        }
    }
    Ok(elements)
}

#[erp_methods]
impl Template<SingleId> {
    /// Bring the templates of a plugin's static files in line with what it ships, once it loads.
    ///
    /// Every load rather than only when data is loaded: the files are compiled into the plugin,
    /// so a new build brings new ones without a new version. A template no longer shipped goes.
    /// When `web` itself loads, the plugins loaded before it get theirs too.
    pub fn on_plugin_loaded(env: &mut Environment, plugin: String) -> Result<()> {
        let plugins = if plugin == "web" {
            env.model_manager.loaded_plugins().to_vec()
        } else {
            vec![plugin.to_string()]
        };
        for plugin in &plugins {
            Self::sync_files(env, plugin.clone())?;
        }
        Self::check_all(env)
    }

    /// Save every template first and link them after, so a template may inherit from one in a
    /// file that comes later.
    fn sync_files(env: &mut Environment, plugin: String) -> Result<()> {
        let assets = &env.model_manager.assets;
        let static_templates = assets
            .files_of(&plugin)
            .into_iter()
            .filter(|(path, _)| path.ends_with(".xml"));
        let mut shipped = Vec::new();
        for (path, content) in static_templates.chain(assets.templates_of(&plugin)) {
            for template in shipped_in(&plugin, path, content)? {
                shipped.push((path, template));
            }
        }
        let mut names = HashSet::new();
        let mut links = Vec::new();
        for (path, template) in &shipped {
            if !names.insert(template.name.as_str()) {
                return Err(format!("{path}: template {} is defined twice", template.key).into());
            }
            let mut values = MapOfFields::default();
            values.insert("key", template.key.as_str());
            values.insert("arch", template.arch.as_str());
            values.insert("file", *path);
            values.insert("sequence", template.sequence);
            let mode = if template.key.is_empty() {
                "extension"
            } else {
                "primary"
            };
            values.insert("mode", mode);
            let id = data::save_record(env, &plugin, &template.name, "template", values, false)?;
            links.push((id, *path, template.inherit.as_deref()));
        }
        for (id, path, inherit) in links {
            let parent = match inherit {
                Some(key) => Some(Self::served_under(env, key.to_string())?.ok_or_else(|| {
                    format!("{path}: t-inherit names {key}, which no loaded plugin ships")
                })?),
                None => None,
            };
            let parent = parent.map(|parent| Template::<SingleId>::from_id(parent, env));
            Template::<SingleId>::from_id(id, env).set_inherit(parent.as_ref(), env)?;
        }
        for name in data::names_of(env, &plugin, "template")? {
            if !names.contains(name.as_str()) {
                data::delete_record(env, &format!("{plugin}.{name}"))?;
            }
        }
        Ok(())
    }

    /// The template served under a key, the one a `t-inherit` names.
    fn served_under(env: &mut Environment, key: String) -> Result<Option<u32>> {
        let env = &mut *env.sudo();
        let found: Template<MultipleIds> = env.search(&make_domain!([("key", "=", key)]))?;
        Ok(found.get_ids_ref().first().copied())
    }

    /// The templates of a bundle, as the `<templates>` document Trame registers.
    ///
    /// Those served from a file of the bundle, every extension applied. `None` when there is
    /// none. Kept in a shared cache until a template changes or a plugin loads.
    pub fn bundle_markup(env: &mut Environment, bundle: String) -> Result<Arc<Option<String>>> {
        env.cached(BUNDLES_CACHE, &bundle, |env| {
            let files: HashSet<String> = env
                .model_manager
                .assets
                .bundle(&bundle)
                .into_iter()
                .collect();
            let templates = Templates::load(env)?;
            let mut markup = String::new();
            for row in templates.served() {
                if !templates
                    .file_of(row)
                    .is_some_and(|file| files.contains(file))
                {
                    continue;
                }
                let nodes = templates.resolve(row.id)?;
                let name = to_markup(&[Node::Text(row.data.key.clone())]).replace('"', "&quot;");
                markup.push_str(&format!("<t t-name=\"{name}\">{}</t>\n", to_markup(&nodes)));
            }
            if markup.is_empty() {
                return Ok(None);
            }
            Ok(Some(format!("<templates>\n{markup}</templates>\n")))
        })
    }

    /// A page: a template rendered on the server, as an HTML document.
    ///
    /// Only the templates of a `templates/` directory: those of `static/` are the browser's.
    pub fn render_page(env: &mut Environment, key: String, mut values: Values) -> Result<String> {
        let resolved = env.cached(RESOLVED_CACHE, "all", |env| {
            let templates = Templates::load(env)?;
            let mut server = HashMap::new();
            let mut browser = HashSet::new();
            for row in templates.served() {
                if templates.file_of(row).is_some_and(is_server_file) {
                    let nodes = templates.resolve(row.id)?;
                    server.insert(row.data.key.clone(), nodes);
                } else {
                    browser.insert(row.data.key.clone());
                }
            }
            Ok((server, browser))
        })?;
        let (server, browser) = resolved.as_ref();
        let import_map = env.model_manager.assets.import_map();
        let html = Renderer::new(server, browser, &import_map).render(&key, &mut values)?;
        Ok(format!("<!doctype html>\n{html}\n"))
    }

    /// Refuse templates that cannot be served: markup that does not parse, an extension whose path
    /// matches nothing, a template inheriting from itself, two templates under one key, or one
    /// inheriting across the server's `templates/` and the browser's `static/`.
    ///
    /// Run once any plugin has loaded, so an extension shipped by a later plugin is checked too —
    /// and its plugin fails to install rather than the page failing to render.
    fn check_all(env: &mut Environment) -> Result<()> {
        let templates = Templates::load(env)?;
        let mut keys = HashSet::new();
        for row in templates.served() {
            if row.data.key.is_empty() {
                return Err(format!("Template #{} is served but has no key", row.id).into());
            }
            if !keys.insert(row.data.key.as_str()) {
                return Err(
                    format!("Two templates are served under the key {}", row.data.key).into(),
                );
            }
            templates.resolve(row.id)?;
        }
        for row in templates.archs.all().filter(|row| row.inherit.is_some()) {
            let own = row.data.file.as_deref().is_some_and(is_server_file);
            let parent = templates.file_of(row).is_some_and(is_server_file);
            if own != parent {
                let (from, to) = if own {
                    ("the server's", "the browser's")
                } else {
                    ("the browser's", "the server's")
                };
                return Err(failed(
                    row,
                    &XmlError(format!("{from} template inherits from {to}")),
                ));
            }
        }
        Ok(())
    }
}

/// Whether a file is one of the templates the server renders: `<plugin>/templates/…`.
fn is_server_file(file: &str) -> bool {
    file.split('/').nth(1) == Some("templates")
}
