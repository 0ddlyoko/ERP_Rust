use crate::database::FieldType;
use crate::database::postgres::{quote_ident, to_sql_param};
use crate::model::ModelManager;
use erp_search::{
    LeftTuple, OrderBy, RightTuple, SearchOperator, SearchOptions, SearchTuple, SearchType,
};
use erp_types::field::{FieldKind, FieldReference, FieldReferenceType};
use postgres::types::ToSql;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Builds a statement and the parameters that go with it.
///
/// Every value reaches the database as a bound parameter; nothing is ever interpolated into the
/// SQL text.
pub(crate) struct QueryBuilder {
    params: Vec<Box<dyn ToSql + Sync + Send>>,
}

impl QueryBuilder {
    pub(crate) fn new() -> Self {
        Self { params: Vec::new() }
    }

    /// Parameters, in the order their placeholders appear.
    pub(crate) fn params(&self) -> Vec<&(dyn ToSql + Sync)> {
        self.params
            .iter()
            .map(|param| param.as_ref() as &(dyn ToSql + Sync))
            .collect()
    }

    /// Bind a value and return its placeholder.
    pub(crate) fn push_value(&mut self, value: &FieldType) -> Result<String> {
        self.bind(value)
    }

    fn bind(&mut self, value: &FieldType) -> Result<String> {
        self.params.push(to_sql_param(value)?);
        Ok(format!("${}", self.params.len()))
    }

