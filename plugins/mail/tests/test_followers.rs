//! What is said about a record and to whom: its followers, the contact it is about, those a
//! message mentions — each user told in their inbox — and the changes followers follow.

use base::BasePlugin;
use erp::Result;
use erp::app::Application;
use erp::data;
use erp::environment::Environment;
use mail::MailPlugin;
use serde_json::{Value, json};
use web::WebPlugin;

mod deals {
    use base::models::BaseContact;
    use code_gen::Model;
    use erp::types::field::{IdMode, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "deal", contact_field = "customer")]
    #[allow(dead_code)]
    pub struct Deal<Mode: IdMode> {
        pub id: Mode,
        name: String,
        #[erp(tracking)]
        stage: Option<String>,
        customer: Reference<BaseContact, SingleId>,
    }
}

struct DealsPlugin;

impl erp::plugin::Plugin for DealsPlugin {
    fn name(&self) -> String {
        "deals".to_string()
    }

    fn init_models(&self, model_manager: &mut erp::model::ModelManager) {
        model_manager.register_model::<deals::Deal<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            r#"<erp>
                <message_subtype id="subtype_deal_stage" name="Stage changed" model="deal" field="stage"/>
                <access_rule id="access_deal">
                    <name>deal: users</name>
                    <model>deal</model>
                    <group ref="base.group_user"/>
                    <domain_read>[]</domain_read>
                    <domain_create>[]</domain_create>
                    <domain_write>[]</domain_write>
                    <domain_delete>[]</domain_delete>
                </access_rule>
            </erp>"#,
        ]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["mail".to_string()]
    }
}

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(WebPlugin {}))?;
    app.register_plugin(Box::new(MailPlugin {}))?;
    app.register_plugin(Box::new(DealsPlugin {}))?;
    app.load_plugin("deals")?;
    Ok(app)
}

fn admin(app: &Application) -> Result<Environment<'_>> {
    let admin = {
        let mut env = app.new_env_as_option(None)?;
        data::resolve(&mut env, "base.user_admin")?.expect("seeded")
    };
    app.new_env_as_option(Some(admin))
}

/// Call a method of a model as the user `env` is, its work kept.
fn call(
    app: &Application,
    uid: Option<u32>,
    model: &str,
    method: &str,
    args: Value,
) -> Result<Value> {
    let mut env = match uid {
        Some(uid) => app.new_env_as_option(Some(uid))?,
        None => admin(app)?,
    };
    let answer = env.call_rpc(model, method, &json!({"ids": [], "args": args}))?;
    env.close()?;
    Ok(answer)
}

fn create(app: &Application, model: &str, values: Value) -> Result<u32> {
    let mut env = admin(app)?;
    let ids = env.call_rpc(model, "create", &json!({ "values": values }))?;
    env.close()?;
    Ok(ids[0].as_u64().expect("an id") as u32)
}

fn contact_of(app: &Application, user: u32) -> Result<u32> {
    let mut env = admin(app)?;
    let rows = env.call_rpc(
        "users",
        "read",
        &json!({"ids": [user], "fields": ["contact"]}),
    )?;
    Ok(rows[0]["contact"].as_u64().expect("a contact") as u32)
}

fn ids(list: &Value) -> Vec<u64> {
    list.as_array()
        .expect("a list")
        .iter()
        .map(|pair| pair[0].as_u64().expect("an id"))
        .collect()
}

