//! A unit of work asking for plugins to be installed: asked once it is committed, never when it
//! is rolled back.

use erp::Result;
use erp::app::Application;
use erp::plugin::take_requested_installs;

#[test]
fn test_installs_are_asked_once_committed() -> Result<()> {
    let app = Application::new_test();
    take_requested_installs();

    let mut env = app.new_env()?;
    env.request_install("contacts");
    env.request_install("contacts");
    assert!(
        take_requested_installs().is_empty(),
        "not before the commit"
    );
    env.close()?;
    assert_eq!(take_requested_installs(), vec!["contacts"]);
    assert!(take_requested_installs().is_empty(), "taken once");

    let mut env = app.new_env()?;
    env.request_install("sales");
    drop(env);
    assert!(take_requested_installs().is_empty(), "rolled back");

    let mut env = app.new_env()?;
    let failed: Result<()> = env.savepoint(|env| {
        env.request_install("stock");
        Err("refused".into())
    });
    assert!(failed.is_err());
    env.close()?;
    assert!(
        take_requested_installs().is_empty(),
        "undone with its savepoint"
    );
    Ok(())
}
