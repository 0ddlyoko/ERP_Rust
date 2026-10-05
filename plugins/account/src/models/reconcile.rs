use crate::matching::{self, Open};
use crate::models::account::Account;
use crate::models::company::CompanyAccount;
use crate::models::journal::Journal;
use crate::models::move_line::{BaseAccountMoveLine, MoveLine};
use crate::models::moves::{Move, MoveState};
use base::models::Contact;
use code_gen::Model;
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Command, FieldType};
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use erp::types::model::MapOfFields;

/// A debit and a credit settling each other for an amount: an invoice and part of its payment.
#[derive(Model)]
#[erp(id = "account_partial_reconcile")]
#[allow(dead_code)]
pub struct PartialReconcile<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    debit_line: Reference<BaseAccountMoveLine, SingleId>,
    #[erp(required, ondelete = "cascade")]
    credit_line: Reference<BaseAccountMoveLine, SingleId>,
    #[erp(description = "In the company's currency")]
    amount: Decimal,
    #[erp(label = "Amount of the debit in its currency", default = 0.0)]
    debit_amount_currency: Decimal,
    #[erp(label = "Amount of the credit in its currency", default = 0.0)]
    credit_amount_currency: Decimal,
    #[erp(ondelete = "set_null")]
    full_reconcile: Reference<BaseAccountFullReconcile, SingleId>,
}

/// Journal items fully settling each other, under one matching number.
#[derive(Model)]
#[erp(id = "account_full_reconcile")]
#[allow(dead_code)]
pub struct FullReconcile<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Matching number")]
    name: String,
    #[erp(label = "Partial matchings", inverse = "full_reconcile")]
    partials: Reference<BaseAccountPartialReconcile, MultipleIds>,
    #[erp(label = "Matched items", inverse = "full_reconcile")]
    lines: Reference<BaseAccountMoveLine, MultipleIds>,
}

impl MoveLine<MultipleIds> {
    /// Settle these journal items against each other: posted, on one account allowing it, none
    /// already settled. Debits and credits are matched oldest first; once all of them are
    /// settled they share a matching number. In a foreign currency, what the rates leave
    /// between them in the company's currency is booked as an exchange difference.
    pub fn reconcile_lines(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let mut account = None;
        let mut foreign: Option<u32> = None;
        let mut all_same_currency = true;
        let mut debits: Vec<(NaiveDate, u32, Decimal, Decimal)> = Vec::new();
        let mut credits: Vec<(NaiveDate, u32, Decimal, Decimal)> = Vec::new();
        for line in self {
            let entry: Move<SingleId> = line.get_move_id(env)?;
            if !matches!(*entry.get_state(env)?, MoveState::Posted) {
                return Err(format!(
                    "{} is not posted: it cannot be matched",
                    entry.get_name(env)?
                )
                .into());
            }
            let line_account: Account<SingleId> = line.get_account(env)?;
            if !*line_account.get_reconcile(env)? {
                return Err(format!(
                    "The account {} does not allow matching",
                    line_account.get_display_name(env)?
                )
                .into());
            }
            match account {
                None => account = Some(line_account.get_id()),
                Some(id) if id != line_account.get_id() => {
                    return Err("Only journal items of the same account can be matched".into());
                }
                _ => {}
            }
            if *line.get_reconciled(env)? {
                continue;
            }
            let currency: Currency<SingleId> = line.get_currency(env)?;
            let in_foreign = line.in_foreign_currency(env)?;
            match (foreign, in_foreign) {
                (None, true) if debits.is_empty() && credits.is_empty() => {
                    foreign = Some(currency.get_id())
                }
                (Some(id), true) if id == currency.get_id() => {}
                (None, false) => {}
                _ => all_same_currency = false,
            }
            let residual = *line.get_amount_residual(env)?;
            let residual_currency = *line.get_amount_residual_currency(env)?;
            let date = line
                .get_date_maturity(env)?
                .copied()
                .or(line.get_date(env)?.copied())
                .unwrap_or_default();
            if residual > Decimal::ZERO || (residual.is_zero() && residual_currency > Decimal::ZERO)
            {
                debits.push((date, line.get_id(), residual, residual_currency));
            } else if residual < Decimal::ZERO || residual_currency < Decimal::ZERO {
                credits.push((date, line.get_id(), -residual, -residual_currency));
            }
        }
        let by_currency = foreign.is_some() && all_same_currency;
        debits.sort_by_key(|(date, id, _, _)| (*date, *id));
        credits.sort_by_key(|(date, id, _, _)| (*date, *id));
        let open = |rows: &[(NaiveDate, u32, Decimal, Decimal)]| -> Vec<Open> {
            rows.iter()
                .map(|(_, id, residual, residual_currency)| Open {
                    id: *id,
                    residual: if by_currency {
                        *residual_currency
                    } else {
                        *residual
                    },
                })
                .collect()
        };
        let pairs = matching::match_lines(&open(&debits), &open(&credits));
        let mut touched: Vec<u32> = self.get_ids_ref().clone();
        for (debit, credit, matched) in pairs {
            let debit_row = debits
                .iter()
                .find(|row| row.1 == debit)
                .copied()
                .expect("a debit");
            let credit_row = credits
                .iter()
                .find(|row| row.1 == credit)
                .copied()
                .expect("a credit");
            let (amount, debit_currency, credit_currency) = if by_currency {
                // In the company's currency, each side counts the matched amount at its own rate.
                let debit_side = prorata(debit_row.2, debit_row.3, matched);
                let credit_side = prorata(credit_row.2, credit_row.3, matched);
                (debit_side.min(credit_side), matched, matched)
            } else {
                (
                    matched,
                    prorata(debit_row.3, debit_row.2, matched),
                    prorata(credit_row.3, credit_row.2, matched),
                )
            };
            let mut values = MapOfFields::default();
            values.insert("debit_line", debit);
            values.insert("credit_line", credit);
            values.insert("amount", amount);
            values.insert("debit_amount_currency", debit_currency);
            values.insert("credit_amount_currency", credit_currency);
            env.create_records("account_partial_reconcile", vec![values])?;
            touched.push(debit);
            touched.push(credit);
            let lines: MoveLine<MultipleIds> = MoveLine::from_ids(vec![debit, credit], env);
            lines.refresh_residuals(env)?;
        }
        touched.sort_unstable();
        touched.dedup();
        let touched: MoveLine<MultipleIds> = MoveLine::from_ids(touched, env);
        if by_currency {
            touched.book_exchange_difference(env)?;
        }
        touched.number_if_settled(env)
    }

