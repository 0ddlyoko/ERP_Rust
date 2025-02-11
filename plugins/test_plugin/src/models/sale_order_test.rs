use code_gen::Model;
use erp::field::IdMode;

#[derive(Model)]
#[erp(table_name="sale_order_test")]
#[allow(dead_code)]
pub(crate) struct SaleOrderTest<Mode: IdMode> {
    id: Mode,
    name: String,
    age: i32,
}

#[derive(Model)]
#[erp(table_name="sale_order_test")]
#[erp(derived_model="")]
#[allow(dead_code)]
pub(crate) struct SaleOrderTest2<Mode: IdMode> {
    id: Mode,
    #[erp(description="New name of the SO")]
    name: String,
}
