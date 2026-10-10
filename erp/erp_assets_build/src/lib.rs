//! Compiling a plugin's browser files into the plugin itself.
//!
//! Called from a plugin's build script. Every file under its static directory ends up embedded in
//! the compiled plugin — TypeScript transpiled to JavaScript on the way — so a plugin is one file
//! carrying everything it serves, and Cargo builds it again whenever one of those files changes.
//!
//! ```ignore
//! // build.rs
//! fn main() {
//!     erp_assets_build::compile("static");
//! }
//!
//! // lib.rs
//! include!(concat!(env!("OUT_DIR"), "/static_files.rs"));
//! ```

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use swc_core::base::Compiler;
use swc_core::base::config::Options;
use swc_core::common::comments::SingleThreadedComments;
use swc_core::common::errors::Handler;
use swc_core::common::{FileName, GLOBALS, Globals, SourceMap};
use swc_core::ecma::ast::{
    CallExpr, Callee, ExportAll, Expr, ImportDecl, Lit, NamedExport, Str, noop_pass,
};
use swc_core::ecma::visit::{VisitMut, VisitMutWith, visit_mut_pass};

/// Embed every file under `static_dir`, relative to the plugin's manifest, into the plugin.
///
/// Writes `static_files.rs` to `OUT_DIR`, defining `STATIC_FILES`: each file's path relative to
/// the static directory, and its content. `MODULE_FILES` holds each JavaScript file once more, in
/// the form a single-file bundle needs ([`bundled_form`]), with the specifiers it imports. A `.ts` file is served as the `.js` it compiles to; a
/// `.d.ts` file only describes types and is left out. Everything else is embedded as it is.
///
/// A relative import written without extension, as TypeScript has it — `./core/session` — is
/// compiled with the extension of the file it names: a browser takes two URLs for two modules,
/// and would otherwise run the same file twice, once under each.
///
/// Panics with the file, the line and what is wrong when a TypeScript file does not compile,
/// which fails the plugin's build with that message.
pub fn compile(static_dir: &str) {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("run by Cargo"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("run by Cargo"));
    let root = manifest.join(static_dir);
    println!("cargo:rerun-if-changed={}", root.display());

    let mut files = Vec::new();
    if root.is_dir() {
        collect(&root, &mut files);
    }
    files.sort();

    let served: HashSet<String> = files
        .iter()
        .map(|source| public_path(&root, source))
        .collect();
    let compiled = out.join("static");
    let bundled = out.join("modules");
    let mut entries = Vec::new();
    let mut modules = Vec::new();
    for source in files {
        let relative = source
            .strip_prefix(&root)
            .expect("collected under the root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative.ends_with(".d.ts") {
            continue;
        }
        let (public, embedded) = match relative.strip_suffix(".ts") {
            Some(stem) => {
                let public = format!("{stem}.js");
                let target = compiled.join(&public);
                let code = fs::read_to_string(&source)
                    .map_err(|error| format!("Cannot read {}: {error}", source.display()))
                    .and_then(|code| transpile_with(&code, &relative, &served))
                    .unwrap_or_else(|error| panic!("{error}"));
                write(&target, &code);
                (public, target)
            }
            None => (relative, source),
        };
        entries.push(format!(
            "    ({:?}, include_bytes!({:?})),\n",
            public,
            embedded.to_string_lossy()
        ));
        if public.ends_with(".js") {
            let target = bundled.join(&public);
            let (code, imports) = fs::read_to_string(&embedded)
                .map_err(|error| format!("Cannot read {}: {error}", embedded.display()))
                .and_then(|code| bundled_form(&code, &public))
                .unwrap_or_else(|error| panic!("{error}"));
            write(&target, &code);
            modules.push(format!(
                "    ({:?}, include_bytes!({:?}), &{:?}),\n",
                public,
                target.to_string_lossy(),
                imports
            ));
        }
    }

    let table = format!(
        "/// Every file this plugin serves, relative to its static directory.\n\
         pub static STATIC_FILES: &[(&str, &[u8])] = &[\n{}];\n\n\
         /// Every JavaScript file this plugin serves, as a bundle holds it, with what it imports.\n\
         pub static MODULE_FILES: &[(&str, &[u8], &[&str])] = &[\n{}];\n",
        entries.concat(),
        modules.concat()
    );
    fs::write(out.join("static_files.rs"), table).expect("the file table can be written");
}

