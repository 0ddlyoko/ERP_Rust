use account::models::{Account, BaseAccount, BaseAccountJournal, BaseAccountMove, Journal, Move};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::data;
use erp::environment::Environment;
use erp::types::field::{
    Command, Decimal, FieldType, IdMode, MultipleIds, Reference, SingleId, Utc,
};
use erp::types::model::MapOfFields;
use product::models::{Product, ProductCategory};
use stock::models::{Location, LocationUsage, StockMove};

/// Where the value of a category's stock is kept, and what it moves against.
#[derive(Model)]
#[erp(id = "product_category")]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductCategoryValuation<Mode: IdMode> {
    id: Mode,
    #[erp(
        label = "Stock valuation account",
        ondelete = "restrict",
        description = "What the stock is worth; left empty, moves are not booked"
    )]
    stock_valuation_account: Reference<BaseAccount, SingleId>,
    #[erp(
        label = "Stock input account",
        ondelete = "restrict",
        description = "Credited for goods received, debited by their vendor bill"
    )]
    stock_input_account: Reference<BaseAccount, SingleId>,
    #[erp(
        label = "Stock output account",
        ondelete = "restrict",
        description = "Debited for goods leaving the stock: the cost of what is sold"
    )]
    stock_output_account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Stock journal", ondelete = "restrict")]
    stock_journal: Reference<BaseAccountJournal, SingleId>,
}

/// The journal entry booking what a move did to the stock's value.
#[derive(Model)]
#[erp(id = "stock_move", methods)]
#[erp(derived_model = "stock::models")]
#[allow(dead_code)]
pub struct StockMoveAccount<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Valuation entry", ondelete = "set_null")]
    account_move: Reference<BaseAccountMove, SingleId>,
}

/// Where a category's stock moves are booked.
struct Valuation {
    valuation: Account<SingleId>,
    input: Account<SingleId>,
    output: Account<SingleId>,
    journal: Journal<SingleId>,
}

/// The accounts and journal valuing `product`'s stock; `None` when its category books nothing.
fn valuation_of(env: &mut Environment, product: &Product<SingleId>) -> Result<Option<Valuation>> {
    let env = &mut *env.sudo();
    let category: ProductCategory<SingleId> = product.get_category(env)?;
    let name = category.get_complete_name(env)?.clone();
    let category: ProductCategoryValuation<SingleId> = env.get_record(category.get_id().into());
    let valuation: Account<SingleId> = category.get_stock_valuation_account(env)?;
    if valuation.is_empty() {
        return Ok(None);
    }
    let input: Account<SingleId> = category.get_stock_input_account(env)?;
    let output: Account<SingleId> = category.get_stock_output_account(env)?;
    if input.is_empty() || output.is_empty() {
        return Err(format!(
            "The category {name} values its stock: set its stock input and output accounts too"
        )
        .into());
    }
    let mut journal: Journal<SingleId> = category.get_stock_journal(env)?;
    if journal.is_empty() {
        let id = data::resolve(env, "stock_account.journal_stock")?
            .ok_or("The stock journal is missing")?;
        journal = env.get_record(id.into());
    }
    Ok(Some(Valuation {
        valuation,
        input,
        output,
        journal,
    }))
}

#[erp_methods]
impl StockMoveAccount<MultipleIds> {
    /// Book the value a move added to the stock or took from it: against the stock input
    /// account for goods coming from or going back to a vendor, against the stock output
    /// account for the others — sales, their returns, inventory differences. Nothing for a
    /// category not valued in accounting.
    pub fn on_moved(&self, env: &mut Environment, sup: Super) -> Result<()> {
        sup.call(env)?;
        for stock_move in self {
            let record: StockMove<SingleId> = env.get_record(stock_move.get_id().into());
            let value = *record.get_value(env)?;
            if value.is_zero() {
                continue;
            }
            let product: Product<SingleId> = record.get_product(env)?;
            let Some(Valuation {
                valuation,
                input,
                output,
                journal,
            }) = valuation_of(env, &product)?
            else {
                continue;
            };
            let source = match record.get_origin(env)?.cloned() {
                Some(origin) => origin,
                None => record.get_name(env)?.clone(),
            };
            let label = format!("{source} — {}", product.get_display_name(&mut env.sudo())?);
            let (source, destination): (Location<SingleId>, Location<SingleId>) = {
                let env = &mut *env.sudo();
                (record.get_location(env)?, record.get_location_dest(env)?)
            };
            let vendor_side = {
                let env = &mut *env.sudo();
                let usage =
                    |location: &Location<SingleId>, env: &mut Environment| -> Result<bool> {
                        Ok(!location.is_empty()
                            && matches!(*location.get_usage(env)?, LocationUsage::Supplier))
                    };
                usage(&source, env)? || usage(&destination, env)?
            };
            let counterpart = if vendor_side { input } else { output };
            let (debit_account, credit_account, amount) = if value > Decimal::ZERO {
                (valuation, counterpart, value)
            } else {
                (counterpart, valuation, -value)
            };
            let item = |account: &Account<SingleId>, debit: Decimal, credit: Decimal| {
                let mut item = MapOfFields::default();
                item.insert("account", account.get_id());
                item.insert("name", label.clone());
                item.insert("product", product.get_id());
                item.insert("debit", debit);
                item.insert("credit", credit);
                item
            };
            let mut entry = MapOfFields::default();
            entry.insert("journal", journal.get_id());
            entry.insert("date", Utc::now().date_naive());
            entry.insert("reference", label.clone());
            entry.insert_field_type(
                "lines",
                FieldType::Commands(vec![Command::Create(vec![
                    item(&debit_account, amount, Decimal::ZERO),
                    item(&credit_account, Decimal::ZERO, amount),
                ])]),
            );
            let env = &mut *env.sudo();
            let entry: Move<MultipleIds> = env.create_new_records_from_maps(vec![entry])?;
            entry.action_post(env)?;
            stock_move.set_account_move(
                Reference::<BaseAccountMove, SingleId>::from(entry.get_ids_ref()[0]),
                env,
            )?;
        }
        Ok(())
    }
}
