use code_gen::Model;
use erp::environment::Environment;
use std::error::Error;

#[derive(Model)]
#[erp(table_name="sale_order_test")]
pub(crate) struct SaleOrderTest {
    id: u32,
    #[erp(description="Name of the SO")]
    name: String,
    #[erp(compute="compute_age")]
    age: i64,
}

impl SaleOrderTest {
    fn compute_age(&mut self, env: &mut Environment) -> Result<(), Box<dyn Error>> {
        self.age = 42;
        Ok(())
    }
}


#[derive(Model)]
#[erp(table_name="sale_order_test")]
#[erp(derived_model="")]
pub(crate) struct SaleOrderTest2 {
    id: u32,
    #[erp(description="New name of the SO")]
    name: String,
}
