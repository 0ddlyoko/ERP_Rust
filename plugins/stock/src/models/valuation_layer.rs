use crate::models::stock_move::BaseStockMove;
use crate::valuation::Layer;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::{BaseProduct, Product};

/// What a move added to the value of the stock, or took from it; for units still in stock,
/// what is left of them and of their value.
#[derive(Model)]
#[erp(id = "stock_valuation_layer", methods)]
#[allow(dead_code)]
pub struct ValuationLayer<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "restrict")]
    product: Reference<BaseProduct, SingleId>,
    #[erp(label = "Move", ondelete = "set_null")]
    stock_move: Reference<BaseStockMove, SingleId>,
    #[erp(default = 0.0)]
    quantity: Decimal,
    #[erp(label = "Unit cost", default = 0.0)]
    unit_cost: Decimal,
    #[erp(default = 0.0)]
    value: Decimal,
    #[erp(label = "Remaining quantity", default = 0.0)]
    remaining_quantity: Decimal,
    #[erp(label = "Remaining value", default = 0.0)]
    remaining_value: Decimal,
}

#[erp_methods]
impl ValuationLayer<SingleId> {
    /// Record what a move did to the stock's value; units coming in stay open for FIFO.
    pub fn record(
        env: &mut Environment,
        product: u32,
        stock_move: u32,
        quantity: Decimal,
        value: Decimal,
        incoming: bool,
    ) -> Result<ValuationLayer<SingleId>> {
        let env = &mut *env.sudo();
        let mut values = MapOfFields::default();
        values.insert("product", product);
        values.insert("stock_move", stock_move);
        values.insert("quantity", quantity);
        values.insert("value", value);
        values.insert(
            "unit_cost",
            if quantity.is_zero() {
                Decimal::ZERO
            } else {
                (value / quantity).round_dp(6)
            },
        );
        if incoming {
            values.insert("remaining_quantity", quantity);
            values.insert("remaining_value", value);
        }
        env.create_new_record_from_map(values)
    }

    /// The layers of `product` with units left, oldest first.
    pub fn open_layers(
        env: &mut Environment,
        product: u32,
    ) -> Result<Vec<(Layer, ValuationLayer<SingleId>)>> {
        let env = &mut *env.sudo();
        let layers: ValuationLayer<MultipleIds> = env.search_with(
            &make_domain!([
                ("product", "=", product),
                ("remaining_quantity", ">", Decimal::ZERO)
            ]),
            &erp_search::SearchOptions::new().order_by(erp_search::OrderBy::asc("id")),
        )?;
        let mut open = Vec::new();
        for layer in &layers {
            open.push((
                Layer {
                    id: layer.get_id(),
                    remaining_quantity: *layer.get_remaining_quantity(env)?,
                    remaining_value: *layer.get_remaining_value(env)?,
                },
                layer.clone(),
            ));
        }
        Ok(open)
    }

    /// Take units out of a layer.
    pub fn consume(
        env: &mut Environment,
        layer: u32,
        quantity: Decimal,
        value: Decimal,
    ) -> Result<()> {
        let env = &mut *env.sudo();
        let layer: ValuationLayer<SingleId> = env.get_record(layer.into());
        let remaining = *layer.get_remaining_quantity(env)? - quantity;
        let remaining_value = *layer.get_remaining_value(env)? - value;
        layer.set_remaining_quantity(remaining, env)?;
        layer.set_remaining_value(remaining_value, env)
    }

    /// How many units of `product` the layers hold.
    pub fn quantity_of(env: &mut Environment, product: u32) -> Result<Decimal> {
        let env = &mut *env.sudo();
        let layers: ValuationLayer<MultipleIds> =
            env.search(&make_domain!([("product", "=", product)]))?;
        Ok(layers.get_quantity(env)?.into_iter().copied().sum())
    }

    /// What the stock of `product` is worth.
    pub fn value_of(env: &mut Environment, product: u32) -> Result<Decimal> {
        let env = &mut *env.sudo();
        let layers: ValuationLayer<MultipleIds> =
            env.search(&make_domain!([("product", "=", product)]))?;
        Ok(layers.get_value(env)?.into_iter().copied().sum())
    }

    /// Under FIFO, the product's cost is what its units left in stock cost on average.
    pub fn refresh_fifo_cost(env: &mut Environment, product: u32) -> Result<()> {
        let open = Self::open_layers(env, product)?;
        let quantity: Decimal = open.iter().map(|(layer, _)| layer.remaining_quantity).sum();
        let value: Decimal = open.iter().map(|(layer, _)| layer.remaining_value).sum();
        if quantity > Decimal::ZERO {
            let env = &mut *env.sudo();
            let product: Product<SingleId> = env.get_record(product.into());
            product.set_standard_price((value / quantity).round_dp(6), env)?;
        }
        Ok(())
    }
}
