use crate::models::l10n_be_vat_return_line::{BaseL10nBeVatReturnLine, L10nBeVatReturnLine};
use account::models::{AccountMoveLine, AccountTaxTag, MoveState};
use base::models::{Company, Contact};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::model::ModelVerbs;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use period::period_of;
use std::collections::BTreeMap;

/// The grids of the periodic VAT return, in the order of the form, with their labels.
pub const GRIDS: [(&str, &str); 29] = [
    ("00", "Opérations soumises à un régime particulier"),
    ("01", "Opérations à 6 %"),
    ("02", "Opérations à 12 %"),
    ("03", "Opérations à 21 %"),
    ("44", "Services intracommunautaires"),
    ("45", "Opérations avec TVA due par le cocontractant"),
    ("46", "Livraisons intracommunautaires exemptées"),
    ("47", "Autres opérations exemptées et exportations"),
    ("48", "Notes de crédit sur 44 et 46"),
    ("49", "Notes de crédit sur les autres opérations"),
    ("81", "Achats de marchandises et matières"),
    ("82", "Achats de services et biens divers"),
    ("83", "Achats de biens d'investissement"),
    ("84", "Notes de crédit reçues sur 86 et 88"),
    ("85", "Notes de crédit reçues sur les autres achats"),
    ("86", "Acquisitions intracommunautaires de biens"),
    ("87", "Autres achats avec TVA due par le cocontractant"),
    ("88", "Services intracommunautaires reçus"),
    ("54", "TVA due sur 01, 02 et 03"),
    ("55", "TVA due sur les acquisitions intracommunautaires"),
    ("56", "TVA due sur les opérations du cocontractant"),
    ("57", "TVA due sur les importations"),
    ("61", "Régularisations en faveur de l'État"),
    ("63", "TVA à reverser sur notes de crédit reçues"),
    ("59", "TVA déductible"),
    ("62", "Régularisations en faveur du déclarant"),
    ("64", "TVA à récupérer sur notes de crédit délivrées"),
    ("71", "Taxe due à l'État"),
    ("72", "Somme due par l'État"),
];

/// A periodic VAT return: the grids of a month or a quarter, worked out from the entries posted
/// in it, and the file Intervat takes.
#[derive(Model)]
#[erp(id = "l10n_be_vat_return", methods)]
#[allow(dead_code)]
pub struct L10nBeVatReturn<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Period")]
    name: String,
    #[erp(label = "From")]
    date_from: NaiveDate,
    #[erp(label = "To")]
    date_to: NaiveDate,
    #[erp(label = "Grids", inverse = "vat_return", owned)]
    lines: Reference<BaseL10nBeVatReturnLine, MultipleIds>,
    #[erp(label = "Due to the State (71)", default = 0.0)]
    amount_due: Decimal,
    #[erp(label = "Due by the State (72)", default = 0.0)]
    amount_refundable: Decimal,
    #[erp(label = "Intervat file")]
    xml: Option<String>,
}

/// The amount of each grid between two dates: the journal items posted in it, marked for the
/// grid, counted with the grid's sign; then 71 or 72, what the VAT due and deductible leave.
pub fn grid_amounts(
    env: &mut Environment,
    date_from: NaiveDate,
    date_to: NaiveDate,
) -> Result<BTreeMap<String, Decimal>> {
    let env = &mut *env.sudo();
    let items: AccountMoveLine<MultipleIds> = env.search(&make_domain!([
        ("parent_state", "=", MoveState::Posted),
        ("date", ">=", date_from),
        ("date", "<=", date_to)
    ]))?;
    let mut grids: BTreeMap<String, Decimal> = GRIDS
        .iter()
        .map(|(grid, _)| (grid.to_string(), Decimal::ZERO))
        .collect();
    for item in &items {
        let tags: AccountTaxTag<MultipleIds> = item.get_tax_tags(env)?;
        if tags.is_empty() {
            continue;
        }
        let balance = *item.get_balance(env)?;
        for tag in &tags {
            let grid = tag.get_grid(env)?.clone();
            let sign = Decimal::from(*tag.get_sign(env)?);
            *grids.entry(grid).or_default() += balance * sign;
        }
    }
    let sum = |grids: &BTreeMap<String, Decimal>, keys: &[&str]| -> Decimal {
        keys.iter()
            .map(|key| grids.get(*key).copied().unwrap_or_default())
            .sum()
    };
    let due = sum(&grids, &["54", "55", "56", "57", "61", "63"]);
    let deductible = sum(&grids, &["59", "62", "64"]);
    if due >= deductible {
        grids.insert("71".to_string(), due - deductible);
        grids.insert("72".to_string(), Decimal::ZERO);
    } else {
        grids.insert("71".to_string(), Decimal::ZERO);
        grids.insert("72".to_string(), deductible - due);
    }
    Ok(grids)
}

mod period {
    use chrono::Datelike;
    use erp::types::field::NaiveDate;

