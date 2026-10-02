//! Views: how a model's records are shown, described in XML, extended by other plugins.

use base::BasePlugin;
use base::models::View;
use erp::app::Application;
use erp::model::ModelManager;
use erp::plugin::Plugin;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
    Ok(app)
}

/// A plugin shipping data files only, views among them.
struct DataPlugin {
    name: &'static str,
    data: &'static [&'static str],
}

impl Plugin for DataPlugin {
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string()]
    }

    fn data(&self) -> Vec<&'static str> {
        self.data.to_vec()
    }
}

fn with_data(name: &'static str, data: &'static [&'static str]) -> Result<Application> {
    let mut app = new_app()?;
    app.register_plugin(Box::new(DataPlugin { name, data }))?;
    app.load_plugin(name)?;
    Ok(app)
}

fn view_of(app: &Application, model: &str, kind: &str) -> Result<String> {
    let mut env = app.new_env()?;
    env.get_empty_record::<View<_>>()
        .load(&mut env, model.to_string(), kind.to_string())
}

#[test]
fn test_the_users_list_is_the_one_base_ships() -> Result<()> {
    let app = new_app()?;
    assert_eq!(
        view_of(&app, "users", "list")?
            .split_whitespace()
            .collect::<String>(),
        "<list><fieldname=\"name\"/><fieldname=\"login\"/><fieldname=\"active\"/>\
         <fieldname=\"groups\"/></list>"
    );
    Ok(())
}

const VIEW_CHANGES: &[&str] = &[r#"<erp>
    <view id="late" name="late" inherit="base.view_users_list" priority="20">
        <xpath expr="//field[@name='login']" position="after"><field name="id"/></xpath>
    </view>
    <view id="early" name="early" inherit="base.view_users_list" priority="10">
        <xpath expr="//field[@name='active']" position="replace"/>
    </view>
</erp>"#];

/// Extensions change the view in place, lowest priority first.
#[test]
fn test_other_plugins_extend_views_with_paths() -> Result<()> {
    let app = with_data("view_changes", VIEW_CHANGES)?;
    assert_eq!(
        view_of(&app, "users", "list")?
            .split_whitespace()
            .collect::<String>(),
        "<list><fieldname=\"name\"/><fieldname=\"login\"/><fieldname=\"id\"/>\
         <fieldname=\"groups\"/></list>"
    );
    Ok(())
}

/// A primary view of lower priority is the one shown, from its parent's final arch.
#[test]
fn test_a_primary_view_of_lower_priority_is_shown() -> Result<()> {
    let app = with_data(
        "view_primary",
        &[r#"<erp>
            <view id="short" name="short" inherit="base.view_users_list" mode="primary" priority="1">
                <xpath expr="//field[@name='groups']" position="replace"/>
            </view>
        </erp>"#],
    )?;
    let list = view_of(&app, "users", "list")?;
    assert!(!list.contains("groups") && list.contains("login"), "{list}");
    Ok(())
}

/// A list or a form nobody declared shows every field the model shows.
#[test]
fn test_a_view_nobody_declared_is_generated() -> Result<()> {
    let app = new_app()?;
    let form = view_of(&app, "users", "form")?;
    assert!(
        form.starts_with("<form>") && form.contains("<field name=\"login\"/>"),
        "{form}"
    );
    assert!(
        !form.contains("password"),
        "a private field is not shown: {form}"
    );
    let error = view_of(&app, "users", "kanban").expect_err("no such view");
    assert!(error.to_string().contains("kanban"), "{error}");
    Ok(())
}

/// A view showing a field its model lacks, or extending with a path that matches nothing, stops
/// its plugin from installing.
#[test]
fn test_a_view_that_cannot_be_shown_fails_its_plugin() -> Result<()> {
    let refused = |name: &'static str, data: &'static [&'static str]| {
        let mut app = new_app().expect("an application");
        app.register_plugin(Box::new(DataPlugin { name, data }))
            .expect("registered");
        app.load_plugin(name).expect_err("refused").to_string()
    };
    let error = refused(
        "view_typo",
        &[
            r#"<erp><view id="typo" name="typo" model="users"><list><field name="logn"/></list></view></erp>"#,
        ],
    );
    assert!(error.contains("logn") && error.contains("users"), "{error}");

    let error = refused(
        "view_nowhere",
        &[r#"<erp>
            <view id="nowhere" name="nowhere" inherit="base.view_users_list">
                <xpath expr="//field[@name='nowhere']" position="after"><field name="id"/></xpath>
            </view>
        </erp>"#],
    );
    assert!(error.contains("nowhere"), "{error}");
    Ok(())
}

/// The error a plugin shipping this form for `users` is refused with.
fn refused_form(form: &str) -> String {
    let data: &'static str = Box::leak(
        format!(r#"<erp><view id="bad" name="bad" model="users"><form>{form}</form></view></erp>"#)
            .into_boxed_str(),
    );
    let data: &'static [&'static str] = Box::leak(vec![data].into_boxed_slice());
    let mut app = new_app().expect("an application");
    app.register_plugin(Box::new(DataPlugin {
        name: "bad_form",
        data,
    }))
    .expect("registered");
    app.load_plugin("bad_form")
        .expect_err("refused")
        .to_string()
}

/// A form of blocks, pages, headings, buttons and a side is shown as written.
#[test]
fn test_a_form_lays_out_its_blocks_pages_and_buttons() -> Result<()> {
    let app = with_data(
        "good_form",
        &[
            r#"<erp><view id="good" name="good" model="users" priority="1"><form>
            <buttons><button name="me" type="method" string="Who am I"/></buttons>
            <block string="{{ login }} \{{ literal }}">
                <h1>User <field name="name"/></h1>
                <block><field name="login"/></block>
                <pages><page name="more" string="More"><field name="active"/></page></pages>
            </block>
            <side><block><field name="groups"/></block><chatter/></side>
        </form></view></erp>"#,
        ],
    )?;
    assert!(view_of(&app, "users", "form")?.contains("<buttons>"));
    assert!(
        view_of(&new_app()?, "users", "form")?.contains("<h1>"),
        "base ships one"
    );
    Ok(())
}

/// What a form may not hold is refused when its plugin installs, saying what and where.
#[test]
fn test_what_a_form_may_not_hold_is_refused() -> Result<()> {
    for (form, expected) in [
        ("<div/>", "<div> cannot stand in <form>"),
        (
            "<block><buttons/></block>",
            "<buttons> cannot stand in <block>",
        ),
        ("<block>loose text</block>", "holds text"),
        ("<h1><block/></h1>", "<block> cannot stand in <h1>"),
        ("<pages><block/></pages>", "only <page>"),
        ("<pages><page string=\"x\"/></pages>", "<page> has no name"),
        (
            "<buttons><button name=\"me\" type=\"url\"/></buttons>",
            "method or action",
        ),
        (
            "<buttons><button type=\"method\"/></buttons>",
            "<button> has no name",
        ),
        ("<field/>", "<field> has no name"),
        ("<block><field name=\"logn\"/></block>", "\"logn\""),
        ("<block string=\"{{ logn }}\"/>", "\"logn\""),
        (
            "<block string=\"{{ login.size }}\"/>",
            "only a field's name",
        ),
        ("<block string=\"{{ login\"/>", "without closing it"),
    ] {
        let error = refused_form(form);
        assert!(error.contains(expected), "{form}: {error}");
    }
    Ok(())
}
