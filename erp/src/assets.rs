//! The files plugins serve to browsers, and the bundles they are grouped in.
//!
//! A plugin embeds its static directory when it is compiled, TypeScript already turned into
//! JavaScript (`erp_assets_build`). Its files are then reachable as `<plugin>/static/<path>`, and
//! it says which of them — or of other plugins' — go in which bundle: `web.assets_backend` for the
//! back office, another for the site, another for the point of sale. Only installed plugins count,
//! in the order they were installed, so a bundle holds exactly what the installed plugins bring.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::{Arc, Mutex};

/// The files a plugin serves, relative to its static directory: what `erp_assets_build` embeds.
pub type StaticFiles = &'static [(&'static str, &'static [u8])];

/// The JavaScript files a plugin serves, as a single-file bundle holds them, with the specifiers
/// each imports: what `erp_assets_build` embeds as `MODULE_FILES`.
pub type ModuleFiles = &'static [(&'static str, &'static [u8], &'static [&'static str])];

/// A bundle built into one file, and the version its URL carries: a hash of its content, so a
/// browser keeps it for good and fetches it again only once it changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Built {
    pub content: String,
    pub version: String,
}

impl Built {
    fn new(content: String) -> Self {
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        Built {
            content,
            version: format!("{:016x}", hasher.finish()),
        }
    }
}

/// Defines and runs the modules of single-file bundles. Shared by every bundle of a page, so a
/// module two bundles hold runs once.
const MODULE_LOADER: &str = r#"(() => {
const modules = (globalThis.erpModules ??= { defined: new Map(), loaded: new Map() });
const define = (name, imports, body) => {
    if (!modules.defined.has(name)) {
        modules.defined.set(name, { imports, body });
    }
};
const load = (name) => {
    const loaded = modules.loaded.get(name);
    if (loaded !== undefined) {
        return loaded.exports;
    }
    const defined = modules.defined.get(name);
    if (defined === undefined) {
        throw new Error(`No bundle loaded holds the module ${name}`);
    }
    const module = { exports: {} };
    modules.loaded.set(name, module);
    defined.body.call(undefined, (specifier) => load(defined.imports[specifier] ?? specifier), module.exports, module);
    return module.exports;
};
"#;

/// The templates a plugin renders on the server, relative to its templates directory. Never
/// served: `erp_assets_build::templates` embeds them apart from the static files.
pub type TemplateFiles = &'static [(&'static str, &'static [u8])];

/// What a plugin adds to one bundle: globs over public paths, such as `web/static/src/**/*.js`.
///
/// A plugin may name another plugin's files, to put them in a bundle that plugin did not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleContribution {
    pub bundle: &'static str,
    pub globs: Vec<&'static str>,
}

impl BundleContribution {
    pub fn new(bundle: &'static str, globs: &[&'static str]) -> Self {
        BundleContribution {
            bundle,
            globs: globs.to_vec(),
        }
    }
}

struct Contribution {
    plugin: String,
    bundle: &'static str,
    globs: Vec<&'static str>,
}

