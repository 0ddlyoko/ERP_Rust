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
        "<list><buttons><buttonname=\"archive\"type=\"method\"string=\"Archive\"/>\
         <buttonname=\"unarchive\"type=\"method\"string=\"Unarchive\"/></buttons>\
         <fieldname=\"name\"/><fieldname=\"login\"/><fieldname=\"active\"/>\
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
        "<list><buttons><buttonname=\"archive\"type=\"method\"string=\"Archive\"/>\
         <buttonname=\"unarchive\"type=\"method\"string=\"Unarchive\"/></buttons>\
         <fieldname=\"name\"/><fieldname=\"login\"/><fieldname=\"id\"/>\
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
    let form = view_of(&app, "session", "form")?;
    assert!(
        form.starts_with("<form>") && form.contains("<field name=\"user\"/>"),
        "{form}"
    );
    assert!(
        !form.contains("secret"),
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

/// A form of blocks, pages, headings, buttons, totals and a side is shown as written.
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
                <pages><page name="more" string="More"><field name="active"/>
                    <totals invisible="!active"><field name="login"/><field name="name"/></totals>
                </page></pages>
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
        (
            "<field name=\"login\" invisible=\"logn === 'x'\"/>",
            "reads \"logn\"",
        ),
        ("<field name=\"login\" readonly=\"\"/>", "readonly is empty"),
        (
            "<buttons><button name=\"me\" type=\"method\" readonly=\"1\"/></buttons>",
            "cannot be readonly",
        ),
        ("<field name=\"login\" nolabel=\"yes\"/>", "0 or 1"),
        (
            "<totals><block/></totals>",
            "<block> cannot stand in <totals>",
        ),
        ("<totals><field name=\"logn\"/></totals>", "\"logn\""),
        (
            "<leader><field name=\"login\" role=\"hero\"/></leader>",
            "role \"hero\"",
        ),
        (
            "<leader><block/></leader>",
            "<block> cannot stand in <leader>",
        ),
        (
            "<leader><actions><button name=\"me\" type=\"url\"/></actions></leader>",
            "method or action",
        ),
        (
            "<related><field name=\"groups\"/></related>",
            "<field> cannot stand in <related>",
        ),
        (
            "<related><link name=\"groups\"/></related>",
            "<link> has no action",
        ),
        (
            "<related><link name=\"grups\" action=\"base.action_groups\"/></related>",
            "\"grups\"",
        ),
    ] {
        let error = refused_form(form);
        assert!(error.contains(expected), "{form}: {error}");
    }
    Ok(())
}

/// A form's leader — its actions, fields standing for the record, tiles — and the links to the
/// records it holds are shown as written.
#[test]
fn test_a_form_leader_and_its_links_are_shown_as_written() -> Result<()> {
    let app = with_data(
        "leading",
        &[
            r#"<erp><view id="leading" name="leading" model="users" priority="1"><form>
            <leader>
                <actions><button name="me" type="method" string="Who am I"/></actions>
                <field name="active" role="status"/>
                <field name="name" role="avatar"/>
                <field name="name" role="title"/>
                <field name="login" role="subtitle"/>
                <field name="login" invisible="!active"/>
            </leader>
            <related invisible="!active"><link name="groups" action="base.action_groups" icon="users"/></related>
            <block><field name="login"/></block>
        </form></view></erp>"#,
        ],
    )?;
    let arch = view_of(&app, "users", "form")?;
    assert!(arch.contains("role=\"title\""), "{arch}");
    assert!(arch.contains("<link name=\"groups\""), "{arch}");
    Ok(())
}

/// Conditions are expressions as Trame reads them: what they read besides fields — strings,
/// properties, the language's words, an arrow function's parameter — is not taken for a field.
#[test]
fn test_conditions_read_fields_only() -> Result<()> {
    let app = with_data(
        "conditions",
        &[
            r#"<erp><view id="conditions" name="conditions" model="users" priority="1"><form>
            <block invisible="!active">
                <field name="login" nolabel="1" readonly="login.length > 3 &amp;&amp; name !== 'x'"
                       required="['admin', &quot;root&quot;].includes(login)"/>
                <field name="groups" invisible="groups.some((group) => group === id) || 0"/>
            </block>
            <pages invisible="1"><page name="p" invisible="name === `x`"/></pages>
        </form></view></erp>"#,
        ],
    )?;
    assert!(view_of(&app, "users", "form")?.contains("invisible"));
    Ok(())
}

