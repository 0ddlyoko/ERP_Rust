use base::models::{Group, Users};
use base::{BasePlugin, DEFAULT_ADMIN_PASSWORD};
use erp::app::Application;
use erp_search_code_gen::make_domain;
use erp_types::field::{IdMode, MultipleIds, Password, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("base")?;
    Ok(app)
}

/// Log in, keeping only the id so the assertions stay readable.
fn login(
    env: &mut erp::environment::Environment,
    login: &str,
    password: &str,
) -> Result<Option<u32>> {
    Ok(Users::identified_by(env, login, password)?.map(|user| user.get_id()))
}

fn create_user(
    env: &mut erp::environment::Environment,
    login: &str,
    password: &str,
    active: bool,
) -> Result<u32> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("login", login);
    map.insert("password", Password::new(password)?);
    map.insert("active", active);
    let ids: MultipleIds = env.create_records("users", vec![map])?;
    Ok(*ids.get_ids_ref().first().unwrap())
}

/// A clear password is never stored, and two accounts sharing one do not share a hash.
#[test]
fn test_hashes_are_salted() -> Result<()> {
    let first = Password::new("hunter2")?;
    let second = Password::new("hunter2")?;

    assert_ne!(first, second, "each hash carries its own salt");
    assert!(first.is_same_password("hunter2"));
    assert!(second.is_same_password("hunter2"));
    assert!(!first.is_same_password("hunter3"));
    Ok(())
}

/// Garbage in place of a hash is rejected rather than accepted or fatal.
#[test]
fn test_malformed_hash_never_verifies() {
    assert!(!Password::default().is_same_password("anything"));
    assert!(!Password::from_hash("not-a-hash").is_same_password("anything"));
}

#[test]
fn test_authenticate_accepts_the_right_password() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let uid = create_user(&mut env, "alice", "s3cret", true)?;
    assert_eq!(login(&mut env, "alice", "s3cret")?, Some(uid));
    Ok(())
}

#[test]
fn test_authenticate_rejects_a_wrong_password() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    create_user(&mut env, "alice", "s3cret", true)?;
    assert_eq!(login(&mut env, "alice", "wrong")?, None);
    Ok(())
}

/// An unknown login is answered exactly like a wrong password.
#[test]
fn test_authenticate_rejects_an_unknown_login() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    create_user(&mut env, "alice", "s3cret", true)?;
    assert_eq!(login(&mut env, "mallory", "s3cret")?, None);
    Ok(())
}

#[test]
fn test_inactive_account_cannot_log_in() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    create_user(&mut env, "retired", "s3cret", false)?;
    assert_eq!(login(&mut env, "retired", "s3cret")?, None);
    Ok(())
}

/// Changing a password invalidates the previous one.
#[test]
fn test_changing_a_password() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let uid = create_user(&mut env, "alice", "old", true)?;
    let user: Users<SingleId> = env.get_record(uid.into());
    user.change_password(&mut env, "new")?;

    assert_eq!(login(&mut env, "alice", "new")?, Some(uid));
    assert_eq!(login(&mut env, "alice", "old")?, None);
    Ok(())
}

/// Credentials survive a commit.
#[test]
fn test_credentials_persist() -> Result<()> {
    let app = new_app()?;

    let mut env = app.new_env_as_option(None)?;
    let uid = create_user(&mut env, "alice", "s3cret", true)?;
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    assert_eq!(login(&mut env, "alice", "s3cret")?, Some(uid));
    Ok(())
}

/// The administrator is seeded, and given a password by `post_init`.
#[test]
fn test_admin_is_seeded_and_usable() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let admin: Users<SingleId> = env.named("base.user_admin")?;
    assert_eq!(
        login(&mut env, "admin", DEFAULT_ADMIN_PASSWORD)?,
        Some(admin.get_id())
    );
    assert_eq!(login(&mut env, "admin", "nope")?, None);
    Ok(())
}

/// Groups are seeded too, and a user belongs to them through the relation.
#[test]
fn test_user_belongs_to_groups() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let admin_record: Users<SingleId> = env.named("base.user_admin")?;
    let admin = admin_record.get_id();
    let group_admin = env.named::<Group<SingleId>>("base.group_admin")?.get_id();
    let group_user = env.named::<Group<SingleId>>("base.group_user")?.get_id();

    admin_record.set_groups(vec![group_user, group_admin].into(), &mut env)?;

    let rows = env.read("group", &SingleId::from(group_admin), &["users"])?;
    assert_eq!(
        rows[0]
            .get_option::<&Vec<u32>>("users")
            .cloned()
            .unwrap_or_default(),
        vec![admin],
        "the group must see its member from the other side"
    );
    Ok(())
}