    /// Book what is left in the company's currency on items settled in their own currency:
    /// the difference the rates made between the invoice and its payment.
    pub fn book_exchange_difference(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let mut leftovers = Vec::new();
        for line in self {
            let residual = *line.get_amount_residual(env)?;
            if !residual.is_zero() && line.get_amount_residual_currency(env)?.is_zero() {
                leftovers.push((line.get_id(), residual));
            }
        }
        if leftovers.is_empty() {
            return Ok(());
        }
        let company = CompanyAccount::current(env)?;
        let journal: Journal<SingleId> = company.exchange_journal(env)?;
        if journal.is_empty() {
            return Err("Set the exchange difference journal of the company to match items in a foreign currency".into());
        }
        let mut items = Vec::new();
        let mut gain = Decimal::ZERO;
        let mut date = NaiveDate::default();
        for (id, residual) in &leftovers {
            let line: MoveLine<SingleId> = env.get_record((*id).into());
            let account: Account<SingleId> = line.get_account(env)?;
            let partner: Contact<SingleId> = line.get_partner(env)?;
            let currency: Currency<SingleId> = line.get_currency(env)?;
            if let Some(line_date) = line.get_date(env)?.copied() {
                date = date.max(line_date);
            }
            // The item is brought to nothing: a debit left open is credited, and the other way.
            let mut item = MapOfFields::default();
            item.insert("account", account.get_id());
            if let Some(partner) = partner.get_optional_id() {
                item.insert("partner", partner);
            }
            item.insert("name", "Exchange difference");
            item.insert("currency", currency.get_id());
            item.insert("amount_currency", Decimal::ZERO);
            if *residual > Decimal::ZERO {
                item.insert("credit", *residual);
            } else {
                item.insert("debit", -*residual);
            }
            items.push(item);
            gain += *residual;
        }
        // A receivable left open in the company's favour is a loss: what was owed is worth less.
        let (counterpart, debit, credit) = if gain > Decimal::ZERO {
            (
                company.required_account(env, "exchange_loss")?,
                gain,
                Decimal::ZERO,
            )
        } else {
            (
                company.required_account(env, "exchange_gain")?,
                Decimal::ZERO,
                -gain,
            )
        };
        let mut item = MapOfFields::default();
        item.insert("account", counterpart.get_id());
        item.insert("name", "Exchange difference");
        item.insert("debit", debit);
        item.insert("credit", credit);
        items.push(item);

        let mut entry = MapOfFields::default();
        entry.insert("journal", journal.get_id());
        entry.insert("date", date);
        entry.insert("reference", "Exchange difference");
        entry.insert_field_type("lines", FieldType::Commands(vec![Command::Create(items)]));
        let entry: Move<MultipleIds> = env.create_new_records_from_maps(vec![entry])?;
        entry.action_post(env)?;
        for (id, _) in leftovers {
            let line: MoveLine<SingleId> = env.get_record(id.into());
            let account: Account<SingleId> = line.get_account(env)?;
            let exchange_lines: MoveLine<MultipleIds> =
                env.search(&erp_search_code_gen::make_domain!([
                    ("move_id", "in", entry.get_ids_ref().clone()),
                    ("account", "=", account.get_id())
                ]))?;
            let mut pair = vec![id];
            pair.extend(exchange_lines.get_ids_ref().iter().copied());
            let pair: MoveLine<MultipleIds> = MoveLine::from_ids(pair, env);
            pair.reconcile_in_company_currency(env)?;
        }
        Ok(())
    }

