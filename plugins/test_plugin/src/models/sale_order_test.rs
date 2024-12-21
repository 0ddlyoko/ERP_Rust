use code_gen::Model;
use erp::environment::Environment;
use std::error::Error;

#[derive(Model)]
#[erp(table_name="sale_order_test")]
pub(crate) struct SaleOrderTest {
    id: u32,
    name: String,
    age: i64,
}

impl SaleOrderTest {
    fn _compute_age(&mut self, env: &mut Environment) -> Result<(), Box<dyn Error>> {
        self.age = 42;
        Ok(())
    }
}
