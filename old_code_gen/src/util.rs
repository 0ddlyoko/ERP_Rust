use proc_macro2::Span;
use syn::Error;
use syn::parse::{Parse, ParseStream, Result};
use syn::token::Eq;


pub fn parse_eq<T: Parse>(input: ParseStream, help: &str) -> Result<T> {
    if input.is_empty() {
        return Err(Error::new(
            input.span(),
            format!(
                "unexpected end of input.\n\
                 help: The correct format looks like `#[odd({help})]`",
            ),
        ));
    }

    input.parse::<Eq>()?;
    input.parse()
}

pub fn generate_unknown_key_error(span: Span, name: &str, valid_strings: &[&str]) -> Error {
    let joined_str = valid_strings.join(", ");
    Error::new(span, format!("Unknown key {}. Valid keys are: {}", name, joined_str))
}

pub fn generate_missing_table_name_error(span: Span) -> Error {
    Error::new(span, "Missing table_name key!".to_string())
}

pub fn generate_field_no_name_error(span: Span) -> Error {
    Error::new(span, "Missing field name".to_string())
}

pub fn generate_field_no_field_error(span: Span) -> Error {
    Error::new(span, "Field should be of type Field<TYPE>".to_string())
}

pub fn option_to_tuple<E>(the_option: Option<E>, default_value: E) -> (bool, E) {
    match the_option {
        Some(value) => (true, value),
        None => (false, default_value),
    }
}
