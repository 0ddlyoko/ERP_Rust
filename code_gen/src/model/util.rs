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

pub fn gen_option_not_one_generic(span: Span) -> Error {
    Error::new(span, "Given optional field should only have one generic argument")
}

pub fn gen_reference_not_two_generic(span: Span) -> Error {
    Error::new(span, "Given reference field should only have two generic argument")
}

pub fn gen_field_no_field_error(span: Span) -> Error {
    Error::new(span, "Field should be of type")
}

pub fn gen_wrong_default_value(span: Span, default_value: &str, name: &str) -> Error {
    Error::new(span, format!("Wrong default value `{}` for field `{}`", default_value, name))
}

pub fn gen_inverse_not_multiple_ids(span: Span) -> Error {
    Error::new(span, "Inverse attribute should only work on Reference<..., MultipleIds>")
}
