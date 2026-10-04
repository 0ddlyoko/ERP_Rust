//! The web plugin: it installs itself, sends the root to the web client, and serves what the
//! installed plugins bring.

use base::BasePlugin;
use erp::Result;
use erp::app::Application;
use erp::assets::{StaticFiles, TemplateFiles};
use erp::data;
use erp::http::{self, Request, Response};
use erp::model::ModelManager;
use erp::plugin::Plugin;
use erp::serde_json::json;
use erp::types::field::SingleId;
use erp::types::model::MapOfFields;
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;
use web::WebPlugin;
use web::models::Template;

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

/// Submit the login form from the site: with the browser's cookie and the form's CSRF token.
fn post_login(app: &Application, login: &str, password: &str, redirect: &str) -> Response {
    let browser = Request::new("GET", "/login").with_header("Cookie", "csrf_id=browser");
    let token = http::csrf::token_for(app, &browser);
    submit_login(app, "csrf_id=browser", &token, login, password, redirect)
}

fn submit_login(
    app: &Application,
    cookie: &str,
    token: &str,
    login: &str,
    password: &str,
    redirect: &str,
) -> Response {
    let form = format!("login={login}&password={password}&redirect={redirect}&csrf_token={token}");
    http::handle(
        app,
        Request::new("POST", "/login")
            .with_header("Cookie", cookie)
            .with_header("Content-Type", "application/x-www-form-urlencoded")
            .with_body(form.into_bytes()),
    )
}

/// The value of a form field, as the page wrote it.
fn field_value(page: &str, name: &str) -> String {
    let marker = format!("name=\"{name}\" type=\"hidden\" value=\"");
    let start = page.find(&marker).expect("the field") + marker.len();
    page[start..]
        .split('"')
        .next()
        .expect("a value")
        .to_string()
}

/// The `name=value` of the session cookie a response sets.
fn session_cookie(response: &Response) -> String {
    let set = response
        .headers()
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
        .map(|(_, value)| value.clone())
        .expect("a cookie is set");
    set.split(';').next().expect("a value").to_string()
}

/// The session cookie of the administrator, logged in with the seeded password.
fn log_in(app: &Application) -> String {
    let response = post_login(app, "admin", base::DEFAULT_ADMIN_PASSWORD, "/web");
    assert_eq!(response.status(), 303, "{}", response.text_body());
    session_cookie(&response)
}

fn get_as(app: &Application, cookie: &str, target: &str) -> Response {
    http::handle(
        app,
        Request::new("GET", target).with_header("Cookie", cookie),
    )
}

/// A page of the web client, as the logged-in administrator.
fn get_logged_in(app: &Application, target: &str) -> Response {
    let cookie = log_in(app);
    get_as(app, &cookie, target)
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

    let response = get_logged_in(&app, "/web");
    assert_eq!(response.status(), 200);
    assert!(
        response
            .header("content-type")
            .is_some_and(|kind| kind.starts_with("text/html"))
    );
    Ok(())
}

// ---- logging in ----

/// Nobody logged in is sent to the login form, and back to the web client after.
#[test]
fn test_the_web_client_asks_to_log_in_first() -> Result<()> {
    let app = new_app()?;
    let response = get(&app, "/web");
    assert_eq!(response.status(), 303);
    assert_eq!(response.header("location"), Some("/login?redirect=/web"));

    let form = get(&app, "/login?redirect=/web");
    assert_eq!(form.status(), 200);
    let page = form.text_body();
    assert!(page.contains("<form class=\"o_login_form\" method=\"post\" action=\"/login\">"));
    assert!(
        page.contains("name=\"redirect\" type=\"hidden\" value=\"/web\""),
        "{page}"
    );
    assert!(!page.contains("o_login_error"), "no error yet");
    assert!(page.contains("/web/assets/web.assets_login.css"));
    assert!(
        get(&app, "/web/assets/web.assets_login.css")
            .text_body()
            .contains(".o_login_form")
    );
    Ok(())
}

/// The right credentials give the browser a session cookie, which the next requests carry.
#[test]
fn test_logging_in_sets_a_session_cookie() -> Result<()> {
    let app = new_app()?;
    let response = post_login(&app, "admin", base::DEFAULT_ADMIN_PASSWORD, "/web");
    assert_eq!(response.status(), 303);
    assert_eq!(response.header("location"), Some("/web"));
    let set = response.header("set-cookie").expect("a cookie");
    for attribute in [
        "session_id=",
        "Path=/",
        "Max-Age=",
        "HttpOnly",
        "SameSite=Lax",
    ] {
        assert!(set.contains(attribute), "{attribute} in {set}");
    }

    let cookie = session_cookie(&response);
    assert_eq!(get_as(&app, &cookie, "/web").status(), 200);
    let again = get_as(&app, &cookie, "/login?redirect=/web/somewhere");
    assert_eq!(again.status(), 303, "already logged in");
    assert_eq!(again.header("location"), Some("/web/somewhere"));
    Ok(())
}

