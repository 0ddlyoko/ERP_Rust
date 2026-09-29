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

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use swc_core::base::Compiler;
use swc_core::base::config::Options;
use swc_core::common::errors::Handler;
use swc_core::common::{FileName, GLOBALS, Globals, SourceMap};

/// Embed every file under `static_dir`, relative to the plugin's manifest, into the plugin.
///
/// Writes `static_files.rs` to `OUT_DIR`, defining `STATIC_FILES`: each file's path relative to
/// the static directory, and its content. A `.ts` file is served as the `.js` it compiles to; a
/// `.d.ts` file only describes types and is left out. Everything else is embedded as it is.
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

    let compiled = out.join("static");
    let mut entries = Vec::new();
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
                fs::create_dir_all(target.parent().expect("a file has a parent"))
                    .expect("the output directory can be created");
                let code = transpile(&source, &relative).unwrap_or_else(|error| panic!("{error}"));
                fs::write(&target, code).expect("the output file can be written");
                (public, target)
            }
            None => (relative, source),
        };
        entries.push(format!(
            "    ({:?}, include_bytes!({:?})),\n",
            public,
            embedded.to_string_lossy()
        ));
    }

    let table = format!(
        "/// Every file this plugin serves, relative to its static directory.\n\
         pub static STATIC_FILES: &[(&str, &[u8])] = &[\n{}];\n",
        entries.concat()
    );
    fs::write(out.join("static_files.rs"), table).expect("the file table can be written");
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
    let map: Arc<SourceMap> = Arc::new(SourceMap::default());
    let file = map.new_source_file(
        Arc::new(FileName::Custom(name.to_string())),
        code.to_string(),
    );
    let diagnostics = Diagnostics::default();
    let handler = Handler::with_emitter_writer(Box::new(diagnostics.clone()), Some(map.clone()));
    let options: Options = serde_json::from_value(serde_json::json!({
        "jsc": {
            "parser": { "syntax": "typescript", "decorators": true },
            "transform": { "decoratorVersion": "2023-11", "useDefineForClassFields": true },
            "target": "es2019",
            "externalHelpers": false
        },
        "module": { "type": "es6" },
        "sourceMaps": false
    }))
    .expect("the options are well formed");

    let compiler = Compiler::new(map);
    GLOBALS
        .set(&Globals::new(), || {
            compiler.process_js_file(file, &handler, &options)
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
    use super::transpile_source;

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
}