    /// Match items on their company amounts only: an item left open by exchange rates with the
    /// entry booking the difference.
    pub fn reconcile_in_company_currency(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let mut debits = Vec::new();
        let mut credits = Vec::new();
        for line in self {
            let residual = *line.get_amount_residual(env)?;
            if residual > Decimal::ZERO {
                debits.push(Open {
                    id: line.get_id(),
                    residual,
                });
            } else if residual < Decimal::ZERO {
                credits.push(Open {
                    id: line.get_id(),
                    residual: -residual,
                });
            }
        }
        for (debit, credit, amount) in matching::match_lines(&debits, &credits) {
            let mut values = MapOfFields::default();
            values.insert("debit_line", debit);
            values.insert("credit_line", credit);
            values.insert("amount", amount);
            env.create_records("account_partial_reconcile", vec![values])?;
            MoveLine::<MultipleIds>::from_ids(vec![debit, credit], env).refresh_residuals(env)?;
        }
        self.number_if_settled(env)
    }

    /// Give the items settling each other a matching number once all of them are settled.
    pub fn number_if_settled(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let group = self.matched_group(env)?;
        if group.is_empty() {
            return Ok(());
        }
        let lines: MoveLine<MultipleIds> = MoveLine::from_ids(group.clone(), env);
        for line in &lines {
            if !*line.get_reconciled(env)? {
                return Ok(());
            }
        }
        let partials: PartialReconcile<MultipleIds> =
            env.search(&erp_search_code_gen::make_domain!([
                "|",
                ("debit_line", "in", group.clone()),
                ("credit_line", "in", group.clone())
            ]))?;
        let existing: FullReconcile<SingleId> = lines
            .into_iter()
            .next()
            .map(|line| line.get_full_reconcile(env))
            .transpose()?
            .unwrap_or_else(|| env.get_record(SingleId::empty()));
        let full = if existing.is_empty() {
            let mut values = MapOfFields::default();
            values.insert("name", "/");
            let full: FullReconcile<SingleId> = env.create_new_record_from_map(values)?;
            full.set_name(format!("M{:05}", full.get_id()), env)?;
            full
        } else {
            existing
        };
        let lines: MoveLine<MultipleIds> = MoveLine::from_ids(group, env);
        lines.set_full_reconcile(&full, env)?;
        partials.set_full_reconcile(&full, env)?;
        Ok(())
    }

    /// Every item linked to these through matchings, these included.
    pub fn matched_group(&self, env: &mut Environment) -> Result<Vec<u32>> {
        let env = &mut *env.sudo();
        let mut group: Vec<u32> = Vec::new();
        let mut todo: Vec<u32> = self.get_ids_ref().clone();
        while let Some(id) = todo.pop() {
            if group.contains(&id) {
                continue;
            }
            group.push(id);
            let line: MoveLine<SingleId> = env.get_record(id.into());
            let as_debit: PartialReconcile<MultipleIds> = line.get_matched_credits(env)?;
            for partial in &as_debit {
                let other: MoveLine<SingleId> = partial.get_credit_line(env)?;
                todo.push(other.get_id());
            }
            let as_credit: PartialReconcile<MultipleIds> = line.get_matched_debits(env)?;
            for partial in &as_credit {
                let other: MoveLine<SingleId> = partial.get_debit_line(env)?;
                todo.push(other.get_id());
            }
        }
        group.sort_unstable();
        Ok(group)
    }

    /// Undo the matchings of these items, and of the items matched with them: all of them open
    /// again.
    pub fn unreconcile_lines(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let group = self.matched_group(env)?;
        let partials: PartialReconcile<MultipleIds> =
            env.search(&erp_search_code_gen::make_domain!([
                "|",
                ("debit_line", "in", group.clone()),
                ("credit_line", "in", group.clone())
            ]))?;
        let lines: MoveLine<MultipleIds> = MoveLine::from_ids(group, env);
        let fulls: Vec<u32> = {
            let mut fulls = Vec::new();
            for line in &lines {
                let full: FullReconcile<SingleId> = line.get_full_reconcile(env)?;
                if let Some(id) = full.get_optional_id() {
                    fulls.push(id);
                }
            }
            fulls.sort_unstable();
            fulls.dedup();
            fulls
        };
        env.delete(
            "account_partial_reconcile",
            &MultipleIds::from(partials.get_ids_ref().clone()),
        )?;
        if !fulls.is_empty() {
            env.delete("account_full_reconcile", &MultipleIds::from(fulls))?;
        }
        lines.refresh_residuals(env)
    }
}

/// `matched` of an item worth `amount` in one currency and `other` in the other, in that other.
fn prorata(other: Decimal, amount: Decimal, matched: Decimal) -> Decimal {
    if amount.is_zero() {
        return Decimal::ZERO;
    }
    (other * matched / amount).round_dp(2)
}