/// The way a browser logs in: the form gives it a cookie and a token for that cookie, and
/// submitting both is what is accepted.
#[test]
fn test_the_login_form_carries_its_csrf_token() -> Result<()> {
    let app = new_app()?;
    let form = get(&app, "/login");
    let set = form
        .header("set-cookie")
        .expect("a cookie to bind the token to");
    assert!(
        set.starts_with("csrf_id=") && set.contains("HttpOnly"),
        "{set}"
    );
    let cookie = session_cookie(&form);
    let token = field_value(&form.text_body(), "csrf_token");

    let password = base::DEFAULT_ADMIN_PASSWORD;
    let response = submit_login(&app, &cookie, &token, "admin", password, "/web");
    assert_eq!(response.status(), 303, "{}", response.text_body());
    assert!(
        response
            .header("set-cookie")
            .is_some_and(|set| set.starts_with("session_id=")),
        "logged in"
    );
    Ok(())
}

/// Without the form's token, or with a token of another browser, the login is refused: another
/// site could otherwise log the browser in as an account of its own.
#[test]
fn test_a_login_without_its_csrf_token_is_refused() -> Result<()> {
    let app = new_app()?;
    let password = base::DEFAULT_ADMIN_PASSWORD;
    let response = submit_login(&app, "csrf_id=browser", "", "admin", password, "/web");
    assert_eq!(response.status(), 400);
    assert!(response.header("set-cookie").is_none());

    let elsewhere = http::csrf::token_for(
        &app,
        &Request::new("GET", "/login").with_header("Cookie", "csrf_id=another"),
    );
    let response = submit_login(
        &app,
        "csrf_id=browser",
        &elsewhere,
        "admin",
        password,
        "/web",
    );
    assert_eq!(response.status(), 400);
    Ok(())
}

/// Wrong credentials show the form again, with what was typed as the login, and no cookie.
#[test]
fn test_wrong_credentials_are_refused() -> Result<()> {
    let app = new_app()?;
    let response = post_login(&app, "admin", "wrong", "/web");
    assert_eq!(response.status(), 401);
    assert!(response.header("set-cookie").is_none());
    let page = response.text_body();
    assert!(page.contains("Wrong login or password."), "{page}");
    assert!(page.contains("value=\"admin\""), "{page}");

    let response = post_login(&app, "nobody", "nothing", "/web");
    assert_eq!(response.status(), 401);
    Ok(())
}

/// Logging in never sends the browser to another site.
#[test]
fn test_the_login_only_redirects_within_the_site() -> Result<()> {
    let app = new_app()?;
    for (asked, sent) in [
        ("/web/somewhere", "/web/somewhere"),
        ("//elsewhere.example", "/web"),
        ("https://elsewhere.example", "/web"),
        ("", "/web"),
    ] {
        let response = post_login(&app, "admin", base::DEFAULT_ADMIN_PASSWORD, asked);
        assert_eq!(response.header("location"), Some(sent), "{asked}");
    }
    Ok(())
}

/// Logging out ends the session: its cookie is forgotten, and no longer lets anybody in.
#[test]
fn test_logging_out_ends_the_session() -> Result<()> {
    let app = new_app()?;
    let cookie = log_in(&app);
    let response = get_as(&app, &cookie, "/logout");
    assert_eq!(response.status(), 303);
    assert_eq!(response.header("location"), Some("/login"));
    let cleared = response.header("set-cookie").expect("cleared");
    assert!(
        cleared.starts_with("session_id=;") && cleared.contains("Max-Age=0"),
        "{cleared}"
    );

    assert_eq!(get_as(&app, &cookie, "/web").status(), 303, "revoked");
    assert_eq!(get(&app, "/logout").status(), 303, "nobody to log out");
    Ok(())
}

/// A cookie naming no live session is nobody, sent to log in like anybody else.
#[test]
fn test_an_unknown_session_cookie_is_nobody() -> Result<()> {
    let app = new_app()?;
    for cookie in ["session_id=1.forged", "session_id=garbage", "other=1"] {
        let response = get_as(&app, cookie, "/web");
        assert_eq!(response.status(), 303, "{cookie}");
    }
    Ok(())
}

/// The JSON the web client page holds about its session.
fn session_info(page: &str) -> erp::serde_json::Value {
    let json = page
        .split("<script type=\"application/json\" id=\"session_info\">")
        .nth(1)
        .and_then(|rest| rest.split("</script>").next())
        .expect("the session's information");
    erp::serde_json::from_str(json).expect("JSON")
}

