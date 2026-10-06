use crate::models::currency::{BaseCurrency, Currency};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use erp::types::model::MapOfFields;

/// How much of a currency one unit of the company's currency bought, from a date on.
#[derive(Model)]
#[erp(id = "currency_rate", name_field = "date", methods)]
#[allow(dead_code)]
pub struct CurrencyRate<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    currency: Reference<BaseCurrency, SingleId>,
    #[erp(index)]
    date: NaiveDate,
    #[erp(
        default = 1.0,
        description = "Units of this currency for one unit of the company's currency"
    )]
    rate: Decimal,
}

#[erp_methods]
impl CurrencyRate<MultipleIds> {
    /// A rate is positive, and a currency has one rate a day.
    pub fn check_rates(&self, env: &mut Environment) -> Result<()> {
        for rate in self {
            let value = *rate.get_rate(env)?;
            let date = *rate.get_date(env)?;
            if value <= Decimal::ZERO {
                return Err(format!("The rate of {date} must be positive, not {value}").into());
            }
            let currency: Currency<SingleId> = rate.get_currency(env)?;
            let currency = currency.get_id();
            let same_day = env.count(
                "currency_rate",
                &erp_search_code_gen::make_domain!([
                    ("currency", "=", currency),
                    ("date", "=", date)
                ]),
            )?;
            if same_day > 1 {
                return Err(format!("The currency already has a rate on {date}").into());
            }
        }
        Ok(())
    }

    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        env.savepoint(|env| {
            let ids: MultipleIds = sup.call_with(values, env)?;
            CurrencyRate::<MultipleIds>::from_ids(ids.clone(), env).check_rates(env)?;
            Ok(ids)
        })
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        env.savepoint(|env| {
            sup.call_with(values, env)?;
            self.check_rates(env)
        })
    }
}