#[test]
fn test_followers_mentions_and_the_inbox() -> Result<()> {
    let app = new_app()?;
    let acme = create(&app, "contact", json!({"name": "Acme"}))?;
    let claire = create(
        &app,
        "users",
        json!({"name": "Claire", "login": "claire", "groups": [1]}),
    )?;
    let claire_contact = contact_of(&app, claire)?;
    let deal = create(
        &app,
        "deal",
        json!({"name": "Big one", "customer": acme, "stage": "new"}),
    )?;

    let info = call(
        &app,
        None,
        "follower",
        "of",
        json!({"model": "deal", "record": deal}),
    )?;
    assert_eq!(
        info["following"], true,
        "whoever creates a record follows it"
    );
    assert_eq!(info["contact"][1], "Acme");
    let subtypes: Vec<&str> = info["subtypes"]
        .as_array()
        .expect("subtypes")
        .iter()
        .map(|subtype| subtype["name"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(subtypes, ["Discussions", "Stage changed"]);
    assert_eq!(
        info["followers"][0]["subtypes"]
            .as_array()
            .map_or(0, Vec::len),
        2
    );

    let posted = call(
        &app,
        None,
        "message",
        "post",
        json!({"model": "deal", "record": deal, "body": "Hello @Claire", "internal": false,
               "mentions": [claire_contact]}),
    )?;
    assert_eq!(posted["kind"], "comment");
    assert_eq!(posted["subtype"], "Discussions");
    let mut recipients = ids(&posted["recipients"]);
    recipients.sort_unstable();
    let mut expected = vec![u64::from(acme), u64::from(claire_contact)];
    expected.sort_unstable();
    assert_eq!(
        recipients, expected,
        "the contact it is about and the one mentioned"
    );

    assert_eq!(
        call(&app, Some(claire), "notification", "unread", json!({}))?,
        1
    );
    let inbox = call(&app, Some(claire), "notification", "inbox", json!({}))?;
    assert_eq!(inbox[0]["reason"], "mention");
    assert_eq!(inbox[0]["record_name"], "Big one");
    assert_eq!(inbox[0]["message"]["body"], "Hello @Claire");
    call(
        &app,
        Some(claire),
        "notification",
        "mark_read",
        json!({"notifications": []}),
    )?;
    assert_eq!(
        call(&app, Some(claire), "notification", "unread", json!({}))?,
        0
    );

    call(
        &app,
        None,
        "follower",
        "add",
        json!({"model": "deal", "record": deal, "contacts": [claire_contact]}),
    )?;
    let mut env = admin(&app)?;
    env.call_rpc(
        "deal",
        "write",
        &json!({"ids": [deal], "values": {"stage": "won"}}),
    )?;
    env.close()?;
    let inbox = call(&app, Some(claire), "notification", "inbox", json!({}))?;
    assert_eq!(inbox[0]["reason"], "follower");
    assert_eq!(inbox[0]["message"]["kind"], "tracking");
    assert_eq!(inbox[0]["message"]["subtype"], "Stage changed");

    let info = call(
        &app,
        None,
        "follower",
        "of",
        json!({"model": "deal", "record": deal}),
    )?;
    let hers = info["followers"]
        .as_array()
        .expect("followers")
        .iter()
        .find(|follower| follower["contact"][0] == claire_contact)
        .expect("Claire follows")
        .clone();
    let discussion = info["subtypes"][0]["id"].clone();
    call(
        &app,
        Some(claire),
        "follower",
        "subscribe",
        json!({"follower": hers["id"], "subtypes": [discussion]}),
    )?;
    let mut env = admin(&app)?;
    env.call_rpc(
        "deal",
        "write",
        &json!({"ids": [deal], "values": {"stage": "lost"}}),
    )?;
    env.close()?;
    let before = call(&app, Some(claire), "notification", "inbox", json!({}))?;
    assert_eq!(
        before.as_array().map_or(0, Vec::len),
        2,
        "no longer told of stages"
    );

    let note = call(
        &app,
        None,
        "message",
        "post",
        json!({"model": "deal", "record": deal, "body": "Between us", "internal": true,
               "mentions": []}),
    )?;
    assert_eq!(note["kind"], "note");
    assert_eq!(note["recipients"], json!([]), "a note is said to nobody");
    let after = call(&app, Some(claire), "notification", "inbox", json!({}))?;
    assert_eq!(
        after.as_array().map_or(0, Vec::len),
        2,
        "nor told to followers"
    );

    call(
        &app,
        Some(claire),
        "follower",
        "forget",
        json!({"follower": hers["id"]}),
    )?;
    let info = call(
        &app,
        None,
        "follower",
        "of",
        json!({"model": "deal", "record": deal}),
    )?;
    assert_eq!(info["followers"].as_array().map_or(0, Vec::len), 1);
    Ok(())
}

/// The thread is read a page at a time, newest first, and narrowed to some kinds of messages.
#[test]
fn test_the_thread_by_pages_and_kinds() -> Result<()> {
    let app = new_app()?;
    let deal = create(&app, "deal", json!({"name": "Paged"}))?;
    for (body, internal) in [("one", false), ("two", true), ("three", false)] {
        call(
            &app,
            None,
            "message",
            "post",
            json!({"model": "deal", "record": deal, "body": body, "internal": internal, "mentions": []}),
        )?;
    }
    let page = |args: Value| -> Result<Vec<String>> {
        let mut args = args;
        args["model"] = json!("deal");
        args["record"] = json!(deal);
        let found = call(&app, None, "message", "thread", args)?;
        Ok(found
            .as_array()
            .expect("a thread")
            .iter()
            .map(|message| {
                message["body"]
                    .as_str()
                    .unwrap_or(message["kind"].as_str().unwrap_or_default())
                    .to_string()
            })
            .collect())
    };
    assert_eq!(page(json!({"limit": 2}))?, ["three", "two"]);
    assert_eq!(page(json!({"offset": 2, "limit": 2}))?, ["one", "creation"]);
    assert_eq!(page(json!({"kinds": ["note"]}))?, ["two"]);
    assert_eq!(page(json!({"kinds": ["comment"]}))?, ["three", "one"]);
    Ok(())
}

/// A message said to people outside the application is queued as a mail to each, at their
/// address — failed at once for one without — while a user mentioned only finds it in their inbox.
#[test]
fn test_messages_are_queued_as_mails_to_those_outside() -> Result<()> {
    let app = new_app()?;
    let acme = create(
        &app,
        "contact",
        json!({"name": "Acme", "email": "hello@acme.test"}),
    )?;
    let nobody = create(&app, "contact", json!({"name": "No Address"}))?;
    let claire = create(
        &app,
        "users",
        json!({"name": "Claire", "login": "claire", "groups": [1]}),
    )?;
    let claire_contact = contact_of(&app, claire)?;
    let deal = create(&app, "deal", json!({"name": "Mailed", "customer": acme}))?;
    let posted = call(
        &app,
        None,
        "message",
        "post",
        json!({"model": "deal", "record": deal, "body": "News", "internal": false,
               "mentions": [nobody, claire_contact]}),
    )?;
    let mut mails: Vec<Value> = posted["mails"].as_array().expect("mails").clone();
    mails.sort_by_key(|mail| mail["recipient"].as_str().unwrap_or_default().to_string());
    assert_eq!(
        mails,
        [
            json!({"recipient": "Acme", "email": "hello@acme.test", "state": "outgoing", "error": null}),
            json!({"recipient": "No Address", "email": null, "state": "failed",
                   "error": "No address to send it to"}),
        ]
    );
    let note = call(
        &app,
        None,
        "message",
        "post",
        json!({"model": "deal", "record": deal, "body": "Quiet", "internal": true, "mentions": [nobody]}),
    )?;
    assert_eq!(note["mails"], json!([]), "a note is mailed to nobody");
    Ok(())
}
