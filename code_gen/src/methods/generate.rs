use crate::methods::parse::{MethodReceiver, ParsedMethod, parse_method};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Error, ImplItem, ItemImpl, Result, Type};

pub fn expand(mut item: ItemImpl) -> Result<TokenStream> {
    let struct_ident = self_struct_ident(&item.self_ty)?;
    let self_ty = item.self_ty.clone();
    let block = block_receiver(&item.self_ty)?;
    let many: Type = syn::parse_quote! { #struct_ident<erp::types::field::MultipleIds> };

    let mut parsed = Vec::new();
    let mut kept = Vec::new();
    for entry in std::mem::take(&mut item.items) {
        let ImplItem::Fn(func) = entry else {
            kept.push(entry);
            continue;
        };
        parsed.push(parse_method(func, block)?);
    }

    let mut in_impl = Vec::new();
    let mut links = Vec::new();
    let mut registrations = Vec::new();

    for method in &parsed {
        let names = Names::of(&struct_ident, block, method);
        in_impl.push(renamed_body(method, &names));
        in_impl.push(dispatcher(method, &many));
        links.push(link(method, &names, &self_ty));

        let link_ident = &names.link;
        let name = method.name.to_string();
        let receiver = match method.receiver {
            MethodReceiver::Records => quote! { erp::types::method::Receiver::Records },
            MethodReceiver::Record => quote! { erp::types::method::Receiver::Record },
            MethodReceiver::Model => quote! { erp::types::method::Receiver::Model },
        };
        registrations.push(quote! {
            model_manager.register_method(
                <#many as erp::types::model::CommonModel<
                    erp::types::field::MultipleIds,
                >>::_get_model_name(),
                #name,
                #link_ident,
                #receiver,
                plugin_name,
            );
        });

        if let Some(per_record) = check_receiver(method) {
            let on = method.on.clone().unwrap_or_default();
            registrations.push(quote! {
                model_manager.register_check(
                    <#many as erp::types::model::CommonModel<
                        erp::types::field::MultipleIds,
                    >>::_get_model_name(),
                    #name,
                    &[#(#on),*],
                    #per_record,
                );
            });
        }

        if method.is_rpc {
            let rpc_ident = &names.rpc;
            links.push(rpc_wrapper(method, &names, &self_ty, &many));
            registrations.push(quote! {
                model_manager.register_rpc(
                    <#many as erp::types::model::CommonModel<
                        erp::types::field::MultipleIds,
                    >>::_get_model_name(),
                    #name,
                    #rpc_ident,
                );
            });
        }
    }

    item.items = kept;
    // An inherent function rather than an impl of `HasMethods`: the derive owns that impl, so
    // that registering the model registers the methods with it. The assertion is what catches
    // this block without the matching `#[erp(methods)]` on the struct, which would otherwise
    // compile and quietly register nothing.
    let registration = (!parsed.is_empty()).then(|| {
        quote! {
            impl erp::model::MethodBlock for #self_ty {
                fn register_block(
                    model_manager: &mut erp::model::ModelManager,
                    plugin_name: &str,
                ) {
                    use erp::types::model::BaseModel;
                    #(#registrations)*
                }
            }

            const _: () = {
                fn declares_methods<T: erp::model::DeclaresMethods>() {}
                fn assert() {
                    // Fails when the struct is missing `#[erp(methods)]`.
                    declares_methods::<#many>();
                }
            };
        }
    });

    Ok(quote! {
        #item

        impl #self_ty {
            #(#in_impl)*
        }

        #(#links)*

        #registration
    })
}

/// Whether a method is a check run after records are created or written — a `check_*` method on
/// records or on one record, taking nothing more and yielding nothing — and if so, whether it is
/// run for each record in turn.
fn check_receiver(method: &ParsedMethod) -> Option<bool> {
    let is_unit = matches!(&method.ret, Type::Tuple(tuple) if tuple.elems.is_empty());
    let is_check =
        method.name.to_string().starts_with("check_") && method.args.is_empty() && is_unit;
    match method.receiver {
        MethodReceiver::Records if is_check => Some(false),
        MethodReceiver::Record if is_check => Some(true),
        _ => None,
    }
}

/// Names the generated items answer to.
struct Names {
    body: Ident,
    link: Ident,
    rpc: Ident,
}

impl Names {
    /// Named after the block's mode too, as a model's two blocks may share their module.
    fn of(struct_ident: &Ident, block: MethodReceiver, method: &ParsedMethod) -> Self {
        let method_name = method.name.to_string();
        let mode = if block == MethodReceiver::Record {
            "one"
        } else {
            "many"
        };
        Self {
            body: Ident::new(&format!("__erp_impl_{method_name}"), Span::call_site()),
            link: Ident::new(
                &format!("__erp_link_{struct_ident}_{mode}_{method_name}"),
                Span::call_site(),
            ),
            rpc: Ident::new(
                &format!("__erp_rpc_{struct_ident}_{mode}_{method_name}"),
                Span::call_site(),
            ),
        }
    }
}

