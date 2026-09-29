//! The files plugins serve to browsers, and the bundles that group them.

use erp::app::Application;
use erp::assets::{BundleContribution, content_type, glob_match};
use erp::model::ModelManager;
use erp::plugin::Plugin;
use std::error::Error;
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))?;
    Ok(app)
}

fn text(app: &Application, path: &str) -> Option<String> {
    app.model_manager
        .assets
        .file(path)
        .map(|content| String::from_utf8_lossy(content).into_owned())
}

/// Puts one of `test_plugin`'s files in its own bundle, and in one of `test_plugin`'s.
struct Contributor;

impl Plugin for Contributor {
    fn name(&self) -> String {
        "contributor".to_string()
    }
    fn init_models(&self, _model_manager: &mut ModelManager) {}
    fn get_depends(&self) -> Vec<String> {
        vec!["test_plugin".to_string()]
    }
    fn assets(&self) -> Vec<BundleContribution> {
        vec![
            BundleContribution::new(
                "test.backend",
                &[
                    "test_plugin/static/lib/*.js",
                    "test_plugin/static/src/counter.js",
                ],
            ),
            BundleContribution::new("contributor.pos", &["test_plugin/static/src/util/*.js"]),
        ]
    }
}

// ---- files ----

/// TypeScript is served as the JavaScript it compiled to when the plugin was built.
#[test]
fn test_typescript_is_served_compiled() -> Result<()> {
    let mut app = new_app()?;
    app.load_plugin("test_plugin")?;

    let counter = text(&app, "test_plugin/static/src/counter.js").expect("compiled");
    assert!(counter.contains("export class Counter"), "got {counter}");
    assert!(
        !counter.contains(": number"),
        "types are stripped:\n{counter}"
    );
    assert!(
        !counter.contains("@state"),
        "the decorator is lowered:\n{counter}"
    );
    assert!(
        counter.contains("./util/format"),
        "imports are kept:\n{counter}"
    );

    assert_eq!(
        text(&app, "test_plugin/static/src/counter.ts"),
        None,
        "no source"
    );
    assert_eq!(
        text(&app, "test_plugin/static/src/types.d.ts"),
        None,
        "no declaration"
    );
    assert_eq!(text(&app, "test_plugin/static/src/types.js"), None);
    Ok(())
}

/// Everything else is served as it was written.
#[test]
fn test_other_files_are_served_as_written() -> Result<()> {
    let mut app = new_app()?;
    app.load_plugin("test_plugin")?;
    assert_eq!(
        text(&app, "/test_plugin/static/lib/vendor.js").as_deref(),
        Some("export const vendor = \"as written\";\n")
    );
    assert!(text(&app, "test_plugin/static/css/style.css").is_some());
    assert!(text(&app, "test_plugin/static/src/counter.xml").is_some());
    Ok(())
}

// ---- bundles ----

#[test]
fn test_a_bundle_holds_what_its_globs_match_in_order() -> Result<()> {
    let mut app = new_app()?;
    app.load_plugin("test_plugin")?;
    let assets = &app.model_manager.assets;
    assert_eq!(
        assets.bundle("test.backend"),
        vec![
            "test_plugin/static/src/counter.js",
            "test_plugin/static/src/util/format.js",
            "test_plugin/static/src/counter.xml",
        ],
        "glob after glob, each by path"
    );
    assert_eq!(
        assets.bundle("test.frontend"),
        vec![
            "test_plugin/static/lib/vendor.js",
            "test_plugin/static/css/style.css"
        ],
        "the same plugin feeds another bundle with other files"
    );
    assert_eq!(assets.bundle("nobody.declares.this"), Vec::<String>::new());
    Ok(())
}

/// A plugin can add files to a bundle it did not declare, after the plugins it depends on, and a
/// file already there keeps its place.
#[test]
fn test_a_plugin_adds_to_another_plugins_bundle() -> Result<()> {
    let mut app = new_app()?;
    app.register_plugin(Box::new(Contributor))?;
    app.load_plugin("contributor")?;
    let assets = &app.model_manager.assets;
    assert_eq!(
        assets.bundle("test.backend"),
        vec![
            "test_plugin/static/src/counter.js",
            "test_plugin/static/src/util/format.js",
            "test_plugin/static/src/counter.xml",
            "test_plugin/static/lib/vendor.js",
        ]
    );
    assert_eq!(
        assets.bundle("contributor.pos"),
        vec!["test_plugin/static/src/util/format.js"]
    );
    assert_eq!(
        assets.bundles(),
        vec!["contributor.pos", "test.backend", "test.frontend"]
    );
    Ok(())
}

/// Only installed plugins serve anything.
#[test]
fn test_a_plugin_that_is_not_installed_serves_nothing() -> Result<()> {
    let mut app = new_app()?;
    app.register_plugin(Box::new(Contributor))?;
    app.load_plugin("test_plugin")?;
    let assets = &app.model_manager.assets;
    assert_eq!(assets.bundle("contributor.pos"), Vec::<String>::new());
    assert!(
        !assets
            .bundle("test.backend")
            .contains(&"test_plugin/static/lib/vendor.js".to_string())
    );

    let app = new_app()?;
    assert_eq!(text(&app, "test_plugin/static/lib/vendor.js"), None);
    Ok(())
}

// ---- globs ----

#[test]
fn test_globs() {
    for (glob, path, expected) in [
        ("web/static/src/*.js", "web/static/src/a.js", true),
        ("web/static/src/*.js", "web/static/src/core/a.js", false),
        ("web/static/src/**/*.js", "web/static/src/a.js", true),
        (
            "web/static/src/**/*.js",
            "web/static/src/core/deep/a.js",
            true,
        ),
        ("web/static/src/**/*.js", "web/static/src/a.ts", false),
        ("web/static/**", "web/static/lib/trame.js", true),
        ("web/static/src/?.js", "web/static/src/a.js", true),
        ("web/static/src/?.js", "web/static/src/ab.js", false),
        ("web/static/src/?.js", "web/static/src/é.js", true),
        (
            "web/static/src/core_*.js",
            "web/static/src/core_rpc.js",
            true,
        ),
        ("/web/static/lib/trame.js", "web/static/lib/trame.js", true),
        ("sale/static/**/*.js", "web/static/src/a.js", false),
    ] {
        assert_eq!(glob_match(glob, path), expected, "{glob} against {path}");
    }
}

#[test]
fn test_content_types() {
    assert_eq!(
        content_type("web/static/lib/trame.js"),
        "text/javascript; charset=utf-8"
    );
    assert_eq!(
        content_type("web/static/css/a.css"),
        "text/css; charset=utf-8"
    );
    assert_eq!(
        content_type("web/static/src/a.xml"),
        "application/xml; charset=utf-8"
    );
    assert_eq!(content_type("web/static/img/logo.svg"), "image/svg+xml");
    assert_eq!(
        content_type("web/static/unknown.bin"),
        "application/octet-stream"
    );
}