/// The page tells the client who is logged in, their groups, and a CSRF token its calls over the
/// session cookie are accepted with.
#[test]
fn test_the_web_client_page_holds_the_session() -> Result<()> {
    let app = new_app()?;
    let cookie = log_in(&app);
    let info = session_info(&get_as(&app, &cookie, "/web").text_body());
    assert_eq!(info["login"], "admin");
    assert!(info["uid"].as_u64().is_some());
    let groups: Vec<&str> = info["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .filter_map(|group| group.as_str())
        .collect();
    assert!(groups.contains(&"base.group_admin"), "{groups:?}");

    let csrf = info["csrf_token"].as_str().expect("a token");
    let call = Request::new("POST", "/jsonrpc")
        .with_header("Cookie", &cookie)
        .with_header("X-CSRF-Token", csrf);
    let token = cookie.trim_start_matches("session_id=").to_string();
    let credentials = erp::jsonrpc::credentials(&app, None, &call).map_err(|error| error.code);
    assert_eq!(credentials, Ok(Some(token)));
    Ok(())
}

/// A name holding markup cannot end the script it is written in.
#[test]
fn test_the_session_information_cannot_close_its_script() -> Result<()> {
    let app = new_app()?;
    {
        let mut env = app.new_env_as_option(None)?;
        let admin = data::resolve(&mut env, "base.user_admin")?.expect("seeded");
        let mut values = MapOfFields::default();
        values.insert("name", "</script><b>&");
        env.write("users", &SingleId::from(admin), values)?;
        env.close()?;
    }
    let page = get_logged_in(&app, "/web").text_body();
    assert!(!page.contains("</script><b>"), "{page}");
    assert_eq!(session_info(&page)["name"], "</script><b>&");
    Ok(())
}

/// The client's services are compiled, served, and loaded by the back office's bundle.
#[test]
fn test_the_client_services_are_in_the_backend_bundle() -> Result<()> {
    let app = new_app()?;
    let module = get(&app, "/web/assets/web.assets_backend.js").text_body();
    for service in ["session", "rpc", "orm"] {
        let path = format!("/static/web/src/core/{service}.js");
        assert!(
            module.contains(&format!("import \"{path}\";")),
            "{service} in {module}"
        );
        assert_eq!(get(&app, &path).status(), 200, "{path}");
    }
    let rpc = get(&app, "/static/web/src/core/rpc.js").text_body();
    assert!(
        rpc.contains("X-CSRF-Token") && rpc.contains("/jsonrpc"),
        "{rpc}"
    );
    assert!(!rpc.contains("@inject"), "decorators are lowered");
    Ok(())
}

/// Every call the `orm` service makes, in the shape it sends them, as the page's session: what the
/// client writes and what the server reads must not drift apart.
#[test]
fn test_the_server_answers_the_calls_of_the_orm_service() -> Result<()> {
    let app = new_app()?;
    let cookie = log_in(&app);
    let info = session_info(&get_as(&app, &cookie, "/web").text_body());
    let csrf = info["csrf_token"].as_str().expect("a token").to_string();
    let mut id = 0;
    let mut call = |method: &str, params: erp::serde_json::Value| {
        id += 1;
        let carried = Request::new("POST", "/jsonrpc")
            .with_header("Cookie", &cookie)
            .with_header("X-CSRF-Token", &csrf);
        let token = erp::jsonrpc::credentials(&app, None, &carried).expect("accepted");
        let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": id});
        let answer =
            erp::jsonrpc::handle(&app, token.as_deref(), &body.to_string()).expect("an answer");
        assert!(answer.get("error").is_none(), "{method}: {answer}");
        answer["result"].clone()
    };

    let created = call("group.create", json!({"values": {"name": "Testers"}}));
    let id_of = created[0].as_u64().expect("an id");
    assert_eq!(
        call(
            "group.write",
            json!({"ids": [id_of], "values": {"name": "Reviewers"}})
        ),
        json!(true)
    );
    let rows = call("group.read", json!({"ids": [id_of], "fields": ["name"]}));
    assert_eq!(rows[0]["name"], "Reviewers");
    let domain = json!([["name", "=", "Reviewers"]]);
    assert_eq!(
        call("group.search", json!({"domain": domain, "limit": 10})),
        json!([id_of])
    );
    let rows = call(
        "group.read_matching",
        json!({"domain": domain, "fields": ["name"], "order": ["name asc"]}),
    );
    assert_eq!(rows[0]["name"], "Reviewers");
    assert_eq!(call("group.count", json!({"domain": domain})), json!(1));
    assert_eq!(call("group.delete", json!({"ids": [id_of]})), json!(1));
    assert_eq!(
        call("users.me", json!({"ids": [], "args": {}})),
        info["uid"]
    );
    let columns = ["name", "login", "active", "groups"];
    let rows = call(
        "users.read_matching",
        json!({"domain": [], "fields": columns, "limit": 100, "offset": 0, "names": true}),
    );
    let admin = rows
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row["login"] == "admin")
        .expect("the administrator is listed");
    assert!(
        admin["groups"].is_array() && admin["active"] == true,
        "{admin}"
    );
    let described = call("users.fields_get", json!({"fields": []}));
    for column in columns {
        assert!(described.get(column).is_some(), "{column} is described");
    }
    assert_eq!(
        call("users.names", json!({"ids": [info["uid"]]})),
        json!([[info["uid"], "Administrator"]])
    );
    let total = call("users.count", json!({"domain": []}));
    assert_eq!(
        total.as_u64(),
        rows.as_array().map(|rows| rows.len() as u64)
    );
    let menus = call("menu.tree", json!({"ids": [], "args": {}}));
    let users = &menus[0]["children"][0]["children"][0]["action"];
    assert_eq!(users["xml_id"], "base.action_users", "{menus}");
    assert_eq!(users["views"], json!(["list", "form"]));
    let found = call("users.name_search", json!({"text": "admin", "limit": 8}));
    assert_eq!(found[0][1], "Administrator", "{found}");
    let fields = call("group.fields_get", json!({"fields": ["name", "id"]}));
    assert_eq!(fields["name"]["type"], "string");
    assert_eq!(fields["id"]["readonly"], true);
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

/// A TypeScript import has no extension; it is compiled with the extension of the file it names,
/// and only that path is served, so a browser loads each file as one module.
#[test]
fn test_an_import_without_extension_is_compiled_with_it() -> Result<()> {
    let app = new_app()?;
    let counter = get(&app, "/static/test_plugin/src/counter.js").text_body();
    assert!(counter.contains("from \"./util/format.js\""), "{counter}");
    let format = get(&app, "/static/test_plugin/src/util/format.js");
    assert!(format.text_body().contains("function twice"));
    assert_eq!(
        get(&app, "/static/test_plugin/src/util/format").status(),
        404,
        "one path per module"
    );

    let list = get(&app, "/static/web/src/views/list/list_view.js").text_body();
    assert!(
        list.contains("from \"@web/views/view.js\""),
        "a plugin's import, completed: {list}"
    );
    let main = get(&app, "/static/web/src/main.js").text_body();
    for import in [
        "./core/session.js",
        "./core/rpc.js",
        "./core/orm.js",
        "./web_client/web_client.js",
    ] {
        assert!(
            main.contains(&format!("\"{import}\"")),
            "{import} in {main}"
        );
    }
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
        get(&app, "/web/assets/nobody.contributes.css").status(),
        404
    );
    assert_eq!(
        get(&app, "/web/assets/nobody.contributes.xml").status(),
        404
    );
    assert_eq!(get(&app, "/web/assets/test.backend.txt").status(), 404);
    Ok(())
}

