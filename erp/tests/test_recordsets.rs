//! Working on recordsets as a whole: keeping those a condition holds for, ordering them, and
//! putting two together — each record once, or one after the other.

use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::error::Error;

mod models {
    use code_gen::Model;
    use erp::types::field::{IdMode, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "shelf")]
    #[allow(dead_code)]
    pub struct Shelf<Mode: IdMode> {
        pub id: Mode,
        name: String,
    }

    #[derive(Model)]
    #[erp(id = "book")]
    #[allow(dead_code)]
    pub struct Book<Mode: IdMode> {
        pub id: Mode,
        name: String,
        pages: i32,
        #[erp(required, ondelete = "restrict")]
        shelf: Reference<BaseShelf, SingleId>,
    }
}

mod reading {
    use crate::models::BaseShelf;
    use code_gen::Model;
    use erp::types::field::{IdMode, Reference, SingleId};

    /// Names the shelf of a book only to read it, saying nothing else of it.
    #[derive(Model)]
    #[erp(id = "book")]
    #[erp(derived_model = "crate::models")]
    #[allow(dead_code)]
    pub struct BookReading<Mode: IdMode> {
        pub id: Mode,
        shelf: Reference<BaseShelf, SingleId>,
    }
}

use models::Book;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<models::Shelf<_>>();
    app.model_manager.register_model::<models::Book<_>>();
    app.model_manager
        .register_model::<reading::BookReading<_>>();
    app.model_manager.post_register();
    app
}

/// Three books on one shelf: 300, 120 and 450 pages.
fn books(env: &mut Environment) -> Result<Book<MultipleIds>> {
    let mut shelf = MapOfFields::default();
    shelf.insert("name", "Novels");
    let shelf = env.create_records("shelf", vec![shelf])?.get_ids_ref()[0];
    let mut values = Vec::new();
    for (name, pages) in [("Dune", 300), ("Candide", 120), ("Ulysses", 450)] {
        let mut book = MapOfFields::default();
        book.insert("name", name);
        book.insert("pages", pages);
        book.insert("shelf", shelf);
        values.push(book);
    }
    let ids = env.create_records("book", values)?;
    Ok(Book::from_ids(ids.get_ids_ref().clone(), env))
}

fn names(env: &mut Environment, books: &Book<MultipleIds>) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for book in books {
        found.push(book.get_name(env)?.clone());
    }
    Ok(found)
}

/// The records a condition holds for, in their order; a record is kept, or none is.
#[test]
fn test_records_are_filtered() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let all = books(&mut env)?;
    let long = all.filtered(&mut env, |book, env| Ok(*book.get_pages(env)? > 200))?;
    assert_eq!(names(&mut env, &long)?, ["Dune", "Ulysses"]);

    let dune: Book<SingleId> = Book::from_id(all.get_ids_ref()[0], &env);
    let kept = dune.filtered(&mut env, |book, env| Ok(*book.get_pages(env)? > 200))?;
    assert_eq!(kept.get_ids(), [dune.get_id()]);
    let dropped = dune.filtered(&mut env, |book, env| Ok(*book.get_pages(env)? > 1000))?;
    assert!(dropped.get_ids().is_empty());
    Ok(())
}

/// Ordered by what a key gives each record, ties keeping their order.
#[test]
fn test_records_are_sorted() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let all = books(&mut env)?;
    let by_pages = all.sorted_by_key(&mut env, |book, env| Ok(*book.get_pages(env)?))?;
    assert_eq!(names(&mut env, &by_pages)?, ["Candide", "Dune", "Ulysses"]);
    let tied = all.sorted_by_key(&mut env, |_, _| Ok(0))?;
    assert_eq!(tied.get_ids(), all.get_ids());
    Ok(())
}

/// `|` holds each record once, `+` both one after the other; records and recordsets mix.
#[test]
fn test_recordsets_are_put_together() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let all = books(&mut env)?;
    let ids = all.get_ids();
    let first: Book<MultipleIds> = Book::from_ids(vec![ids[0], ids[1]], &env);
    let last: Book<MultipleIds> = Book::from_ids(vec![ids[1], ids[2]], &env);

    assert_eq!((&first | &last).get_ids(), [ids[0], ids[1], ids[2]]);
    assert_eq!((&first + &last).get_ids(), [ids[0], ids[1], ids[1], ids[2]]);

    let ulysses: Book<SingleId> = Book::from_id(ids[2], &env);
    assert_eq!((&first | &ulysses).get_ids(), [ids[0], ids[1], ids[2]]);
    assert_eq!((ulysses.clone() + ulysses).get_ids(), [ids[2], ids[2]]);
    Ok(())
}

/// A struct naming a field without `required` leaves it required.
#[test]
fn test_naming_a_field_again_keeps_it_required() {
    let app = new_app();
    let book = app.model_manager.get_model("book");
    assert!(book.get_internal_field("shelf").required);
}

/// A required field left empty is an input error naming the field, told as such to a remote
/// caller; any other refusal is a business one, and the server's own failures are internal.
#[test]
fn test_errors_say_whose_they_are() -> Result<()> {
    let app = new_app();
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "book.create",
        "params": {"values": {"name": "Loose", "pages": 10}},
        "id": 1
    })
    .to_string();
    let answer = erp::jsonrpc::handle(&app, None, &body).expect("answered");
    assert_eq!(answer["error"]["data"]["kind"], "input", "{answer}");
    assert_eq!(answer["error"]["data"]["field"], "shelf", "{answer}");

    use erp::errors::ErrorKind;
    assert_eq!(
        erp::Error::from("Closed on Sundays").kind(),
        ErrorKind::Business
    );
    let broken: Box<dyn Error + Send + Sync> = erp::Error::internal("a bug").into();
    assert_eq!(erp::Error::from(broken).kind(), ErrorKind::Internal);
    Ok(())
}
