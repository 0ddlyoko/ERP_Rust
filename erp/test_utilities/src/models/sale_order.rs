use crate::models::{BaseSaleOrderLine, SaleOrderLine};
use code_gen::Model;
use erp::environment::Environment;
use erp::field::{EnumType, IdMode, MultipleIds, Reference};
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
#[allow(dead_code)]
pub struct SaleOrder<Mode: IdMode> {
    pub id: Mode,
    #[erp(default="0ddlyoko")]
    name: String,
    state: SaleOrderState,
    #[erp(compute="compute_total_price", depends=["lines.total_price"])]
    total_price: i32,
    // TODO Add inverse="..."
    lines: Reference<BaseSaleOrderLine, MultipleIds>,
}

impl SaleOrder<MultipleIds> {
    pub fn compute_total_price(
        &self,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>> {
        for sale_order in self {
            let lines: SaleOrderLine<_> = sale_order.get_lines(env)?;
            let total_prices = lines.get_total_price(env)?;
            sale_order.set_total_price(total_prices.into_iter().sum(), env)?;
        }
        Ok(())
    }
}
