use erp::app::Application;
use erp_internal_types::{FinalInternalField, InternalField};
use erp_types::field::{FieldKind, FieldType};
use std::error::Error;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

/// Each declared Rust type lands on the right kind, including through `Option` and `Reference`.
#[test]
fn test_kind_is_derived_from_the_declared_type() {
    let app = new_app();
    let invoice = app.model_manager.get_model("invoice");

    assert_eq!(invoice.get_internal_field("name").kind, FieldKind::String);
    assert_eq!(
        invoice.get_internal_field("amount_untaxed").kind,
        FieldKind::Decimal
    );
    assert_eq!(invoice.get_internal_field("due_date").kind, FieldKind::Date);
    assert_eq!(
        invoice.get_internal_field("created_at").kind,
        FieldKind::DateTime
    );
    assert_eq!(
        invoice.get_internal_field("signed_on").kind,
        FieldKind::Date,
        "Option<T> keeps the kind of T"
    );

    let order = app.model_manager.get_model("sale_order");
    assert_eq!(
        order.get_internal_field("total_price").kind,
        FieldKind::Integer
    );
    assert_eq!(
        order.get_internal_field("state").kind,
        FieldKind::String,
        "an enum is stored as text"
    );
    assert_eq!(
        order.get_internal_field("lines").kind,
        FieldKind::Refs,
        "one2many"
    );

    let line = app.model_manager.get_model("sale_order_line");
    assert_eq!(
        line.get_internal_field("order").kind,
        FieldKind::Ref,
        "many2one"
    );
}

/// Storage is decided by the kind, not by whatever default the field happens to carry.
#[test]
fn test_is_stored_follows_the_kind() {
    let app = new_app();
    let order = app.model_manager.get_model("sale_order");
    let line = app.model_manager.get_model("sale_order_line");

    assert!(!order.is_stored("lines"), "a one2many has no column");
    assert!(line.is_stored("order"), "a many2one holds the foreign key");
    assert!(order.is_stored("total_price"));
    assert!(FieldKind::Refs.is_relational() && FieldKind::Ref.is_relational());
    assert!(!FieldKind::String.is_relational());
}

/// A relation no longer carries a sentinel default.
#[test]
fn test_relations_have_no_default() {
    let app = new_app();
    let line = app.model_manager.get_model("sale_order_line");
    let order = app.model_manager.get_model("sale_order");

    assert_eq!(
        line.get_default_value("order"),
        None,
        "a many2one starts empty, not at id 0"
    );
    assert_eq!(order.get_default_value("lines"), None);
}

/// Declaring a field `Option<T>` is what makes it start empty; a bare `T` keeps its default.
#[test]
fn test_default_follows_optionality() -> Result<()> {
    let app = new_app();
    let invoice = app.model_manager.get_model("invoice");

    assert_eq!(invoice.get_default_value("signed_on"), None);
    assert!(invoice.get_default_value("due_date").is_some());
    assert_eq!(
        invoice.get_default_value("tax_rate"),
        Some(FieldType::Decimal("0.21".parse()?))
    );
    Ok(())
}

/// Two structs contributing to the same field must agree on its type.
#[test]
#[should_panic(expected = "declared as")]
fn test_conflicting_kinds_are_rejected() {
    let mut field = FinalInternalField::new("amount");

    field.register_internal_field(&InternalField {
        name: "amount".to_string(),
        kind: FieldKind::Integer,
        default_value: None,
        label: None,
        description: None,
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
    });
    field.register_internal_field(&InternalField {
        name: "amount".to_string(),
        kind: FieldKind::Decimal,
        default_value: None,
        label: None,
        description: None,
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
    });
}

/// An extension may add a default to a field that had none, without redeclaring its type.
#[test]
fn test_an_extension_can_supply_a_default() {
    let mut field = FinalInternalField::new("label");

    field.register_internal_field(&InternalField {
        name: "label".to_string(),
        kind: FieldKind::String,
        default_value: None,
        label: None,
        description: None,
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
    });
    assert_eq!(field.default_value, None);

    field.register_internal_field(&InternalField {
        name: "label".to_string(),
        kind: FieldKind::String,
        default_value: Some(FieldType::String("draft".to_string())),
        label: None,
        description: None,
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
    });
    assert_eq!(
        field.default_value,
        Some(FieldType::String("draft".to_string()))
    );
}
