use crate::models::location::{BaseStockLocation, Location, LocationUsage};
use crate::models::warehouse::partner_location;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::{BaseProduct, Product};

/// How much of a product is at a location, how much of it is promised to transfers, and how
/// much was counted there.
#[derive(Model)]
#[erp(id = "stock_quant", methods)]
#[allow(dead_code)]
pub struct Quant<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    product: Reference<BaseProduct, SingleId>,
    #[erp(
        required,
        ondelete = "cascade",
        domain = r#"[["usage", "=", "internal"]]"#
    )]
    location: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "On hand", default = 0.0)]
    quantity: Decimal,
    #[erp(label = "Reserved", default = 0.0)]
    reserved_quantity: Decimal,
    #[erp(
        label = "Counted quantity",
        description = "What a count found; applied, it becomes what is on hand"
    )]
    inventory_quantity: Option<Decimal>,
    #[erp(
        label = "Difference",
        compute = "compute_inventory_diff",
        depends = ["inventory_quantity", "quantity"],
        stored
    )]
    inventory_diff: Decimal,
}

#[erp_methods]
impl Quant<SingleId> {
    /// The quant of `product` at `location`, made when there is none.
    pub fn of(env: &mut Environment, product: u32, location: u32) -> Result<Quant<SingleId>> {
        let env = &mut *env.sudo();
        let found: Quant<MultipleIds> = env.search(&make_domain!([
            ("product", "=", product),
            ("location", "=", location)
        ]))?;
        if let Some(quant) = found.into_iter().next() {
            return Ok(quant);
        }
        let mut values = MapOfFields::default();
        values.insert("product", product);
        values.insert("location", location);
        env.create_new_record_from_map(values)
    }

    /// Add `delta` of `product` at `location`, in the product's unit.
    pub fn add(env: &mut Environment, product: u32, location: u32, delta: Decimal) -> Result<()> {
        let quant = Self::of(env, product, location)?;
        let env = &mut *env.sudo();
        let quantity = *quant.get_quantity(env)? + delta;
        quant.set_quantity(quantity, env)
    }

    /// The quants of `product` at these locations.
    pub fn at(
        env: &mut Environment,
        product: u32,
        locations: Vec<u32>,
    ) -> Result<Quant<MultipleIds>> {
        let env = &mut *env.sudo();
        env.search_with(
            &make_domain!([
                ("product", "=", product),
                ("location", "in", locations.to_vec())
            ]),
            &erp_search::SearchOptions::new().order_by(erp_search::OrderBy::asc("id")),
        )
    }

    /// What of `product` at these locations is on hand and not promised.
    pub fn available(env: &mut Environment, product: u32, locations: Vec<u32>) -> Result<Decimal> {
        let quants = Self::at(env, product, locations)?;
        let env = &mut *env.sudo();
        let mut available = Decimal::ZERO;
        for quant in &quants {
            available += *quant.get_quantity(env)? - *quant.get_reserved_quantity(env)?;
        }
        Ok(available)
    }

    /// Promise up to `quantity` of `product` from these locations, quant by quant; what could be
    /// promised.
    pub fn reserve(
        env: &mut Environment,
        product: u32,
        locations: Vec<u32>,
        quantity: Decimal,
    ) -> Result<Decimal> {
        let quants = Self::at(env, product, locations)?;
        let env = &mut *env.sudo();
        let mut left = quantity;
        for quant in &quants {
            if left <= Decimal::ZERO {
                break;
            }
            let free = *quant.get_quantity(env)? - *quant.get_reserved_quantity(env)?;
            if free <= Decimal::ZERO {
                continue;
            }
            let take = free.min(left);
            let reserved = *quant.get_reserved_quantity(env)? + take;
            quant.set_reserved_quantity(reserved, env)?;
            left -= take;
        }
        Ok(quantity - left)
    }

