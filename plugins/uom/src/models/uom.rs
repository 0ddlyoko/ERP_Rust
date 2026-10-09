use crate::conversion::{self, Rounding};
use crate::models::uom_category::{BaseUomCategory, UomCategory};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;

#[selection]
pub enum UomType {
    #[selection(label = "Reference unit of its category")]
    #[default]
    Reference,
    #[selection(label = "Bigger than the reference unit")]
    Bigger,
    #[selection(label = "Smaller than the reference unit")]
    Smaller,
}

/// A unit quantities are counted in, worth `ratio` reference units of its category.
#[derive(Model)]
#[erp(id = "uom", order = "name, id", methods)]
#[allow(dead_code)]
pub struct Uom<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(required, ondelete = "restrict")]
    category: Reference<BaseUomCategory, SingleId>,
    #[erp(label = "Type")]
    uom_type: UomType,
    #[erp(
        default = 1.0,
        description = "How many reference units of the category one of this unit is worth"
    )]
    ratio: Decimal,
    #[erp(
        label = "Rounding precision",
        default = 0.01,
        description = "Quantities in this unit are rounded to a multiple of this"
    )]
    rounding: Decimal,
    #[erp(default = true)]
    active: bool,
}

#[erp_methods]
impl Uom<SingleId> {
    /// `quantity` of this unit, counted in `to`, rounded to the precision of `to`.
    ///
    /// Errs between units measuring different things: kilograms are no number of hours.
    pub fn convert_to(
        &self,
        env: &mut Environment,
        quantity: Decimal,
        to: Uom<SingleId>,
        rounding: Rounding,
    ) -> Result<Decimal> {
        if self.get_id() == to.get_id() {
            let precision = *to.get_rounding(env)?;
            return Ok(conversion::round_to(quantity, precision, rounding));
        }
        self.check_same_category(env, to.clone())?;
        let from_ratio = *self.get_ratio(env)?;
        let to_ratio = *to.get_ratio(env)?;
        let precision = *to.get_rounding(env)?;
        Ok(conversion::convert(
            quantity, from_ratio, to_ratio, precision, rounding,
        )?)
    }

    /// A price per this unit, as a price per `to`: a unit priced 2 is a dozen priced 24. Not
    /// rounded: a unit price keeps its decimals until it is multiplied by a quantity.
    pub fn convert_price(
        &self,
        env: &mut Environment,
        price: Decimal,
        to: Uom<SingleId>,
    ) -> Result<Decimal> {
        if self.get_id() == to.get_id() {
            return Ok(price);
        }
        self.check_same_category(env, to.clone())?;
        Ok(price * *to.get_ratio(env)? / *self.get_ratio(env)?)
    }

    /// Errs between units measuring different things: kilograms are no number of hours.
    fn check_same_category(&self, env: &mut Environment, to: Uom<SingleId>) -> Result<()> {
        let from_category: UomCategory<SingleId> = self.get_category(env)?;
        let to_category: UomCategory<SingleId> = to.get_category(env)?;
        if from_category.get_id() != to_category.get_id() {
            let from_name = self.get_name(env)?.clone();
            let to_name = to.get_name(env)?.clone();
            return Err(format!(
                "Cannot convert {from_name} into {to_name}: they do not measure the same thing"
            )
            .into());
        }
        Ok(())
    }
}

#[erp_methods]
impl Uom<MultipleIds> {
    /// A reference unit is worth exactly one, a bigger unit more and a smaller one less; a
    /// rounding is positive; a category has a single active reference unit.
    pub fn check_ratios(&self, env: &mut Environment) -> Result<()> {
        for uom in self {
            let name = uom.get_name(env)?.clone();
            let ratio = *uom.get_ratio(env)?;
            let fits = match *uom.get_uom_type(env)? {
                UomType::Reference => ratio == Decimal::ONE,
                UomType::Bigger => ratio > Decimal::ONE,
                UomType::Smaller => ratio > Decimal::ZERO && ratio < Decimal::ONE,
                _ => ratio > Decimal::ZERO,
            };
            if !fits {
                return Err(format!(
                    "The ratio of {name}, {ratio}, does not fit its type: a reference unit is \
                     worth 1, a bigger one more than 1, a smaller one between 0 and 1"
                )
                .into());
            }
            if *uom.get_rounding(env)? <= Decimal::ZERO {
                return Err(format!("The rounding precision of {name} must be positive").into());
            }
            let category: UomCategory<SingleId> = uom.get_category(env)?;
            let category = category.get_id();
            let references = env.count(
                "uom",
                &make_domain!([
                    ("category", "=", category),
                    ("uom_type", "=", "reference"),
                    ("active", "=", true)
                ]),
            )?;
            if references > 1 {
                return Err(format!(
                    "The category of {name} would have {references} reference units: it must \
                     have one"
                )
                .into());
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
            Uom::<MultipleIds>::from_ids(ids.clone(), env).check_ratios(env)?;
            Ok(ids)
        })
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        env.savepoint(|env| {
            sup.call_with(values, env)?;
            self.check_ratios(env)
        })
    }
}
