use crate::models::account::BaseAccount;
use crate::models::fiscal_position::BaseAccountFiscalPosition;
use crate::models::payment_term::BaseAccountPaymentTerm;
use base::models::BaseContact;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;

/// What invoicing a contact needs: its terms, its fiscal position, its bank accounts, and the
/// accounts it is recorded on when they differ from the company's.
#[derive(Model)]
#[erp(id = "contact")]
#[erp(derived_model = "base::models")]
#[allow(dead_code)]
pub struct ContactAccount<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Customer payment terms", ondelete = "set_null")]
    customer_payment_term: Reference<BaseAccountPaymentTerm, SingleId>,
    #[erp(label = "Supplier payment terms", ondelete = "set_null")]
    supplier_payment_term: Reference<BaseAccountPaymentTerm, SingleId>,
    #[erp(label = "Fiscal position", ondelete = "set_null", tracking)]
    fiscal_position: Reference<BaseAccountFiscalPosition, SingleId>,
    #[erp(label = "Customer account", ondelete = "restrict")]
    account_receivable: Reference<BaseAccount, SingleId>,
    #[erp(label = "Supplier account", ondelete = "restrict")]
    account_payable: Reference<BaseAccount, SingleId>,
    #[erp(label = "Bank accounts", inverse = "contact", owned)]
    bank_accounts: Reference<BaseContactBank, MultipleIds>,
}

/// A bank account of a contact: where to pay a supplier, where a customer pays from.
#[derive(Model)]
#[erp(id = "contact_bank", name_field = "iban", methods)]
#[allow(dead_code)]
pub struct ContactBank<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    contact: Reference<BaseContact, SingleId>,
    #[erp(label = "IBAN")]
    iban: String,
    #[erp(label = "BIC")]
    bic: Option<String>,
    #[erp(label = "Bank")]
    bank_name: Option<String>,
}

/// `iban` without its spaces, in capitals, when its check digits are right.
///
/// Errs on a number that is not an IBAN: two letters, two check digits, then the account, the
/// whole passing the modulo 97 test of ISO 13616.
pub fn normalize_iban(iban: &str) -> std::result::Result<String, String> {
    let compact: String = iban
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_uppercase();
    let valid_shape = compact.len() >= 15
        && compact.len() <= 34
        && compact.chars().all(|c| c.is_ascii_alphanumeric())
        && compact[..2].chars().all(|c| c.is_ascii_alphabetic())
        && compact[2..4].chars().all(|c| c.is_ascii_digit());
    if !valid_shape {
        return Err(format!("{iban} is not an IBAN"));
    }
    let rearranged = format!("{}{}", &compact[4..], &compact[..4]);
    let mut remainder: u32 = 0;
    for c in rearranged.chars() {
        let value = c.to_digit(36).expect("alphanumeric");
        for digit in value.to_string().chars() {
            remainder = (remainder * 10 + digit.to_digit(10).expect("a digit")) % 97;
        }
    }
    if remainder != 1 {
        return Err(format!("{iban} is not an IBAN: its check digits are wrong"));
    }
    Ok(compact)
}

#[erp_methods]
impl ContactBank<MultipleIds> {
    /// The IBAN is kept as it is checked: without spaces, in capitals.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        for bank in &mut values {
            if let Some(iban) = bank.get_option::<&String>("iban").cloned() {
                bank.insert("iban", normalize_iban(&iban)?);
            }
        }
        Ok(sup.call_with(values, env)?)
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        let mut values = values;
        if let Some(iban) = values.get_option::<&String>("iban").cloned() {
            values.insert("iban", normalize_iban(&iban)?);
        }
        Ok(sup.call_with(values, env)?)
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_iban;

    #[test]
    fn test_ibans() {
        assert_eq!(
            normalize_iban("BE68 5390 0754 7034"),
            Ok("BE68539007547034".to_string())
        );
        assert_eq!(
            normalize_iban("fr14 2004 1010 0505 0001 3m02 606"),
            Ok("FR1420041010050500013M02606".to_string())
        );
        assert!(
            normalize_iban("BE69 5390 0754 7034").is_err(),
            "wrong check digits"
        );
        assert!(normalize_iban("BE68").is_err());
        assert!(normalize_iban("1268539007547034").is_err());
    }
}
