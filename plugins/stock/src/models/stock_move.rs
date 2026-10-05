use crate::models::extensions::{ProductCategoryStock, StockCostMethod};
use crate::models::location::{BaseStockLocation, Location};
use crate::models::picking::{BaseStockPicking, Picking};
use crate::models::quant::Quant;
use crate::models::valuation_layer::ValuationLayer;
use crate::valuation::{self, CostMethod, Layer};
use code_gen::{Model, erp_methods, selection};
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId, Timestamp, Utc};
use erp::types::model::MapOfFields;
use product::models::{BaseProduct, Product, ProductCategory};
use uom::conversion::Rounding;
use uom::models::{BaseUom, Uom};

#[selection]
pub enum MoveStatus {
    #[default]
    #[selection(label = "New")]
    Draft,
    #[selection(label = "Waiting availability")]
    Confirmed,
    #[selection(label = "Partially available")]
    PartiallyAvailable,
    #[selection(label = "Available")]
    Assigned,
    #[selection(label = "Done")]
    Done,
    #[selection(label = "Cancelled")]
    Cancel,
}

/// A quantity of a product going from one location to another: what a transfer is made of,
/// and what changes the stock and its value once done.
#[derive(Model)]
#[erp(id = "stock_move", methods)]
#[allow(dead_code)]
pub struct StockMove<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Transfer", ondelete = "cascade")]
    picking: Reference<BaseStockPicking, SingleId>,
    #[erp(label = "Description")]
    name: String,
    #[erp(
        required,
        ondelete = "restrict",
        domain = r#"[["product_type", "!=", "service"]]"#
    )]
    product: Reference<BaseProduct, SingleId>,
    #[erp(label = "Demand", default = 0.0)]
    product_uom_qty: Decimal,
    #[erp(label = "Quantity done", default = 0.0)]
    quantity: Decimal,
    #[erp(label = "Unit", ondelete = "restrict", compute = "compute_uom", depends = ["product"], stored, editable)]
    uom: Reference<BaseUom, SingleId>,
    #[erp(
        label = "Source location",
        required,
        ondelete = "restrict",
        compute = "compute_location",
        depends = ["picking.location"],
        stored,
        editable
    )]
    location: Reference<BaseStockLocation, SingleId>,
    #[erp(
        label = "Destination location",
        required,
        ondelete = "restrict",
        compute = "compute_location_dest",
        depends = ["picking.location_dest"],
        stored,
        editable
    )]
    location_dest: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "Status")]
    state: MoveStatus,
    #[erp(label = "Reserved", default = 0.0)]
    reserved_quantity: Decimal,
    #[erp(
        label = "Unit cost",
        default = 0.0,
        description = "What a unit coming in cost: the purchase price; the product's cost when left at nothing"
    )]
    price_unit: Decimal,
    #[erp(
        label = "Value",
        default = 0.0,
        description = "What the move added to the stock's value, or took from it"
    )]
    value: Decimal,
    #[erp(label = "Source document")]
    origin: Option<String>,
    #[erp(label = "Date done")]
    date_done: Option<Timestamp>,
}

impl StockMove<SingleId> {
    pub fn is_status(&self, env: &mut Environment, status: MoveStatus) -> Result<bool> {
        Ok(erp::types::field::Selection::key(self.get_state(env)?)
            == erp::types::field::Selection::key(&status))
    }

    /// `quantity` in the move's unit, in the product's unit.
    pub fn to_product_quantity(&self, env: &mut Environment, quantity: Decimal) -> Result<Decimal> {
        let env = &mut *env.sudo();
        let product: Product<SingleId> = self.get_product(env)?;
        let product_uom: Uom<SingleId> = product.get_uom(env)?;
        let uom: Uom<SingleId> = self.get_uom(env)?;
        if uom.is_empty() || uom.get_id() == product_uom.get_id() {
            return Ok(quantity);
        }
        uom.convert_to(env, quantity, &product_uom, Rounding::HalfUp)
    }

