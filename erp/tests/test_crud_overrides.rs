//! `create`, `write` and `delete` overridden on a model: by the struct declaring it, and by an
//! extension, and reached whichever way records are created, written or deleted.

use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use serde_json::json;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod notes {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
    use erp::types::model::MapOfFields;

    use erp::Result;

    #[derive(Model)]
    #[erp(id = "shelf")]
    #[allow(dead_code)]
    pub struct Shelf<Mode: IdMode> {
        pub id: Mode,
        #[erp(inverse = "shelf")]
        notes: Reference<BaseNote, MultipleIds>,
    }

    /// Overrides its own `create`, `write` and `delete`, in the struct declaring the model.
    #[derive(Model)]
    #[erp(id = "note", methods)]
    #[allow(dead_code)]
    pub struct Note<Mode: IdMode> {
        pub id: Mode,
        name: String,
        #[erp(ondelete = "cascade")]
        shelf: Reference<BaseShelf, SingleId>,
        #[erp(compute = "compute_length", depends = ["name"], stored)]
        length: i32,
    }

    #[erp_methods]
    impl Note<MultipleIds> {
        /// Names are kept in capitals.
        pub fn create(
            &self,
            env: &mut Environment,
            values: Vec<MapOfFields>,
            sup: Super,
        ) -> Result<MultipleIds> {
            let values: Vec<MapOfFields> = values.into_iter().map(shouted).collect();
            sup.call_with(values, env)
        }

        /// So are names written; the length is the compute's alone to write.
        pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
            if values.fields.contains_key("length") {
                return Err("the length is computed".into());
            }
            sup.call_with(shouted(values), env)
        }

        pub fn compute_length(&self, env: &mut Environment) -> Result<()> {
            for note in self {
                let length = i32::try_from(note.get_name(env)?.len())?;
                note.set_length(length, env)?;
            }
            Ok(())
        }

        /// A note named KEEP is never deleted.
        pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
            for note in self {
                if note.get_name(env)? == "KEEP" {
                    return Err("this note is kept".into());
                }
            }
            sup.call(env)
        }
    }

    fn shouted(mut values: MapOfFields) -> MapOfFields {
        if let Some(name) = values
            .get_option::<&String>("name")
            .map(|name| name.to_uppercase())
        {
            values.insert("name", name);
        }
        values
    }

    /// Extends the model and overrides `create` again: it runs first, and reaches the other
    /// through `sup`.
    #[derive(Model)]
    #[erp(id = "note", methods)]
    #[erp(derived_model = "")]
    #[allow(dead_code)]
    pub struct NoteSigned<Mode: IdMode> {
        pub id: Mode,
    }

    #[erp_methods]
    impl NoteSigned<MultipleIds> {
        /// Signs each note, then counts what was created into its name.
        pub fn create(
            &self,
            env: &mut Environment,
            values: Vec<MapOfFields>,
            sup: Super,
        ) -> Result<MultipleIds> {
            let values: Vec<MapOfFields> = values
                .into_iter()
                .map(|mut values| {
                    let name = values
                        .get_option::<&String>("name")
                        .cloned()
                        .unwrap_or_default();
                    values.insert("name", format!("{name} signed"));
                    values
                })
                .collect();
            let created: MultipleIds = sup.call_with(values, env)?;
            let count = created.get_ids_ref().len();
            for note in Note::<MultipleIds>::from_ids(created.clone(), env) {
                let name = note.get_name(env)?.clone();
                note.set_name(format!("{name} ({count})"), env)?;
            }
            Ok(created)
        }
    }
}

/// Its `create` never reaches the ORM: it makes no record.
mod ghosts {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds};
    use erp::types::model::MapOfFields;
    use std::error::Error;

    #[derive(Model)]
    #[erp(id = "ghost", methods)]
    #[allow(dead_code)]
    pub struct Ghost<Mode: IdMode> {
        pub id: Mode,
        name: Option<String>,
    }

    #[erp_methods]
    impl Ghost<MultipleIds> {
        pub fn create(
            &self,
            env: &mut Environment,
            values: Vec<MapOfFields>,
        ) -> Result<MultipleIds, Box<dyn Error + Send + Sync>> {
            let _ = (env, values);
            Ok(MultipleIds::default())
        }
    }
}

fn new_app(signed: bool) -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<notes::Shelf<_>>();
    app.model_manager.register_model::<notes::Note<_>>();
    app.model_manager.register_model::<ghosts::Ghost<_>>();
    if signed {
        app.model_manager.register_model::<notes::NoteSigned<_>>();
    }
    app.model_manager.post_register();
    app
}

fn note(env: &mut Environment, name: &str) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("name", name);
    Ok(env.create_records("note", vec![values])?.get_ids_ref()[0])
}

