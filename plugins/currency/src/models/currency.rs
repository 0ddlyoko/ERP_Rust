use crate::models::company::CompanyCurrency;
use crate::models::currency_rate::{BaseCurrencyRate, CurrencyRate};
use crate::money;
use base::models::Company;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId, Utc};
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;

#[selection]
pub enum SymbolPosition {
    #[selection(label = "After the amount")]
    #[default]
    After,
    #[selection(label = "Before the amount")]
    Before,
}

/// What amounts are counted in: `EUR`, with its symbol, its smallest coin, and its rates.
#[derive(Model)]
#[erp(id = "currency", order = "name, id", methods)]
#[allow(dead_code)]
pub struct Currency<Mode: IdMode> {
    id: Mode,
    #[erp(description = "ISO 4217 code, e.g. EUR")]
    name: String,
    #[erp(label = "Currency name")]
    full_name: Option<String>,
    symbol: String,
    #[erp(label = "Symbol position")]
    position: SymbolPosition,
    #[erp(
        label = "Rounding",
        default = 0.01,
        description = "Amounts in this currency are rounded to a multiple of this"
    )]
    rounding: Decimal,
    #[erp(label = "Rates", inverse = "currency")]
    rates: Reference<BaseCurrencyRate, MultipleIds>,
    #[erp(
        label = "Current rate",
        compute = "compute_rate",
        depends = ["rates.rate", "rates.date"]
    )]
    rate: Decimal,
    #[erp(default = true)]
    active: bool,
}

#[erp_methods]
impl Currency<SingleId> {
    /// The currency the current company keeps its books in.
    pub fn of_company(env: &mut Environment) -> Result<Currency<SingleId>> {
        let company = Company::current(env)?;
        let company: CompanyCurrency<SingleId> = env.get_record(company.get_id().into());
        let env = &mut *env.sudo();
        company.get_currency(env)
    }

    /// The rate in force on `date`: the latest one set on or before it, 1 without any — and 1
    /// for the company's own currency, which the others are relative to.
    pub fn rate_at(&self, env: &mut Environment, date: NaiveDate) -> Result<Decimal> {
        if self.is_empty() || Self::of_company(env)?.get_id() == self.get_id() {
            return Ok(Decimal::ONE);
        }
        let currency = self.get_id();
        let env = &mut *env.sudo();
        let rates: CurrencyRate<MultipleIds> = env.search_with(
            &make_domain!([("currency", "=", currency), ("date", "<=", date)]),
            &SearchOptions::new()
                .order_by(OrderBy::desc("date"))
                .with_limit(1),
        )?;
        match rates.into_iter().next() {
            Some(rate) => Ok(*rate.get_rate(env)?),
            None => Ok(Decimal::ONE),
        }
    }

    /// `amount` of this currency rounded to its smallest coin.
    pub fn round(&self, env: &mut Environment, amount: Decimal) -> Result<Decimal> {
        Ok(money::round(amount, *self.get_rounding(env)?))
    }

    /// Whether `amount` of this currency rounds to nothing.
    pub fn is_zero(&self, env: &mut Environment, amount: Decimal) -> Result<bool> {
        Ok(money::is_zero(amount, *self.get_rounding(env)?))
    }

    /// `amount` of this currency in `to`, at the rates in force on `date`, rounded to `to`.
    pub fn convert(
        &self,
        env: &mut Environment,
        amount: Decimal,
        to: Currency<SingleId>,
        date: NaiveDate,
    ) -> Result<Decimal> {
        if self.get_id() == to.get_id() {
            return to.round(env, amount);
        }
        let from_rate = self.rate_at(env, date)?;
        let to_rate = to.rate_at(env, date)?;
        let rounding = *to.get_rounding(env)?;
        Ok(money::convert(amount, from_rate, to_rate, rounding)?)
    }

    /// `amount` written as people read it: `1234.50 €`, `$ 1234.50`.
    pub fn format(&self, env: &mut Environment, amount: Decimal) -> Result<String> {
        let rounding = *self.get_rounding(env)?;
        let places = rounding.normalize().scale();
        let amount = money::round(amount, rounding).round_dp(places);
        let amount = format!("{amount:.prec$}", prec = places as usize);
        let symbol = self.get_symbol(env)?.clone();
        Ok(match *self.get_position(env)? {
            SymbolPosition::Before => format!("{symbol} {amount}"),
            _ => format!("{amount} {symbol}"),
        })
    }
}

#[erp_methods]
impl Currency<MultipleIds> {
    /// The rate in force today.
    pub fn compute_rate(&self, env: &mut Environment) -> Result<()> {
        let today = Utc::now().date_naive();
        for currency in self {
            let rate = currency.rate_at(env, today)?;
            currency.set_rate(rate, env)?;
        }
        Ok(())
    }
}