/// The styles of a bundle are one file, each part preceded by where it comes from.
#[test]
fn test_a_bundle_serves_its_styles_as_one_file() -> Result<()> {
    let app = new_app()?;
    let response = get(&app, "/web/assets/test.frontend.css");
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.header("content-type"),
        Some("text/css; charset=utf-8")
    );
    let sheet = response.text_body();
    assert!(
        sheet.starts_with("/* test_plugin/static/css/style.css */\n"),
        "{sheet}"
    );
    let file = get(&app, "/static/test_plugin/css/style.css").text_body();
    assert!(sheet.contains(file.trim_end()));
    Ok(())
}

#[test]
fn test_trame_is_served() -> Result<()> {
    let app = new_app()?;
    for target in ["/static/web/lib/trame.js", "/static/web/lib/trame.min.js"] {
        let response = get(&app, target);
        assert_eq!(response.status(), 200, "{target}");
        assert_eq!(
            response.header("content-type"),
            Some("text/javascript; charset=utf-8")
        );
        assert!(
            response.text_body().contains("registerTemplates"),
            "{target}"
        );
    }
    Ok(())
}

// ---- templates ----

/// A plugin shipping template files and nothing else, after `test_plugin` whose component it
/// extends.
struct TemplatePlugin {
    name: &'static str,
    files: StaticFiles,
    templates: TemplateFiles,
}

impl Plugin for TemplatePlugin {
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn get_depends(&self) -> Vec<String> {
        vec!["web".to_string(), "test_plugin".to_string()]
    }

    fn static_files(&self) -> StaticFiles {
        self.files
    }

    fn template_files(&self) -> TemplateFiles {
        self.templates
    }
}

fn with_templates(name: &'static str, files: StaticFiles) -> Result<Application> {
    install(TemplatePlugin {
        name,
        files,
        templates: &[],
    })
}

fn with_server_templates(name: &'static str, templates: TemplateFiles) -> Result<Application> {
    install(TemplatePlugin {
        name,
        files: &[],
        templates,
    })
}

fn install(plugin: TemplatePlugin) -> Result<Application> {
    let name = plugin.name;
    let mut app = new_app()?;
    app.register_plugin(Box::new(plugin))?;
    app.load_plugin(name)?;
    Ok(app)
}

fn refused(name: &'static str, files: StaticFiles) -> String {
    refused_install(TemplatePlugin {
        name,
        files,
        templates: &[],
    })
}

fn refused_install(plugin: TemplatePlugin) -> String {
    let name = plugin.name;
    match install(plugin) {
        Ok(_) => panic!("{name} installed"),
        Err(error) => error.to_string(),
    }
}