fn name_of(env: &mut Environment, id: u32) -> Result<String> {
    let rows = env.read("note", &MultipleIds::from(vec![id]), &["name"])?;
    Ok(rows[0].get::<&String>("name").clone())
}

/// What the declaring struct overrode runs, before the ORM's own work.
#[test]
fn test_the_declaring_struct_overrides() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let id = note(&mut env, "hello")?;
    assert_eq!(name_of(&mut env, id)?, "HELLO");

    let mut values = MapOfFields::default();
    values.insert("name", "again");
    env.write("note", &MultipleIds::from(vec![id]), values)?;
    assert_eq!(name_of(&mut env, id)?, "AGAIN");
    Ok(())
}

/// An extension's override runs first and reaches the one below it, which reaches the ORM; what
/// it does after the records exist is kept.
#[test]
fn test_an_extension_overrides_on_top() -> Result<()> {
    let app = new_app(true);
    let mut env = app.new_env()?;
    let id = note(&mut env, "hello")?;
    assert_eq!(name_of(&mut env, id)?, "HELLO SIGNED (1)");
    Ok(())
}

/// A remote call reaches the overrides like a call in process.
#[test]
fn test_a_remote_call_goes_through_the_overrides() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let ids = env.call_rpc("note", "create", &json!({"values": {"name": "remote"}}))?;
    let id = ids[0].as_u64().unwrap() as u32;
    assert_eq!(name_of(&mut env, id)?, "REMOTE");
    env.call_rpc(
        "note",
        "write",
        &json!({"ids": [id], "values": {"name": "written"}}),
    )?;
    assert_eq!(name_of(&mut env, id)?, "WRITTEN");
    Ok(())
}

/// Lines a one2many creates through its commands go through the lines' `create`.
#[test]
fn test_commands_go_through_the_overrides() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let shelf = env.call_rpc(
        "shelf",
        "create",
        &json!({"values": {"notes": {"create": [{"name": "inline"}]}}}),
    )?;
    let read = env.call_rpc("shelf", "read", &json!({"ids": shelf, "fields": ["notes"]}))?;
    let id = read[0]["notes"][0].as_u64().unwrap() as u32;
    assert_eq!(name_of(&mut env, id)?, "INLINE");
    Ok(())
}

/// `delete` can refuse, for records deleted directly and for those a cascade deletes.
#[test]
fn test_delete_can_refuse_even_in_a_cascade() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let gone = note(&mut env, "gone")?;
    assert_eq!(env.delete("note", &MultipleIds::from(vec![gone]))?, 1);

    let kept = note(&mut env, "keep")?;
    let refused = env.delete("note", &MultipleIds::from(vec![kept]));
    assert_eq!(refused.expect_err("kept").to_string(), "this note is kept");
    drop(env);

    let mut env = app.new_env()?;
    let shelf = env.call_rpc(
        "shelf",
        "create",
        &json!({"values": {"notes": {"create": [{"name": "keep"}]}}}),
    )?;
    let refused = env.call_rpc("shelf", "delete", &json!({"ids": shelf}));
    assert_eq!(refused.expect_err("kept").to_string(), "this note is kept");
    Ok(())
}

/// Creating, writing and deleting one record goes through the same overrides as a recordset, and
/// so does a setter; a compute filling in its field does not.
#[test]
fn test_one_record_and_setters_go_through_the_overrides() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let mut values = MapOfFields::default();
    values.insert("name", "single");
    let note = notes::Note::<SingleId>::create(values, &mut env)?;
    assert_eq!(note.get_name(&mut env)?, "SINGLE");
    assert_eq!(*note.get_length(&mut env)?, 6, "the compute is not refused");

    let mut values = MapOfFields::default();
    values.insert("name", "written");
    note.write(values, &mut env)?;
    assert_eq!(note.get_name(&mut env)?, "WRITTEN");

    note.set_name("set".to_string(), &mut env)?;
    assert_eq!(note.get_name(&mut env)?, "SET");
    assert_eq!(*note.get_length(&mut env)?, 3);

    let refused = note.set_length(9, &mut env);
    assert_eq!(
        refused.expect_err("refused").to_string(),
        "the length is computed"
    );

    note.set_name("keep".to_string(), &mut env)?;
    let refused = note.delete(&mut env);
    assert_eq!(refused.expect_err("kept").to_string(), "this note is kept");
    Ok(())
}

/// Creating one record whose `create` makes none gives an empty record.
#[test]
fn test_a_create_making_no_record_gives_an_empty_one() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let ghost = ghosts::Ghost::<SingleId>::create(MapOfFields::default(), &mut env)?;
    assert!(ghost.is_empty());
    Ok(())
}
