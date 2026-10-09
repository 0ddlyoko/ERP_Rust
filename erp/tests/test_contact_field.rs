//! The contact a record is about — an order's customer — named by its model, and refused when it
//! is no many2one to contacts.

use erp::app::Application;

mod models {
    use code_gen::Model;
    use erp::types::field::{IdMode, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "contact")]
    #[allow(dead_code)]
    pub struct Contact<Mode: IdMode> {
        pub id: Mode,
        name: String,
    }

    #[derive(Model)]
    #[erp(id = "deal", contact_field = "customer")]
    #[allow(dead_code)]
    pub struct Deal<Mode: IdMode> {
        pub id: Mode,
        customer: Reference<BaseContact, SingleId>,
    }

    #[derive(Model)]
    #[erp(id = "misaddressed", contact_field = "name")]
    #[allow(dead_code)]
    pub struct Misaddressed<Mode: IdMode> {
        pub id: Mode,
        name: String,
    }
}

/// The field a model names is kept as the contact its records are about.
#[test]
fn test_a_model_names_the_contact_its_records_are_about() {
    let mut app = Application::new_test();
    app.model_manager.register_model::<models::Contact<_>>();
    app.model_manager.register_model::<models::Deal<_>>();
    app.model_manager.post_register();
    let deal = app.model_manager.get_model("deal");
    assert_eq!(deal.contact_field.as_deref(), Some("customer"));
    assert_eq!(app.model_manager.get_model("contact").contact_field, None);
}

/// A field that does not point to contacts is refused when the model is registered.
#[test]
#[should_panic(
    expected = "is about the contact of its field \"name\", which is no many2one to contacts"
)]
fn test_a_contact_field_not_pointing_to_contacts_is_refused() {
    let mut app = Application::new_test();
    app.model_manager.register_model::<models::Contact<_>>();
    app.model_manager
        .register_model::<models::Misaddressed<_>>();
    app.model_manager.post_register();
}
