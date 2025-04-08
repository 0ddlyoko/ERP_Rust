use proc_macro2::TokenStream;
use syn::Expr;
use syn::__private::quote::quote;
use syn::__private::ToTokens;

pub enum SearchType {
    And(Box<SearchType>, Box<SearchType>),
    Or(Box<SearchType>, Box<SearchType>),
    Tuple(SearchTuple),
    Nothing,
}

pub enum SearchKey {
    And,
    Or,
    Tuple(SearchTuple),
}

pub struct SearchTuple {
    pub left: Expr,
    pub operator: SearchOperator,
    pub right: Expr,
}

pub enum SearchOperator {
    Operator(erp_search::SearchOperator),
    Expr(Expr),
}

impl ToTokens for SearchType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let quote: TokenStream = match self {
            SearchType::And(left, right) => {
                quote! {
                    erp_search::SearchType::And(
                        Box::new(#left),
                        Box::new(#right),
                    )
                }
            },
            SearchType::Or(left, right) => {
                quote! {
                    erp_search::SearchType::Or(
                        Box::new(#left),
                        Box::new(#right),
                    )
                }
            },
            SearchType::Tuple(tuple) => {
                quote! {
                    erp_search::SearchType::Tuple(#tuple)
                }
            },
            SearchType::Nothing => {
                quote! {
                    erp_search::SearchType::Nothing
                }
            },
        };
        tokens.extend(quote);
    }
}

impl ToTokens for SearchTuple {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let SearchTuple { left, operator, right } = self;
        let quote = quote! {
            erp_search::SearchTuple {
                left: #left.into(),
                operator: #operator,
                right: #right.into(),
            }
        };

        tokens.extend(quote);
    }
}

impl ToTokens for SearchOperator {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let quote = match self {
            SearchOperator::Operator(operator) => {
                match operator {
                    erp_search::SearchOperator::Equal => quote! { erp_search::SearchOperator::Equal },
                    erp_search::SearchOperator::NotEqual => quote! { erp_search::SearchOperator::NotEqual },
                    erp_search::SearchOperator::In => quote! { erp_search::SearchOperator::In },
                    erp_search::SearchOperator::NotIn => quote! { erp_search::SearchOperator::NotIn },
                    erp_search::SearchOperator::Greater => quote! { erp_search::SearchOperator::Greater },
                    erp_search::SearchOperator::GreaterEqual => quote! { erp_search::SearchOperator::GreaterEqual },
                    erp_search::SearchOperator::Lower => quote! { erp_search::SearchOperator::Lower },
                    erp_search::SearchOperator::LowerEqual => quote! { erp_search::SearchOperator::LowerEqual },
                }
            },
            SearchOperator::Expr(expr) => quote! {#expr.try_into()?},
        };

        tokens.extend(quote);
    }
}