/// Embed the server's templates under `templates_dir`, relative to the plugin's manifest.
///
/// Writes `template_files.rs` to `OUT_DIR`, defining `TEMPLATE_FILES`: each `.xml` file's path
/// relative to that directory, and its content. Kept apart from the static files, so a browser
/// never downloads what only the server renders.
pub fn templates(templates_dir: &str) {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("run by Cargo"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("run by Cargo"));
    let root = manifest.join(templates_dir);
    println!("cargo:rerun-if-changed={}", root.display());

    let mut files = Vec::new();
    if root.is_dir() {
        collect(&root, &mut files);
    }
    files.sort();
    let mut entries = Vec::new();
    for source in files {
        let relative = source
            .strip_prefix(&root)
            .expect("collected under the root")
            .to_string_lossy()
            .replace('\\', "/");
        if relative.ends_with(".xml") {
            entries.push(format!(
                "    ({:?}, include_bytes!({:?})),\n",
                relative,
                source.to_string_lossy()
            ));
        }
    }
    let table = format!(
        "/// Every template this plugin renders on the server, relative to its templates directory.\n\
         pub static TEMPLATE_FILES: &[(&str, &[u8])] = &[\n{}];\n",
        entries.concat()
    );
    fs::write(out.join("template_files.rs"), table).expect("the file table can be written");
}

/// Write a generated file, creating its directory.
fn write(target: &Path, code: &str) {
    fs::create_dir_all(target.parent().expect("a file has a parent"))
        .expect("the output directory can be created");
    fs::write(target, code).expect("the output file can be written");
}

/// The path a file is served at, relative to the static directory: a `.ts` as its `.js`.
fn public_path(root: &Path, source: &Path) -> String {
    let relative = source
        .strip_prefix(root)
        .expect("collected under the root")
        .to_string_lossy()
        .replace('\\', "/");
    match relative.strip_suffix(".ts") {
        Some(stem) if !relative.ends_with(".d.ts") => format!("{stem}.js"),
        _ => relative,
    }
}

/// Every file under `dir`, hidden ones left out.
fn collect(dir: &Path, files: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).unwrap_or_else(|error| {
        panic!("Cannot read {}: {error}", dir.display());
    });
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        let hidden = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if hidden {
            continue;
        }
        if path.is_dir() {
            collect(&path, files);
        } else {
            files.push(path);
        }
    }
}

/// TypeScript in, JavaScript a browser runs out.
///
/// Types are stripped rather than checked, as esbuild does: checking them is the editor's and the
/// CI's job, not the build's. Standard decorators — the ones Trame's `@state accessor` are — are
/// lowered, the target matches Trame's (ES2019), and helpers are written into the file rather than
/// imported from a package no browser can resolve.
pub fn transpile(source: &Path, name: &str) -> Result<String, String> {
    let code = fs::read_to_string(source)
        .map_err(|error| format!("Cannot read {}: {error}", source.display()))?;
    transpile_source(&code, name)
}

/// Same as [`transpile`], from the source text.
pub fn transpile_source(code: &str, name: &str) -> Result<String, String> {
    transpile_with(code, name, &HashSet::new())
}

/// Same, giving relative imports the extension of the file they name among `public`: the paths
/// the plugin serves, relative to its static directory.
pub fn transpile_with(code: &str, name: &str, public: &HashSet<String>) -> Result<String, String> {
    let options = serde_json::json!({
        "jsc": {
            "parser": { "syntax": "typescript", "decorators": true },
            "transform": { "decoratorVersion": "2023-11", "useDefineForClassFields": true },
            "target": "es2019",
            "externalHelpers": false
        },
        "module": { "type": "es6" },
        "sourceMaps": false
    });
    let resolver = WithExtensions {
        importer: name,
        public,
    };
    process(code, name, options, resolver)
}

/// A JavaScript module as a bundle holds it, and the specifiers it imports, as written.
///
/// The code is the body of a function given `require`, `exports` and `module`: its imports become
/// calls to `require`, a dynamic `import()` one made once the promise resolves, and its exports
/// properties of `exports` read through getters, so they stay live bindings. Nothing else is
/// lowered: the module is already what a browser runs. It is minified, its classes and functions
/// keeping their names; the files served apart, for debugging, are not.
pub fn bundled_form(code: &str, name: &str) -> Result<(String, Vec<String>), String> {
    let options = serde_json::json!({
        "jsc": {
            "parser": { "syntax": "ecmascript" },
            "target": "esnext",
            "externalHelpers": false,
            "minify": {
                "compress": true,
                "mangle": { "keep_classnames": true, "keep_fnames": true },
                "keep_classnames": true,
                "keep_fnames": true
            }
        },
        "module": { "type": "commonjs", "ignoreDynamic": false },
        "minify": true,
        "sourceMaps": false
    });
    let imports = Imports::default();
    let code = process(code, name, options, imports.clone())?;
    let mut specifiers = imports.0.lock().expect("not poisoned").clone();
    let mut seen = HashSet::new();
    specifiers.retain(|specifier| seen.insert(specifier.clone()));
    Ok((code, specifiers))
}

