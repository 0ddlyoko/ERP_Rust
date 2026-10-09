use code_gen::Model;
use erp::types::field::IdMode;

/// What of a record's thread a follower may follow: its discussions — every model's — or the
/// changes of one of its tracked fields — a task's stage, an order's status.
///
/// Declared in the data of the plugin bringing the field, `model` and `field` naming it: renaming
/// the field is a change of that data. One that is `default` is followed by whoever starts to
/// follow a record of its model.
#[derive(Model)]
#[erp(id = "message_subtype", order = "sequence, id")]
#[allow(dead_code)]
pub struct MessageSubtype<Mode: IdMode> {
    pub id: Mode,
    name: String,
    model: Option<String>,
    field: Option<String>,
    #[erp(default = true)]
    default: bool,
    #[erp(default = 10)]
    sequence: i32,
}
