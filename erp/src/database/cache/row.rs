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

    /// Order two cells of the same field.
    ///
    /// NULLs sort last, matching PostgreSQL's default for ascending order. Cells whose types do
    /// not line up compare equal, because a sort comparator has to stay consistent.
    pub(crate) fn compare_cells(left: &Option<FieldType>, right: &Option<FieldType>) -> Ordering {
        match (left, right) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => {
                Self::compare(left, &right.clone().into()).unwrap_or(Ordering::Equal)
            }
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
            SearchOperator::In => Self::is_member(cell_value, right),
            SearchOperator::NotIn => !Self::is_member(cell_value, right),
            SearchOperator::Like => Self::matches_pattern(cell_value, right, false),
            SearchOperator::ILike => Self::matches_pattern(cell_value, right, true),
            SearchOperator::Greater => Self::ordered(cell_value, right, Ordering::is_gt),
            SearchOperator::GreaterEqual => Self::ordered(cell_value, right, Ordering::is_ge),
            SearchOperator::Lower => Self::ordered(cell_value, right, Ordering::is_lt),
            SearchOperator::LowerEqual => Self::ordered(cell_value, right, Ordering::is_le),
        }
    }

    /// Compare a cell against a value and report whether the ordering is the one wanted.
    ///
    /// A missing cell, or a right-hand side of another type, matches nothing.
    fn ordered(cell: &Option<FieldType>, right: &RightTuple, accept: fn(Ordering) -> bool) -> bool {
        let Some(cell) = cell else {
            return false;
        };
        Self::compare(cell, right).is_some_and(accept)
    }

    /// Membership of a cell in an array. Anything but an array matches nothing.
    fn is_member(cell: &Option<FieldType>, right: &RightTuple) -> bool {
        let (Some(cell), RightTuple::Array(members)) = (cell, right) else {
            return false;
        };
        members.iter().any(|member| cell == member)
    }

    /// Match a text cell against an SQL pattern.
    fn matches_pattern(cell: &Option<FieldType>, right: &RightTuple, ignore_case: bool) -> bool {
        let (Some(FieldType::String(cell)), RightTuple::String(pattern)) = (cell, right) else {
            return false;
        };
        if ignore_case {
            sql_like(&cell.to_lowercase(), &pattern.to_lowercase())
        } else {
            sql_like(cell, pattern)
        }
    }
}

/// Match a string against an SQL `LIKE` pattern: `%` stands for any run of characters, `_` for
/// exactly one.
///
/// Greedy with backtracking on the last `%`, which is enough for patterns of this shape and
/// avoids pulling a regex engine into the workspace.
fn sql_like(value: &str, pattern: &str) -> bool {
    let value: Vec<char> = value.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let (mut v, mut p) = (0usize, 0usize);
    let mut last_wildcard: Option<usize> = None;
    let mut resume_at = 0usize;

    while v < value.len() {
        if p < pattern.len() && (pattern[p] == '_' || pattern[p] == value[v]) {
            v += 1;
            p += 1;
        } else if p < pattern.len() && pattern[p] == '%' {
            last_wildcard = Some(p);
            resume_at = v;
            p += 1;
        } else if let Some(wildcard) = last_wildcard {
            p = wildcard + 1;
            resume_at += 1;
            v = resume_at;
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|c| *c == '%')
}