/// Every file and bundle the installed plugins declared.
///
/// Held by the model manager, cleared before plugin libraries are unloaded: the contents are
/// slices of those libraries.
#[derive(Default)]
pub struct AssetRegistry {
    files: HashMap<String, &'static [u8]>,
    templates: HashMap<String, &'static [u8]>,
    contributions: Vec<Contribution>,
    imports: BTreeMap<&'static str, &'static str>,
    modules: HashMap<String, (&'static [u8], &'static [&'static str])>,
    built: Mutex<HashMap<String, Option<Arc<Built>>>>,
}

impl AssetRegistry {
    /// Record what a plugin being installed serves, and what it adds to bundles.
    pub fn register(
        &mut self,
        plugin: &str,
        files: StaticFiles,
        contributions: Vec<BundleContribution>,
    ) {
        for (path, content) in files {
            self.files
                .insert(format!("{plugin}/static/{path}"), content);
        }
        for contribution in contributions {
            self.contributions.push(Contribution {
                plugin: plugin.to_string(),
                bundle: contribution.bundle,
                globs: contribution.globs,
            });
        }
        self.forget_built();
    }

    /// Record the JavaScript files of a plugin being installed as single-file bundles hold them.
    pub fn register_modules(&mut self, plugin: &str, modules: ModuleFiles) {
        for (path, content, imports) in modules {
            self.modules
                .insert(format!("{plugin}/static/{path}"), (content, imports));
        }
        self.forget_built();
    }

    /// Record names scripts import instead of a path. A later plugin naming the same one wins, so
    /// a plugin can serve its own build of a library another one brings.
    pub fn register_imports(&mut self, imports: Vec<(&'static str, &'static str)>) {
        self.imports.extend(imports);
        self.forget_built();
    }

    fn forget_built(&mut self) {
        self.built
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    /// The import map a page declares before loading any module, so a browser resolves
    /// `import … from "trame"` as the compiled TypeScript still writes it.
    ///
    /// Each plugin serving files is reached as `@<plugin>/`, its `static/src`: `@web/core/orm.js`
    /// is the very URL a relative import of that file reaches, so the same module either way.
    pub fn import_map(&self) -> String {
        let mut imports: BTreeMap<String, String> = self
            .files
            .keys()
            .filter_map(|path| path.split_once("/static/"))
            .map(|(plugin, _)| (format!("@{plugin}/"), format!("/static/{plugin}/src/")))
            .collect();
        imports.extend(self.imports.iter().map(|(name, path)| {
            (
                name.to_string(),
                format!("/static/{}", path.replacen("/static/", "/", 1)),
            )
        }));
        serde_json::json!({ "imports": imports }).to_string()
    }

    /// A file by its public path, `<plugin>/static/<path>`.
    pub fn file(&self, path: &str) -> Option<&'static [u8]> {
        self.files.get(path.trim_start_matches('/')).copied()
    }

    /// Record the templates a plugin being installed renders on the server.
    pub fn register_templates(&mut self, plugin: &str, templates: TemplateFiles) {
        for (path, content) in templates {
            self.templates
                .insert(format!("{plugin}/templates/{path}"), content);
        }
    }

    /// The server templates of a plugin, as `<plugin>/templates/<path>`, in path order.
    pub fn templates_of(&self, plugin: &str) -> Vec<(&str, &'static [u8])> {
        let prefix = format!("{plugin}/templates/");
        let mut templates: Vec<(&str, &'static [u8])> = self
            .templates
            .iter()
            .filter(|(path, _)| path.starts_with(&prefix))
            .map(|(path, content)| (path.as_str(), *content))
            .collect();
        templates.sort_unstable_by_key(|(path, _)| *path);
        templates
    }

    /// The files a plugin serves, by public path, in path order.
    pub fn files_of(&self, plugin: &str) -> Vec<(&str, &'static [u8])> {
        let prefix = format!("{plugin}/static/");
        let mut files: Vec<(&str, &'static [u8])> = self
            .files
            .iter()
            .filter(|(path, _)| path.starts_with(&prefix))
            .map(|(path, content)| (path.as_str(), *content))
            .collect();
        files.sort_unstable_by_key(|(path, _)| *path);
        files
    }

    /// The files of a bundle, in the order a browser must load them.
    ///
    /// Contributions in the order their plugins were installed, so a plugin's files come after
    /// those of the plugins it depends on; within one contribution, by path, so the order never
    /// depends on how the files were listed. A file matched twice keeps its first place.
    ///
    /// A glob matching nothing is logged: it is a mistyped path far more often than an intent.
    pub fn bundle(&self, name: &str) -> Vec<String> {
        let mut paths: Vec<&String> = self.files.keys().collect();
        paths.sort();
        let mut seen = HashSet::new();
        let mut bundle = Vec::new();
        for contribution in self.contributions.iter().filter(|c| c.bundle == name) {
            for glob in &contribution.globs {
                let mut matched = false;
                for path in &paths {
                    if glob_match(glob, path) {
                        matched = true;
                        if seen.insert(path.as_str()) {
                            bundle.push((*path).clone());
                        }
                    }
                }
                if !matched {
                    tracing::warn!(
                        plugin = %contribution.plugin,
                        bundle = %name,
                        glob = %glob,
                        "A glob of a bundle matches no file"
                    );
                }
            }
        }
        bundle
    }

    /// Every bundle some installed plugin contributes to.
    pub fn bundles(&self) -> Vec<&'static str> {
        let mut names: Vec<&'static str> = self.contributions.iter().map(|c| c.bundle).collect();
        names.sort_unstable();
        names.dedup();
        names
    }
}

impl AssetRegistry {
    /// The JavaScript of a bundle as one file: each module it holds, those they import with them,
    /// defined then run in the bundle's order. `None` when it holds no JavaScript.
    ///
    /// Built once until a plugin loads. A module served without its bundled form — a plugin
    /// embedding files without `erp_assets_build` — is imported from its own URL instead.
    pub fn script(&self, bundle: &str) -> Option<Arc<Built>> {
        self.built_once(format!("{bundle}.js"), || {
            let entries = self.bundle_scripts(bundle)?;
            if entries.is_empty() {
                return None;
            }
            let mut content = format!("// Bundle {bundle}\n{MODULE_LOADER}");
            let mut separate = Vec::new();
            let mut queue: VecDeque<String> = entries.iter().cloned().collect();
            let mut seen: HashSet<String> = queue.iter().cloned().collect();
            while let Some(path) = queue.pop_front() {
                let Some((code, imports)) = self.modules.get(&path) else {
                    separate.push(path);
                    continue;
                };
                let mut resolved = serde_json::Map::new();
                for specifier in imports.iter() {
                    let Some(target) = self.resolve(&path, specifier) else {
                        continue;
                    };
                    if seen.insert(target.clone()) {
                        queue.push_back(target.clone());
                    }
                    resolved.insert(specifier.to_string(), target.into());
                }
                content.push_str(&format!(
                    "define({}, {}, function (require, exports, module) {{\n{}\n}});\n",
                    serde_json::Value::from(path.as_str()),
                    serde_json::Value::Object(resolved),
                    String::from_utf8_lossy(code).trim_end()
                ));
            }
            let mut run = String::new();
            for path in &entries {
                if separate.contains(path) {
                    tracing::warn!(bundle = %bundle, module = %path, "A module has no bundled form: imported on its own");
                    run.push_str(&format!("await import({});\n", url_of(path)));
                } else {
                    run.push_str(&format!("load({});\n", serde_json::Value::from(path.as_str())));
                }
            }
            content.push_str(&format!("return (async () => {{\n{run}}})();\n}})();\n"));
            Some(content)
        })
    }

    /// The JavaScript of a bundle as a module importing each of its files from its own URL, in
    /// order: what debugging wants, every file where it was written.
    pub fn script_imports(&self, bundle: &str) -> Option<String> {
        let mut module = format!("// Bundle {bundle}\n");
        for path in self.bundle_scripts(bundle)? {
            module.push_str(&format!("import {};\n", url_of(&path)));
        }
        Some(module)
    }

    /// The CSS files of a bundle as one file, each preceded by its path, so the browser shows
    /// where a rule is from. `None` when it holds none. Built once until a plugin loads.
    pub fn stylesheet(&self, bundle: &str) -> Option<Arc<Built>> {
        self.built_once(format!("{bundle}.css"), || {
            if !self.bundles().contains(&bundle) {
                return None;
            }
            let mut sheet = String::new();
            let paths = self.bundle(bundle);
            if !paths.iter().any(|path| path.ends_with(".css")) {
                return None;
            }
            for path in paths {
                if !path.ends_with(".css") {
                    continue;
                }
                if let Some(content) = self.file(&path) {
                    sheet.push_str(&format!("/* {path} */\n"));
                    sheet.push_str(&String::from_utf8_lossy(content));
                    if !sheet.ends_with('\n') {
                        sheet.push('\n');
                    }
                }
            }
            Some(sheet)
        })
    }

    fn built_once(
        &self,
        key: String,
        build: impl FnOnce() -> Option<String>,
    ) -> Option<Arc<Built>> {
        let mut built = self
            .built
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        built
            .entry(key)
            .or_insert_with(|| build().map(|content| Arc::new(Built::new(content))))
            .clone()
    }

    fn bundle_scripts(&self, bundle: &str) -> Option<Vec<String>> {
        if !self.bundles().contains(&bundle) {
            return None;
        }
        let mut scripts = self.bundle(bundle);
        scripts.retain(|path| path.ends_with(".js"));
        Some(scripts)
    }

    /// The public path of the module `specifier` names, imported from the one at `importer`:
    /// relative, `@<plugin>/` for a plugin's `static/src`, a name of the import map, or a URL
    /// under `/static/`.
    fn resolve(&self, importer: &str, specifier: &str) -> Option<String> {
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let mut segments: Vec<&str> = importer.split('/').collect();
            segments.pop();
            for part in specifier.split('/') {
                match part {
                    "." | "" => {}
                    ".." => {
                        segments.pop()?;
                    }
                    part => segments.push(part),
                }
            }
            return Some(segments.join("/"));
        }
        if let Some(path) = specifier.strip_prefix('@') {
            let (plugin, rest) = path.split_once('/')?;
            return Some(format!("{plugin}/static/src/{rest}"));
        }
        if let Some(path) = specifier.strip_prefix("/static/") {
            let (plugin, rest) = path.split_once('/')?;
            return Some(format!("{plugin}/static/{rest}"));
        }
        self.imports.get(specifier).map(|path| path.to_string())
    }
}

/// The URL a file is served at, as a JavaScript string.
fn url_of(path: &str) -> String {
    let url = match path.split_once("/static/") {
        Some((plugin, rest)) => format!("/static/{plugin}/{rest}"),
        None => format!("/static/{path}"),
    };
    serde_json::Value::from(url).to_string()
}

/// The media type a browser needs to use a file, from its extension.
pub fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Whether a path matches a glob: `?` is one character, `*` any run within one segment, and `**`
/// as a whole segment any number of segments, none included.
pub fn glob_match(glob: &str, path: &str) -> bool {
    let glob: Vec<&str> = glob.trim_start_matches('/').split('/').collect();
    let path: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    match_segments(&glob, &path)
}

fn match_segments(glob: &[&str], path: &[&str]) -> bool {
    match glob.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| match_segments(rest, &path[skip..])),
        Some((segment, rest)) => match path.split_first() {
            Some((part, others)) => {
                let segment: Vec<char> = segment.chars().collect();
                let part: Vec<char> = part.chars().collect();
                match_segment(&segment, &part) && match_segments(rest, others)
            }
            None => false,
        },
    }
}

fn match_segment(glob: &[char], text: &[char]) -> bool {
    match glob.split_first() {
        None => text.is_empty(),
        Some(('*', rest)) => (0..=text.len()).any(|skip| match_segment(rest, &text[skip..])),
        Some(('?', rest)) => !text.is_empty() && match_segment(rest, &text[1..]),
        Some((wanted, rest)) => text.first() == Some(wanted) && match_segment(rest, &text[1..]),
    }
}
