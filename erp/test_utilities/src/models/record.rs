use code_gen::Model;
use erp::types::field::IdMode;

/// Named after the structural element on purpose.
///
/// It pins down that `<record>` with no `model` attribute reaches the model called `record`, and
/// not the long form — the same rule fields follow with `<field>`.
#[derive(Model)]
#[erp(id = "record")]
#[allow(dead_code)]
pub struct Record<Mode: IdMode> {
    pub id: Mode,
    name: String,
}
