use syn::parse::Parse;
use syn::spanned::Spanned;
use syn::{Error, FnArg, ImplItemFn, Pat, PatType, Result, ReturnType, Type};

/// What a method works on: the records of a `Model<MultipleIds>` block, the one record of a
/// `Model<SingleId>` block, or — taking no `self` — the model itself.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MethodReceiver {
    Records,
    Record,
    Model,
}

pub struct ParsedMethod {
    pub name: syn::Ident,
    pub receiver: MethodReceiver,
    /// Arguments between the environment and the `super` cursor.
    pub args: Vec<(syn::Ident, Type)>,
    /// Type the method yields, unwrapped from its `Result`.
    pub ret: Type,
    /// Whether the method answers to a remote caller.
    pub is_rpc: bool,
    /// For a check, the fields whose writing runs it.
    pub check: Option<Vec<String>>,
    /// Whether the method declared a `super` cursor.
    ///
    /// Optional, because most implementations never call the one they override, and a parameter
    /// nothing reads is noise.
    pub has_sup: bool,
    /// The method as written, with the `super` cursor still untyped.
    pub item: ImplItemFn,
}

/// What the `#[erp(...)]` attributes of a method say.
#[derive(Default)]
pub struct MethodAttributes {
    pub is_rpc: bool,
    /// For a check: the fields whose writing runs it.
    pub check: Option<Vec<String>>,
}

/// Whether the method is reachable from outside the process, and whether it is a check — run
/// once records are created, and once one of the fields it names is written.
///
/// Overridability is not asked for — every method of an `#[erp_methods]` block has it, the block
/// being the boundary. Being callable remotely is asked for every time, because the two are
/// different trust boundaries: a plugin calling a method already runs in process with full access
/// to the database, a remote caller does not. Forgetting the attribute leaves an endpoint that
/// does not exist, which is the safe direction to fail in.
pub fn read_method_attributes(item: &ImplItemFn) -> Result<MethodAttributes> {
    let mut attributes = MethodAttributes::default();
    for attr in item.attrs.iter().filter(|a| a.meta.path().is_ident("erp")) {
        attr.parse_args_with(|input: syn::parse::ParseStream| {
            let key: syn::Ident = input.parse()?;
            match key.to_string().as_str() {
                "rpc" => {
                    if attributes.is_rpc {
                        return Err(Error::new(attr.span(), "Duplicate #[erp(rpc)]"));
                    }
                    attributes.is_rpc = true;
                    Ok(())
                }
                "check" => {
                    if attributes.check.is_some() {
                        return Err(Error::new(attr.span(), "Duplicate #[erp(check)]"));
                    }
                    if !input.peek(syn::Token![=]) {
                        return Err(Error::new(
                            key.span(),
                            "A check names the fields whose writing runs it: \
                             #[erp(check = [\"field\", \"lines.price\"])]",
                        ));
                    }
                    input.parse::<syn::Token![=]>()?;
                    let content;
                    syn::bracketed!(content in input);
                    let fields =
                        content.parse_terminated(<syn::LitStr as Parse>::parse, syn::Token![,])?;
                    if fields.is_empty() {
                        return Err(Error::new(
                            key.span(),
                            "A check names at least one field whose writing runs it",
                        ));
                    }
                    attributes.check = Some(fields.iter().map(syn::LitStr::value).collect());
                    Ok(())
                }
                "overridable" => Err(Error::new(
                    key.span(),
                    "Every method of an #[erp_methods] block is overridable, so this is not \
                     needed. A method that should not be is declared in a plain impl block \
                     instead.",
                )),
                other => Err(Error::new(
                    key.span(),
                    format!("Unknown key {other}. The keys on a method are: rpc, check"),
                )),
            }
        })?;
    }
    Ok(attributes)
}

/// Split a method into the pieces the generator needs.
///
/// The shape is positional — `&self` if the method works on records, the environment, the
/// declared arguments, and optionally a `sup: Super` cursor last — because every contributor to a
/// chain has to agree on it, and position is what lets the macro tell the arguments from the rest.
/// `&self` works on the records of the block — several on `Model<MultipleIds>`, one on
/// `Model<SingleId>` — and a method without it works on the model.
pub fn parse_method(item: ImplItemFn, block: MethodReceiver) -> Result<ParsedMethod> {
    let MethodAttributes { is_rpc, check } = read_method_attributes(&item)?;
    let signature_help = "A method of an #[erp_methods] block takes &self — or nothing, to work \
                          on the model — an &mut Environment, its own arguments, and may end with \
                          a `sup: Super` cursor";
    let inputs: Vec<&FnArg> = item.sig.inputs.iter().collect();

    let (receiver, env_at) = match inputs.first() {
        Some(FnArg::Receiver(_)) => (block, 1),
        Some(FnArg::Typed(_)) => (MethodReceiver::Model, 0),
        None => return Err(Error::new(item.sig.span(), signature_help)),
    };
    if inputs.len() < env_at + 1 {
        return Err(Error::new(item.sig.span(), signature_help));
    }

    let has_sup = inputs.last().is_some_and(|last| is_super(last));
    let last_arg = if has_sup {
        inputs.len() - 1
    } else {
        inputs.len()
    };

    let mut args = Vec::new();
    for input in &inputs[env_at + 1..last_arg] {
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
        receiver,
        args,
        ret,
        is_rpc,
        check,
        has_sup,
        item,
    })
}

pub(crate) fn is_super(arg: &FnArg) -> bool {
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
pub(crate) fn unwrap_result(output: &ReturnType) -> Result<Type> {
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
