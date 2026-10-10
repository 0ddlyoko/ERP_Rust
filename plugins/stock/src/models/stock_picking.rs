use crate::models::stock_location::{BaseStockLocation, StockLocation};
use crate::models::stock_move::{BaseStockMove, MoveStatus, StockMove};
use crate::models::stock_picking_type::{BaseStockPickingType, PickingKind, StockPickingType};
use crate::models::stock_warehouse::StockWarehouse;
use base::models::{BaseContact, Contact};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::{
    Command, Decimal, FieldType, IdMode, MultipleIds, NaiveDate, Reference, SingleId, Timestamp,
    Utc,
};
use erp::types::model::MapOfFields;
use sequence::models::Sequence;

#[selection]
pub enum PickingState {
    #[default]
    #[selection(label = "Draft")]
    Draft,
    #[selection(label = "Waiting")]
    Confirmed,
    #[selection(label = "Ready")]
    Assigned,
    #[selection(label = "Done")]
    Done,
    #[selection(label = "Cancelled")]
    Cancel,
}

/// A transfer: a receipt, a delivery or an internal move of goods, made of moves, numbered by
/// its kind.
#[derive(Model)]
#[erp(
    id = "stock_picking",
    contact_field = "partner",
    order = "scheduled_date, id",
    methods
)]
#[allow(dead_code)]
pub struct StockPicking<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Reference", default = "/", index = "trigram")]
    name: String,
    #[erp(label = "Operation type", required, ondelete = "restrict")]
    picking_type: Reference<BaseStockPickingType, SingleId>,
    #[erp(label = "Contact", ondelete = "restrict", tracking)]
    partner: Reference<BaseContact, SingleId>,
    #[erp(label = "Source location", ondelete = "restrict", compute = "compute_location", depends = ["picking_type"], stored, editable)]
    location: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "Destination location", ondelete = "restrict", compute = "compute_location_dest", depends = ["picking_type"], stored, editable)]
    location_dest: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "Scheduled date", index)]
    scheduled_date: Option<NaiveDate>,
    #[erp(label = "Date done")]
    date_done: Option<Timestamp>,
    #[erp(label = "Source document", index = "trigram")]
    origin: Option<String>,
    #[erp(label = "Status", tracking, index, readonly)]
    state: PickingState,
    #[erp(label = "Operations", inverse = "picking", owned)]
    moves: Reference<BaseStockMove, MultipleIds>,
    #[erp(label = "Back order of", ondelete = "set_null")]
    backorder: Reference<BaseStockPicking, SingleId>,
    #[erp(label = "Return of", ondelete = "set_null")]
    returned_picking: Reference<BaseStockPicking, SingleId>,
    note: Option<String>,
}

#[erp_methods]
impl StockPicking<SingleId> {
    pub fn is_state(&self, env: &mut Environment, state: PickingState) -> Result<bool> {
        Ok(*self.get_state(env)? == state)
    }

    /// The kind of the transfer: receipt, delivery or internal.
    pub fn kind(&self, env: &mut Environment) -> Result<PickingKind> {
        let picking_type: StockPickingType<SingleId> = self.get_picking_type(env)?;
        Ok(*picking_type.get_code(&mut env.sudo())?)
    }

    /// The moves of the transfer not cancelled.
    pub fn live_moves(&self, env: &mut Environment) -> Result<Vec<StockMove<SingleId>>> {
        let moves: StockMove<MultipleIds> = self.get_moves(env)?;
        let mut live = Vec::new();
        for stock_move in moves {
            if !stock_move.is_status(env, MoveStatus::Cancel)? {
                live.push(stock_move);
            }
        }
        Ok(live)
    }

    /// Waiting or ready, as the moves' promises say.
    fn refresh_state(&self, env: &mut Environment) -> Result<()> {
        let moves = self.live_moves(env)?;
        let mut ready = !moves.is_empty();
        for stock_move in &moves {
            if !stock_move.is_status(env, MoveStatus::Assigned)?
                && !stock_move.is_status(env, MoveStatus::Done)?
            {
                ready = false;
            }
        }
        let state = if ready {
            PickingState::Assigned
        } else {
            PickingState::Confirmed
        };
        self.set_state(state, &mut env.sudo())
    }

