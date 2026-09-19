use syn::spanned::Spanned;
use syn::{Error, FnArg, ImplItemFn, Pat, PatType, Result, ReturnType, Type};

pub struct ParsedMethod {
    pub name: syn::Ident,
    /// Arguments between the environment and the `super` cursor.
    pub args: Vec<(syn::Ident, Type)>,
    /// Type the method yields, unwrapped from its `Result`.
    pub ret: Type,
    /// The method as written, with the `super` cursor still untyped.
    pub item: ImplItemFn,
}

/// Whether a method carries `#[erp(overridable)]`.
///
/// A plain method passes through untouched: being overridable is a commitment, so it is opted
/// into rather than assumed. Declaring and overriding use the same attribute, because a
/// contributor has no way of knowing whether it is the first — and should not have to.
pub fn is_overridable(item: &ImplItemFn) -> Result<bool> {
    let mut found = false;
    for attr in item.attrs.iter().filter(|a| a.meta.path().is_ident("erp")) {
        attr.parse_args_with(|input: syn::parse::ParseStream| {
            let key: syn::Ident = input.parse()?;
            match key.to_string().as_str() {
                "overridable" => Ok(()),
                other => Err(Error::new(
                    key.span(),
                    format!("Unknown key {other}. The only key on a method is: overridable"),
                )),
            }
        })?;
        if found {
            return Err(Error::new(attr.span(), "Duplicate #[erp(overridable)]"));
        }
        found = true;
    }
    Ok(found)
}

/// Split a marked method into the pieces the generator needs.
///
/// The shape is fixed — `&self`, the environment, the declared arguments, then the `super`
/// cursor — because every contributor to a chain has to agree on it, and a positional rule is
/// what lets the macro tell the arguments from the rest.
pub fn parse_method(item: ImplItemFn) -> Result<ParsedMethod> {
    let signature_help = "An overridable method takes &self, an &mut Environment, its own \
                          arguments, then a `sup: Super` cursor";
    let inputs: Vec<&FnArg> = item.sig.inputs.iter().collect();

    let Some(FnArg::Receiver(_)) = inputs.first() else {
        return Err(Error::new(item.sig.span(), signature_help));
    };
    if inputs.len() < 3 {
        return Err(Error::new(item.sig.span(), signature_help));
    }

    let sup = inputs[inputs.len() - 1];
    if !is_super(sup) {
        return Err(Error::new(
            sup.span(),
            "The last argument of an overridable method must be the `Super` cursor",
        ));
    }

    let mut args = Vec::new();
    for input in &inputs[2..inputs.len() - 1] {
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