    /// Release `quantity` of `product` promised from these locations.
    pub fn unreserve(
        env: &mut Environment,
        product: u32,
        locations: Vec<u32>,
        quantity: Decimal,
    ) -> Result<()> {
        let quants = Self::at(env, product, locations)?;
        let env = &mut *env.sudo();
        let mut left = quantity;
        for quant in &quants {
            if left <= Decimal::ZERO {
                break;
            }
            let reserved = *quant.get_reserved_quantity(env)?;
            let release = reserved.min(left);
            quant.set_reserved_quantity(reserved - release, env)?;
            left -= release;
        }
        Ok(())
    }
}

#[erp_methods]
impl Quant<MultipleIds> {
    /// One line per product and location: a count entered for a place already holding the
    /// product goes on its line, rather than splitting what is there in two.
    #[erp(check = ["product", "location"])]
    pub fn check_one_per_location(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        for quant in self {
            let product: Product<SingleId> = quant.get_product(env)?;
            let location: Location<SingleId> = quant.get_location(env)?;
            let same = env.count(
                "stock_quant",
                &make_domain!([
                    ("product", "=", product.get_id()),
                    ("location", "=", location.get_id())
                ]),
            )?;
            if same > 1 {
                let product_name = product.get_display_name(env)?.clone();
                let location_name = location.get_complete_name(env)?.clone();
                return Err(format!(
                    "{product_name} already has a line at {location_name}: count it there"
                )
                .into());
            }
        }
        Ok(())
    }

    pub fn compute_inventory_diff(&self, env: &mut Environment) -> Result<()> {
        for quant in self {
            let diff = match quant.get_inventory_quantity(env)?.copied() {
                Some(counted) => counted - *quant.get_quantity(env)?,
                None => Decimal::ZERO,
            };
            quant.set_inventory_diff(diff, env)?;
        }
        Ok(())
    }

    /// Make what was counted what is on hand: the difference moves from or to the inventory
    /// adjustment location, valued as any move is, and the count is cleared.
    #[erp(rpc)]
    pub fn action_apply_inventory(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            let adjustment = partner_location(env, LocationUsage::Inventory)?;
            for quant in self {
                let Some(counted) = quant.get_inventory_quantity(env)?.copied() else {
                    continue;
                };
                if counted < Decimal::ZERO {
                    return Err("A counted quantity is not negative".into());
                }
                let diff = counted - *quant.get_quantity(env)?;
                let location: Location<SingleId> = quant.get_location(env)?;
                if !location.is_internal(env)? {
                    return Err("Only stock locations are counted".into());
                }
                if !diff.is_zero() {
                    let product: Product<SingleId> = quant.get_product(env)?;
                    let (source, destination) = if diff > Decimal::ZERO {
                        (adjustment.get_id(), location.get_id())
                    } else {
                        (location.get_id(), adjustment.get_id())
                    };
                    let uom: uom::models::Uom<SingleId> = product.get_uom(&mut env.sudo())?;
                    let mut values = MapOfFields::default();
                    values.insert("name", "Inventory adjustment");
                    values.insert("product", product.get_id());
                    values.insert("product_uom_qty", diff.abs());
                    values.insert("quantity", diff.abs());
                    values.insert("uom", uom.get_id());
                    values.insert("location", source);
                    values.insert("location_dest", destination);
                    let adjustment_move: crate::models::StockMove<SingleId> =
                        env.create_new_record_from_map(values)?;
                    adjustment_move.do_move(env)?;
                }
                quant.set_inventory_quantity(None::<Decimal>, env)?;
            }
            Ok(true)
        })
    }

    /// Forget what was counted: nothing is applied, the line shows what is on hand again.
    #[erp(rpc)]
    pub fn action_clear_inventory(&self, env: &mut Environment) -> Result<bool> {
        for quant in self {
            quant.set_inventory_quantity(None::<Decimal>, env)?;
        }
        Ok(true)
    }
}
