//! Every model a plugin declares can be extended by any other plugin: its struct, the `Base…`
//! type its derive makes — what an extension and a reference name it by — and its selections are
//! re-exported by the plugin's `models` module.
//!
//! Read from the sources rather than compiled, because a type the derive makes is public whether
//! or not anything outside its module reaches it, and no lint sees the difference.

use std::fs;
use std::path::{Path, PathBuf};

/// A type a model file declares, which its plugin's `models` must re-export.
struct Declared {
    file: PathBuf,
    name: String,
}

/// The Rust files under a directory, at any depth.
fn sources(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("a readable directory").flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// `sale_order` as the derive names its base: `BaseSaleOrder`.
fn base_name(id: &str) -> String {
    let words: String = id
        .split('_')
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                .unwrap_or_default()
        })
        .collect();
    format!("Base{words}")
}

/// The name a `pub struct Name<…>` or `pub enum Name` line declares.
fn declared_name(line: &str, keyword: &str) -> Option<String> {
    let rest = line.trim().strip_prefix(keyword)?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// What a file declares as models and selections: each model struct, the base of a model it
/// declares rather than extends, and each `#[selection]` enum.
fn declared_in(file: &Path) -> Vec<Declared> {
    let text = fs::read_to_string(file).expect("a readable source");
    let lines: Vec<&str> = text.lines().collect();
    let mut declared = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at].trim();
        let model = line == "#[derive(Model)]";
        let selection = line == "#[selection]";
        if !model && !selection {
            at += 1;
            continue;
        }
        let mut attributes = String::new();
        at += 1;
        while at < lines.len() && lines[at].trim().starts_with("#[") {
            attributes.push_str(lines[at]);
            at += 1;
        }
        let Some(line) = lines.get(at) else { break };
        let keyword = if model { "pub struct " } else { "pub enum " };
        if let Some(name) = declared_name(line, keyword) {
            declared.push(Declared {
                file: file.to_path_buf(),
                name,
            });
            let id = attributes
                .split("id = \"")
                .nth(1)
                .and_then(|rest| rest.split('"').next());
            if model
                && !attributes.contains("derived_model")
                && let Some(id) = id
            {
                declared.push(Declared {
                    file: file.to_path_buf(),
                    name: base_name(id),
                });
            }
        }
    }
    declared
}

/// Whether a word stands in a text on its own, not inside a longer name.
fn mentions(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        let apart = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '_');
        apart(before) && apart(after)
    })
}

#[test]
fn test_every_model_of_every_plugin_is_exported_with_its_base() {
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugins");
    let mut missing = Vec::new();
    for plugin in fs::read_dir(&plugins).expect("the plugins").flatten() {
        let source = plugin.path().join("src");
        let models = [source.join("models.rs"), source.join("models/mod.rs")]
            .into_iter()
            .find(|path| path.exists());
        let mut files = Vec::new();
        if source.is_dir() {
            sources(&source, &mut files);
        }
        let declared: Vec<Declared> = files.iter().flat_map(|file| declared_in(file)).collect();
        if declared.is_empty() {
            continue;
        }
        let exports = models
            .map(|path| fs::read_to_string(path).expect("a readable models module"))
            .unwrap_or_default();
        for item in declared {
            if !mentions(&exports, &item.name) {
                missing.push(format!(
                    "{} (declared in {})",
                    item.name,
                    item.file
                        .strip_prefix(&plugins)
                        .unwrap_or(&item.file)
                        .display()
                ));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "Not re-exported by their plugin's models module, so no other plugin can extend them:\n  {}",
        missing.join("\n  ")
    );
}
