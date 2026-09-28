//! Controllers: methods of plugins that answer URLs, overridden the way model methods are.

use base::BasePlugin;
use erp::app::Application;
use erp::http::{self, ControllerRegistry, Request, Response};
use erp::model::ModelManager;
use erp::plugin::Plugin;
use erp_search::SearchType;
use std::error::Error;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod shop {
    use code_gen::{Controller, erp_routes};
    use erp::environment::Environment;
    use erp::http::{HttpError, Request, Response};
    use erp::types::field::SingleId;
    use erp::types::model::MapOfFields;
    use erp_search::SearchType;
    use std::error::Error;
    use test_utilities::models::Tag;

    type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

    #[derive(Controller)]
    #[erp(id = "shop")]
    pub struct Shop;

    #[erp_routes]
    impl Shop {
        #[erp(route = "/shop")]
        pub fn index(&self, env: &mut Environment, request: &Request) -> Result<Response> {
            Ok(Response::html(self.banner(env, request)?))
        }

        /// Not a route: an overridable piece of one.
        pub fn banner(&self, env: &mut Environment, request: &Request) -> Result<String> {
            let _ = (env, request);
            Ok("Shop".to_string())
        }

        #[erp(route = "/shop/<item>")]
        pub fn item(
            &self,
            env: &mut Environment,
            request: &Request,
            item: u32,
            colour: Option<String>,
        ) -> Result<Response> {
            let _ = (env, request);
            Ok(Response::text(format!("item {item} {colour:?}")))
        }

        #[erp(route = "/shop/new", methods = ["POST"])]
        pub fn create(
            &self,
            env: &mut Environment,
            request: &Request,
            name: String,
        ) -> Result<Response> {
            let _ = request;
            let mut values = MapOfFields::default();
            values.insert("name", name);
            env.create_records("tag", vec![values])?;
            Ok(Response::text("created"))
        }

        #[erp(route = "/shop/broken", methods = ["POST"])]
        pub fn broken(&self, env: &mut Environment, request: &Request) -> Result<Response> {
            let _ = request;
            let mut values = MapOfFields::default();
            values.insert("name", "written before failing");
            env.create_records("tag", vec![values])?;
            Err("failed on purpose, with details nobody outside should see".into())
        }

        #[erp(route = "/shop/gone")]
        pub fn gone(&self, env: &mut Environment, request: &Request) -> Result<Response> {
            let _ = (env, request);
            Err(HttpError::not_found("This item was sold").into())
        }

        /// A record, read from the id in the URL.
        #[erp(route = "/shop/tag/<tag>")]
        pub fn tag(
            &self,
            env: &mut Environment,
            request: &Request,
            tag: Tag<SingleId>,
            like: Option<Tag<SingleId>>,
        ) -> Result<Response> {
            let _ = request;
            let name = tag.get_name(env)?.clone();
            let like = match like {
                Some(like) => like.get_name(env)?.clone(),
                None => "nothing".to_string(),
            };
            Ok(Response::text(format!("{name} like {like}")))
        }

        #[erp(route = "/shop/tags")]
        pub fn tags(&self, env: &mut Environment, request: &Request) -> Result<Response> {
            let _ = request;
            let count = env.count("tag", &SearchType::Nothing)?;
            Ok(Response::text(count.to_string()))
        }
    }
}

mod branded {
    use code_gen::{Controller, erp_routes};
    use erp::environment::Environment;
    use erp::http::{Request, Response};
    use std::error::Error;

    type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

    /// Extends `shop` from another plugin.
    #[derive(Controller)]
    #[erp(id = "shop")]
    pub struct ShopBranded;

    #[erp_routes]
    impl ShopBranded {
        pub fn banner(
            &self,
            env: &mut Environment,
            request: &Request,
            sup: Super,
        ) -> Result<String> {
            let _ = request;
            Ok(format!("{} (branded)", sup.call(env)?))
        }

        /// Same method, answering another URL.
        #[erp(route = "/boutique/<item>")]
        pub fn item(
            &self,
            env: &mut Environment,
            request: &Request,
            item: u32,
            colour: Option<String>,
            sup: Super,
        ) -> Result<Response> {
            let _ = (request, item, colour);
            let below = sup.call(env)?;
            Ok(Response::text(format!("{}!", below.text_body())))
        }
    }
}

