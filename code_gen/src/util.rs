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
                help: The correct format looks like `#[erp({help})]`",
            ),
        ));
    }

    input.parse::<Eq>()?;
    input.parse()
}

pub fn gen_unknown_key_error(span: Span, name: &str, valid_strings: &[&str]) -> Error {
    let joined_str = valid_strings.join(", ");
    Error::new(span, format!("Unknown key {}. Valid keys are: {}", name, joined_str))
}

pub fn gen_missing_key_error(span: Span, name: &str) -> Error {
    Error::new(span, format!("Missing `{}` key!", name))
}

pub fn gen_field_no_field_error(span: Span) -> Error {
    Error::new(span, "Field should be of type")
}