    /// Validate the transfer: what was done moves, the moves done for less than asked leave a
    /// back order with the rest; nothing done at all is everything asked done.
    pub fn validate_one(&self, env: &mut Environment) -> Result<Option<StockPicking<SingleId>>> {
        if self.is_state(env, PickingState::Done)? || self.is_state(env, PickingState::Cancel)? {
            let status = if self.is_state(env, PickingState::Done)? {
                "done"
            } else {
                "cancelled"
            };
            return Err(format!("{} is already {status}", self.get_name(env)?).into());
        }
        if self.is_state(env, PickingState::Draft)? {
            StockPicking::<MultipleIds>::from_ids(vec![self.get_id()], env).action_confirm(env)?;
        }
        let moves = self.live_moves(env)?;
        if moves.is_empty() {
            return Err(format!("{} has nothing to transfer", self.get_name(env)?).into());
        }
        let mut nothing_done = true;
        for stock_move in &moves {
            if !stock_move.get_quantity(env)?.is_zero() {
                nothing_done = false;
            }
        }
        if nothing_done {
            for stock_move in &moves {
                let demand = *stock_move.get_product_uom_qty(env)?;
                stock_move.set_quantity(demand, env)?;
            }
        }
        let mut rest = Vec::new();
        for stock_move in &moves {
            let demand = *stock_move.get_product_uom_qty(env)?;
            let done = *stock_move.get_quantity(env)?;
            if done < demand {
                rest.push((stock_move.clone(), demand - done));
            }
            if done.is_zero() {
                stock_move.unreserve(env)?;
                stock_move.set_state(MoveStatus::Cancel, env)?;
            } else {
                stock_move.set_product_uom_qty(done, env)?;
                stock_move.do_move(env)?;
            }
        }
        self.set_state(PickingState::Done, &mut env.sudo())?;
        self.set_date_done(Utc::now(), &mut env.sudo())?;
        if rest.is_empty() {
            return Ok(None);
        }
        let backorder = self.copy_with(env, rest, None)?;
        backorder.set_backorder(self, env)?;
        StockPicking::<MultipleIds>::from_ids(vec![backorder.get_id()], env).action_confirm(env)?;
        Ok(Some(backorder))
    }

    /// A transfer of the same kind holding `rest`: `(move to copy, quantity)` — or, given a
    /// return type, the reverse of it.
    fn copy_with(
        &self,
        env: &mut Environment,
        rest: Vec<(StockMove<SingleId>, Decimal)>,
        reverse_with: Option<StockPickingType<SingleId>>,
    ) -> Result<StockPicking<SingleId>> {
        let picking_type: StockPickingType<SingleId> = match &reverse_with {
            Some(picking_type) => picking_type.clone(),
            None => self.get_picking_type(env)?,
        };
        let source: StockLocation<SingleId> = self.get_location(env)?;
        let destination: StockLocation<SingleId> = self.get_location_dest(env)?;
        let (source, destination) = if reverse_with.is_some() {
            (destination, source)
        } else {
            (source, destination)
        };
        let partner: Contact<SingleId> = self.get_partner(env)?;
        let mut moves = Vec::new();
        for (stock_move, quantity) in rest {
            let product: product::models::Product<SingleId> = stock_move.get_product(env)?;
            let uom: uom::models::Uom<SingleId> = stock_move.get_uom(env)?;
            let mut values = MapOfFields::default();
            values.insert("name", stock_move.get_name(env)?.clone());
            values.insert("product", product.get_id());
            values.insert("product_uom_qty", quantity);
            if let Some(uom) = uom.get_optional_id() {
                values.insert("uom", uom);
            }
            values.insert("location", source.get_id());
            values.insert("location_dest", destination.get_id());
            let price = if reverse_with.is_some() {
                // Goods coming back come back at what they left at.
                let value = *stock_move.get_value(env)?;
                let done = *stock_move.get_quantity(env)?;
                if done.is_zero() {
                    Decimal::ZERO
                } else {
                    (value.abs() / done).round_dp(6)
                }
            } else {
                *stock_move.get_price_unit(env)?
            };
            values.insert("price_unit", price);
            values.insert_option("origin", stock_move.get_origin(env)?.cloned());
            let carried = StockMove::<MultipleIds>::from_ids(vec![stock_move.get_id()], env)
                .copy_values(env)?;
            values.fields.extend(carried.fields);
            moves.push(values);
        }
        let mut values = MapOfFields::default();
        values.insert("picking_type", picking_type.get_id());
        values.insert("location", source.get_id());
        values.insert("location_dest", destination.get_id());
        if let Some(partner) = partner.get_optional_id() {
            values.insert("partner", partner);
        }
        values.insert_option("origin", self.get_origin(env)?.cloned());
        values.insert_option("scheduled_date", self.get_scheduled_date(env)?.copied());
        values.insert_field_type("moves", FieldType::Commands(vec![Command::Create(moves)]));
        env.create_new_record_from_map(values)
    }