/// The environment carries who it runs for, and the portal user when nobody authenticated.
#[test]
fn test_environment_carries_the_user() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let portal: Users<SingleId> = env.named("base.user_portal")?;

    assert_eq!(
        env.uid(),
        Some(portal.get_id()),
        "a caller who authenticated as nobody is the portal user"
    );
    let uid = create_user(&mut env.sudo(), "alice", "s3cret", true)?;
    env.close()?;

    let env = app.new_env_as(uid)?;
    assert_eq!(env.uid(), Some(uid));
    Ok(())
}

/// Three accounts are seeded, and only one of them is a login.
#[test]
fn test_the_seeded_accounts() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    for name in ["base.user_root", "base.user_admin", "base.user_portal"] {
        assert!(
            env.named::<Users<SingleId>>(name).is_ok(),
            "{name} is missing"
        );
    }

    assert!(
        env.named::<Group<SingleId>>("base.user_admin").is_err(),
        "an identifier naming a user must not be read as a group"
    );
    assert!(
        env.named::<Users<SingleId>>("base.nothing_declared")
            .is_err(),
        "an identifier nobody declared names nothing"
    );

    assert!(login(&mut env, "admin", DEFAULT_ADMIN_PASSWORD)?.is_some());
    Ok(())
}

/// Neither of the two the framework acts as can be logged into.
///
/// Not by a rule that forbids it: they hold no password, and an account with none holds an empty
/// hash, which nothing verifies against — including the empty password.
#[test]
fn test_the_framework_accounts_have_no_password() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    for login_name in ["root", "portal"] {
        let account: Users<SingleId> = env.named(&format!("base.user_{login_name}"))?;
        assert!(!account.has_password(&mut env)?, "{login_name} has one");

        for attempt in ["", "root", "portal", "admin", DEFAULT_ADMIN_PASSWORD] {
            assert_eq!(
                login(&mut env, login_name, attempt)?,
                None,
                "{login_name} accepted {attempt:?}"
            );
        }
    }
    Ok(())
}

/// Acting as root is something the process does, and it says so about itself.
#[test]
fn test_acting_as_root() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let root: Users<SingleId> = env.named("base.user_root")?;
    let root = root.get_id();

    assert!(!env.is_root(), "the portal user is not root");
    {
        let acting = env.as_root()?;
        assert_eq!(acting.uid(), Some(root));
        assert!(acting.is_root());
    }
    assert!(!env.is_root(), "and it is given back");
    Ok(())
}

/// A password hash is never handed back by a plain read of the user list.
#[test]
fn test_listing_users_does_not_expose_hashes() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    create_user(&mut env, "alice", "s3cret", true)?;
    let ids = env.search_ids("users", &make_domain!([("login", "=", "alice")]))?;
    let rows = env.read("users", &MultipleIds::from(ids), &["login", "active"])?;

    assert!(
        !rows[0].contains_key("password"),
        "only the fields asked for come back"
    );
    Ok(())
}

/// Checking a password is a question you ask a user, not a free function.
#[test]
fn test_check_password_on_the_record() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let uid = create_user(&mut env, "alice", "s3cret", true)?;
    let user: Users<SingleId> = env.get_record(uid.into());

    assert!(user.check_password(&mut env, "s3cret")?);
    assert!(!user.check_password(&mut env, "wrong")?);
    assert!(user.has_password(&mut env)?);
    Ok(())
}

/// Authenticating hands back the user, not just an id.
#[test]
fn test_authenticate_returns_the_record() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    create_user(&mut env, "alice", "s3cret", true)?;
    let user = Users::identified_by(&mut env, "alice", "s3cret")?.expect("should log in");

    assert_eq!(user.get_login(&mut env)?, &"alice".to_string());
    assert!(*user.get_active(&mut env)?);
    Ok(())
}

/// A field changed through the record is saved without anything else being asked for.
///
/// The setter marks it dirty in cache; closing the environment writes it. No explicit write, no
/// explicit flush.
#[test]
fn test_a_record_change_persists_on_its_own() -> Result<()> {
    let app = new_app()?;

    let mut env = app.new_env_as_option(None)?;
    let uid = create_user(&mut env, "alice", "old", true)?;
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    let user: Users<SingleId> = env.get_record(uid.into());
    user.change_password(&mut env, "new")?;
    user.set_name("Alice".to_string(), &mut env)?;
    env.close()?;

    let mut env = app.new_env_as_option(None)?;
    assert_eq!(login(&mut env, "alice", "new")?, Some(uid));
    assert_eq!(login(&mut env, "alice", "old")?, None);
    let user: Users<SingleId> = env.get_record(uid.into());
    assert_eq!(user.get_name(&mut env)?, &"Alice".to_string());
    Ok(())
}
