//! The order a model's records come in when nobody asks for one: searched, or as the lines of a
//! one2many.

use erp::app::Application;
use erp::environment::Environment;
use erp::search::{OrderBy, SearchOptions, SearchType};
use erp_types::field::{FieldType, IdMode, MultipleIds};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod music {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    #[derive(Model)]
    #[erp(id = "album", order = "year desc, id")]
    #[allow(dead_code)]
    pub struct Album<Mode: IdMode> {
        pub id: Mode,
        year: i32,
        #[erp(inverse = "album")]
        tracks: Reference<BaseTrack, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "track", order = "position, id")]
    #[allow(dead_code)]
    pub struct Track<Mode: IdMode> {
        pub id: Mode,
        position: i32,
        #[erp(ondelete = "cascade")]
        album: Reference<BaseAlbum, SingleId>,
    }

    /// Albums come oldest first, once this extension is loaded.
    #[derive(Model)]
    #[erp(id = "album", order = "year, id")]
    #[erp(derived_model = "")]
    #[allow(dead_code)]
    pub struct AlbumOldestFirst<Mode: IdMode> {
        pub id: Mode,
    }

    /// Its songs come by a rank worked out and kept — worked out from the setlist's songs
    /// themselves, which reading them sorted must not compute again.
    #[derive(Model)]
    #[erp(id = "setlist")]
    #[allow(dead_code)]
    pub struct Setlist<Mode: IdMode> {
        pub id: Mode,
        #[erp(inverse = "setlist")]
        songs: Reference<BaseSong, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "song", order = "rank desc, id", methods)]
    #[allow(dead_code)]
    pub struct Song<Mode: IdMode> {
        pub id: Mode,
        minutes: i32,
        #[erp(ondelete = "cascade")]
        setlist: Reference<BaseSetlist, SingleId>,
        #[erp(compute = "compute_rank", depends = ["minutes", "setlist.songs"], stored)]
        rank: i32,
    }

    #[erp_methods]
    impl Song<MultipleIds> {
        pub fn compute_rank(&self, env: &mut Environment) -> Result<()> {
            for song in self {
                let setlist: Setlist<SingleId> = song.get_setlist(env)?;
                let songs: Song<MultipleIds> = setlist.get_songs(env)?;
                let rank = *song.get_minutes(env)? * 100 + songs.get_ids_ref().len() as i32;
                song.set_rank(rank, env)?;
            }
            Ok(())
        }
    }

    #[derive(Model)]
    #[erp(id = "unsorted", order = "rank desc")]
    #[allow(dead_code)]
    pub struct Unsorted<Mode: IdMode> {
        pub id: Mode,
        year: i32,
    }
}

fn new_app(oldest_first: bool) -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<music::Album<_>>();
    app.model_manager.register_model::<music::Track<_>>();
    app.model_manager.register_model::<music::Setlist<_>>();
    app.model_manager.register_model::<music::Song<_>>();
    if oldest_first {
        app.model_manager
            .register_model::<music::AlbumOldestFirst<_>>();
    }
    app.model_manager.post_register();
    app
}

fn create(env: &mut Environment, model: &str, values: MapOfFields) -> Result<u32> {
    Ok(env.create_records(model, vec![values])?.get_ids_ref()[0])
}

fn album(env: &mut Environment, year: i32) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("year", year);
    create(env, "album", values)
}

fn track(env: &mut Environment, album: u32, position: i32) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("album", album);
    values.insert("position", position);
    create(env, "track", values)
}

fn albums(app: &Application) -> Result<(u32, u32, u32)> {
    let mut env = app.new_env()?;
    let made = (
        album(&mut env, 2010)?,
        album(&mut env, 2024)?,
        album(&mut env, 2017)?,
    );
    env.close()?;
    Ok(made)
}

/// Searched without an order, records come in their model's; one asked for comes first.
#[test]
fn test_a_search_follows_the_model_order_unless_asked_another() -> Result<()> {
    let app = new_app(false);
    let (old, new, middle) = albums(&app)?;
    let mut env = app.new_env()?;
    assert_eq!(
        env.search_ids("album", &SearchType::Nothing)?,
        vec![new, middle, old],
        "the newest first, as the model says"
    );
    let by_id = SearchOptions::new().order_by(OrderBy::asc("id"));
    assert_eq!(
        env.search_ids_with("album", &SearchType::Nothing, &by_id)?,
        vec![old, new, middle],
        "the order asked for"
    );
    Ok(())
}

/// An extension of the model may order its records otherwise.
#[test]
fn test_an_extension_changes_the_model_order() -> Result<()> {
    let app = new_app(true);
    let (old, new, middle) = albums(&app)?;
    let mut env = app.new_env()?;
    assert_eq!(
        env.search_ids("album", &SearchType::Nothing)?,
        vec![old, middle, new]
    );
    Ok(())
}

/// The lines of a one2many come in the order of their model, not in the order they were made.
#[test]
fn test_one2many_lines_follow_their_model_order() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let album = album(&mut env, 2020)?;
    let third = track(&mut env, album, 3)?;
    let first = track(&mut env, album, 1)?;
    let second = track(&mut env, album, 2)?;
    env.close()?;

    let mut env = app.new_env()?;
    let rows = env.read("album", &MultipleIds::from(vec![album]), &["tracks"])?;
    assert_eq!(
        rows[0].fields.get("tracks"),
        Some(&Some(FieldType::Refs(vec![first, second, third])))
    );
    Ok(())
}

/// Lines may come by a field worked out and kept, even one worked out from these very lines.
#[test]
fn test_one2many_lines_follow_a_computed_order() -> Result<()> {
    let app = new_app(false);
    let mut env = app.new_env()?;
    let setlist = create(&mut env, "setlist", MapOfFields::default())?;
    let song = |env: &mut Environment, minutes: i32| -> Result<u32> {
        let mut values = MapOfFields::default();
        values.insert("setlist", setlist);
        values.insert("minutes", minutes);
        create(env, "song", values)
    };
    let short = song(&mut env, 3)?;
    let long = song(&mut env, 5)?;
    let middle = song(&mut env, 4)?;
    env.close()?;

    let mut env = app.new_env()?;
    let rows = env.read("setlist", &MultipleIds::from(vec![setlist]), &["songs"])?;
    assert_eq!(
        rows[0].fields.get("songs"),
        Some(&Some(FieldType::Refs(vec![long, middle, short])))
    );
    Ok(())
}

/// An order on a field the model does not keep is refused when the model is registered.
#[test]
#[should_panic(expected = "is ordered by \"rank\", which is none of its fields kept in a column")]
fn test_an_order_on_a_field_the_model_lacks_is_refused() {
    let mut app = Application::new_test();
    app.model_manager.register_model::<music::Unsorted<_>>();
    app.model_manager.post_register();
}
