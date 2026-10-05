//! What the views offer the plugins: highlighted buttons, methods answering with a record to
//! show, and amounts of money — compiled and served to the browser.

use base::BasePlugin;
use erp::Result;
use erp::app::Application;
use erp::http::{self, Request};
use web::WebPlugin;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.load_plugin("web")?;
    Ok(app)
}

fn script(app: &Application, path: &str) -> String {
    let response = http::handle(app, Request::new("GET", path));
    assert_eq!(response.status(), 200, "{path}");
    response.text_body()
}

/// A button with `highlight="1"` is the form's main action, drawn as the primary button.
#[test]
fn test_a_highlighted_button_is_primary() -> Result<()> {
    let app = new_app()?;
    let compiler = script(&app, "/static/web/src/views/form/form_compiler.js");
    assert!(compiler.contains("\"highlight\""), "{compiler}");
    assert!(compiler.contains("o_button_primary"), "{compiler}");
    Ok(())
}

/// A method answering `{type: "open", action, id}` sends the user to that record, from a form
/// as from a list.
#[test]
fn test_a_method_may_answer_with_a_record_to_open() -> Result<()> {
    let app = new_app()?;
    let view = script(&app, "/static/web/src/views/view.js");
    assert!(view.contains("export function opensRecord"), "{view}");
    assert!(view.contains("\"open\""), "{view}");
    for path in [
        "/static/web/src/views/form/form_view.js",
        "/static/web/src/views/list/list_view.js",
    ] {
        let source = script(&app, path);
        assert!(source.contains("opensRecord(answer)"), "{path}: {source}");
    }
    Ok(())
}

/// Amounts show two decimals; a decimal shows as many as the view's `digits` says.
#[test]
fn test_amounts_of_money_have_their_widget() -> Result<()> {
    let app = new_app()?;
    let decimal = script(&app, "/static/web/src/views/widgets/decimal_widget.js");
    assert!(decimal.contains("widgets.add(\"monetary\""), "{decimal}");
    assert!(decimal.contains("minimumFractionDigits"), "{decimal}");
    assert!(decimal.contains("digits"), "{decimal}");
    Ok(())
}
