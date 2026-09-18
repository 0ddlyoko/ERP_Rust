use crate::database::FieldType;
use erp_search::{RightTuple, SearchOperator};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Default, Clone)]
pub(crate) struct Row {
    pub(crate) cells: HashMap<String, Option<FieldType>>,
}

impl Row {
    pub(crate) fn get_cell(&self, field_name: &str) -> &Option<FieldType> {
        self.cells.get(field_name).unwrap_or(&None)
    }

    pub(crate) fn set_cell(&mut self, field_name: &str, cell: Option<FieldType>) {
        self.cells.insert(field_name.to_string(), cell);
    }

    /// Order a cell against a domain's right-hand value.
    ///
    /// Returns `None` when the two sides are not of comparable types, which the caller reports as
    /// "does not match" rather than as an error.
    fn compare(cell: &FieldType, right: &RightTuple) -> Option<Ordering> {
        match (cell, right) {
            (FieldType::String(cell), RightTuple::String(right)) => Some(cell.cmp(right)),
            (FieldType::Integer(cell), RightTuple::Integer(right)) => Some(cell.cmp(right)),
            (FieldType::UInteger(cell), RightTuple::UInteger(right)) => Some(cell.cmp(right)),
            (FieldType::Decimal(cell), RightTuple::Decimal(right)) => Some(cell.cmp(right)),
            (FieldType::Boolean(cell), RightTuple::Boolean(right)) => Some(cell.cmp(right)),
            (FieldType::Date(cell), RightTuple::Date(right)) => Some(cell.cmp(right)),
            (FieldType::DateTime(cell), RightTuple::DateTime(right)) => Some(cell.cmp(right)),
            _ => None,
        }
    }

    /// Check if this row is valid for given domain
    pub(crate) fn is_valid(
        &self,
        field_name: &str,
        operator: &SearchOperator,
        right: &RightTuple,
    ) -> bool {
        let cell_value = self.get_cell(field_name);
        match operator {
            SearchOperator::Equal => match (right, cell_value) {
                (RightTuple::None, None) => true,
                (left, Some(right)) => left == right,
                _ => false,
            },
            SearchOperator::NotEqual => match (right, cell_value) {
                (RightTuple::None, None) => false,
                (left, Some(right)) => left != right,
                _ => true,
            },
            _ => {
                let Some(cell_value) = cell_value else {
                    return false;
                };
                let Some(ordering) = Self::compare(cell_value, right) else {
                    return false;
                };
                match operator {
                    SearchOperator::Greater => ordering.is_gt(),
                    SearchOperator::GreaterEqual => ordering.is_ge(),
                    SearchOperator::Lower => ordering.is_lt(),
                    _ => ordering.is_le(),
                }
            }
        }
    }
}