/// A search lists the fields typing searches in, each narrowed by a domain if it says so, and
/// filters to tick; a list may have buttons acting on the records selected.
#[test]
fn test_a_search_and_list_buttons_are_shown_as_written() -> Result<()> {
    let app = with_data(
        "searching",
        &[r#"<erp>
            <view id="search" name="search" model="users" priority="1"><search>
                <field name="name"/>
                <field name="login" domain='[["active", "=", true]]'/>
                <filter name="archived" string="Archived" domain='["|", ["active", "=", false], ["groups.name", "=", "x"]]'/>
                <filter name="by_creation" string="Created" group_by="create_date:month"/>
            </search></view>
            <view id="buttons" name="buttons" model="users" priority="1"><list decoration-muted="!active">
                <buttons><button name="me" type="method" string="Me"/></buttons>
                <field name="name"/>
                <field name="active" widget="badge" decoration-success="active"/>
            </list></view>
        </erp>"#],
    )?;
    assert!(view_of(&app, "users", "search")?.contains("archived"));
    assert!(view_of(&app, "users", "search")?.contains("by_creation"));
    assert!(view_of(&app, "users", "list")?.contains("<buttons>"));
    assert!(view_of(&app, "users", "list")?.contains("decoration-muted"));
    Ok(())
}

/// A search nobody declared searches by the name of the records.
#[test]
fn test_a_search_nobody_declared_searches_by_name() -> Result<()> {
    let app = new_app()?;
    assert_eq!(
        view_of(&app, "session", "search")?,
        "<search></search>",
        "a session has no name"
    );
    assert_eq!(
        view_of(&app, "group", "search")?,
        "<search><field name=\"name\"/></search>"
    );
    Ok(())
}

/// What a search may not hold is refused when its plugin installs.
#[test]
fn test_what_a_search_may_not_hold_is_refused() -> Result<()> {
    for (search, expected) in [
        ("<block/>", "<block> cannot stand in <search>"),
        ("<filter name=\"x\"/>", "<filter> has no domain"),
        ("<filter domain='[]'/>", "<filter> has no name"),
        ("<filter name=\"x\" domain='[[\"active\"]]'/>", "is not one"),
        (
            "<filter name=\"x\" domain='[[\"actif\", \"=\", true]]'/>",
            "\"actif\"",
        ),
        ("<field name=\"login\" domain='nope'/>", "is not one"),
        ("<filter name=\"x\" group_by=\"logn\"/>", "\"logn\""),
        ("<filter name=\"x\" group_by=\"logn:month\"/>", "\"logn\""),
    ] {
        let data: &'static str = Box::leak(
            format!(r#"<erp><view id="bad" name="bad" model="users"><search>{search}</search></view></erp>"#)
                .into_boxed_str(),
        );
        let data: &'static [&'static str] = Box::leak(vec![data].into_boxed_slice());
        let mut app = new_app()?;
        app.register_plugin(Box::new(DataPlugin {
            name: "bad_search",
            data,
        }))?;
        let error = app
            .load_plugin("bad_search")
            .expect_err("refused")
            .to_string();
        assert!(error.contains(expected), "{search}: {error}");
    }
    Ok(())
}

/// A decoration names a colour the client knows, and reads fields of the model; a kanban holds
/// fields, gathered by one the model has.
#[test]
fn test_what_a_list_may_not_decorate_is_refused() -> Result<()> {
    for (list, expected) in [
        (
            r#"<list decoration-pink="active"><field name="name"/></list>"#,
            "decoration-pink is no decoration",
        ),
        (
            r#"<list decoration-muted="!actif"><field name="name"/></list>"#,
            "reads \"actif\"",
        ),
        (
            r#"<list><field name="name" decoration-danger="logn === 'x'"/></list>"#,
            "reads \"logn\"",
        ),
        (
            r#"<kanban default_group_by="stat"><field name="name"/></kanban>"#,
            "\"stat\"",
        ),
        (
            r#"<kanban><block/></kanban>"#,
            "<block> cannot stand in <kanban>",
        ),
    ] {
        let data: &'static str = Box::leak(
            format!(r#"<erp><view id="bad" name="bad" model="users">{list}</view></erp>"#)
                .into_boxed_str(),
        );
        let data: &'static [&'static str] = Box::leak(vec![data].into_boxed_slice());
        let mut app = new_app()?;
        app.register_plugin(Box::new(DataPlugin {
            name: "bad_list",
            data,
        }))?;
        let error = app
            .load_plugin("bad_list")
            .expect_err("refused")
            .to_string();
        assert!(error.contains(expected), "{list}: {error}");
    }
    Ok(())
}