    /// Promise what the move needs from its source, when its source is the company's stock.
    pub fn reserve(&self, env: &mut Environment) -> Result<()> {
        if self.is_status(env, MoveStatus::Done)? || self.is_status(env, MoveStatus::Cancel)? {
            return Ok(());
        }
        let source: Location<SingleId> = self.get_location(env)?;
        let demand = *self.get_product_uom_qty(env)?;
        if !source.is_internal(env)? {
            self.set_state(MoveStatus::Assigned, env)?;
            return Ok(());
        }
        let needed = self.to_product_quantity(env, demand)? - *self.get_reserved_quantity(env)?;
        let product: Product<SingleId> = self.get_product(env)?;
        let locations = source.and_below(env)?;
        let reserved = if needed > Decimal::ZERO {
            Quant::reserve(env, product.get_id(), &locations, needed)?
        } else {
            Decimal::ZERO
        };
        let total = *self.get_reserved_quantity(env)? + reserved;
        self.set_reserved_quantity(total, env)?;
        let wanted = self.to_product_quantity(env, demand)?;
        let status = if total >= wanted {
            MoveStatus::Assigned
        } else if total > Decimal::ZERO {
            MoveStatus::PartiallyAvailable
        } else {
            MoveStatus::Confirmed
        };
        self.set_state(status, env)
    }

    /// Release what the move had promised.
    pub fn unreserve(&self, env: &mut Environment) -> Result<()> {
        let reserved = *self.get_reserved_quantity(env)?;
        if reserved.is_zero() {
            return Ok(());
        }
        let source: Location<SingleId> = self.get_location(env)?;
        let product: Product<SingleId> = self.get_product(env)?;
        let locations = source.and_below(env)?;
        Quant::unreserve(env, product.get_id(), &locations, reserved)?;
        self.set_reserved_quantity(Decimal::ZERO, env)
    }

    /// Move the quantity done: out of the source and into the destination; valued when it
    /// enters or leaves the company's stock.
    ///
    /// Refused when the source is the company's stock and does not hold that much.
    pub fn do_move(&self, env: &mut Environment) -> Result<()> {
        if self.is_status(env, MoveStatus::Done)? {
            return Ok(());
        }
        if self.is_status(env, MoveStatus::Cancel)? {
            return Err("A cancelled move is not done".into());
        }
        let done = *self.get_quantity(env)?;
        if done < Decimal::ZERO {
            return Err("A quantity done is not negative".into());
        }
        let quantity = self.to_product_quantity(env, done)?;
        let product: Product<SingleId> = self.get_product(env)?;
        let source: Location<SingleId> = self.get_location(env)?;
        let destination: Location<SingleId> = self.get_location_dest(env)?;
        let from_stock = source.is_internal(env)?;
        let to_stock = destination.is_internal(env)?;
        self.unreserve(env)?;
        if from_stock && quantity > Decimal::ZERO {
            let locations = source.and_below(env)?;
            let available = Quant::available(env, product.get_id(), &locations)?;
            if available < quantity {
                let name = product.get_display_name(&mut env.sudo())?.clone();
                let location = source.get_complete_name(&mut env.sudo())?.clone();
                return Err(format!(
                    "Only {available} of {name} are available in {location}, {quantity} are asked"
                )
                .into());
            }
            self.take_from(env, product.get_id(), &locations, quantity)?;
        }
        if to_stock && quantity > Decimal::ZERO {
            Quant::add(env, product.get_id(), destination.get_id(), quantity)?;
        }
        let value = match (from_stock, to_stock) {
            (false, true) => self.value_in(env, &product, quantity)?,
            (true, false) => -self.value_out(env, &product, quantity)?,
            _ => Decimal::ZERO,
        };
        let mut values = MapOfFields::default();
        values.insert("state", MoveStatus::Done);
        values.insert("value", value);
        values.insert("date_done", Utc::now());
        env.sudo()
            .write("stock_move", &SingleId::from(self.get_id()), values)?;
        StockMove::<MultipleIds>::from_ids(vec![self.get_id()], env).on_moved(env)
    }