/// `code` compiled with swc's `options`, `pass` run over it first.
fn process<P: VisitMut>(
    code: &str,
    name: &str,
    options: serde_json::Value,
    pass: P,
) -> Result<String, String> {
    let map: Arc<SourceMap> = Arc::new(SourceMap::default());
    let file = map.new_source_file(
        Arc::new(FileName::Custom(name.to_string())),
        code.to_string(),
    );
    let diagnostics = Diagnostics::default();
    let handler = Handler::with_emitter_writer(Box::new(diagnostics.clone()), Some(map.clone()));
    let options: Options = serde_json::from_value(options).expect("the options are well formed");
    let compiler = Compiler::new(map);
    GLOBALS
        .set(&Globals::new(), || {
            compiler.process_js_with_custom_pass(
                file,
                None,
                &handler,
                &options,
                SingleThreadedComments::default(),
                |_| visit_mut_pass(pass),
                |_| noop_pass(),
            )
        })
        .map(|output| output.code)
        .map_err(|error| {
            let written = diagnostics.text();
            let detail = if written.trim().is_empty() {
                format!("{error:?}")
            } else {
                written
            };
            format!("Cannot compile {name}:\n{detail}")
        })
}

/// Collects the specifiers a module imports, statically or not.
#[derive(Clone, Default)]
struct Imports(Arc<Mutex<Vec<String>>>);

impl Imports {
    fn add(&self, specifier: &Str) {
        self.0
            .lock()
            .expect("not poisoned")
            .push(specifier.value.to_atom_lossy().to_string());
    }
}

impl VisitMut for Imports {
    fn visit_mut_import_decl(&mut self, import: &mut ImportDecl) {
        if !import.type_only {
            self.add(&import.src);
        }
    }

    fn visit_mut_export_all(&mut self, export: &mut ExportAll) {
        self.add(&export.src);
    }

    fn visit_mut_named_export(&mut self, export: &mut NamedExport) {
        if let Some(src) = &export.src {
            self.add(src);
        }
    }

    fn visit_mut_call_expr(&mut self, call: &mut CallExpr) {
        call.visit_mut_children_with(self);
        if let (Callee::Import(_), Some(argument)) = (&call.callee, call.args.first())
            && let Expr::Lit(Lit::Str(specifier)) = &*argument.expr
        {
            self.add(specifier);
        }
    }
}

/// Gives relative imports the extension of the file they name.
struct WithExtensions<'a> {
    importer: &'a str,
    public: &'a HashSet<String>,
}

impl WithExtensions<'_> {
    fn complete(&self, specifier: &mut Str) {
        let written = specifier.value.to_atom_lossy().to_string();
        if let Some(completed) = complete(self.importer, &written, self.public) {
            *specifier = Str {
                span: specifier.span,
                value: completed.into(),
                raw: None,
            };
        }
    }
}

impl VisitMut for WithExtensions<'_> {
    fn visit_mut_import_decl(&mut self, import: &mut ImportDecl) {
        self.complete(&mut import.src);
    }

    fn visit_mut_export_all(&mut self, export: &mut ExportAll) {
        self.complete(&mut export.src);
    }

    fn visit_mut_named_export(&mut self, export: &mut NamedExport) {
        if let Some(src) = &mut export.src {
            self.complete(src);
        }
    }

    fn visit_mut_call_expr(&mut self, call: &mut CallExpr) {
        call.visit_mut_children_with(self);
        if !matches!(call.callee, Callee::Import(_)) {
            return;
        }
        if let Some(argument) = call.args.first_mut()
            && let Expr::Lit(Lit::Str(specifier)) = &mut *argument.expr
        {
            self.complete(specifier);
        }
    }
}

/// `specifier`, imported from `importer`, with the extension of the served file it names: `None`
/// when it is neither relative nor a plugin's, already names a file, or names none.
///
/// `@web/core/orm` names `core/orm` under the `src` of plugin `web`, which another plugin's files
/// are not here to check: it is given `.js`, as every compiled module has.
pub fn complete(importer: &str, specifier: &str, public: &HashSet<String>) -> Option<String> {
    if let Some(path) = specifier.strip_prefix('@') {
        let (_, rest) = path.split_once('/')?;
        let last = rest.rsplit('/').next().unwrap_or(rest);
        return (!rest.is_empty() && !last.contains('.')).then(|| format!("{specifier}.js"));
    }
    if !(specifier.starts_with("./") || specifier.starts_with("../")) {
        return None;
    }
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
    let target = segments.join("/");
    if public.contains(&target) {
        return None;
    }
    public
        .contains(&format!("{target}.js"))
        .then(|| format!("{specifier}.js"))
}

/// Collects what the compiler reports, to hand it back as the error.
#[derive(Clone, Default)]
struct Diagnostics(Arc<Mutex<Vec<u8>>>);

impl Diagnostics {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().expect("not poisoned")).into_owned()
    }
}

