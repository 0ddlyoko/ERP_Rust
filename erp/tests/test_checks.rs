//! Methods marked `#[erp(check)]` run on their own once records are created or written: a refusal
//! undoes the work, and a check naming fields runs only when one of them is written.

use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod models {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
    use std::error::Error;

    type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

    thread_local! {
        /// How many times stamps were counted, by the test running on this thread.
        pub static STAMPS_COUNTED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        /// How many cartons had their load checked, by the test running on this thread.
        pub static LOADS_CHECKED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
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
        carton: Reference<BaseCarton, SingleId>,
    }

    #[derive(Model)]
    #[erp(id = "carton", methods)]
    #[allow(dead_code)]
    pub struct Carton<Mode: IdMode> {
        pub id: Mode,
        name: String,
        #[erp(inverse = "carton")]
        parcels: Reference<BaseParcel, MultipleIds>,
        truck: Reference<BaseTruck, SingleId>,
    }

    #[erp_methods]
    impl Carton<MultipleIds> {
        /// A carton holds at most 100 kg.
        #[erp(check = ["parcels.weight"])]
        pub fn check_load(&self, env: &mut Environment) -> Result<()> {
            for carton in self {
                LOADS_CHECKED.set(LOADS_CHECKED.get() + 1);
                let parcels: Parcel<MultipleIds> = carton.get_parcels(env)?;
                if parcels.sum(env, |parcel, env| Ok(*parcel.get_weight(env)?))? > 100 {
                    return Err("A carton holds at most 100 kg".into());
                }
            }
            Ok(())
        }
    }

    #[derive(Model)]
    #[erp(id = "truck", methods)]
    #[allow(dead_code)]
    pub struct Truck<Mode: IdMode> {
        pub id: Mode,
        name: String,
        #[erp(inverse = "truck")]
        cartons: Reference<BaseCarton, MultipleIds>,
    }

    #[erp_methods]
    impl Truck<MultipleIds> {
        /// A truck carries at most 150 kg, the parcels of all its cartons.
        #[erp(check = ["cartons.parcels.weight"])]
        pub fn check_load(&self, env: &mut Environment) -> Result<()> {
            for truck in self {
                let cartons: Carton<MultipleIds> = truck.get_cartons(env)?;
                let parcels: Parcel<MultipleIds> = cartons.get_parcels(env)?;
                if parcels.sum(env, |parcel, env| Ok(*parcel.get_weight(env)?))? > 150 {
                    return Err("A truck carries at most 150 kg".into());
                }
            }
            Ok(())
        }
    }

    #[erp_methods]
    impl Parcel<MultipleIds> {
        /// A parcel weighs something.
        #[erp(check = ["weight"])]
        pub fn check_weight(&self, env: &mut Environment) -> Result<()> {
            for parcel in self {
                if *parcel.get_weight(env)? <= 0 {
                    return Err("A parcel weighs something".into());
                }
            }
            Ok(())
        }

        /// Stamps are counted only when they change.
        #[erp(check = ["stamps"])]
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
        /// Named like a check, without being one: never run on its own.
        pub fn check_label(&self, _env: &mut Environment) -> Result<()> {
            Err("never run on its own".into())
        }

        /// A parcel has a name, checked one parcel at a time.
        #[erp(check = ["name"])]
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
    app.model_manager.register_model::<models::Carton<_>>();
    app.model_manager.register_model::<models::Truck<_>>();
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

/// A record breaking a check is refused; in a savepoint, nothing of the creation is kept.
#[test]
fn test_a_check_refuses_a_creation() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let error = env
        .savepoint(|env| env.create_records("parcel", vec![parcel("Books", 2), parcel("Air", 0)]))
        .expect_err("refused");
    assert_eq!(error.to_string(), "A parcel weighs something");
    assert_eq!(count(&mut env)?, 0);

    let error = env
        .savepoint(|env| env.create_records("parcel", vec![parcel(" ", 1)]))
        .expect_err("a check on one record runs for each");
    assert_eq!(error.to_string(), "A parcel has a name");
    Ok(())
}

/// A write breaking a check is refused, a setter's as any other; in a savepoint, it is undone.
#[test]
fn test_a_check_refuses_a_write() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let ids = env.create_records("parcel", vec![parcel("Books", 2)])?;
    let books: Parcel<MultipleIds> = Parcel::from_ids(ids.get_ids_ref().clone(), &env);
    let books = books.ensure_one()?;
    assert!(env.savepoint(|env| books.set_weight(0, env)).is_err());
    assert_eq!(*books.get_weight(&mut env)?, 2, "undone");
    Ok(())
}