    /// Statement selecting the ids of a model matching a domain, ordered and paginated.
    ///
    /// The domain is resolved into a set of ids first, then wrapped, which gives `ORDER BY` a
    /// place to name the model's own columns.
    pub(crate) fn select_ids(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<String> {
        self.select_columns(model_name, &["id"], domain, model_manager, options)
    }

    /// Same, returning the requested columns alongside the id.
    pub(crate) fn select_columns(
        &mut self,
        model_name: &str,
        fields: &[&str],
        domain: &SearchType,
        model_manager: &ModelManager,
        options: &SearchOptions,
    ) -> Result<String> {
        let model = model_manager.try_get_model(model_name)?;
        let table = quote_ident(&model.table_name);
        let inner = self.domain_ids(model_name, domain, model_manager)?;

        let mut columns: Vec<String> = vec![quote_ident("id")];
        for field in fields {
            if *field == "id" {
                continue;
            }
            // A one2many has no column; it is read from the other side.
            if model.try_get_internal_field(field)?.kind.is_stored() {
                columns.push(quote_ident(field));
            }
        }

        let mut sql = format!(
            "SELECT {} FROM {table} WHERE {} IN ({inner})",
            columns.join(", "),
            quote_ident("id")
        );
        sql.push_str(&order_clause(&options.order));
        if let Some(limit) = options.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if options.offset > 0 {
            sql.push_str(&format!(" OFFSET {}", options.offset));
        }
        Ok(sql)
    }

    /// Statement counting the records matching a domain.
    pub(crate) fn select_count(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<String> {
        let model = model_manager.try_get_model(model_name)?;
        let inner = self.domain_ids(model_name, domain, model_manager)?;
        Ok(format!(
            "SELECT COUNT(*) FROM {} WHERE {} IN ({inner})",
            quote_ident(&model.table_name),
            quote_ident("id")
        ))
    }

    /// Resolve a domain into a statement yielding one column of ids.
    ///
    /// Set operations are nested subqueries rather than joins, because a domain like
    /// `["&", ("lines.price", ">=", 30), ("lines.price", "<=", 50)]` must accept a record whose
    /// branches are satisfied by two *different* children. A single join would not.
    fn domain_ids(
        &mut self,
        model_name: &str,
        domain: &SearchType,
        model_manager: &ModelManager,
    ) -> Result<String> {
        Ok(match domain {
            // An empty domain filters nothing, so it selects everything.
            SearchType::Nothing => {
                let model = model_manager.try_get_model(model_name)?;
                format!(
                    "SELECT {} FROM {}",
                    quote_ident("id"),
                    quote_ident(&model.table_name)
                )
            }
            SearchType::And(left, right) => {
                let left = self.domain_ids(model_name, left, model_manager)?;
                let right = self.domain_ids(model_name, right, model_manager)?;
                format!("({left}) INTERSECT ({right})")
            }
            SearchType::Or(left, right) => {
                let left = self.domain_ids(model_name, left, model_manager)?;
                let right = self.domain_ids(model_name, right, model_manager)?;
                format!("({left}) UNION ({right})")
            }
            SearchType::Tuple(SearchTuple {
                left: LeftTuple { path },
                operator,
                right,
            }) => {
                let mut path = path.clone();
                path.reverse();
                self.path_ids(model_name, &mut path, operator, right, model_manager)?
            }
        })
    }

    /// Walk a dotted path, innermost first, mapping ids back out through each relation.
    fn path_ids(
        &mut self,
        model_name: &str,
        path: &mut Vec<String>,
        operator: &SearchOperator,
        right: &RightTuple,
        model_manager: &ModelManager,
    ) -> Result<String> {
        let Some(current_field) = path.pop() else {
            return Err(format!("Empty field path on model {model_name}").into());
        };
        if path.is_empty() {
            return self.leaf_ids(model_name, &current_field, operator, right, model_manager);
        }

        let model = model_manager.try_get_model(model_name)?;
        let field = model.try_get_internal_field(&current_field)?;
        let Some(FieldReference {
            target_model,
            inverse_field,
        }) = &field.inverse
        else {
            return Err(format!(
                "Field {model_name}.{current_field} is not relational, so it cannot be traversed"
            )
            .into());
        };
        let target = model_manager.try_get_model(target_model)?;
        let inner = self.path_ids(&target.name, path, operator, right, model_manager)?;

        Ok(if field.kind == FieldKind::Ref {
            // many2one: this table holds the foreign key.
            format!(
                "SELECT {} FROM {} WHERE {} IN ({inner})",
                quote_ident("id"),
                quote_ident(&model.table_name),
                quote_ident(&current_field)
            )
        } else {
            // one2many: the children hold it, so project them back through it.
            let FieldReferenceType::O2M { inverse_field } = inverse_field else {
                return Err(format!(
                    "Field {model_name}.{current_field} is a many2one where a one2many was expected"
                )
                .into());
            };
            let inverse = quote_ident(inverse_field);
            format!(
                "SELECT {inverse} FROM {} WHERE {} IN ({inner}) AND {inverse} IS NOT NULL",
                quote_ident(&target.table_name),
                quote_ident("id")
            )
        })
    }

    fn leaf_ids(
        &mut self,
        model_name: &str,
        field_name: &str,
        operator: &SearchOperator,
        right: &RightTuple,
        model_manager: &ModelManager,
    ) -> Result<String> {
        let model = model_manager.try_get_model(model_name)?;
        // "id" is a real column but is not in the registry.
        if field_name != "id" {
            model.try_get_internal_field(field_name)?;
        }
        let column = quote_ident(field_name);
        let condition = self.condition(&column, operator, right)?;
        Ok(format!(
            "SELECT {} FROM {} WHERE {condition}",
            quote_ident("id"),
            quote_ident(&model.table_name)
        ))
    }

    /// Translate one comparison, reproducing the in-memory backend's NULL and array semantics.
    fn condition(
        &mut self,
        column: &str,
        operator: &SearchOperator,
        right: &RightTuple,
    ) -> Result<String> {
        Ok(match (operator, right) {
            (SearchOperator::Equal, RightTuple::None) => format!("{column} IS NULL"),
            (SearchOperator::NotEqual, RightTuple::None) => format!("{column} IS NOT NULL"),

            (SearchOperator::Equal | SearchOperator::In, RightTuple::Array(members)) => {
                match self.bind_all(members)? {
                    // An empty set matches nothing.
                    None => "FALSE".to_string(),
                    Some(placeholders) => format!("{column} IN ({placeholders})"),
                }
            }
            // A null cell is "not in" any set, which a bare NOT IN would drop.
            (SearchOperator::NotEqual | SearchOperator::NotIn, RightTuple::Array(members)) => {
                match self.bind_all(members)? {
                    None => "TRUE".to_string(),
                    Some(placeholders) => {
                        format!("({column} IS NULL OR {column} NOT IN ({placeholders}))")
                    }
                }
            }
            // `in` against something that is not a set matches nothing, as in the cache backend.
            (SearchOperator::In, _) => "FALSE".to_string(),
            (SearchOperator::NotIn, _) => "TRUE".to_string(),

            (SearchOperator::Like | SearchOperator::ILike, RightTuple::String(pattern)) => {
                let keyword = if matches!(operator, SearchOperator::Like) {
                    "LIKE"
                } else {
                    "ILIKE"
                };
                let placeholder = self.bind(&FieldType::String(pattern.clone()))?;
                format!("{column} {keyword} {placeholder}")
            }
            (SearchOperator::Like | SearchOperator::ILike, _) => "FALSE".to_string(),

            (operator, right) => {
                let Some(value) = right_to_field(right) else {
                    return Ok("FALSE".to_string());
                };
                let placeholder = self.bind(&value)?;
                match operator {
                    SearchOperator::Equal => format!("{column} = {placeholder}"),
                    // A null cell differs from any value, which `<>` alone would not report.
                    SearchOperator::NotEqual => {
                        format!("({column} IS NULL OR {column} <> {placeholder})")
                    }
                    SearchOperator::Greater => format!("{column} > {placeholder}"),
                    SearchOperator::GreaterEqual => format!("{column} >= {placeholder}"),
                    SearchOperator::Lower => format!("{column} < {placeholder}"),
                    SearchOperator::LowerEqual => format!("{column} <= {placeholder}"),
                    SearchOperator::Like
                    | SearchOperator::ILike
                    | SearchOperator::In
                    | SearchOperator::NotIn => unreachable!("handled above"),
                }
            }
        })
    }

    /// Bind every member of a set, or `None` when the set is empty.
    fn bind_all(&mut self, members: &[RightTuple]) -> Result<Option<String>> {
        let mut placeholders = Vec::with_capacity(members.len());
        for member in members {
            let Some(value) = right_to_field(member) else {
                continue;
            };
            placeholders.push(self.bind(&value)?);
        }
        Ok(if placeholders.is_empty() {
            None
        } else {
            Some(placeholders.join(", "))
        })
    }
}

/// `ORDER BY`, always ending on the id so the result is reproducible.
fn order_clause(order: &[OrderBy]) -> String {
    let mut keys: Vec<String> = order
        .iter()
        .map(|key| {
            let direction = if key.descending { "DESC" } else { "ASC" };
            // NULLS LAST on ascending is PostgreSQL's own default, and what the in-memory
            // backend reproduces.
            format!("{} {direction}", quote_ident(&key.field))
        })
        .collect();
    keys.push(format!("{} ASC", quote_ident("id")));
    format!(" ORDER BY {}", keys.join(", "))
}

/// A domain's right-hand value as something bindable. `None` has no parameter of its own.
fn right_to_field(right: &RightTuple) -> Option<FieldType> {
    Some(match right {
        RightTuple::String(value) => FieldType::String(value.clone()),
        RightTuple::Integer(value) => FieldType::Integer(*value),
        RightTuple::UInteger(value) => FieldType::UInteger(*value),
        RightTuple::Decimal(value) => FieldType::Decimal(*value),
        RightTuple::Boolean(value) => FieldType::Boolean(*value),
        RightTuple::Date(value) => FieldType::Date(*value),
        RightTuple::DateTime(value) => FieldType::DateTime(*value),
        RightTuple::Array(_) | RightTuple::None => return None,
    })
}
