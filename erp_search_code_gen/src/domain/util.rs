use proc_macro2::Span;
use syn::Error;
use erp_search::UnknownSearchOperatorError;

pub fn gen_invalid_or_unknown_attribute(span: Span) -> Error {
    Error::new(span, "Invalid or unknown attribute")
}

pub fn gen_invalid_search_string(span: Span, name: &str, valid_strings: &[&str]) -> Error {
    let joined_str = valid_strings.join(", ");
    Error::new(span, format!("Invalid attribute \"{}\". Valid keys are: {}", name, joined_str))
}

pub fn gen_invalid_tuple_len(span: Span) -> Error {
    Error::new(span, "Tuple should have 3 elements")
}

pub fn gen_invalid_tuple_operator(span: Span, operator: String) -> Error {
    Error::new(span, UnknownSearchOperatorError { search_operator: operator })
}

pub fn gen_and_or_or_without_enough_tuple(span: Span) -> Error {
    Error::new(span, "Given \"&\" or \"|\" doesn't have enough tuple")
}
