use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Expr, ExprLit, Result};
use syn::Expr::{Lit, Tuple};
use syn::Lit::Str;
use syn::spanned::Spanned;
use crate::domain::search_key::{SearchKey, SearchOperator, SearchTuple, SearchType};
use crate::domain::util::{gen_and_or_or_without_enough_tuple, gen_invalid_or_unknown_attribute, gen_invalid_search_string, gen_invalid_tuple_len, gen_invalid_tuple_operator};

pub fn derive(items: &Punctuated<Expr, Comma>) -> Result<TokenStream> {
    if items.is_empty() {
        return Ok(quote! {
            erp_search::SearchType::Nothing
        });
    }

    /// Transform given expression into a usable SearchKey
    fn transform_to_search_key(expr: &Expr) -> Result<SearchKey> {
        match expr {
            Lit(ExprLit { lit: Str(lit_str), ..}) => {
                let str_value = lit_str.value();
                match str_value.as_str() {
                    "&" => Ok(SearchKey::And),
                    "|" => Ok(SearchKey::Or),
                    _ => Err(gen_invalid_search_string(expr.span(), str_value.as_str(), &["&", "|"])),
                }
            },
            Tuple(expr) => {
                let elems = &expr.elems;
                if elems.len() != 3 {
                    return Err(gen_invalid_tuple_len(expr.span()));
                }
                let operator: SearchOperator = match &elems[1] {
                    Lit(ExprLit { lit: Str(field_name), ..}) => {
                        let field_name = field_name.value();
                        let search_operator: std::result::Result<erp_search::SearchOperator, _> = field_name.as_str().try_into();
                        match search_operator {
                            Ok(search_operator) => SearchOperator::Operator(search_operator),
                            Err(_) => return Err(gen_invalid_tuple_operator(elems[1].span(), field_name)),
                        }
                    }
                    expr => SearchOperator::Expr(expr.clone()),
                };
                Ok(SearchKey::Tuple(SearchTuple {
                    left: elems[0].clone(),
                    operator,
                    right: elems[2].clone(),
                }))
            },
            _ => Err(gen_invalid_or_unknown_attribute(expr.span())),
        }
    }

    /// Transform given list into a usable SearchType
    fn parse_value(value: &mut Vec<(&Expr, SearchKey)>) -> Result<SearchType> {
        if value.is_empty() {
            return Ok(SearchType::Nothing);
        }
        let search_key = value.remove(0);
        match search_key.1 {
            SearchKey::And | SearchKey::Or => {
                let left_value = parse_value(value)?;
                // TODO Check if it's possible that "Nothing" is returned here
                if matches!(left_value, SearchType::Nothing) {
                    return Err(gen_and_or_or_without_enough_tuple(search_key.0.span()));
                }
                let right_value = parse_value(value)?;
                // TODO Check if it's possible that "Nothing" is returned here
                if matches!(right_value, SearchType::Nothing) {
                    return Err(gen_and_or_or_without_enough_tuple(search_key.0.span()));
                }
                Ok(if matches!(search_key.1, SearchKey::And) {
                    SearchType::And(Box::new(left_value), Box::new(right_value))
                } else {
                    SearchType::Or(Box::new(left_value), Box::new(right_value))
                })
            }
            SearchKey::Tuple(tuple) => Ok(SearchType::Tuple(tuple)),
        }
    }

    let items: Result<Vec<(&Expr, SearchKey)>> = items.iter().map(|expr| {
        let transform: Result<SearchKey> = transform_to_search_key(expr);
        let transform = transform?;
        Ok((expr, transform))
    }).collect();

    let mut items = items?;

    let mut result = parse_value(&mut items)?;

    loop {
        if items.is_empty() {
            break;
        }
        let new_result = parse_value(&mut items)?;
        result = SearchType::And(Box::new(result), Box::new(new_result));
    }

    // Now, "result" contains the whole domain.
    // Transform it into code

    let result = quote! {
        #result
    };

    Ok(result)
}
