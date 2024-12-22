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

    fn from_string(t: String) -> Self {
        match t.as_ref() {
            "draft" => SaleOrderState::Draft,
            "sent" => SaleOrderState::Sent,
            "paid" => SaleOrderState::Paid,
            "cancelled" => SaleOrderState::Cancelled,
            _ => SaleOrderState::Cancelled,
        }
    }
}

#[derive(Model)]
#[erp(table_name="sale_order")]
pub struct SaleOrder {
    pub id: u32,
    #[erp(default="0ddlyoko")]
    pub name: String,
    pub state: SaleOrderState,
    #[erp(default=42i64)]
    pub price: i64,
    #[erp(default=10)]
    pub amount: i64,
    // TODO Add support for this:
    // #[erp(computed="compute_total_price")]
    pub total_price: i64,
}

impl SaleOrder {
    // TODO Re-add support for computed methods
    pub fn compute_total_price(
        &mut self,
        _environment: &Environment,
    ) -> Result<(), Box<dyn Error>> {
        self.total_price = self.price * self.amount;
        Ok(())
    }
}