fn templates(app: &Application, bundle: &str) -> String {
    let response = get(app, &format!("/web/assets/{bundle}.xml"));
    assert_eq!(response.status(), 200, "{}", response.text_body());
    assert_eq!(
        response.header("content-type"),
        Some("application/xml; charset=utf-8")
    );
    response.text_body()
}

const COUNTER: &str = "<templates>\n\
    <t t-name=\"test_plugin.Counter\"><span>{{ count }}</span></t>\n\
    </templates>\n";

/// The `.xml` next to a component is a template, served in the bundles holding that file.
#[test]
fn test_a_component_template_is_served_in_its_bundle() -> Result<()> {
    let app = new_app()?;
    assert_eq!(templates(&app, "test.backend"), COUNTER);
    assert_eq!(
        get(&app, "/web/assets/test.frontend.xml").status(),
        404,
        "no template file in that bundle"
    );
    let backend = templates(&app, "web.assets_backend");
    for template in [
        "<t t-name=\"web.WebClient\">",
        "<t t-name=\"web.ListView\">",
        "<t t-name=\"web.Widget\">",
        "<t t-name=\"web.BoolWidget\">",
    ] {
        assert!(backend.contains(template), "{template} in {backend}");
    }
    for template in [
        "<t t-name=\"web.ActionManager\">",
        "<t t-name=\"web.Sidebar\">",
        "<t t-name=\"web.FormView\">",
        "<t t-name=\"web.NotificationCenter\">",
    ] {
        assert!(backend.contains(template), "{template} in {backend}");
    }
    Ok(())
}

/// The list view's pieces travel with the back office: its component, template and styles.
#[test]
fn test_the_list_view_is_in_the_backend_bundle() -> Result<()> {
    let app = new_app()?;
    let module = get(&app, "/web/assets/web.assets_backend.js").text_body();
    for path in [
        "/static/web/src/views/view.js",
        "/static/web/src/views/list/list_view.js",
        "/static/web/src/views/widgets/widget.js",
        "/static/web/src/views/widgets/string_widget.js",
        "/static/web/src/views/widgets/integer_widget.js",
        "/static/web/src/views/widgets/decimal_widget.js",
        "/static/web/src/views/widgets/bool_widget.js",
        "/static/web/src/views/widgets/date_widget.js",
        "/static/web/src/views/widgets/datetime_widget.js",
        "/static/web/src/views/widgets/many2one_widget.js",
        "/static/web/src/views/widgets/x2many_widget.js",
        "/static/web/src/views/widgets/record_search.js",
        "/static/web/src/views/widgets/tags_widget.js",
        "/static/web/src/views/widgets/list_widget.js",
        "/static/web/src/views/widgets/selection_widget.js",
        "/static/web/src/views/widgets/statusbar_widget.js",
        "/static/web/src/core/domain.js",
        "/static/web/src/views/search/search_model.js",
        "/static/web/src/views/search/search_bar.js",
        "/static/web/src/views/list/list_actions.js",
        "/static/web/src/core/models.js",
        "/static/web/src/core/views.js",
        "/static/web/src/core/menus.js",
        "/static/web/src/web_client/action_manager.js",
        "/static/web/src/web_client/sidebar.js",
        "/static/web/src/views/form/form_view.js",
        "/static/web/src/views/form/form_compiler.js",
        "/static/web/src/views/form/form_body.js",
        "/static/web/src/core/router.js",
        "/static/web/src/core/breadcrumb.js",
        "/static/web/src/core/list_memory.js",
        "/static/web/src/core/notifications.js",
        "/static/web/src/web_client/notification_center.js",
    ] {
        assert!(
            module.contains(&format!("import \"{path}\";")),
            "{path} in {module}"
        );
    }
    let styles = get(&app, "/web/assets/web.assets_backend.css").text_body();
    assert!(styles.contains(".o_list_table"), "{styles}");
    Ok(())
}

/// The web client's component is in its bundle, beside its template.
#[test]
fn test_the_web_client_component_is_in_the_backend_bundle() -> Result<()> {
    let app = new_app()?;
    let module = get(&app, "/web/assets/web.assets_backend.js").text_body();
    assert!(
        module.contains("import \"/static/web/src/web_client/web_client.js\";"),
        "{module}"
    );
    let component = get(&app, "/static/web/src/web_client/web_client.js");
    assert_eq!(component.status(), 200);
    assert!(component.text_body().contains("web.WebClient"));
    assert!(
        component.text_body().contains("from \"trame\""),
        "imported by name, as written"
    );
    Ok(())
}

