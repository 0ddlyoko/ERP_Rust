use code_gen::Model;

#[derive(Model)]
#[erp(table_name="sale_order_test")]
pub(crate) struct SaleOrderTest {
    id: u32,
    name: String,
    age: i32,
}

#[derive(Model)]
#[erp(table_name="sale_order_test")]
#[erp(derived_model="")]
pub(crate) struct SaleOrderTest2 {
    id: u32,
    #[erp(description="New name of the SO")]
    name: String,
}