mod clash {
    use code_gen::{Controller, erp_routes};
    use erp::environment::Environment;
    use erp::http::{Request, Response};
    use std::error::Error;

    #[derive(Controller)]
    #[erp(id = "other_shop")]
    pub struct OtherShop;

    #[erp_routes]
    impl OtherShop {
        #[erp(route = "/shop/<anything>")]
        pub fn anything(
            &self,
            env: &mut Environment,
            request: &Request,
            anything: String,
        ) -> Result<Response, Box<dyn Error + Send + Sync>> {
            let _ = (env, request);
            Ok(Response::text(anything))
        }
    }
}

struct ShopPlugin;

impl Plugin for ShopPlugin {
    fn name(&self) -> String {
        "shop".to_string()
    }
    fn init_models(&self, _model_manager: &mut ModelManager) {}
    fn init_controllers(&self, controllers: &mut ControllerRegistry) {
        controllers.register::<shop::Shop>();
    }
    fn get_depends(&self) -> Vec<String> {
        vec!["test_lib_plugin".to_string()]
    }
}

struct BrandedPlugin;

impl Plugin for BrandedPlugin {
    fn name(&self) -> String {
        "shop_branded".to_string()
    }
    fn init_models(&self, _model_manager: &mut ModelManager) {}
    fn init_controllers(&self, controllers: &mut ControllerRegistry) {
        controllers.register::<branded::ShopBranded>();
    }
    fn get_depends(&self) -> Vec<String> {
        vec!["shop".to_string()]
    }
}

struct ClashPlugin;

impl Plugin for ClashPlugin {
    fn name(&self) -> String {
        "other_shop".to_string()
    }
    fn init_models(&self, _model_manager: &mut ModelManager) {}
    fn init_controllers(&self, controllers: &mut ControllerRegistry) {
        controllers.register::<clash::OtherShop>();
    }
    fn get_depends(&self) -> Vec<String> {
        vec!["shop".to_string()]
    }
}

fn shop_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(ShopPlugin))?;
    app.load_plugin("shop")?;
    Ok(app)
}

fn branded_app() -> Result<Application> {
    let mut app = shop_app()?;
    app.register_plugin(Box::new(BrandedPlugin))?;
    app.load_plugin("shop_branded")?;
    Ok(app)
}

fn get(app: &Application, target: &str) -> Response {
    http::handle(app, Request::new("GET", target))
}

fn tags(app: &Application) -> Result<u32> {
    let mut env = app.new_env()?;
    env.count("tag", &SearchType::Nothing)
}

// ---- answering ----

#[test]
fn test_base_answers_the_root() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;

    let response = get(&app, "/");
    assert_eq!(response.status(), 200);
    assert!(
        response
            .header("content-type")
            .is_some_and(|kind| kind.starts_with("text/html"))
    );
    assert!(response.text_body().contains("running"));
    Ok(())
}

#[test]
fn test_arguments_come_from_the_path_and_the_query() -> Result<()> {
    let app = shop_app()?;
    assert_eq!(get(&app, "/shop/7").text_body(), "item 7 None");
    assert_eq!(
        get(&app, "/shop/7?colour=dark%20red").text_body(),
        "item 7 Some(\"dark red\")"
    );
    Ok(())
}

/// A parameter that cannot be read is the caller's mistake, and says which one it is.
#[test]
fn test_a_bad_parameter_is_a_400_naming_it() -> Result<()> {
    let app = shop_app()?;
    let response = get(&app, "/shop/seven");
    assert_eq!(response.status(), 400);
    assert!(
        response.text_body().contains("item"),
        "got {}",
        response.text_body()
    );

    let response = http::handle(&app, Request::new("POST", "/shop/new"));
    assert_eq!(response.status(), 400);
    assert!(
        response.text_body().contains("name"),
        "got {}",
        response.text_body()
    );
    Ok(())
}

