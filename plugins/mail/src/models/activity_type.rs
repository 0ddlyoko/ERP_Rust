use code_gen::Model;
use erp::types::field::IdMode;

/// A kind of thing to do about a record — a call, a meeting — with the icon it is shown with and
/// how many days ahead one is planned by default. Declared in data, as subtypes are.
#[derive(Model)]
#[erp(id = "activity_type", order = "sequence, id")]
#[allow(dead_code)]
pub struct ActivityType<Mode: IdMode> {
    pub id: Mode,
    name: String,
    icon: Option<String>,
    #[erp(default = 0)]
    delay: i32,
    #[erp(default = 10)]
    sequence: i32,
}
