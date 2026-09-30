//! The web plugin: it installs itself, sends the root to the web client, and serves what the
//! installed plugins bring.

use base::BasePlugin;
use erp::app::Application;
use erp::http::{self, Request, Response};
use std::error::Error;
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;
use web::WebPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// `base`, which brings `web` along on its own, and `test_plugin`, which serves files.
fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))?;
    app.load_plugin("base")?;
    app.load_plugin("test_plugin")?;
    Ok(app)
}

fn get(app: &Application, target: &str) -> Response {
    http::handle(app, Request::new("GET", target))
}

#[test]
fn test_web_installs_itself_with_base() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.load_plugin("base")?;
    assert!(app.plugin_manager.is_installed("web"));
    Ok(())
}

#[test]
fn test_the_root_sends_the_browser_to_the_web_client() -> Result<()> {
    let app = new_app()?;
    let response = get(&app, "/");
    assert_eq!(response.status(), 303);
    assert_eq!(response.header("location"), Some("/web"));

    let response = get(&app, "/web");
    assert_eq!(response.status(), 200);
    assert!(
        response
            .header("content-type")
            .is_some_and(|kind| kind.starts_with("text/html"))
    );
    Ok(())
}

// ---- static files ----

#[test]
fn test_a_plugin_file_is_served_under_static() -> Result<()> {
    let app = new_app()?;
    let response = get(&app, "/static/test_plugin/src/counter.js");
    assert_eq!(response.status(), 200, "got {}", response.text_body());
    assert_eq!(
        response.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert!(response.text_body().contains("export class Counter"));

    let response = get(&app, "/static/test_plugin/css/style.css");
    assert_eq!(
        response.header("content-type"),
        Some("text/css; charset=utf-8")
    );
    Ok(())
}

/// A TypeScript import has no extension, and the browser asks for exactly what it reads.
#[test]
fn test_an_import_without_extension_finds_its_javascript() -> Result<()> {
    let app = new_app()?;
    let response = get(&app, "/static/test_plugin/src/util/format");
    assert_eq!(response.status(), 200);
    assert!(response.text_body().contains("function twice"));
    Ok(())
}

#[test]
fn test_what_no_installed_plugin_serves_is_a_404() -> Result<()> {
    let app = new_app()?;
    for target in [
        "/static/test_plugin/src/counter.ts",
        "/static/test_plugin/src/types.d.ts",
        "/static/test_plugin/nowhere.js",
        "/static/nobody/src/counter.js",
        "/static/test_plugin/src/../../base/static/x.js",
        "/static/test_plugin",
    ] {
        assert_eq!(get(&app, target).status(), 404, "{target}");
    }

    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))?;
    app.load_plugin("base")?;
    assert_eq!(
        get(&app, "/static/test_plugin/src/counter.js").status(),
        404,
        "test_plugin is registered, not installed"
    );
    Ok(())
}

/// The browser keeps what it was sent, and is told when it still holds the current version.
#[test]
fn test_files_are_validated_with_their_tag() -> Result<()> {
    let app = new_app()?;
    let first = get(&app, "/static/test_plugin/lib/vendor.js");
    let tag = first.header("etag").expect("tagged").to_string();

    let again = http::handle(
        &app,
        Request::new("GET", "/static/test_plugin/lib/vendor.js").with_header("If-None-Match", &tag),
    );
    assert_eq!(again.status(), 304);
    assert!(again.body().is_empty());

    let stale = http::handle(
        &app,
        Request::new("GET", "/static/test_plugin/lib/vendor.js")
            .with_header("If-None-Match", "\"something else\""),
    );
    assert_eq!(stale.status(), 200);
    Ok(())
}

// ---- bundles ----

/// A bundle is a module importing its JavaScript files in order; its templates and styles are not
/// scripts, so they are left out of it.
#[test]
fn test_a_bundle_imports_its_scripts_in_order() -> Result<()> {
    let app = new_app()?;
    let response = get(&app, "/web/assets/test.backend.js");
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    let imports: Vec<String> = response
        .text_body()
        .lines()
        .filter(|line| line.starts_with("import"))
        .map(str::to_string)
        .collect();
    assert_eq!(
        imports,
        vec![
            "import \"/static/test_plugin/src/counter.js\";",
            "import \"/static/test_plugin/src/util/format.js\";",
        ]
    );
    for import in &imports {
        let url = import
            .trim_start_matches("import \"")
            .trim_end_matches("\";");
        assert_eq!(get(&app, url).status(), 200, "{url} is served");
    }
    Ok(())
}

#[test]
fn test_an_unknown_bundle_is_a_404() -> Result<()> {
    let app = new_app()?;
    assert_eq!(get(&app, "/web/assets/nobody.contributes.js").status(), 404);
    assert_eq!(
        get(&app, "/web/assets/test.backend").status(),
        404,
        "no extension"
    );
    assert_eq!(
        get(&app, "/web/assets/test.backend.css").status(),
        404,
        "not yet"
    );
    Ok(())
}

/// The compiled library is found under the symbol its file name gives, `erp_create_plugin_web`,
/// and only once its build of `erp` is the one of the application loading it.
///
/// Whether the library on disk comes from this test's build depends on the Cargo command that
/// produced it, so both outcomes are accepted — but never a third: a library from another build is
/// refused before any of its code runs, rather than failing later or aborting the process.
#[test]
fn test_the_library_exports_the_plugin_under_its_name() -> Result<()> {
    let directory = std::env::current_exe()?
        .parent()
        .and_then(|deps| deps.parent())
        .map(std::path::Path::to_path_buf)
        .expect("tests run from target/<profile>/deps");
    let library = directory.join(format!(
        "{}web{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    ));
    assert_eq!(
        erp::plugin::plugin_symbol(&library),
        "erp_create_plugin_web"
    );
    assert_eq!(
        erp::plugin::plugin_build_symbol(&library),
        "erp_plugin_build_web"
    );

    let mut manager = erp::plugin::PluginManager::default();
    match manager.register_plugin_from_file(&library) {
        Ok(()) => {
            let again = manager.register_plugin(Box::new(WebPlugin {}));
            assert!(
                again.is_err_and(|error| error.to_string().contains("\"web\"")),
                "the library registered the plugin named web"
            );
        }
        Err(error) => assert!(
            error.is::<erp::plugin::errors::PluginBuildMismatchError>(),
            "anything but a build mismatch is a real failure: {error}"
        ),
    }
    Ok(())
}

/// Start the application the way the server does, on this database: from the plugin directory,
/// then whatever the database says is installed, then what installs itself.
fn boot(database: &erp::database::cache::CacheDatabase) -> Result<Application> {
    let directory = std::env::temp_dir().join(format!("erp_web_boot_{}", std::process::id()));
    std::fs::create_dir_all(&directory)?;
    let mut app = Application::new_test();
    app.set_config(erp::config::Config {
        plugin_path: directory.to_string_lossy().into_owned(),
        ..erp::config::Config::default()
    });
    app.cache_db = database.clone();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.load()?;
    Ok(app)
}

/// The second boot loads again what the first one installed, `web` depending on `base` included.
#[test]
fn test_a_restart_loads_again_what_installed_itself() -> Result<()> {
    let database = erp::database::cache::CacheDatabase::default();
    let first = boot(&database)?;
    assert!(first.plugin_manager.is_installed("web"));
    drop(first);

    let second = boot(&database)?;
    assert!(second.plugin_manager.is_installed("web"));
    assert_eq!(get(&second, "/").header("location"), Some("/web"));
    Ok(())
}