    /// Take `quantity` off the quants of these locations, unpromised quantities first.
    fn take_from(
        &self,
        env: &mut Environment,
        product: u32,
        locations: &[u32],
        quantity: Decimal,
    ) -> Result<()> {
        let quants = Quant::at(env, product, locations)?;
        let env = &mut *env.sudo();
        let mut left = quantity;
        for quant in &quants {
            if left <= Decimal::ZERO {
                break;
            }
            let on_hand = *quant.get_quantity(env)?;
            let free = on_hand - *quant.get_reserved_quantity(env)?;
            let take = free.max(Decimal::ZERO).min(left);
            if take > Decimal::ZERO {
                quant.set_quantity(on_hand - take, env)?;
                left -= take;
            }
        }
        for quant in &quants {
            if left <= Decimal::ZERO {
                break;
            }
            let on_hand = *quant.get_quantity(env)?;
            let take = on_hand.max(Decimal::ZERO).min(left);
            if take > Decimal::ZERO {
                quant.set_quantity(on_hand - take, env)?;
                let reserved = (*quant.get_reserved_quantity(env)?).min(on_hand - take);
                quant.set_reserved_quantity(reserved.max(Decimal::ZERO), env)?;
                left -= take;
            }
        }
        Ok(())
    }

    /// The cost method of the product's category.
    fn cost_method(env: &mut Environment, product: &Product<SingleId>) -> Result<CostMethod> {
        let env = &mut *env.sudo();
        let category: ProductCategory<SingleId> = product.get_category(env)?;
        let category: ProductCategoryStock<SingleId> = env.get_record(category.get_id().into());
        Ok(match *category.get_cost_method(env)? {
            StockCostMethod::Standard => CostMethod::Standard,
            StockCostMethod::Fifo => CostMethod::Fifo,
            _ => CostMethod::Average,
        })
    }

    /// Value units coming in: a layer at their cost; the average cost moves with them.
    fn value_in(
        &self,
        env: &mut Environment,
        product: &Product<SingleId>,
        quantity: Decimal,
    ) -> Result<Decimal> {
        let method = Self::cost_method(env, product)?;
        let rounding = *Currency::of_company(env)?.get_rounding(&mut env.sudo())?;
        let env = &mut *env.sudo();
        let standard = *product.get_standard_price(env)?;
        let given = *self.get_price_unit(env)?;
        let unit_cost = if given.is_zero() { standard } else { given };
        let value = valuation::incoming_value(method, quantity, unit_cost, standard, rounding);
        if method == CostMethod::Average {
            let on_hand = ValuationLayer::quantity_of(env, product.get_id())?;
            let average = valuation::new_average(on_hand, standard, quantity, unit_cost);
            product.set_standard_price(average, env)?;
        }
        ValuationLayer::record(env, product.get_id(), self.get_id(), quantity, value, true)?;
        if method == CostMethod::Fifo {
            ValuationLayer::refresh_fifo_cost(env, product.get_id())?;
        }
        Ok(value)
    }

