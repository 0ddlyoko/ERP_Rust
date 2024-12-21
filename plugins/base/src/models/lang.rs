use code_gen::Model;

#[derive(Model)]
#[erp(table_name="lang")]
pub struct Lang {
    id: u32,
    name: String,
    code: String,
}
