use code_gen::Model;
use erp::environment::Environment;
use erp::field::EnumType;
use std::error::Error;

#[derive(Debug, PartialEq, Eq, Default, Copy, Clone)]
pub enum SaleOrderState {
    #[default]
    Draft,
    Sent,
    Paid,
    Cancelled,
}

impl EnumType for SaleOrderState {
    fn to_string(&self) -> String {
        match self {
            SaleOrderState::Draft => String::from("draft"),
            SaleOrderState::Sent => String::from("sent"),
            SaleOrderState::Paid => String::from("paid"),
            SaleOrderState::Cancelled => String::from("cancelled"),
        }
    }

    fn from_string(t: &str) -> &Self {
        match t {
            "draft" => &SaleOrderState::Draft,
            "sent" => &SaleOrderState::Sent,
            "paid" => &SaleOrderState::Paid,
            "cancelled" => &SaleOrderState::Cancelled,
            _ => &SaleOrderState::Cancelled,
        }
    }
}

#[derive(Model)]
#[erp(table_name="sale_order")]
pub struct SaleOrder {
    pub id: u32,
    #[erp(default="0ddlyoko")]
    name: String,
    state: SaleOrderState,
    #[erp(default=42)]
    price: i32,
    #[erp(default=10)]
    amount: i32,
    #[erp(compute="compute_total_price", depends=["price", "amount"])]
    total_price: i32,
}

impl SaleOrder {
    pub fn compute_total_price(
        &mut self,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>> {
        // TODO Pass value and not reference to the value
        // self.set_total_price(*self.get_price(env)? * *self.get_amount(env)?, env)
        self.set_total_price(&(*self.get_price(env)? * *self.get_amount(env)?), env)
    }
}
