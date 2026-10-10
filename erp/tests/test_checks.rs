//! `check_*` methods run on their own once records are created or written: a refusal undoes the
//! work, and a check naming fields with `on` runs only when one of them is written.

use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod models {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, SingleId};
    use std::error::Error;

    type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

    thread_local! {
        /// How many times stamps were counted, by the test running on this thread.
        pub static STAMPS_COUNTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    #[derive(Model)]
    #[erp(id = "parcel", methods)]
    #[allow(dead_code)]
    pub struct Parcel<Mode: IdMode> {
        pub id: Mode,
        name: String,
        weight: i32,
        #[erp(default = 0)]
        stamps: i32,
    }

    #[erp_methods]
    impl Parcel<MultipleIds> {
        /// A parcel weighs something.
        pub fn check_weight(&self, env: &mut Environment) -> Result<()> {
            for parcel in self {
                if *parcel.get_weight(env)? <= 0 {
                    return Err("A parcel weighs something".into());
                }
            }
            Ok(())
        }

        /// Stamps are counted only when they change.
        #[erp(on = ["stamps"])]
        pub fn check_stamps(&self, env: &mut Environment) -> Result<()> {
            STAMPS_COUNTED.set(STAMPS_COUNTED.get() + 1);
            for parcel in self {
                if *parcel.get_stamps(env)? > 10 {
                    return Err("At most ten stamps".into());
                }
            }
            Ok(())
        }
    }

    #[erp_methods]
    impl Parcel<SingleId> {
        /// A parcel has a name, checked one parcel at a time.
        pub fn check_name(&self, env: &mut Environment) -> Result<()> {
            if self.get_name(env)?.trim().is_empty() {
                return Err("A parcel has a name".into());
            }
            Ok(())
        }
    }
}

use models::Parcel;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Parcel<_>>();
    app.model_manager.post_register();
    app
}

fn parcel(name: &str, weight: i32) -> MapOfFields {
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert("weight", weight);
    values
}

fn count(env: &mut Environment) -> Result<u32> {
    env.count("parcel", &erp::search::SearchType::Nothing)
}

/// A record breaking a check is refused, and nothing of the creation is kept.
#[test]
fn test_a_check_refuses_a_creation() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let error = env
        .create_records("parcel", vec![parcel("Books", 2), parcel("Air", 0)])
        .expect_err("refused");
    assert_eq!(error.to_string(), "A parcel weighs something");
    assert_eq!(count(&mut env)?, 0);

    let error = env
        .create_records("parcel", vec![parcel(" ", 1)])
        .expect_err("a check on one record runs for each");
    assert_eq!(error.to_string(), "A parcel has a name");
    Ok(())
}

/// A write breaking a check is refused and undone, a setter's as any other.
#[test]
fn test_a_check_refuses_a_write() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let ids = env.create_records("parcel", vec![parcel("Books", 2)])?;
    let books: Parcel<MultipleIds> = Parcel::from_ids(ids.get_ids_ref().clone(), &env);
    let books = books.ensure_one()?;
    assert!(books.set_weight(0, &mut env).is_err());
    assert_eq!(*books.get_weight(&mut env)?, 2, "undone");
    Ok(())
}

/// A check `on` fields runs when one of them is written, and not otherwise.
#[test]
fn test_a_check_on_fields_runs_when_they_change() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let mut values = parcel("Letters", 1);
    values.insert("stamps", 12);
    assert!(
        env.create_records("parcel", vec![values]).is_err(),
        "always on creation"
    );

    let ids = env.create_records("parcel", vec![parcel("Letters", 1)])?;
    let letters = Parcel::<MultipleIds>::from_ids(ids.get_ids_ref().clone(), &env).ensure_one()?;
    let counted = models::STAMPS_COUNTED.get();
    letters.set_weight(3, &mut env)?;
    assert_eq!(
        models::STAMPS_COUNTED.get(),
        counted,
        "not run when stamps are not written"
    );
    assert!(letters.set_stamps(11, &mut env).is_err());
    Ok(())
}