impl Write for Diagnostics {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("not poisoned").extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{bundled_form, complete, transpile_source, transpile_with};
    use std::collections::HashSet;

    #[test]
    fn test_typescript_becomes_javascript_a_browser_runs() {
        let code = transpile_source(
            "import type { T } from './t';\n\
             declare function state(target: unknown, context: ClassAccessorDecoratorContext);\n\
             export class Counter {\n\
                 @state accessor n: number = 0;\n\
                 get double(): number { return this.n * 2; }\n\
             }\n",
            "counter.ts",
        )
        .expect("compiles");
        assert!(!code.contains(": number"), "types are stripped:\n{code}");
        assert!(
            !code.contains("import type"),
            "type-only imports are gone:\n{code}"
        );
        assert!(!code.contains("@state"), "decorators are lowered:\n{code}");
        assert!(
            !code.contains("@swc/helpers"),
            "helpers are written in:\n{code}"
        );
        assert!(code.contains("export class Counter"));
    }

    #[test]
    fn test_a_file_that_does_not_compile_names_where() {
        let error = transpile_source("export class A {\n    x: number = ;\n}\n", "broken.ts")
            .expect_err("refused");
        assert!(error.contains("broken.ts"), "got {error}");
        assert!(error.contains(":2:"), "the line: {error}");
    }

    fn served() -> HashSet<String> {
        [
            "src/main.js",
            "src/core/session.js",
            "src/core/rpc.js",
            "lib/vendor.js",
            "src/data.json",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    }

    #[test]
    fn test_a_relative_import_is_given_the_extension_of_its_file() {
        let served = served();
        let from_main = |specifier: &str| complete("src/main.ts", specifier, &served);
        assert_eq!(
            from_main("./core/session"),
            Some("./core/session.js".to_string())
        );
        assert_eq!(
            from_main("../lib/vendor"),
            Some("../lib/vendor.js".to_string())
        );
        assert_eq!(from_main("./core/session.js"), None, "already the file");
        assert_eq!(from_main("./data.json"), None, "already the file");
        assert_eq!(
            from_main("./core/missing"),
            None,
            "names no file: left for the 404"
        );
        assert_eq!(from_main("trame"), None, "not relative: the import map's");
        assert_eq!(
            from_main("@web/core/orm"),
            Some("@web/core/orm.js".to_string())
        );
        assert_eq!(from_main("@web/core/orm.js"), None, "already the file");
        assert_eq!(from_main("@web"), None, "a plugin, not a file of it");
        assert_eq!(from_main("../../outside"), None, "out of the plugin");
        assert_eq!(
            complete("src/core/rpc.ts", "./session", &served),
            Some("./session.js".to_string())
        );
    }

    /// Every way a module names another is completed, so each file is one module.
    #[test]
    fn test_imports_exports_and_dynamic_imports_are_completed() {
        let code = transpile_with(
            "import { Session } from './core/session';\n\
             import './core/rpc';\n\
             export { Rpc } from './core/rpc';\n\
             export * from './core/session';\n\
             import { mount } from 'trame';\n\
             export const later = () => import('./core/rpc');\n\
             export const used = [Session, mount];\n",
            "src/main.ts",
            &served(),
        )
        .expect("compiles");
        for completed in [
            "from \"./core/session.js\"",
            "import \"./core/rpc.js\"",
            "export { Rpc } from \"./core/rpc.js\"",
            "export * from \"./core/session.js\"",
            "import(\"./core/rpc.js\")",
            "from 'trame'",
        ] {
            assert!(code.contains(completed), "{completed} in:\n{code}");
        }
        assert!(
            !code.contains("'./core/session'") && !code.contains("\"./core/session\""),
            "{code}"
        );
    }

    /// A module becomes a function body requiring what it imported, its exports kept live, and is
    /// minified.
    #[test]
    fn test_a_module_is_given_its_bundled_form() {
        let (code, imports) = bundled_form(
            "import { mount } from 'trame';\n\
             import './core/rpc.js';\n\
             export { Rpc } from './core/rpc.js';\n\
             export let count = 0;\n\
             export function bump() { count += 1; mount(); }\n\
             export default class Main { #hidden = 1; }\n\
             export const later = () => import('./lazy.js');\n",
            "src/main.js",
        )
        .expect("compiles");
        assert_eq!(
            imports,
            vec!["trame", "./core/rpc.js", "./lazy.js"],
            "{code}"
        );
        assert!(code.contains("require(\"trame\")"), "{code}");
        assert!(code.contains("require(\"./lazy.js\")"), "{code}");
        assert!(!code.contains("import "), "{code}");
        assert!(!code.contains("export "), "{code}");
        assert!(code.contains("class Main{#"), "nothing lowered: {code}");
        assert!(
            !code.contains("let count") && code.contains("get count()"),
            "minified, its exports named as they are: {code}"
        );
    }
}
