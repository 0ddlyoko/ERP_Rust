//! A one2many declaring a domain holds only the records pointing back that match it: as read,
//! once one is added or changed, and once a field its domain reads along a path changes.

use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;

mod models {
    use code_gen::Model;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "shelf")]
    #[allow(dead_code)]
    pub struct Shelf<Mode: IdMode> {
        pub id: Mode,
        name: String,
        #[erp(default = true)]
        open: bool,
        #[erp(inverse = "shelf")]
        books: Reference<BaseBook, MultipleIds>,
        #[erp(inverse = "shelf", domain = r#"[["pages", ">", 200]]"#)]
        long_books: Reference<BaseBook, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "author")]
    #[allow(dead_code)]
    pub struct Author<Mode: IdMode> {
        pub id: Mode,
        name: String,
        #[erp(inverse = "author", domain = r#"[["shelf.open", "=", true]]"#)]
        books_on_open_shelves: Reference<BaseBook, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "book")]
    #[allow(dead_code)]
    pub struct Book<Mode: IdMode> {
        pub id: Mode,
        name: String,
        pages: i32,
        shelf: Reference<BaseShelf, SingleId>,
        author: Reference<BaseAuthor, SingleId>,
    }
}

use models::{Author, Book, Shelf};

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Shelf<_>>();
    app.model_manager.register_model::<Author<_>>();
    app.model_manager.register_model::<Book<_>>();
    app.model_manager.post_register();
    app
}

fn create(env: &mut Environment, model: &str, values: &[(&str, MapValue)]) -> Result<u32> {
    let mut map = MapOfFields::default();
    for (name, value) in values {
        match value {
            MapValue::Text(text) => map.insert(name, *text),
            MapValue::Number(number) => map.insert(name, *number),
            MapValue::Id(id) => map.insert(name, *id),
        }
    }
    Ok(env.create_records(model, vec![map])?.get_ids_ref()[0])
}

enum MapValue {
    Text(&'static str),
    Number(i32),
    Id(u32),
}

use MapValue::{Id, Number, Text};

fn names(env: &mut Environment, books: &Book<MultipleIds>) -> Result<Vec<String>> {
    let mut found = Vec::new();
    for book in books {
        found.push(book.get_name(env)?.clone());
    }
    found.sort();
    Ok(found)
}

/// The names of the books `get` gives, sorted.
fn held(
    env: &mut Environment,
    get: impl FnOnce(&mut Environment) -> Result<Book<MultipleIds>>,
) -> Result<Vec<String>> {
    let books = get(env)?;
    names(env, &books)
}

/// Read, a one2many with a domain holds the matching records; one without, all of them.
#[test]
fn test_a_domain_filters_what_is_read() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let shelf = create(&mut env, "shelf", &[("name", Text("Novels"))])?;
    for (name, pages) in [("Candide", 120), ("Dune", 400), ("Ulysses", 700)] {
        create(
            &mut env,
            "book",
            &[
                ("name", Text(name)),
                ("pages", Number(pages)),
                ("shelf", Id(shelf)),
            ],
        )?;
    }
    let shelf: Shelf<SingleId> = Shelf::from_id(shelf, &env);
    let long: Book<MultipleIds> = shelf.get_long_books(&mut env)?;
    assert_eq!(names(&mut env, &long)?, ["Dune", "Ulysses"]);
    let all: Book<MultipleIds> = shelf.get_books(&mut env)?;
    assert_eq!(names(&mut env, &all)?, ["Candide", "Dune", "Ulysses"]);
    Ok(())
}

/// A record added, or changed into or out of the domain, is held or not right away.
#[test]
fn test_a_domain_follows_changes() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let shelf_id = create(&mut env, "shelf", &[("name", Text("Novels"))])?;
    let shelf: Shelf<SingleId> = Shelf::from_id(shelf_id, &env);
    let dune = create(
        &mut env,
        "book",
        &[
            ("name", Text("Dune")),
            ("pages", Number(400)),
            ("shelf", Id(shelf_id)),
        ],
    )?;
    assert_eq!(held(&mut env, |env| shelf.get_long_books(env))?, ["Dune"]);

    create(
        &mut env,
        "book",
        &[
            ("name", Text("Candide")),
            ("pages", Number(120)),
            ("shelf", Id(shelf_id)),
        ],
    )?;
    create(
        &mut env,
        "book",
        &[
            ("name", Text("Ulysses")),
            ("pages", Number(700)),
            ("shelf", Id(shelf_id)),
        ],
    )?;
    assert_eq!(
        held(&mut env, |env| shelf.get_long_books(env))?,
        ["Dune", "Ulysses"]
    );

    let dune: Book<SingleId> = Book::from_id(dune, &env);
    dune.set_pages(90, &mut env)?;
    assert_eq!(
        held(&mut env, |env| shelf.get_long_books(env))?,
        ["Ulysses"]
    );
    assert_eq!(
        shelf
            .get_books::<Book<MultipleIds>>(&mut env)?
            .get_ids()
            .len(),
        3
    );
    Ok(())
}

/// A domain reading along a path follows a change at its end: books of a shelf being closed.
#[test]
fn test_a_domain_follows_a_path() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let open = create(&mut env, "shelf", &[("name", Text("Open"))])?;
    let other = create(&mut env, "shelf", &[("name", Text("Other"))])?;
    let author_id = create(&mut env, "author", &[("name", Text("Voltaire"))])?;
    let author: Author<SingleId> = Author::from_id(author_id, &env);
    create(
        &mut env,
        "book",
        &[
            ("name", Text("Candide")),
            ("pages", Number(120)),
            ("shelf", Id(open)),
            ("author", Id(author_id)),
        ],
    )?;
    create(
        &mut env,
        "book",
        &[
            ("name", Text("Zadig")),
            ("pages", Number(90)),
            ("shelf", Id(other)),
            ("author", Id(author_id)),
        ],
    )?;
    assert_eq!(
        held(&mut env, |env| author.get_books_on_open_shelves(env))?,
        ["Candide", "Zadig"]
    );

    let other: Shelf<SingleId> = Shelf::from_id(other, &env);
    other.set_open(false, &mut env)?;
    assert_eq!(
        held(&mut env, |env| author.get_books_on_open_shelves(env))?,
        ["Candide"]
    );
    Ok(())
}
