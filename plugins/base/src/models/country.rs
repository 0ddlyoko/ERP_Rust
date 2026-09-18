use code_gen::Model;
use erp::types::field::IdMode;

#[derive(Model)]
#[erp(id = "country")]
#[allow(dead_code)]
pub struct Country<Mode: IdMode> {
    id: Mode,
    name: String,
    code: String,
}