/// A check naming fields runs when one of them is written, and not otherwise.
#[test]
fn test_a_check_on_fields_runs_when_they_change() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let mut values = parcel("Letters", 1);
    values.insert("stamps", 12);
    assert!(
        env.savepoint(|env| env.create_records("parcel", vec![values]))
            .is_err(),
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
    assert!(env.savepoint(|env| letters.set_stamps(11, env)).is_err());
    Ok(())
}

/// A method named like a check but not marked is not run on its own.
#[test]
fn test_only_marked_methods_are_checks() {
    let app = new_app();
    let checks: Vec<String> = app
        .model_manager
        .get_model("parcel")
        .checks
        .iter()
        .map(|check| check.method.clone())
        .collect();
    assert_eq!(checks, ["check_weight", "check_stamps", "check_name"]);
}

fn named(name: &str) -> MapOfFields {
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values
}

fn parcel_in(name: &str, weight: i32, carton_id: u32) -> MapOfFields {
    let mut values = parcel(name, weight);
    values.insert("carton", carton_id);
    values
}

/// A check naming a field of the records a relation holds runs when they change: a parcel made
/// heavier, one added, or one moved in.
#[test]
fn test_a_check_follows_a_path() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let carton_id = env
        .create_records("carton", vec![named("Kitchen")])?
        .get_ids_ref()[0];
    let ids = env.create_records(
        "parcel",
        vec![
            parcel_in("Pans", 40, carton_id),
            parcel_in("Plates", 50, carton_id),
        ],
    )?;
    let pans =
        Parcel::<MultipleIds>::from_ids(ids.get_ids_ref()[..1].to_vec(), &env).ensure_one()?;

    let error = env
        .savepoint(|env| pans.set_weight(60, env))
        .expect_err("110 kg in the carton");
    assert_eq!(error.to_string(), "A carton holds at most 100 kg");
    assert_eq!(*pans.get_weight(&mut env)?, 40, "undone");

    let error = env
        .savepoint(|env| env.create_records("parcel", vec![parcel_in("Glasses", 20, carton_id)]))
        .expect_err("110 kg once added");
    assert_eq!(error.to_string(), "A carton holds at most 100 kg");

    let loose = env
        .create_records("parcel", vec![parcel("Books", 30)])?
        .get_ids_ref()[0];
    let loose = Parcel::<MultipleIds>::from_ids(vec![loose], &env).ensure_one()?;
    let carton = models::Carton::<SingleId>::from_id(carton_id, &env);
    let error = env
        .savepoint(|env| loose.set_carton(&carton, env))
        .expect_err("120 kg once moved in");
    assert_eq!(error.to_string(), "A carton holds at most 100 kg");
    Ok(())
}

/// A path may cross several relations: the parcels of a truck's cartons.
#[test]
fn test_a_check_follows_a_longer_path() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let truck = env
        .create_records("truck", vec![named("Van")])?
        .get_ids_ref()[0];
    let mut cartons = Vec::new();
    for name in ["Kitchen", "Garage"] {
        let mut values = named(name);
        values.insert("truck", truck);
        cartons.push(env.create_records("carton", vec![values])?.get_ids_ref()[0]);
    }
    env.create_records("parcel", vec![parcel_in("Pans", 80, cartons[0])])?;
    let ids = env.create_records("parcel", vec![parcel_in("Tools", 60, cartons[1])])?;
    let tools = Parcel::<MultipleIds>::from_ids(ids.get_ids_ref().clone(), &env).ensure_one()?;
    let error = tools
        .set_weight(80, &mut env)
        .expect_err("160 kg in the truck");
    assert_eq!(error.to_string(), "A truck carries at most 150 kg");
    Ok(())
}

/// Deleting a record a check reaches through a path checks again what it led back to; a record
/// deleted itself is not checked.
#[test]
fn test_a_deletion_checks_what_it_led_to() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let carton_id = env
        .create_records("carton", vec![named("Kitchen")])?
        .get_ids_ref()[0];
    let ids = env.create_records(
        "parcel",
        vec![
            parcel_in("Pans", 40, carton_id),
            parcel_in("Plates", 50, carton_id),
        ],
    )?;
    let checked = models::LOADS_CHECKED.get();
    env.delete("parcel", &MultipleIds::from(ids.get_ids_ref()[0]))?;
    assert_eq!(
        models::LOADS_CHECKED.get(),
        checked + 1,
        "the carton is checked again"
    );

    let checked = models::LOADS_CHECKED.get();
    env.delete("carton", &MultipleIds::from(carton_id))?;
    assert_eq!(
        models::LOADS_CHECKED.get(),
        checked,
        "a deleted carton is not checked"
    );
    Ok(())
}
