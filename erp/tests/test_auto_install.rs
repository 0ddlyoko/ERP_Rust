//! Plugins that install themselves once every plugin they depend on is installed.

use base::BasePlugin;
use erp::app::Application;
use erp::model::ModelManager;
use erp::plugin::Plugin;
use erp::util::dependency::CircularDependencyError;
use erp_search_code_gen::make_domain;
use erp_types::field::MultipleIds;
use std::error::Error;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// A plugin that declares nothing but its name, what it depends on, and whether it installs
/// itself.
struct Stub {
    name: &'static str,
    depends: &'static [&'static str],
    auto_install: bool,
}

impl Plugin for Stub {
    fn name(&self) -> String {
        self.name.to_string()
    }
    fn init_models(&self, _model_manager: &mut ModelManager) {}
    fn get_depends(&self) -> Vec<String> {
        self.depends.iter().map(|name| name.to_string()).collect()
    }
    fn auto_install(&self) -> bool {
        self.auto_install
    }
}

fn stub(name: &'static str, depends: &'static [&'static str], auto_install: bool) -> Box<Stub> {
    Box::new(Stub {
        name,
        depends,
        auto_install,
    })
}

/// `glue` needs `test_lib_plugin`, and `glue_extra` needs `glue`: both install themselves.
/// `bridge` also needs `sales`, which nobody installed, and `sales` is an ordinary plugin.
fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(stub("glue", &["test_lib_plugin"], true))?;
    app.register_plugin(stub("glue_extra", &["glue"], true))?;
    app.register_plugin(stub("sales", &["base"], false))?;
    app.register_plugin(stub("bridge", &["test_lib_plugin", "sales"], true))?;
    Ok(app)
}

fn installed(app: &Application, names: &[&str]) -> Vec<bool> {
    names
        .iter()
        .map(|name| app.plugin_manager.is_installed(name))
        .collect()
}

#[test]
fn test_a_plugin_installs_itself_once_its_dependencies_are() -> Result<()> {
    let mut app = new_app()?;
    app.load_plugin("base")?;
    assert_eq!(installed(&app, &["glue", "glue_extra"]), [false, false]);

    app.load_plugin("test_lib_plugin")?;
    assert_eq!(
        installed(&app, &["glue", "glue_extra"]),
        [true, true],
        "and one auto-installed plugin can complete the dependencies of another"
    );
    Ok(())
}

/// Every dependency, not some: `bridge` waits for `sales`, and `sales` never installs itself.
#[test]
fn test_every_dependency_has_to_be_installed() -> Result<()> {
    let mut app = new_app()?;
    app.load_plugin("test_lib_plugin")?;
    assert_eq!(installed(&app, &["sales", "bridge"]), [false, false]);

    app.load_plugin("sales")?;
    assert_eq!(installed(&app, &["sales", "bridge"]), [true, true]);
    Ok(())
}

#[test]
fn test_what_installs_itself_is_reported_in_order() -> Result<()> {
    let mut app = new_app()?;
    app.load_plugin("base")?;
    app.load_plugin("sales")?;
    assert_eq!(
        app.auto_install_plugins()?,
        Vec::<String>::new(),
        "nothing is ready yet"
    );

    // Installed by hand without going through `load_plugin`, so the loop has something to do.
    let mut app = new_app()?;
    app.load_plugin("sales")?;
    app.register_plugin(stub("later", &["sales"], true))?;
    assert_eq!(app.auto_install_plugins()?, vec!["later".to_string()]);
    Ok(())
}

/// Installed on its own is installed: the row says so, so a restart loads it again.
#[test]
fn test_an_auto_installed_plugin_is_recorded_as_installed() -> Result<()> {
    let mut app = new_app()?;
    app.load_plugin("base")?;
    app.load_plugin("test_lib_plugin")?;
    let mut env = app.new_env_as_option(None)?;
    let ids = env.search_ids(
        "plugin",
        &make_domain!([("name", "=", "glue_extra"), ("state", "=", "installed")]),
    )?;
    assert_eq!(
        env.read("plugin", &MultipleIds::from(ids), &["name"])?
            .len(),
        1
    );
    Ok(())
}

/// With nothing to wait for, a plugin marked auto-install installs itself at the first chance.
#[test]
fn test_a_plugin_depending_on_nothing_installs_right_away() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(stub("standalone", &[], true))?;
    app.load_plugin("base")?;
    assert!(app.plugin_manager.is_installed("standalone"));
    Ok(())
}

/// Plugins depending on each other would recurse until the stack overflows; the load is refused,
/// naming the cycle, and the process survives it.
#[test]
fn test_a_dependency_cycle_is_refused() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(stub("left", &["right"], false))?;
    app.register_plugin(stub("right", &["middle"], false))?;
    app.register_plugin(stub("middle", &["left"], false))?;
    let error = app.load_plugin("left").unwrap_err();
    assert!(error.is::<CircularDependencyError>(), "got {error}");
    assert!(
        error
            .to_string()
            .contains("left -> right -> middle -> left"),
        "got {error}"
    );
    Ok(())
}

/// Auto-install never loops: plugins waiting on each other are never ready, so nothing installs.
#[test]
fn test_auto_installed_plugins_waiting_on_each_other_never_install() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(stub("ping", &["pong"], true))?;
    app.register_plugin(stub("pong", &["ping"], true))?;
    app.load_plugin("base")?;
    assert_eq!(installed(&app, &["ping", "pong"]), [false, false]);
    Ok(())
}