#[test]
fn test_a_posted_form_fills_arguments_and_commits() -> Result<()> {
    let app = shop_app()?;
    let response = http::handle(
        &app,
        Request::new("POST", "/shop/new")
            .with_header("Content-Type", "application/x-www-form-urlencoded")
            .with_body("name=Blue+tag"),
    );
    assert_eq!(response.status(), 200, "got {}", response.text_body());
    assert_eq!(tags(&app)?, 1, "what the controller wrote was committed");
    Ok(())
}

#[test]
fn test_unknown_urls_and_methods_are_refused() -> Result<()> {
    let app = shop_app()?;
    assert_eq!(get(&app, "/nowhere").status(), 404);

    let response = http::handle(&app, Request::new("DELETE", "/shop"));
    assert_eq!(response.status(), 405);
    assert_eq!(response.header("allow"), Some("GET"));
    Ok(())
}

/// A fixed segment wins over a parameter: `/shop/new` is not item "new".
#[test]
fn test_a_fixed_segment_wins_over_a_parameter() -> Result<()> {
    let app = shop_app()?;
    let response = http::handle(&app, Request::new("POST", "/shop/new?name=Red"));
    assert_eq!(response.text_body(), "created");
    Ok(())
}

/// A failure is answered in general terms, and nothing the request wrote survives it.
#[test]
fn test_a_failing_controller_rolls_back() -> Result<()> {
    let app = shop_app()?;
    let response = http::handle(&app, Request::new("POST", "/shop/broken"));
    assert_eq!(response.status(), 500);
    assert!(
        !response.text_body().contains("details"),
        "nothing internal is shown"
    );
    assert_eq!(tags(&app)?, 0);
    Ok(())
}

#[test]
fn test_a_controller_answers_with_its_own_status() -> Result<()> {
    let app = shop_app()?;
    let response = get(&app, "/shop/gone");
    assert_eq!(response.status(), 404);
    assert_eq!(response.text_body(), "This item was sold");
    Ok(())
}

/// Access rights apply: nobody authenticated is the portal user, who may not read tags.
#[test]
fn test_a_refused_read_is_a_403() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(ShopPlugin))?;
    app.load_plugin("base")?;
    app.load_plugin("shop")?;
    assert_eq!(get(&app, "/shop/tags").status(), 403);
    Ok(())
}

// ---- overriding ----

#[test]
fn test_an_override_reaches_the_method_below() -> Result<()> {
    let app = shop_app()?;
    assert_eq!(get(&app, "/shop").text_body(), "Shop");

    let app = branded_app()?;
    assert_eq!(
        get(&app, "/shop").text_body(),
        "Shop (branded)",
        "the route of the base plugin calls the overridden helper"
    );
    Ok(())
}

/// An override that declares a route moves its method there.
#[test]
fn test_an_override_can_move_a_route() -> Result<()> {
    let app = branded_app()?;
    assert_eq!(
        get(&app, "/boutique/7?colour=red").text_body(),
        "item 7 Some(\"red\")!"
    );
    assert_eq!(get(&app, "/shop/7").status(), 404);
    Ok(())
}

/// Two controllers answering the same URL would leave load order to decide which one does.
#[test]
#[should_panic(expected = "answer the same URL")]
fn test_two_controllers_cannot_answer_the_same_url() {
    let mut app = shop_app().expect("the shop");
    app.register_plugin(Box::new(ClashPlugin))
        .expect("registered");
    let _ = app.load_plugin("other_shop");
}

#[test]
fn test_the_registry_lists_its_routes() -> Result<()> {
    let app = branded_app()?;
    let mut routes: Vec<String> = app
        .model_manager
        .controllers
        .routes()
        .into_iter()
        .map(|(verbs, pattern, method)| format!("{} {pattern} {method}", verbs.join(",")))
        .collect();
    routes.sort();
    assert_eq!(
        routes,
        vec![
            "GET /boutique/<item> shop.item",
            "GET /shop shop.index",
            "GET /shop/gone shop.gone",
            "GET /shop/tag/<tag> shop.tag",
            "GET /shop/tags shop.tags",
            "POST /shop/broken shop.broken",
            "POST /shop/new shop.create",
        ]
    );
    Ok(())
}

