use code_gen::Model;

#[derive(Model)]
#[erp(table_name="country")]
pub struct Country {
    id: u32,
    name: String,
    code: String,
}
