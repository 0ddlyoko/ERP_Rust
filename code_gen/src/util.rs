use proc_macro2::Span;
use syn::Error;
use syn::parse::{Parse, ParseStream, Result};
use syn::token::Eq;


pub fn parse_eq<T: Parse>(input: ParseStream, help: &str) -> Result<T> {
    if input.is_empty() {
        return Err(syn::Error::new(
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

pub fn generate_only_struct_allowed_error(span: Span) -> Error {
    syn::Error::new(span, format!("Model derive can only be on struct"))
}

pub fn generate_unknown_key_error(span: Span, name: &str, valid_strings: &[&str]) -> Error {
    let joined_str = valid_strings.join(", ");
    syn::Error::new(span, format!("Unknown key {}. Valid keys are: {}", name, joined_str))
}

pub fn generate_missing_table_name_error(span: Span) -> Error {
    syn::Error::new(span, format!("Missing table_name key!"))
}

pub fn generate_field_no_name_error(span: Span) -> Error {
    syn::Error::new(span, format!("Missing field name"))
}

pub fn option_to_tuple<E>(the_option: Option<E>, default_value: E) -> (bool, E) {
    let is_present = the_option.is_some();
    let value = match the_option {
        Some(value) => value,
        None => default_value,
    };
    return (is_present, value);
}
