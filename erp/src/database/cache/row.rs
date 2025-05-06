use std::collections::HashMap;
use erp_search::{SearchOperator, SearchTuple, SearchType};
use crate::database::FieldType;

#[derive(Default)]
pub(crate) struct Row {
    id: u32,
    cells: HashMap<String, Option<FieldType>>,
}

impl Row {
    pub(crate) fn get_cell(&self, field_name: &str) -> &Option<FieldType> {
        self.cells.get(field_name).unwrap_or_else(|| &None)
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
                        // TODO Allow other value than String, even None
                        if let Some(cell_value) = cell_value {
                            cell_value.is_same(right)
                        } else {
                            right.is_empty()
                        }
                    },
                    SearchOperator::NotEqual => {
                        // TODO Allow other value than String, even None
                        if let Some(cell_value) = cell_value {
                            !cell_value.is_same(right)
                        } else {
                            !right.is_empty()
                        }
                    },
                    SearchOperator::In => {
                        todo!()
                    },
                    SearchOperator::NotIn => {
                        todo!()
                    },
                    SearchOperator::Greater => {
                        todo!()
                    },
                    SearchOperator::GreaterEqual => {
                        todo!()
                    },
                    SearchOperator::Lower => {
                        todo!()
                    },
                    SearchOperator::LowerEqual => {
                        todo!()
                    },
                }
            },
            SearchType::Nothing => true,
        }
    }
}