    /// Value units going out, positive: at the standard or average cost, or the oldest layers'.
    fn value_out(
        &self,
        env: &mut Environment,
        product: &Product<SingleId>,
        quantity: Decimal,
    ) -> Result<Decimal> {
        let method = Self::cost_method(env, product)?;
        let rounding = *Currency::of_company(env)?.get_rounding(&mut env.sudo())?;
        let env = &mut *env.sudo();
        let standard = *product.get_standard_price(env)?;
        let value = match method {
            CostMethod::Fifo => {
                let layers = ValuationLayer::open_layers(env, product.get_id())?;
                let open: Vec<Layer> = layers.iter().map(|(layer, _)| *layer).collect();
                let (value, taken) = valuation::fifo_out(&open, quantity, standard, rounding);
                for (id, taken_quantity, taken_value) in taken {
                    ValuationLayer::consume(env, id, taken_quantity, taken_value)?;
                }
                value
            }
            _ => {
                // The whole of what is left leaves at what is left of its value: no cent strays.
                let on_hand = ValuationLayer::quantity_of(env, product.get_id())?;
                let stock_value = ValuationLayer::value_of(env, product.get_id())?;
                if method == CostMethod::Average && on_hand == quantity && on_hand > Decimal::ZERO {
                    stock_value
                } else {
                    valuation::outgoing_value(method, quantity, standard, standard, rounding)
                }
            }
        };
        ValuationLayer::record(
            env,
            product.get_id(),
            self.get_id(),
            -quantity,
            -value,
            false,
        )?;
        if method == CostMethod::Fifo {
            ValuationLayer::refresh_fifo_cost(env, product.get_id())?;
        }
        Ok(value)
    }
}

#[erp_methods]
impl StockMove<MultipleIds> {
    /// Where the transfer comes from.
    pub fn compute_location(&self, env: &mut Environment) -> Result<()> {
        for stock_move in self {
            let picking: Picking<SingleId> = stock_move.get_picking(env)?;
            let source: Location<SingleId> = if picking.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                picking.get_location(env)?
            };
            stock_move.set_location(&source, env)?;
        }
        Ok(())
    }

    /// Where the transfer goes.
    pub fn compute_location_dest(&self, env: &mut Environment) -> Result<()> {
        for stock_move in self {
            let picking: Picking<SingleId> = stock_move.get_picking(env)?;
            let destination: Location<SingleId> = if picking.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                picking.get_location_dest(env)?
            };
            stock_move.set_location_dest(&destination, env)?;
        }
        Ok(())
    }

    /// The unit the product is counted in.
    pub fn compute_uom(&self, env: &mut Environment) -> Result<()> {
        for stock_move in self {
            let product: Product<SingleId> = stock_move.get_product(env)?;
            let uom: Uom<SingleId> = if product.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                product.get_uom(&mut env.sudo())?
            };
            stock_move.set_uom(&uom, env)?;
        }
        Ok(())
    }

    /// What follows a move done, valued: nothing here; accounting books the value it moved.
    pub fn on_moved(&self, _env: &mut Environment) -> Result<()> {
        Ok(())
    }

    /// A move's quantities are not negative; one done stays as it was.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        for stock_move in &values {
            for field in ["product_uom_qty", "quantity"] {
                if stock_move
                    .get_option::<&Decimal>(field)
                    .is_some_and(|quantity| *quantity < Decimal::ZERO)
                {
                    return Err("A move's quantities are not negative".into());
                }
            }
        }
        sup.call_with(values, env)
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        for field in ["product_uom_qty", "quantity"] {
            if values
                .get_option::<&Decimal>(field)
                .is_some_and(|quantity| *quantity < Decimal::ZERO)
            {
                return Err("A move's quantities are not negative".into());
            }
        }
        const MOVED: [&str; 5] = [
            "product",
            "product_uom_qty",
            "quantity",
            "location",
            "location_dest",
        ];
        if values
            .fields
            .keys()
            .any(|field| MOVED.contains(&field.as_str()))
        {
            for stock_move in self {
                if stock_move.is_status(env, MoveStatus::Done)? {
                    return Err(format!(
                        "\"{}\" is done: it stays as it was",
                        stock_move.get_name(env)?
                    )
                    .into());
                }
            }
        }
        sup.call_with(values, env)
    }

    /// Done moves stay; the others go with what they promised.
    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        for stock_move in self {
            if stock_move.is_status(env, MoveStatus::Done)? {
                return Err(format!("\"{}\" is done: it stays", stock_move.get_name(env)?).into());
            }
            stock_move.unreserve(env)?;
        }
        sup.call(env)
    }
}
