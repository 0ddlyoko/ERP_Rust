use code_gen::Model;
use erp::field::IdMode;

#[derive(Model)]
#[erp(table_name="lang")]
#[allow(dead_code)]
pub struct Lang<Mode: IdMode> {
    id: Mode,
    name: String,
    code: String,
}
