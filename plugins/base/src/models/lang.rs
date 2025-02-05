use code_gen::Model;
use erp::field::IdMode;

#[derive(Model)]
#[erp(table_name="lang")]
pub struct Lang<Mode: IdMode> {
    id: Mode,
    name: String,
    code: String,
}
