use code_gen::Model;
use erp::field::IdMode;

#[derive(Model)]
#[erp(table_name="country")]
#[allow(dead_code)]
pub struct Country<Mode: IdMode> {
    id: Mode,
    name: String,
    code: String,
}
