//! What becomes of records pointing to one being deleted: emptied, deleted with it, or holding
//! the deletion back — as their many2one says, or as an extension of their model changed it to.

use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{FieldType, IdMode, MultipleIds};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod library {
    use code_gen::Model;
    use erp::types::field::{IdMode, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "shelf")]
    #[allow(dead_code)]
    pub struct Shelf<Mode: IdMode> {
        pub id: Mode,
    }

    #[derive(Model)]
    #[erp(id = "book")]
    #[allow(dead_code)]
    pub struct Book<Mode: IdMode> {
        pub id: Mode,
        shelf: Reference<BaseShelf, SingleId>,
    }

    #[derive(Model)]
    #[erp(id = "chapter")]
    #[allow(dead_code)]
    pub struct Chapter<Mode: IdMode> {
        pub id: Mode,
        #[erp(ondelete = "cascade")]
        book: Reference<BaseBook, SingleId>,
        #[erp(ondelete = "cascade")]
        next: Reference<BaseChapter, SingleId>,
    }

    #[derive(Model)]
    #[erp(id = "loan")]
    #[allow(dead_code)]
    pub struct Loan<Mode: IdMode> {
        pub id: Mode,
        #[erp(ondelete = "restrict")]
        book: Reference<BaseBook, SingleId>,
    }

    #[derive(Model)]
    #[erp(id = "label")]
    #[allow(dead_code)]
    pub struct Label<Mode: IdMode> {
        pub id: Mode,
        #[erp(required)]
        shelf: Reference<BaseShelf, SingleId>,
    }

    /// Loans of a deleted book go with it, once this extension is loaded.
    #[derive(Model)]
    #[erp(id = "loan")]
    #[erp(derived_model = "")]
    #[allow(dead_code)]
    pub struct LoanEnds<Mode: IdMode> {
        pub id: Mode,
        #[erp(ondelete = "cascade")]
        book: Reference<BaseBook, SingleId>,
    }
}

fn new_app(loans_end: bool) -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<library::Shelf<_>>();
    app.model_manager.register_model::<library::Book<_>>();
    app.model_manager.register_model::<library::Chapter<_>>();
    app.model_manager.register_model::<library::Loan<_>>();
    app.model_manager.register_model::<library::Label<_>>();
    if loans_end {
        app.model_manager.register_model::<library::LoanEnds<_>>();
    }
    app.model_manager.post_register();
    app
}

fn create(env: &mut Environment, model: &str, values: &[(&str, u32)]) -> Result<u32> {
    let mut map = MapOfFields::default();
    for (field, id) in values {
        map.insert(field, *id);
    }
    Ok(env.create_records(model, vec![map])?.get_ids_ref()[0])
}

fn exists(env: &mut Environment, model: &str, id: u32) -> Result<bool> {
    Ok(env.existing(model, vec![id]).is_ok())
}

fn delete(env: &mut Environment, model: &str, id: u32) -> Result<u32> {
    env.delete(model, &MultipleIds::from(vec![id]))
}

/// By default, a record pointing to one deleted is kept, with its field emptied.
#[test]
fn test_a_deleted_record_is_emptied_from_what_points_to_it() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let shelf = create(&mut env, "shelf", &[])?;
    let book = create(&mut env, "book", &[("shelf", shelf)])?;
    env.close()?;

    let mut env = app.new_env()?;
    delete(&mut env, "shelf", shelf)?;
    env.close()?;

    let mut env = app.new_env()?;
    let rows = env.read("book", &MultipleIds::from(vec![book]), &["shelf"])?;
    assert_eq!(
        rows[0].fields.get("shelf"),
        Some(&None::<FieldType>),
        "the book is off any shelf"
    );
    Ok(())
}

/// A required field cannot be emptied, so the deletion is refused instead.
#[test]
fn test_a_required_field_holds_the_deletion_back() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let shelf = create(&mut env, "shelf", &[])?;
    let label = create(&mut env, "label", &[("shelf", shelf)])?;
    env.close()?;

    let mut env = app.new_env()?;
    let error = delete(&mut env, "shelf", shelf).expect_err("a label needs its shelf");
    assert_eq!(
        error.to_string(),
        format!(
            "shelf #{shelf} cannot be deleted: label #{label} points to it through \"shelf\", \
             which is required"
        )
    );
    assert!(exists(&mut env, "shelf", shelf)?);
    Ok(())
}

/// `restrict` refuses the deletion while anything points to the record, and lets it go after.
#[test]
fn test_restrict_holds_the_deletion_back() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let book = create(&mut env, "book", &[])?;
    let loan = create(&mut env, "loan", &[("book", book)])?;
    let chapter = create(&mut env, "chapter", &[("book", book)])?;
    env.close()?;

    let mut env = app.new_env()?;
    let error = delete(&mut env, "book", book).expect_err("the book is lent");
    assert!(error.to_string().contains("which keeps what it points to"));
    drop(env);

    let mut env = app.new_env()?;
    assert!(
        exists(&mut env, "chapter", chapter)?,
        "nothing was half done"
    );
    delete(&mut env, "loan", loan)?;
    delete(&mut env, "book", book)?;
    assert!(!exists(&mut env, "book", book)?);
    Ok(())
}

/// `cascade` deletes what points to the record along with it, and what points to those in
/// turn — once each, even when they point to each other.
#[test]
fn test_cascade_deletes_what_points_to_the_record() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let book = create(&mut env, "book", &[])?;
    let other = create(&mut env, "book", &[])?;
    let first = create(&mut env, "chapter", &[("book", book)])?;
    let second = create(&mut env, "chapter", &[("book", book), ("next", first)])?;
    let elsewhere = create(&mut env, "chapter", &[("book", other), ("next", second)])?;
    let mut next = MapOfFields::default();
    next.insert("next", second);
    env.write("chapter", &MultipleIds::from(vec![first]), next)?;
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(delete(&mut env, "book", book)?, 1);
    env.close()?;

    let mut env = app.new_env()?;
    assert!(!exists(&mut env, "chapter", first)?);
    assert!(!exists(&mut env, "chapter", second)?);
    assert!(
        !exists(&mut env, "chapter", elsewhere)?,
        "a chapter following a deleted one goes too"
    );
    assert!(
        exists(&mut env, "book", other)?,
        "only what points is deleted"
    );
    Ok(())
}

/// An extension of a model changes what its many2one does, as it would change any field.
#[test]
fn test_an_extension_changes_what_a_many2one_does() -> Result<()> {
    let app = new_app(true);
    let mut env = app.new_env()?;
    let book = create(&mut env, "book", &[])?;
    let loan = create(&mut env, "loan", &[("book", book)])?;
    env.close()?;

    let mut env = app.new_env()?;
    delete(&mut env, "book", book)?;
    assert!(
        !exists(&mut env, "loan", loan)?,
        "the loan ended with the book"
    );
    Ok(())
}
