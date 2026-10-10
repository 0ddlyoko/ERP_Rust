use crate::models::account::{Account, BaseAccount};
use crate::models::company::CompanyAccount;
use crate::models::fiscal_position::FiscalPosition;
use crate::models::journal::Journal;
use crate::models::moves::{BaseAccountMove, Move, MoveType};
use crate::models::product::{ProductAccount, ProductCategoryAccount};
use crate::models::tax::{BaseAccountTax, Tax, TaxDocument};
use crate::tax_engine::{self, LineInput, LineResult};
use code_gen::{Model, erp_methods};
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId, Utc};
use product::models::{BaseProduct, Product};
use uom::models::{BaseUom, Uom};

/// A line of an invoice: a product or a description, how many, at what price, with which taxes.
///
/// What the product says fills it in — label, account, price, taxes, unit — and anything filled
/// in may be changed by hand.
#[derive(Model)]
#[erp(
    id = "account_invoice_line",
    order = "sequence, id",
    name_field = "name",
    methods
)]
#[allow(dead_code)]
pub struct InvoiceLine<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    move_id: Reference<BaseAccountMove, SingleId>,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(ondelete = "restrict")]
    product: Reference<BaseProduct, SingleId>,
    #[erp(
        label = "Label",
        compute = "compute_name",
        depends = ["product"],
        stored,
        editable
    )]
    name: Option<String>,
    #[erp(
        ondelete = "restrict",
        compute = "compute_account",
        depends = ["product", "move_id.journal", "move_id.fiscal_position"],
        stored,
        editable
    )]
    account: Reference<BaseAccount, SingleId>,
    #[erp(default = 1.0)]
    quantity: Decimal,
    #[erp(
        label = "Unit",
        ondelete = "restrict",
        compute = "compute_uom",
        depends = ["product"],
        stored,
        editable
    )]
    uom: Reference<BaseUom, SingleId>,
    #[erp(
        label = "Unit price",
        compute = "compute_price_unit",
        depends = ["product", "move_id.currency"],
        stored,
        editable
    )]
    price_unit: Decimal,
    #[erp(label = "Discount (%)", default = 0.0)]
    discount: Decimal,
    #[erp(
        compute = "compute_taxes",
        depends = ["product"],
        stored,
        editable,
        relation = "account_invoice_line_tax_rel"
    )]
    taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(
        label = "Untaxed",
        compute = "compute_amounts",
        depends = ["quantity", "price_unit", "discount", "taxes", "move_id.currency"],
        stored
    )]
    price_subtotal: Decimal,
    #[erp(
        label = "Tax",
        compute = "compute_amounts",
        depends = ["quantity", "price_unit", "discount", "taxes", "move_id.currency"],
        stored
    )]
    price_tax: Decimal,
    #[erp(
        label = "Total",
        compute = "compute_amounts",
        depends = ["quantity", "price_unit", "discount", "taxes", "move_id.currency"],
        stored
    )]
    price_total: Decimal,
}

#[erp_methods]
impl InvoiceLine<SingleId> {
    fn invoice(&self, env: &mut Environment) -> Result<Move<SingleId>> {
        self.get_move_id(env)
    }

    /// The line taxed, rounded in the invoice's currency.
    pub fn taxed(&self, env: &mut Environment, document: TaxDocument) -> Result<LineResult> {
        let invoice = self.invoice(env)?;
        let currency = invoice.currency_or_company(env)?;
        let rounding = *currency.get_rounding(&mut env.sudo())?;
        let taxes: Tax<MultipleIds> = self.get_taxes(env)?;
        let mut specs = Vec::new();
        for tax in &taxes {
            specs.push(tax.spec(env, document)?);
        }
        Ok(tax_engine::compute(&LineInput {
            price_unit: *self.get_price_unit(env)?,
            quantity: *self.get_quantity(env)?,
            discount: *self.get_discount(env)?,
            taxes: specs,
            rounding,
        })?)
    }

    /// Whether the line belongs to a sale, priced and taxed for customers.
    pub fn is_sale(&self, env: &mut Environment) -> Result<bool> {
        let invoice = self.invoice(env)?;
        Ok(matches!(
            *invoice.get_move_type(env)?,
            MoveType::OutInvoice | MoveType::OutRefund
        ))
    }
}