/// The arguments, as the tuple every contributor's chain is keyed on.
fn args_tuple(method: &ParsedMethod) -> TokenStream {
    let types = method.args.iter().map(|(_, ty)| ty);
    quote! { (#(#types,)*) }
}

/// The cursor type, spelled out so the author does not have to.
fn super_type(method: &ParsedMethod) -> TokenStream {
    let args = args_tuple(method);
    let ret = &method.ret;
    quote! { erp::types::method::Super<'_, #args, #ret> }
}

/// The method as written, under a name nothing calls directly.
///
/// Renaming is what makes the override work: the name the author wrote now belongs to the
/// dispatcher, so every call site — including ones compiled before the overriding plugin existed
/// — goes through the chain.
fn renamed_body(method: &ParsedMethod, names: &Names) -> TokenStream {
    let mut func = method.item.clone();
    func.attrs.retain(|a| !a.meta.path().is_ident("erp"));
    func.sig.ident = names.body.clone();
    func.vis = syn::Visibility::Inherited;

    // Only when the author asked for a cursor: otherwise the last parameter is an ordinary
    // argument, and rewriting its type would silently replace it.
    if method.has_sup {
        let sup_ty = super_type(method);
        if let Some(syn::FnArg::Typed(sup)) = func.sig.inputs.last_mut() {
            *sup.ty = syn::parse_quote! { #sup_ty };
        }
    }
    quote! {
        #[doc(hidden)]
        #func
    }
}

/// The name the author wrote, now resolving to the top of the chain: on the records, the record,
/// or — for a method without `self` — no record at all.
fn dispatcher(method: &ParsedMethod, many: &Type) -> TokenStream {
    let name = &method.name;
    let output = &method.item.sig.output;
    let vis = &method.item.vis;
    let params = method.args.iter().map(|(ident, ty)| quote! { #ident: #ty });
    let values = method.args.iter().map(|(ident, _)| quote! { #ident });
    let method_name = name.to_string();
    let docs: Vec<_> = method
        .item
        .attrs
        .iter()
        .filter(|a| a.meta.path().is_ident("doc"))
        .collect();
    let (receiver, ids) = match method.receiver {
        MethodReceiver::Records => (
            quote! { &self, },
            quote! { erp::types::model::CommonModel::get_id_mode(self).clone() },
        ),
        MethodReceiver::Record => (
            quote! { &self, },
            quote! { erp::types::model::CommonModel::get_id_mode(self).into() },
        ),
        MethodReceiver::Model => (quote! {}, quote! { ::core::default::Default::default() }),
    };

    quote! {
        #(#docs)*
        ///
        /// Dispatched from the most derived implementation, which is why calling it from another
        /// method of this model reaches an override rather than the implementation next to it.
        #vis fn #name(
            #receiver
            env: &mut erp::environment::Environment,
            #(#params,)*
        ) #output {
            use erp::types::model::BaseModel;
            let ids: erp::types::field::MultipleIds = #ids;
            let args = (#(#values,)*);
            env.call_method(
                <#many as erp::types::model::CommonModel<
                    erp::types::field::MultipleIds,
                >>::_get_model_name(),
                #method_name,
                &ids,
                &args,
            )
        }
    }
}

/// The entry stored in the chain.
///
/// Takes the recordset as ids and rebuilds this struct from them, because the contributors to one
/// chain are different Rust types and cannot share a pointer naming any one of them.
fn link(method: &ParsedMethod, names: &Names, self_ty: &Type) -> TokenStream {
    let body = &names.body;
    let link = &names.link;
    let args = args_tuple(method);
    let ret = &method.ret;
    let sup_ty = super_type(method);
    let forwarded = method.args.iter().enumerate().map(|(index, _)| {
        let index = syn::Index::from(index);
        quote! { ::core::clone::Clone::clone(&args.#index) }
    });
    let target = match method.receiver {
        MethodReceiver::Model => quote! { <#self_ty>:: },
        _ => quote! { record. },
    };
    // A method that never calls the one it overrides does not have to declare the cursor.
    let call = if method.has_sup {
        quote! { #target #body(env, #(#forwarded,)* sup) }
    } else {
        quote! {
            let _ = sup;
            #target #body(env, #(#forwarded,)*)
        }
    };
    let record = record_of(method, self_ty, quote! { ids });

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #link(
            ids: erp::types::field::MultipleIds,
            env: &mut dyn erp::types::environment::ErasedEnvironment,
            args: &#args,
            sup: #sup_ty,
        ) -> ::core::result::Result<
            #ret,
            ::std::boxed::Box<dyn ::std::error::Error + Send + Sync>,
        > {
            let env = erp::environment::Environment::from_erased(env);
            #record
            #call
        }
    }
}

/// `Model<MultipleIds>` or `Model<SingleId>`, from the type the block is on.
fn block_receiver(self_ty: &Type) -> Result<MethodReceiver> {
    let Type::Path(path) = self_ty else {
        return Err(Error::new(self_ty.span(), "Expected a struct"));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(Error::new(self_ty.span(), "Expected a struct"));
    };
    let mode = match &segment.arguments {
        syn::PathArguments::AngleBracketed(arguments) => arguments.args.first(),
        _ => None,
    };
    let mode = mode.map(|mode| quote! { #mode }.to_string());
    match mode
        .as_deref()
        .map(|mode| mode.rsplit("::").next().unwrap_or(mode).trim())
    {
        Some("MultipleIds") => Ok(MethodReceiver::Records),
        Some("SingleId") => Ok(MethodReceiver::Record),
        _ => Err(Error::new(
            self_ty.span(),
            "An #[erp_methods] block is on Model<MultipleIds>, for records, or on Model<SingleId>, \
             for one record",
        )),
    }
}

/// Rebuild what a method works on from the ids the chain carries: the records, the one record —
/// or none, as a `SingleId` may be empty, refusing several — or nothing, for a method of the model.
fn record_of(method: &ParsedMethod, self_ty: &Type, ids: TokenStream) -> TokenStream {
    let name = method.name.to_string();
    match method.receiver {
        MethodReceiver::Records => quote! {
            let record = <#self_ty as erp::types::model::CommonModel<
                erp::types::field::MultipleIds,
            >>::create_instance(#ids);
        },
        MethodReceiver::Record => quote! {
            let record = match #ids.as_single() {
                Ok(id) => <#self_ty as erp::types::model::CommonModel<
                    erp::types::field::SingleId,
                >>::create_instance(id),
                Err(count) => {
                    return Err(format!("{} works on one record, not {}", #name, count).into());
                }
            };
        },
        MethodReceiver::Model => quote! { let _ = #ids; },
    }
}

fn self_struct_ident(self_ty: &Type) -> Result<Ident> {
    let Type::Path(path) = self_ty else {
        return Err(Error::new(self_ty.span(), "Expected a struct"));
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.clone())
        .ok_or_else(|| Error::new(self_ty.span(), "Expected a struct"))
}

/// The method, wrapped so a remote caller can reach it.
///
/// Takes the request's parameters as they arrived, like everything else reachable by name, and
/// reads the shape it expects out of them: the records to act on, and the arguments.
///
/// Calls the method by its own name rather than the renamed body, so a remote call goes through
/// the override chain exactly like an internal one.
///
/// Arguments arrive named rather than positional: a caller that sends `{"days": 3}` keeps
/// working when a second argument is added, one that sends `[3]` does not. The structs are local
/// to the wrapper, so nothing outside ever names them.
fn rpc_wrapper(method: &ParsedMethod, names: &Names, self_ty: &Type, many: &Type) -> TokenStream {
    let call = &method.name;
    let rpc = &names.rpc;
    let ret = &method.ret;
    let fields = method.args.iter().map(|(ident, ty)| quote! { #ident: #ty });
    let values = method.args.iter().map(|(ident, _)| quote! { args.#ident });
    let record = record_of(method, self_ty, quote! { ids });
    let target = match method.receiver {
        MethodReceiver::Model => quote! { <#self_ty>:: },
        _ => quote! { record. },
    };

    // A method taking nothing is called without saying so; one taking something is not, because
    // a missing argument is a mistake rather than a default.
    let (args_derive, args_default) = if method.args.is_empty() {
        (
            quote! { #[derive(erp::serde::Deserialize, Default)] },
            quote! { #[serde(default)] },
        )
    } else {
        (quote! { #[derive(erp::serde::Deserialize)] }, quote! {})
    };

    // Asserted separately from the wrapper's own use of them, so a missing impl is reported on
    // the type the author wrote rather than deep inside generated code where the name means
    // nothing.
    let arg_bounds = method.args.iter().map(|(ident, ty)| {
        quote_spanned! {ty.span()=>
            let _ = |_: &#ty| {
                fn assert_argument_is_readable_from_json<
                    T: for<'de> erp::serde::Deserialize<'de>,
                >() {}
                assert_argument_is_readable_from_json::<#ty>();
                stringify!(#ident)
            };
        }
    });
    let ret_bound = quote_spanned! {method.ret.span()=>
        fn assert_return_is_writable_to_json<T: erp::serde::Serialize>() {}
        assert_return_is_writable_to_json::<#ret>();
    };

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #rpc(
            env: &mut erp::environment::Environment,
            _model: &str,
            params: &erp::serde_json::Value,
        ) -> ::core::result::Result<
            erp::serde_json::Value,
            ::std::boxed::Box<dyn ::std::error::Error + Send + Sync>,
        > {
            #(#arg_bounds)*
            #ret_bound

            #args_derive
            #[serde(crate = "erp::serde")]
            struct Args {
                #(#fields,)*
            }

            #[derive(erp::serde::Deserialize)]
            #[serde(crate = "erp::serde")]
            struct Call {
                #[serde(default)]
                ids: Vec<u32>,
                #args_default
                args: Args,
            }

            let call: Call = erp::serde_json::from_value(params.clone())?;
            let args = call.args;
            let ids = env.existing(
                <#many as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::_get_model_name(),
                call.ids,
            )?;
            #record
            let out = #target #call(env, #(#values,)*)?;
            Ok(erp::serde_json::to_value(out)?)
        }
    }
}
