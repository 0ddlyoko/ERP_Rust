use code_gen::Model;
use erp::types::field::IdMode;

#[derive(Model)]
#[erp(id = "lang")]
#[allow(dead_code)]
pub struct Lang<Mode: IdMode> {
    id: Mode,
    name: String,
    code: String,
}