/// The page names what scripts import by name, before loading any of them.
#[test]
fn test_the_web_client_page_declares_the_import_map() -> Result<()> {
    let app = new_app()?;
    let page = get_logged_in(&app, "/web").text_body();
    let map = page
        .split("<script type=\"importmap\">")
        .nth(1)
        .and_then(|rest| rest.split("</script>").next())
        .expect("an import map");
    let map: erp::serde_json::Value = erp::serde_json::from_str(map)?;
    assert_eq!(map["imports"]["trame"], "/static/web/lib/trame.js");
    assert_eq!(
        map["imports"]["@web/"], "/static/web/src/",
        "each plugin by its name"
    );
    assert_eq!(map["imports"]["@test_plugin/"], "/static/test_plugin/src/");
    let map_at = page.find("importmap").expect("declared");
    let bundle_at = page
        .find("/web/assets/web.assets_backend.js")
        .expect("loaded");
    assert!(map_at < bundle_at, "declared before any module loads");
    assert_eq!(get(&app, "/static/web/lib/trame.js").status(), 200);
    Ok(())
}

/// A template shipped in a file is a record named after its key.
#[test]
fn test_templates_are_records_named_after_their_key() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    for (external_id, file) in [
        ("test_plugin.Counter", "test_plugin/static/src/counter.xml"),
        ("web.WebClient", "web/static/src/web_client/web_client.xml"),
    ] {
        let id = data::resolve(&mut env, external_id)?.expect("synchronised");
        let rows = env.read("template", &SingleId::from(id), &["key", "file"])?;
        assert_eq!(rows[0].get::<&String>("key"), external_id);
        assert_eq!(
            rows[0].get_option::<&String>("file").map(String::as_str),
            Some(file)
        );
    }
    Ok(())
}

const EXTENSIONS: StaticFiles = &[
    (
        "src/a.xml",
        br#"<templates>
            <t t-inherit="test_plugin.Counter">
                <xpath expr="//span" position="inside"><b>a1</b></xpath>
            </t>
            <t t-inherit="test_plugin.Counter">
                <xpath expr="//b" position="after"><i>a2</i></xpath>
            </t>
        </templates>"#,
    ),
    (
        "src/b.xml",
        br#"<templates>
            <t t-name="web_extension.Counter" t-inherit="test_plugin.Counter">
                <xpath expr="//span" position="replace"><em/></xpath>
            </t>
            <t t-inherit="test_plugin.Counter">
                <xpath expr="//span" position="attributes">
                    <attribute name="class">last</attribute>
                </xpath>
            </t>
        </templates>"#,
    ),
];

/// Extensions apply in the order a bundle loads them; a derived template starts from the final
/// markup of its parent, is served where its parent is, and leaves the parent as it is.
#[test]
fn test_other_plugins_extend_templates_with_paths() -> Result<()> {
    let app = with_templates("web_extension", EXTENSIONS)?;
    assert_eq!(
        templates(&app, "test.backend"),
        "<templates>\n\
         <t t-name=\"test_plugin.Counter\"><span class=\"last\">{{ count }}<b>a1</b><i>a2</i></span></t>\n\
         <t t-name=\"web_extension.Counter\"><em/></t>\n\
         </templates>\n"
    );
    Ok(())
}

/// An extension whose path matches nothing stops its plugin from installing, instead of the web
/// client failing to render later.
#[test]
fn test_an_extension_matching_nothing_fails_its_plugin() -> Result<()> {
    let error = refused(
        "web_broken",
        &[(
            "src/broken.xml",
            br#"<templates><t t-inherit="test_plugin.Counter">
                <xpath expr="//nowhere" position="inside"><p/></xpath>
            </t></templates>"#,
        )],
    );
    assert!(error.contains("//nowhere"), "{error}");
    assert!(
        error.contains("web_broken/static/src/broken.xml"),
        "{error}"
    );
    Ok(())
}

