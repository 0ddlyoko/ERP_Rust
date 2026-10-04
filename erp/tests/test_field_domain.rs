//! A relation's domain, declared on its field: given to clients, so they offer only the records it
//! matches, and changed by an extension like any other attribute of the field.

use erp::app::Application;
use erp_types::field::IdMode;
use erp_types::model::MapOfFields;
use serde_json::{Value, json};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod library {
    use code_gen::Model;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "shelf")]
    #[allow(dead_code)]
    pub struct Shelf<Mode: IdMode> {
        pub id: Mode,
        name: String,
        full: bool,
        #[erp(inverse = "shelf", domain = r#"[["title", "!=", ""]]"#)]
        books: Reference<BaseBook, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "book")]
    #[allow(dead_code)]
    pub struct Book<Mode: IdMode> {
        pub id: Mode,
        title: Option<String>,
        #[erp(domain = r#"[["full", "=", false]]"#)]
        shelf: Reference<BaseShelf, SingleId>,
        other: Reference<BaseShelf, SingleId>,
    }

    /// Books may go on any shelf named "Library", full or not, once this extension is loaded.
    #[derive(Model)]
    #[erp(id = "book")]
    #[erp(derived_model = "")]
    #[allow(dead_code)]
    pub struct BookAnywhere<Mode: IdMode> {
        pub id: Mode,
        #[erp(domain = r#"[["name", "ilike", "Library"]]"#)]
        shelf: Reference<BaseShelf, SingleId>,
    }
}

fn new_app(anywhere: bool) -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<library::Shelf<_>>();
    app.model_manager.register_model::<library::Book<_>>();
    if anywhere {
        app.model_manager
            .register_model::<library::BookAnywhere<_>>();
    }
    app.model_manager.post_register();
    app
}

fn described(app: &Application, model: &str) -> Result<Value> {
    let mut env = app.new_env()?;
    env.call_rpc(model, "fields_get", &json!({}))
}

/// A client reads the domain a relation declares; a relation declaring none has none.
#[test]
fn test_fields_get_gives_the_domain() -> Result<()> {
    let app = new_app(false);
    let book = described(&app, "book")?;
    assert_eq!(book["shelf"]["domain"], json!([["full", "=", false]]));
    assert_eq!(book["other"].get("domain"), None);
    let shelf = described(&app, "shelf")?;
    assert_eq!(shelf["books"]["domain"], json!([["title", "!=", ""]]));
    Ok(())
}

/// The last struct declaring a domain for the field gives it.
#[test]
fn test_an_extension_replaces_the_domain() -> Result<()> {
    let app = new_app(true);
    let book = described(&app, "book")?;
    assert_eq!(
        book["shelf"]["domain"],
        json!([["name", "ilike", "Library"]])
    );
    Ok(())
}

/// The domain is what a client offers, not a check: a record outside it can still be chosen.
#[test]
fn test_the_domain_does_not_restrict_writes() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", "Full");
    values.insert("full", true);
    let full = env.create_records("shelf", vec![values])?;
    let mut values = MapOfFields::default();
    values.insert("shelf", full.get_ids_ref()[0]);
    let book = env.create_records("book", vec![values])?;
    let rows = env.read("book", &book, &["shelf"])?;
    assert_eq!(
        rows[0].get_option::<&u32>("shelf"),
        Some(&full.get_ids_ref()[0])
    );
    Ok(())
}
