use syn::spanned::Spanned;
use syn::{Error, FnArg, ImplItemFn, Pat, PatType, Result, ReturnType, Type};

pub struct ParsedMethod {
    pub name: syn::Ident,
    /// Arguments between the environment and the `super` cursor.
    pub args: Vec<(syn::Ident, Type)>,
    /// Type the method yields, unwrapped from its `Result`.
    pub ret: Type,
    /// Whether the method declared a `super` cursor.
    ///
    /// Optional, because most implementations never call the one they override, and a parameter
    /// nothing reads is noise.
    pub has_sup: bool,
    /// The method as written, with the `super` cursor still untyped.
    pub item: ImplItemFn,
}

/// Reject the per-method attribute that opting in used to need.
///
/// Every method of an `#[erp_methods]` block is overridable now: the block is the boundary, and
/// repeating the fact on each method said nothing. Methods whose signature cannot be a chain link
/// — anything borrowing — belong in a plain `impl` block beside it.
pub fn check_no_stale_attribute(item: &ImplItemFn) -> Result<()> {
    match item.attrs.iter().find(|a| a.meta.path().is_ident("erp")) {
        Some(attr) => Err(Error::new(
            attr.span(),
            "Every method of an #[erp_methods] block is overridable, so this attribute is not \
             needed. A method that should not be is declared in a plain impl block instead.",
        )),
        None => Ok(()),
    }
}

/// Split a method into the pieces the generator needs.
///
/// The shape is positional — `&self`, the environment, the declared arguments, and optionally a
/// `sup: Super` cursor last — because every contributor to a chain has to agree on it, and
/// position is what lets the macro tell the arguments from the rest.
pub fn parse_method(item: ImplItemFn) -> Result<ParsedMethod> {
    let signature_help = "A method of an #[erp_methods] block takes &self, an &mut Environment, \
                          its own arguments, and may end with a `sup: Super` cursor";
    let inputs: Vec<&FnArg> = item.sig.inputs.iter().collect();

    let Some(FnArg::Receiver(_)) = inputs.first() else {
        return Err(Error::new(item.sig.span(), signature_help));
    };
    if inputs.len() < 2 {
        return Err(Error::new(item.sig.span(), signature_help));
    }

    let has_sup = inputs.last().is_some_and(|last| is_super(last));
    let last_arg = if has_sup {
        inputs.len() - 1
    } else {
        inputs.len()
    };

    let mut args = Vec::new();
    for input in &inputs[2..last_arg] {
        let FnArg::Typed(PatType { pat, ty, .. }) = input else {
            return Err(Error::new(input.span(), signature_help));
        };
        let Pat::Ident(ident) = pat.as_ref() else {
            return Err(Error::new(
                pat.span(),
                "An argument of an overridable method must be a plain name, because it becomes a \
                 field of the generated argument struct",
            ));
        };
        args.push((ident.ident.clone(), (**ty).clone()));
    }

    let ret = unwrap_result(&item.sig.output)?;
    Ok(ParsedMethod {
        name: item.sig.ident.clone(),
        args,
        ret,
        has_sup,
        item,
    })
}

fn is_super(arg: &FnArg) -> bool {
    let FnArg::Typed(PatType { ty, .. }) = arg else {
        return false;
    };
    let Type::Path(path) = ty.as_ref() else {
        return false;
    };
    path.path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "Super")
}

/// Pull `T` out of a `Result<T>` or `Result<T, E>` return type.
fn unwrap_result(output: &ReturnType) -> Result<Type> {
    let ReturnType::Type(_, ty) = output else {
        return Err(Error::new(
            output.span(),
            "An overridable method must return a Result: a link in a chain has to be able to fail",
        ));
    };
    let Type::Path(path) = ty.as_ref() else {
        return Err(Error::new(ty.span(), "Expected a Result<...> return type"));
    };
    let last = path
        .path
        .segments
        .last()
        .ok_or_else(|| Error::new(ty.span(), "Expected a Result<...> return type"))?;
    if last.ident != "Result" {
        return Err(Error::new(
            ty.span(),
            "An overridable method must return a Result: a link in a chain has to be able to fail",
        ));
    }
    let syn::PathArguments::AngleBracketed(generics) = &last.arguments else {
        return Err(Error::new(
            last.span(),
            "Expected a Result with at least one type argument",
        ));
    };
    generics
        .args
        .iter()
        .find_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty.clone()),
            _ => None,
        })
        .ok_or_else(|| {
            Error::new(
                last.span(),
                "Expected a Result with at least one type argument",
            )
        })
}