    /// The moves of a transfer carry its source document.
    fn align_moves(&self, env: &mut Environment) -> Result<()> {
        let Some(origin) = self.get_origin(env)?.cloned() else {
            return Ok(());
        };
        for stock_move in self.live_moves(env)? {
            if stock_move.get_origin(env)?.is_none()
                && !stock_move.is_status(env, MoveStatus::Done)?
            {
                stock_move.set_origin(Some(origin.clone()), env)?;
            }
        }
        Ok(())
    }
}

#[erp_methods]
impl StockPicking<MultipleIds> {
    /// Where the operation type's transfers come from. Each location has its own compute: one
    /// filling both would undo the other when it is chosen by hand.
    pub fn compute_location(&self, env: &mut Environment) -> Result<()> {
        for picking in self {
            let picking_type: StockPickingType<SingleId> = picking.get_picking_type(env)?;
            let source: StockLocation<SingleId> = if picking_type.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                picking_type.get_default_location_src(&mut env.sudo())?
            };
            picking.set_location(&source, env)?;
        }
        Ok(())
    }

    /// Where the operation type's transfers go.
    pub fn compute_location_dest(&self, env: &mut Environment) -> Result<()> {
        for picking in self {
            let picking_type: StockPickingType<SingleId> = picking.get_picking_type(env)?;
            let destination: StockLocation<SingleId> = if picking_type.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                picking_type.get_default_location_dest(&mut env.sudo())?
            };
            picking.set_location_dest(&destination, env)?;
        }
        Ok(())
    }

    /// A transfer is numbered by its kind when created, `WH/OUT/00001`, and scheduled today;
    /// its moves go where it goes unless said otherwise.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let today = Utc::now().date_naive();
        let mut values = values;
        for picking in &mut values {
            if picking.get_option::<&NaiveDate>("scheduled_date").is_none() {
                picking.insert("scheduled_date", today);
            }
            if picking
                .get_option::<&String>("name")
                .is_none_or(|name| name == "/")
                && let Some(picking_type) = picking.get_option::<&u32>("picking_type").copied()
            {
                let picking_type: StockPickingType<SingleId> = env.get_record(picking_type.into());
                let numbering: Sequence<SingleId> = picking_type.get_sequence(&mut env.sudo())?;
                if !numbering.is_empty() {
                    picking.insert("name", numbering.next(env, today)?);
                }
            }
        }
        let ids: MultipleIds = sup.call_with(values, env)?;
        for picking in StockPicking::<MultipleIds>::from_ids(ids.clone(), env) {
            picking.align_moves(env)?;
        }
        Ok(ids)
    }

    /// A done or cancelled transfer stays as it was.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        const KEPT: [&str; 4] = ["picking_type", "location", "location_dest", "moves"];
        if values
            .fields
            .keys()
            .any(|field| KEPT.contains(&field.as_str()))
        {
            for picking in self {
                if picking.is_state(env, PickingState::Done)?
                    || picking.is_state(env, PickingState::Cancel)?
                {
                    return Err(
                        format!("{} is closed: it cannot change", picking.get_name(env)?).into(),
                    );
                }
            }
        }
        let moved = values.contains_key("origin") || values.contains_key("moves");
        sup.call_with(values, env)?;
        if moved {
            for picking in self {
                picking.align_moves(env)?;
            }
        }
        Ok(())
    }

    /// Only a draft or cancelled transfer is deleted.
    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        for picking in self {
            if !picking.is_state(env, PickingState::Draft)?
                && !picking.is_state(env, PickingState::Cancel)?
            {
                return Err(format!(
                    "{} is under way: cancel it rather than delete it",
                    picking.get_name(env)?
                )
                .into());
            }
        }
        sup.call(env)
    }

    /// Plan the transfers: their moves wait for the goods, and promise what there is.
    #[erp(rpc)]
    pub fn action_confirm(&self, env: &mut Environment) -> Result<bool> {
        for picking in self {
            if !picking.is_state(env, PickingState::Draft)? {
                continue;
            }
            for stock_move in picking.live_moves(env)? {
                if stock_move.is_status(env, MoveStatus::Draft)? {
                    stock_move.set_state(MoveStatus::Confirmed, env)?;
                }
            }
            picking.set_state(PickingState::Confirmed, &mut env.sudo())?;
        }
        self.action_assign(env)
    }

    /// Check what is available: each move promises what it can.
    #[erp(rpc)]
    pub fn action_assign(&self, env: &mut Environment) -> Result<bool> {
        for picking in self {
            if picking.is_state(env, PickingState::Done)?
                || picking.is_state(env, PickingState::Cancel)?
                || picking.is_state(env, PickingState::Draft)?
            {
                continue;
            }
            for stock_move in picking.live_moves(env)? {
                stock_move.reserve(env)?;
            }
            picking.refresh_state(env)?;
        }
        Ok(true)
    }

    /// Release what the transfers promised.
    #[erp(rpc)]
    pub fn do_unreserve(&self, env: &mut Environment) -> Result<bool> {
        for picking in self {
            for stock_move in picking.live_moves(env)? {
                if !stock_move.is_status(env, MoveStatus::Done)? {
                    stock_move.unreserve(env)?;
                    stock_move.set_state(MoveStatus::Confirmed, env)?;
                }
            }
            if !picking.is_state(env, PickingState::Draft)?
                && !picking.is_state(env, PickingState::Done)?
            {
                picking.set_state(PickingState::Confirmed, &mut env.sudo())?;
            }
        }
        Ok(true)
    }

    /// Validate the transfers; one done in part opens its back order.
    #[erp(rpc)]
    pub fn button_validate(&self, env: &mut Environment) -> Result<Value> {
        let backorders = env.savepoint(|env| {
            let mut backorders = Vec::new();
            for picking in self {
                if let Some(backorder) = picking.validate_one(env)? {
                    backorders.push(backorder.get_id());
                }
            }
            Self::from_ids(self.get_ids_ref().clone(), env).on_done(env)?;
            Ok(backorders)
        })?;
        Ok(match backorders.as_slice() {
            [single] => json!({"type": "open", "action": "stock.action_pickings", "id": single}),
            _ => json!(true),
        })
    }

    /// Cancel the transfers not done: their moves release what they promised.
    #[erp(rpc)]
    pub fn action_cancel(&self, env: &mut Environment) -> Result<bool> {
        env.savepoint(|env| {
            for picking in self {
                if picking.is_state(env, PickingState::Done)? {
                    return Err(format!(
                        "{} is done: return it rather than cancel it",
                        picking.get_name(env)?
                    )
                    .into());
                }
                for stock_move in picking.live_moves(env)? {
                    stock_move.unreserve(env)?;
                    stock_move.set_state(MoveStatus::Cancel, env)?;
                }
                picking.set_state(PickingState::Cancel, &mut env.sudo())?;
            }
            Ok(true)
        })
    }

    /// Return what a done transfer moved: a new transfer the other way, of the warehouse's
    /// receipts for a delivery and its deliveries for a receipt, ready to validate.
    #[erp(rpc)]
    pub fn action_return(&self, env: &mut Environment) -> Result<Value> {
        let mut made = Vec::new();
        env.savepoint(|env| {
            for picking in self {
                if !picking.is_state(env, PickingState::Done)? {
                    return Err(format!(
                        "{} is not done: there is nothing to return",
                        picking.get_name(env)?
                    )
                    .into());
                }
                let picking_type: StockPickingType<SingleId> = picking.get_picking_type(env)?;
                let warehouse: StockWarehouse<SingleId> =
                    picking_type.get_warehouse(&mut env.sudo())?;
                let return_type: StockPickingType<SingleId> = {
                    let env = &mut *env.sudo();
                    match *picking_type.get_code(env)? {
                        PickingKind::Outgoing => warehouse.get_in_type(env)?,
                        PickingKind::Incoming => warehouse.get_out_type(env)?,
                        _ => picking_type.clone(),
                    }
                };
                let mut rest = Vec::new();
                for stock_move in picking.live_moves(env)? {
                    let done = *stock_move.get_quantity(env)?;
                    if !done.is_zero() {
                        rest.push((stock_move, done));
                    }
                }
                let returned = picking.copy_with(env, rest, Some(return_type))?;
                returned.set_returned_picking(&picking, env)?;
                let origin = format!("Return of {}", picking.get_name(env)?);
                returned.set_origin(Some(origin), env)?;
                Self::from_ids(vec![returned.get_id()], env).action_confirm(env)?;
                made.push(returned.get_id());
            }
            Ok(())
        })?;
        Ok(match made.as_slice() {
            [single] => json!({"type": "open", "action": "stock.action_pickings", "id": single}),
            [] => json!({"type": "reload"}),
            several => json!({"type": "open", "action": "stock.action_pickings", "ids": several}),
        })
    }

    /// What follows transfers done; sales and purchases count what was delivered or received.
    pub fn on_done(&self, _env: &mut Environment) -> Result<()> {
        Ok(())
    }
}
