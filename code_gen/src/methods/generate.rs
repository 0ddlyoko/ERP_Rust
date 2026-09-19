use crate::methods::parse::{ParsedMethod, is_overridable, parse_method};
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Error, ImplItem, ItemImpl, Result, Type};

pub fn expand(mut item: ItemImpl) -> Result<TokenStream> {
    let struct_ident = self_struct_ident(&item.self_ty)?;
    let self_ty = item.self_ty.clone();

    let mut parsed = Vec::new();
    let mut kept = Vec::new();
    for entry in std::mem::take(&mut item.items) {
        let ImplItem::Fn(func) = entry else {
            kept.push(entry);
            continue;
        };
        match is_overridable(&func)? {
            true => parsed.push(parse_method(func)?),
            false => kept.push(ImplItem::Fn(func)),
        }
    }

    let mut in_impl = Vec::new();
    let mut links = Vec::new();
    let mut registrations = Vec::new();

    for method in &parsed {
        let names = Names::of(&struct_ident, method);
        in_impl.push(renamed_body(method, &names));
        in_impl.push(dispatcher(method));
        links.push(link(method, &names, &self_ty));

        let link_ident = &names.link;
        let name = method.name.to_string();
        registrations.push(quote! {
            model_manager.register_method(
                <Self as erp::types::model::CommonModel<
                    erp::types::field::MultipleIds,
                >>::_get_model_name(),
                #name,
                #link_ident,
                plugin_name,
            );
        });
    }

    item.items = kept;
    // An inherent function rather than an impl of `HasMethods`: the derive owns that impl, so
    // that registering the model registers the methods with it. The assertion is what catches
    // this block without the matching `#[erp(methods)]` on the struct, which would otherwise
    // compile and quietly register nothing.
    let registration = (!parsed.is_empty()).then(|| {
        quote! {
            impl #self_ty {
                #[doc(hidden)]
                fn __erp_register_methods(
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
                    declares_methods::<#self_ty>();
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

/// Names the generated items answer to.
struct Names {
    body: Ident,
    link: Ident,
}

impl Names {
    fn of(struct_ident: &Ident, method: &ParsedMethod) -> Self {
        let method_name = method.name.to_string();
        Self {
            body: Ident::new(&format!("__erp_impl_{method_name}"), Span::call_site()),
            link: Ident::new(
                &format!("__erp_link_{struct_ident}_{method_name}"),
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

    let sup_ty = super_type(method);
    if let Some(syn::FnArg::Typed(sup)) = func.sig.inputs.last_mut() {
        *sup.ty = syn::parse_quote! { #sup_ty };
    }
    quote! {
        #[doc(hidden)]
        #func
    }
}

/// The name the author wrote, now resolving to the top of the chain.
fn dispatcher(method: &ParsedMethod) -> TokenStream {
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

    quote! {
        #(#docs)*
        ///
        /// Dispatched from the most derived implementation, which is why calling it from another
        /// method of this model reaches an override rather than the implementation next to it.
        #vis fn #name(
            &self,
            env: &mut erp::environment::Environment,
            #(#params,)*
        ) #output {
            use erp::types::model::BaseModel;
            let ids: erp::types::field::MultipleIds =
                erp::types::model::CommonModel::get_id_mode(self).clone();
            let args = (#(#values,)*);
            env.call_method(
                <Self as erp::types::model::CommonModel<
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
            let record = <#self_ty as erp::types::model::CommonModel<
                erp::types::field::MultipleIds,
            >>::create_instance(ids);
            record.#body(env, #(#forwarded,)* sup)
        }
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
