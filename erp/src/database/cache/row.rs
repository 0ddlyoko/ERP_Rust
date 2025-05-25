use std::collections::HashMap;
use erp_search::{RightTuple, SearchOperator, SearchTuple, SearchType};
use crate::database::FieldType;

#[derive(Default, Clone)]
pub(crate) struct Row {
    pub(crate) id: u32,
    pub(crate) cells: HashMap<String, Option<FieldType>>,
}

impl Row {
    pub(crate) fn get_cell(&self, field_name: &str) -> &Option<FieldType> {
        self.cells.get(field_name).unwrap_or(&None)
    }

    pub(crate) fn set_cell(&mut self, field_name: &str, cell: Option<FieldType>) {
        self.cells.insert(field_name.to_string(), cell);
    }

    /// Check if this row is valid for given domain
    ///
    /// This method will be deleted later once we implement the domain with dots
    pub(crate) fn is_valid(&self, domain: &SearchType) -> bool {
        match domain {
            SearchType::And(left, right) => {
                self.is_valid(left) && self.is_valid(right)
            },
            SearchType::Or(left, right) => {
                self.is_valid(left) || self.is_valid(right)
            },
            SearchType::Tuple(SearchTuple { left, operator, right }) => {
                let cell_value = self.get_cell(left);
                match operator {
                    SearchOperator::Equal => {
                        match (right, cell_value) {
                            (RightTuple::None, None) => true,
                            (left, Some(right)) => left == right,
                            _ => false,
                        }
                    },
                    SearchOperator::NotEqual => {
                        match (right, cell_value) {
                            (RightTuple::None, None) => false,
                            (left, Some(right)) => left != right,
                            _ => true,
                        }
                    },
                    SearchOperator::Greater => {
                        match (right, cell_value) {
                            (RightTuple::None, None) => false,
                            (RightTuple::Integer(right), Some(FieldType::Integer(cell_value))) => cell_value > right,
                            (RightTuple::UInteger(right), Some(FieldType::UInteger(cell_value))) => cell_value > right,
                            (RightTuple::Float(right), Some(FieldType::Float(cell_value))) => cell_value > right,
                            (RightTuple::Boolean(right), Some(FieldType::Boolean(cell_value))) => cell_value > right,
                            _ => false,
                        }
                    },
                    SearchOperator::GreaterEqual => {
                        match (right, cell_value) {
                            (RightTuple::None, None) => false,
                            (RightTuple::Integer(right), Some(FieldType::Integer(cell_value))) => cell_value >= right,
                            (RightTuple::UInteger(right), Some(FieldType::UInteger(cell_value))) => cell_value >= right,
                            (RightTuple::Float(right), Some(FieldType::Float(cell_value))) => cell_value >= right,
                            (RightTuple::Boolean(right), Some(FieldType::Boolean(cell_value))) => cell_value >= right,
                            _ => false,
                        }
                    },
                    SearchOperator::Lower => {
                        match (right, cell_value) {
                            (RightTuple::None, None) => false,
                            (RightTuple::Integer(right), Some(FieldType::Integer(cell_value))) => cell_value < right,
                            (RightTuple::UInteger(right), Some(FieldType::UInteger(cell_value))) => cell_value < right,
                            (RightTuple::Float(right), Some(FieldType::Float(cell_value))) => cell_value < right,
                            (RightTuple::Boolean(right), Some(FieldType::Boolean(cell_value))) => cell_value < right,
                            _ => false,
                        }
                    },
                    SearchOperator::LowerEqual => {
                        match (right, cell_value) {
                            (RightTuple::None, None) => false,
                            (RightTuple::Integer(right), Some(FieldType::Integer(cell_value))) => cell_value <= right,
                            (RightTuple::UInteger(right), Some(FieldType::UInteger(cell_value))) => cell_value <= right,
                            (RightTuple::Float(right), Some(FieldType::Float(cell_value))) => cell_value <= right,
                            (RightTuple::Boolean(right), Some(FieldType::Boolean(cell_value))) => cell_value <= right,
                            _ => false,
                        }
                    },
                }
            },
            SearchType::Nothing => true,
        }
    }
}