#[erp_methods]
impl InvoiceLine<MultipleIds> {
    /// The product's description for this side of the business, else its name.
    pub fn compute_name(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            if product.is_empty() {
                line.set_name(None::<String>, env)?;
                continue;
            }
            let invoice = line.invoice(env)?;
            let sale = invoice.is_sale_document(env)?;
            let env = &mut *env.sudo();
            let description = if sale {
                product.get_description_sale(env)?.cloned()
            } else {
                product.get_description_purchase(env)?.cloned()
            };
            let name = product.get_display_name(env)?.clone();
            let label = match description {
                Some(description) if !description.trim().is_empty() => {
                    format!("{name}\n{description}")
                }
                _ => name,
            };
            line.set_name(Some(label), env)?;
        }
        Ok(())
    }

    /// The product's income or expense account, else its category's, else the journal's, else
    /// the company's — as the invoice's fiscal position maps it.
    pub fn compute_account(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let invoice = line.invoice(env)?;
            if invoice.is_empty() {
                line.set_account(None::<&Account<SingleId>>, env)?;
                continue;
            }
            let sale = invoice.is_sale_document(env)?;
            let product: Product<SingleId> = line.get_product(env)?;
            let mut account: Account<SingleId> = env.get_record(SingleId::empty());
            {
                let env = &mut *env.sudo();
                if !product.is_empty() {
                    let product: ProductAccount<SingleId> = product.as_model();
                    account = if sale {
                        product.get_income_account(env)?
                    } else {
                        product.get_expense_account(env)?
                    };
                    if account.is_empty() {
                        let product: Product<SingleId> = product.as_model();
                        let category: product::models::ProductCategory<SingleId> =
                            product.get_category(env)?;
                        let category: ProductCategoryAccount<SingleId> = category.as_model();
                        account = if sale {
                            category.get_income_account(env)?
                        } else {
                            category.get_expense_account(env)?
                        };
                    }
                }
                if account.is_empty() {
                    let journal: Journal<SingleId> = invoice.get_journal(env)?;
                    if !journal.is_empty() {
                        account = journal.get_default_account(env)?;
                    }
                }
                if account.is_empty() {
                    let company = CompanyAccount::current(env)?;
                    account = if sale {
                        company.get_account_income(env)?
                    } else {
                        company.get_account_expense(env)?
                    };
                }
            }
            let position: FiscalPosition<SingleId> = invoice.get_fiscal_position(env)?;
            let account = position.map_account(env, account)?;
            line.set_account(&account, env)?;
        }
        Ok(())
    }

    /// The unit the product is sold in, or bought in.
    pub fn compute_uom(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            if product.is_empty() {
                line.set_uom(None::<&Uom<SingleId>>, env)?;
                continue;
            }
            let sale = line.invoice(env)?.is_sale_document(env)?;
            let uom: Uom<SingleId> = env.sudo_with(|env| {
                if sale {
                    product.get_uom(env)
                } else {
                    product.get_purchase_uom(env)
                }
            })?;
            line.set_uom(&uom, env)?;
        }
        Ok(())
    }

    /// The product's sales price, or its cost on a vendor bill, for the line's unit, in the
    /// invoice's currency at the invoice's date. Changing the unit afterwards keeps the price:
    /// the unit is often itself just filled in from the product.
    pub fn compute_price_unit(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            if product.is_empty() {
                line.set_price_unit(Decimal::ZERO, env)?;
                continue;
            }
            let invoice = line.invoice(env)?;
            let sale = invoice.is_sale_document(env)?;
            let uom: Uom<SingleId> = line.get_uom(env)?;
            let price = {
                let env = &mut *env.sudo();
                let price = if sale {
                    *product.get_list_price(env)?
                } else {
                    *product.get_standard_price(env)?
                };
                let product_uom: Uom<SingleId> = product.get_uom(env)?;
                if uom.is_empty() || uom.get_id() == product_uom.get_id() {
                    price
                } else {
                    // A price per kg is, per gram, a thousandth of it: the price converts as one unit does.
                    let ratio_line = *uom.get_ratio(env)?;
                    let ratio_product = *product_uom.get_ratio(env)?;
                    price * ratio_line / ratio_product
                }
            };
            let company_currency = Currency::of_company(env)?;
            let currency = invoice.currency_or_company(env)?;
            let date = invoice
                .get_invoice_date(env)?
                .copied()
                .unwrap_or_else(|| Utc::now().date_naive());
            let price = if currency.get_id() == company_currency.get_id() {
                price
            } else {
                let from = company_currency.rate_at(env, date)?;
                let to = currency.rate_at(env, date)?;
                price * to / from
            };
            line.set_price_unit(price, env)?;
        }
        Ok(())
    }

    /// The product's customer or vendor taxes, as the invoice's fiscal position maps them; none
    /// without a product. Taxes given by hand stay until the product changes.
    pub fn compute_taxes(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let product: Product<SingleId> = line.get_product(env)?;
            if product.is_empty() {
                line.set_taxes(&Tax::<MultipleIds>::empty(env), env)?;
                continue;
            }
            let invoice = line.invoice(env)?;
            let sale = invoice.is_sale_document(env)?;
            let taxes: Tax<MultipleIds> = env.sudo_with(|env| {
                let product: ProductAccount<SingleId> = product.as_model();
                if sale {
                    product.get_taxes(env)
                } else {
                    product.get_supplier_taxes(env)
                }
            })?;
            let position: FiscalPosition<SingleId> = invoice.get_fiscal_position(env)?;
            let mapped = position.map_taxes(env, taxes.get_ids_ref().clone())?;
            let mapped: Tax<MultipleIds> = Tax::from_ids(mapped, env);
            line.set_taxes(&mapped, env)?;
        }
        Ok(())
    }

    /// The untaxed amount, the tax and the total of each line.
    pub fn compute_amounts(&self, env: &mut Environment) -> Result<()> {
        for line in self {
            let invoice = line.invoice(env)?;
            if invoice.is_empty() {
                line.set_price_subtotal(Decimal::ZERO, env)?;
                line.set_price_tax(Decimal::ZERO, env)?;
                line.set_price_total(Decimal::ZERO, env)?;
                continue;
            }
            let result = line.taxed(env, TaxDocument::Invoice)?;
            line.set_price_subtotal(result.subtotal, env)?;
            line.set_price_tax(result.total - result.subtotal, env)?;
            line.set_price_total(result.total, env)?;
        }
        Ok(())
    }

    /// Lines of a posted invoice are what was declared: they change only once it is back to
    /// draft.
    pub fn write(
        &self,
        env: &mut Environment,
        values: erp::types::model::MapOfFields,
        sup: Super,
    ) -> Result<()> {
        for line in self {
            let invoice = line.invoice(env)?;
            if !invoice.is_empty() && !invoice.is_draft(env)? {
                return Err(format!(
                    "{} is no longer a draft: reset it to draft to change its lines",
                    invoice.get_name(env)?
                )
                .into());
            }
        }
        sup.call_with(values, env)
    }
}