/// URL encoding is undone, and a malformed escape is kept as written rather than refused.
#[test]
fn test_query_strings_are_decoded() {
    let request = Request::new("GET", "/x?a=caf%C3%A9&b=1+2&c=100%&d=%zz&e");
    assert_eq!(request.query("a"), Some("café"));
    assert_eq!(request.query("b"), Some("1 2"));
    assert_eq!(request.query("c"), Some("100%"));
    assert_eq!(request.query("d"), Some("%zz"));
    assert_eq!(request.query("e"), Some(""));
    assert_eq!(request.query("f"), None);
    assert_eq!(request.path(), "/x");
}

// ---- records as parameters ----

fn make_tag(app: &Application, name: &str) -> Result<u32> {
    let mut env = app.new_env_as_option(None)?;
    let mut values = erp::types::model::MapOfFields::default();
    values.insert("name", name);
    let id = env.create_records("tag", vec![values])?.ids[0];
    env.close()?;
    Ok(id)
}

/// An argument typed as a record is read from the id the URL carries.
#[test]
fn test_a_record_is_read_from_its_id() -> Result<()> {
    let app = shop_app()?;
    let red = make_tag(&app, "red")?;
    let blue = make_tag(&app, "blue")?;

    assert_eq!(
        get(&app, &format!("/shop/tag/{red}")).text_body(),
        "red like nothing"
    );
    assert_eq!(
        get(&app, &format!("/shop/tag/{red}?like={blue}")).text_body(),
        "red like blue"
    );
    Ok(())
}

#[test]
fn test_an_id_naming_nothing_is_a_404() -> Result<()> {
    let app = shop_app()?;
    let response = get(&app, "/shop/tag/999");
    assert_eq!(response.status(), 404);
    assert!(
        response.text_body().contains("tag"),
        "got {}",
        response.text_body()
    );

    assert_eq!(get(&app, "/shop/tag/red").status(), 400, "not an id at all");
    Ok(())
}

/// An optional record that names nothing is simply absent; one that is not an id is still refused.
#[test]
fn test_an_optional_record_naming_nothing_is_absent() -> Result<()> {
    let app = shop_app()?;
    let red = make_tag(&app, "red")?;
    assert_eq!(
        get(&app, &format!("/shop/tag/{red}?like=999")).text_body(),
        "red like nothing"
    );
    assert_eq!(
        get(&app, &format!("/shop/tag/{red}?like=blue")).status(),
        400
    );
    Ok(())
}

/// A record the caller may not see is answered like one that does not exist, so the answer does
/// not say which ids exist; a model closed to the caller is refused as such.
#[test]
fn test_a_record_is_read_with_the_callers_rights() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(ShopPlugin))?;
    app.load_plugin("base")?;
    app.load_plugin("shop")?;
    let public = make_tag(&app, "public")?;
    let secret = make_tag(&app, "secret")?;
    assert_eq!(
        get(&app, &format!("/shop/tag/{public}")).status(),
        403,
        "tags are closed"
    );

    let portal = app
        .model_manager
        .identities
        .default_user()
        .expect("base names the anonymous caller");
    let mut env = app.new_env_as_option(None)?;
    let mut group = erp::types::model::MapOfFields::default();
    group.insert("name", "Anonymous readers");
    group.insert("users", erp::types::field::FieldType::Refs(vec![portal]));
    let group = env.create_records("group", vec![group])?.ids[0];
    let mut rule = erp::types::model::MapOfFields::default();
    rule.insert("name", "public tags");
    rule.insert("model", "tag");
    rule.insert("group", erp::types::field::FieldType::Ref(group));
    rule.insert("domain_read", r#"[["name", "=", "public"]]"#);
    env.create_records("access_rule", vec![rule])?;
    env.close()?;

    assert_eq!(get(&app, &format!("/shop/tag/{public}")).status(), 200);
    assert_eq!(get(&app, &format!("/shop/tag/{secret}")).status(), 404);
    assert_eq!(get(&app, "/shop/tag/999").status(), 404);
    assert_eq!(
        get(&app, &format!("/shop/tag/{public}?like={secret}")).text_body(),
        "public like nothing",
        "an optional record the caller may not see is absent too"
    );
    Ok(())
}
