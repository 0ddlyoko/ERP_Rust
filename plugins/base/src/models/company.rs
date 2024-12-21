use code_gen::Model;

#[derive(Model)]
#[erp(table_name="company")]
pub struct Company {
    id: u32,
    name: String,
    // TODO Link to contact
}