#[test]
fn test_what_a_template_file_may_not_hold() -> Result<()> {
    let error = refused(
        "web_orphan",
        &[(
            "src/orphan.xml",
            br#"<templates><t t-inherit="nobody.Thing"><xpath expr="//a"/></t></templates>"#,
        )],
    );
    assert!(error.contains("nobody.Thing"), "{error}");

    let error = refused(
        "web_misnamed",
        &[(
            "src/misnamed.xml",
            br#"<templates><t t-name="other.Thing"><div/></t></templates>"#,
        )],
    );
    assert!(error.contains("web_misnamed.<Name>"), "{error}");

    let error = refused(
        "web_twice",
        &[
            (
                "src/one.xml",
                br#"<templates><t t-name="web_twice.Thing"><div/></t></templates>"#,
            ),
            (
                "src/two.xml",
                br#"<templates><t t-name="web_twice.Thing"><p/></t></templates>"#,
            ),
        ],
    );
    assert!(error.contains("web_twice.Thing"), "{error}");

    let error = refused(
        "web_stray",
        &[("src/stray.xml", br#"<templates><div/></templates>"#)],
    );
    assert!(error.contains("<div>"), "{error}");
    Ok(())
}

/// The resolved templates are kept, and forgotten once a template changes; what a transaction has
/// not committed is seen by it alone.
#[test]
fn test_resolved_templates_follow_committed_changes() -> Result<()> {
    let app = new_app()?;
    assert_eq!(templates(&app, "test.backend"), COUNTER);

    let mut env = app.new_env_as_option(None)?;
    let counter = data::resolve(&mut env, "test_plugin.Counter")?.expect("synchronised");
    let mut values = MapOfFields::default();
    values.insert("arch", "<span>changed</span>");
    env.write("template", &SingleId::from(counter), values)?;
    let seen = Template::<SingleId>::bundle_markup(&mut env, "test.backend")?;
    assert!(
        seen.as_deref()
            .is_some_and(|markup| markup.contains("changed"))
    );
    assert_eq!(templates(&app, "test.backend"), COUNTER, "not committed");
    env.close()?;

    assert!(templates(&app, "test.backend").contains("<span>changed</span>"));
    Ok(())
}

/// A new build of a plugin ships other templates: the next start brings the database in line,
/// what is no longer shipped included.
#[test]
fn test_a_restart_follows_the_files_of_a_new_build() -> Result<()> {
    let database = erp::database::cache::CacheDatabase::default();
    let first = boot(&database, Some(EXTENSIONS))?;
    assert!(templates(&first, "test.backend").contains("web_extension.Counter"));
    drop(first);

    let second = boot(&database, Some(&EXTENSIONS[..1]))?;
    assert_eq!(
        templates(&second, "test.backend"),
        "<templates>\n\
         <t t-name=\"test_plugin.Counter\"><span>{{ count }}<b>a1</b><i>a2</i></span></t>\n\
         </templates>\n"
    );
    let mut env = second.new_env_as_option(None)?;
    assert_eq!(data::resolve(&mut env, "web_extension.Counter")?, None);
    Ok(())
}

// ---- pages ----

/// `/web` is the page template calling the base layout, loading the back office's bundle.
#[test]
fn test_the_web_client_page_is_rendered_from_its_template() -> Result<()> {
    let app = new_app()?;
    let response = get_logged_in(&app, "/web");
    assert_eq!(response.status(), 200, "{}", response.text_body());
    let page = response.text_body();
    assert!(
        page.starts_with("<!doctype html>\n<html lang=\"en\">"),
        "{page}"
    );
    for expected in [
        "<meta charset=\"utf-8\">",
        "<title>ERP</title>",
        "<link rel=\"stylesheet\" href=\"/web/assets/web.assets_backend.css\">",
        "<script type=\"module\" src=\"/web/assets/web.assets_backend.js\"></script>",
        "<div class=\"o_web_client_root\"></div>",
    ] {
        assert!(page.contains(expected), "{expected} in {page}");
    }
    assert!(!page.contains(" t-"), "no directive left: {page}");

    let styles = get(&app, "/web/assets/web.assets_backend.css").text_body();
    assert!(
        styles.contains(".o_sidebar") && styles.contains("--o-accent"),
        "{styles}"
    );
    let scripts = get(&app, "/web/assets/web.assets_backend.js").text_body();
    assert!(scripts.contains("import \"/static/web/src/main.js\";"));
    assert!(
        !templates(&app, "web.assets_backend").contains("web.Layout"),
        "page templates stay on the server"
    );
    Ok(())
}

const PAGES: TemplateFiles = &[(
    "pages/pages.xml",
    br#"<templates>
        <t t-name="web_pages.Frame">
            <section><h2><t t-out="heading"/></h2><t t-out="0"/><p t-out="note"/></section>
        </t>
        <t t-name="web_pages.Page">
            <t t-call="web_pages.Frame">
                <t t-set="heading" t-value="'A &lt; B'"/>
                <t t-set="note">kept <b>as markup</b></t>
                <br/><em>body</em>
            </t>
            <i t-out="heading"/>
        </t>
        <t t-name="web_pages.Unsupported"><p t-foreach="x" t-as="y">no</p></t>
        <t t-name="web_pages.Missing"><t t-call="web_pages.Nowhere"/></t>
        <t t-name="web_pages.Loop"><t t-call="web_pages.Loop"/></t>
    </templates>"#,
)];

fn render(app: &Application, key: &str) -> Result<String> {
    let mut env = app.new_env_as_option(None)?;
    Template::<SingleId>::render_page(&mut env, key, web::qweb::Values::new())
}

/// A called template gets its caller's body as `0` and the values set in it, and those values
/// end with the call; text is escaped, a rendered body is not; void elements are not closed.
#[test]
fn test_a_template_calls_another_with_values_and_a_body() -> Result<()> {
    let app = with_server_templates("web_pages", PAGES)?;
    let page = render(&app, "web_pages.Page")?;
    assert_eq!(
        page.split_whitespace().collect::<String>(),
        "<!doctypehtml><section><h2>A&lt;B</h2><br><em>body</em><p>kept<b>asmarkup</b></p></section><i></i>"
    );
    Ok(())
}

#[test]
fn test_what_the_server_does_not_render_is_refused() -> Result<()> {
    let app = with_server_templates("web_pages", PAGES)?;
    for (key, expected) in [
        ("web_pages.Unsupported", "t-foreach"),
        ("web_pages.Missing", "web_pages.Nowhere"),
        ("web_pages.Loop", "deep"),
        ("web_pages.Unknown", "web_pages.Unknown"),
    ] {
        let error = render(&app, key).expect_err(key).to_string();
        assert!(error.contains(expected), "{key}: {error}");
    }
    Ok(())
}

/// What the server renders is never downloaded; what the browser renders is never rendered here.
#[test]
fn test_server_and_browser_templates_stay_apart() -> Result<()> {
    let app = new_app()?;
    for target in [
        "/static/web/templates/layout.xml",
        "/static/web/pages/layout.xml",
        "/static/web/layout.xml",
    ] {
        assert_eq!(get(&app, target).status(), 404, "{target}");
    }
    assert_eq!(
        get(&app, "/static/web/src/web_client/web_client.xml").status(),
        200,
        "a component's template is the browser's"
    );

    let error = render(&app, "web.WebClient").expect_err("the browser's");
    assert!(error.to_string().contains("browser"), "{error}");

    let error = refused_install(TemplatePlugin {
        name: "web_crossing",
        files: &[(
            "src/crossing.xml",
            br#"<templates><t t-inherit="web.Layout">
                <xpath expr="//body" position="inside"><p/></xpath>
            </t></templates>"#,
        )],
        templates: &[],
    });
    assert!(error.contains("inherits from the server's"), "{error}");
    Ok(())
}

/// A page template is extended like any other, and the page follows.
#[test]
fn test_another_plugin_extends_the_layout() -> Result<()> {
    let app = with_server_templates(
        "web_layout",
        &[(
            "pages/layout.xml",
            br#"<templates><t t-inherit="web.Layout">
                <xpath expr="//body" position="attributes">
                    <attribute name="class">o_extended</attribute>
                </xpath>
            </t></templates>"#,
        )],
    )?;
    let page = get_logged_in(&app, "/web").text_body();
    assert!(page.contains("<body class=\"o_extended\">"), "{page}");
    Ok(())
}

// ---- views ----

/// The client asks for a view over the protocol, as the page's session.
#[test]
fn test_a_view_is_loaded_over_the_protocol() -> Result<()> {
    let app = new_app()?;
    let cookie = log_in(&app);
    let info = session_info(&get_as(&app, &cookie, "/web").text_body());
    let csrf = info["csrf_token"].as_str().expect("a token");
    let carried = Request::new("POST", "/jsonrpc")
        .with_header("Cookie", &cookie)
        .with_header("X-CSRF-Token", csrf);
    let token = erp::jsonrpc::credentials(&app, None, &carried).map_err(|error| error.message)?;
    let body = json!({
        "jsonrpc": "2.0",
        "method": "view.load",
        "params": {"ids": [], "args": {"model": "users", "kind": "list"}},
        "id": 1,
    });
    let answer = erp::jsonrpc::handle(&app, token.as_deref(), &body.to_string()).expect("owed");
    let arch = answer["result"]
        .as_str()
        .unwrap_or_else(|| panic!("{answer}"));
    assert!(arch.starts_with("<list>"), "{arch}");
    Ok(())
}

/// The compiled library is found under the symbol its file name gives, `erp_create_plugin_web`.
///
/// It links against the one shared `erp`, the same as this test's, so it always matches.
#[test]
fn test_the_library_exports_the_plugin_under_its_name() -> Result<()> {
    let directory = std::env::current_exe()?
        .parent()
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
    manager.register_plugin_from_file(&library)?;
    let again = manager.register_plugin(Box::new(WebPlugin {}));
    assert!(
        again.is_err_and(|error| error.to_string().contains("\"web\"")),
        "the library registered the plugin named web"
    );
    Ok(())
}

/// Start the application the way the server does, on this database: from the plugin directory,
/// then whatever the database says is installed, then what installs itself.
///
/// With template files, `web_extension` ships them, and is installed if it is not yet.
fn boot(
    database: &erp::database::cache::CacheDatabase,
    templates: Option<StaticFiles>,
) -> Result<Application> {
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
    if let Some(files) = templates {
        app.register_plugin(Box::new(TestLibPlugin {}))?;
        app.register_plugin(Box::new(TestPlugin {}))?;
        app.register_plugin(Box::new(TemplatePlugin {
            name: "web_extension",
            files,
            templates: &[],
        }))?;
    }
    app.load()?;
    if templates.is_some() && !app.plugin_manager.is_installed("web_extension") {
        app.load_plugin("web_extension")?;
    }
    Ok(app)
}

/// The second boot loads again what the first one installed, `web` depending on `base` included.
#[test]
fn test_a_restart_loads_again_what_installed_itself() -> Result<()> {
    let database = erp::database::cache::CacheDatabase::default();
    let first = boot(&database, None)?;
    assert!(first.plugin_manager.is_installed("web"));
    drop(first);

    let second = boot(&database, None)?;
    assert!(second.plugin_manager.is_installed("web"));
    assert_eq!(get(&second, "/").header("location"), Some("/web"));
    Ok(())
}