    /// The month, `(year, month, None)`, or the quarter, `(year, None, quarter)`, two dates span
    /// exactly; nothing otherwise.
    pub fn period_of(from: NaiveDate, to: NaiveDate) -> Option<(i32, Option<u32>, Option<u32>)> {
        if from.day() != 1 || from.year() != to.year() {
            return None;
        }
        let next = to.succ_opt()?;
        if next.day() != 1 {
            return None;
        }
        let months = (next.year() - from.year()) * 12 + next.month() as i32 - from.month() as i32;
        match months {
            1 => Some((from.year(), Some(from.month()), None)),
            3 if (from.month() - 1).is_multiple_of(3) => {
                Some((from.year(), None, Some((from.month() - 1) / 3 + 1)))
            }
            _ => None,
        }
    }
}

/// The Intervat file of a return: the declarant, the period, the grids not nought.
pub fn intervat_xml(
    vat_number: &str,
    name: &str,
    date_from: NaiveDate,
    date_to: NaiveDate,
    grids: &BTreeMap<String, Decimal>,
) -> Result<String> {
    let Some((year, month, quarter)) = period_of(date_from, date_to) else {
        return Err("A VAT return covers a calendar month or a quarter".into());
    };
    let digits: String = vat_number.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != 10 {
        return Err("Set the company's Belgian VAT number to file the return".into());
    }
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
    };
    let period = match (month, quarter) {
        (Some(month), _) => format!("<ns2:Month>{month}</ns2:Month>"),
        (_, Some(quarter)) => format!("<ns2:Quarter>{quarter}</ns2:Quarter>"),
        _ => unreachable!("a period is a month or a quarter"),
    };
    let amounts: String = GRIDS
        .iter()
        .filter_map(|(grid, _)| {
            let amount = grids.get(*grid).copied().unwrap_or_default();
            (!amount.is_zero()).then(|| {
                format!(
                    "\n      <ns2:Amount GridNumber=\"{}\">{:.2}</ns2:Amount>",
                    grid.parse::<u32>().unwrap_or_default(),
                    amount.round_dp(2)
                )
            })
        })
        .collect();
    let refund = grids.get("72").is_some_and(|amount| !amount.is_zero());
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<ns2:VATConsignment xmlns="http://www.minfin.fgov.be/InputCommon" xmlns:ns2="http://www.minfin.fgov.be/VATConsignment" VATDeclarationsNbr="1">
  <ns2:VATDeclaration SequenceNumber="1">
    <ns2:Declarant>
      <VATNumber>{digits}</VATNumber>
      <Name>{}</Name>
    </ns2:Declarant>
    <ns2:Period>
      {period}
      <ns2:Year>{year}</ns2:Year>
    </ns2:Period>
    <ns2:Data>{amounts}
    </ns2:Data>
    <ns2:ClientListingNihil>NO</ns2:ClientListingNihil>
    <ns2:Ask Restitution="{}" Payment="NO"/>
  </ns2:VATDeclaration>
</ns2:VATConsignment>
"#,
        escape(name),
        if refund { "YES" } else { "NO" }
    ))
}

#[erp_methods]
impl L10nBeVatReturn<MultipleIds> {
    /// Work the grids out again from the entries of the period, and the Intervat file with them.
    #[erp(rpc)]
    pub fn action_compute(&self, env: &mut Environment) -> Result<bool> {
        for vat_return in self {
            let from = *vat_return.get_date_from(env)?;
            let to = *vat_return.get_date_to(env)?;
            if from > to {
                return Err("The period ends before it starts".into());
            }
            let grids = grid_amounts(env, from, to)?;
            let lines = GRIDS
                .iter()
                .map(|(grid, label)| {
                    let mut line = MapOfFields::default();
                    line.insert("vat_return", vat_return.get_id());
                    line.insert("grid", *grid);
                    line.insert("name", *label);
                    line.insert("amount", grids.get(*grid).copied().unwrap_or_default());
                    line
                })
                .collect();
            let company = Company::current(env)?;
            let (vat, name) = {
                let env = &mut *env.sudo();
                let contact: Contact<SingleId> = company.get_contact(env)?;
                (
                    contact.get_vat(env)?.cloned().unwrap_or_default(),
                    contact.get_name(env)?.clone(),
                )
            };
            let xml = intervat_xml(&vat, &name, from, to, &grids).ok();
            let old: L10nBeVatReturnLine<MultipleIds> = vat_return.get_lines(env)?;
            old.delete(env)?;
            let _: L10nBeVatReturnLine<MultipleIds> = env.create_new_records_from_maps(lines)?;
            vat_return.set_amount_due(grids.get("71").copied().unwrap_or_default(), env)?;
            vat_return.set_amount_refundable(grids.get("72").copied().unwrap_or_default(), env)?;
            vat_return.set_xml(xml, env)?;
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::period::period_of;
    use erp::types::field::NaiveDate;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
    }

    #[test]
    fn test_periods() {
        assert_eq!(
            period_of(date("2026-02-01"), date("2026-02-28")),
            Some((2026, Some(2), None))
        );
        assert_eq!(
            period_of(date("2026-04-01"), date("2026-06-30")),
            Some((2026, None, Some(2)))
        );
        assert_eq!(
            period_of(date("2026-12-01"), date("2026-12-31")),
            Some((2026, Some(12), None))
        );
        assert_eq!(
            period_of(date("2026-02-01"), date("2026-04-30")),
            None,
            "not a quarter"
        );
        assert_eq!(period_of(date("2026-02-02"), date("2026-02-28")), None);
        assert_eq!(
            period_of(date("2026-01-01"), date("2026-12-31")),
            None,
            "a year is not filed"
        );
    }
}
