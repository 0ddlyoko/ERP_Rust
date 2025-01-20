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

impl From<SaleOrderState> for &'static str {
    fn from(value: SaleOrderState) -> &'static str {
        match value {
            SaleOrderState::Draft => "draft",
            SaleOrderState::Sent => "sent",
            SaleOrderState::Paid => "paid",
            SaleOrderState::Cancelled => "cancelled",
        }
    }
}

impl From<&str> for &SaleOrderState {
    fn from(value: &str) -> Self {
        match value {
            "draft" => &SaleOrderState::Draft,
            "sent" => &SaleOrderState::Sent,
            "paid" => &SaleOrderState::Paid,
            "cancelled" => &SaleOrderState::Cancelled,
            _ => &SaleOrderState::Cancelled,
        }
    }
}

impl EnumType for SaleOrderState {
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
        self.set_total_price(*self.get_price(env)? * *self.get_amount(env)?, env)
    }
}
