use crate::methods::parse::{MethodRole, ParsedMethod, parse_method, role_of};
use erp::util::string::StringTransform;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Error, ImplItem, ItemImpl, Result, Type};

pub fn expand(mut item: ItemImpl) -> Result<TokenStream> {
    let struct_ident = self_struct_ident(&item.self_ty)?;
    let self_ty = &item.self_ty;

    let mut parsed = Vec::new();
    let mut kept = Vec::new();
    for entry in std::mem::take(&mut item.items) {
        let ImplItem::Fn(func) = entry else {
            kept.push(entry);
            continue;
        };
        match role_of(&func)? {
            Some(role) => parsed.push(parse_method(func, role)?),
            None => kept.push(ImplItem::Fn(func)),
        }
    }

    let mut declarations = Vec::new();
    let mut in_impl = Vec::new();
    let mut links = Vec::new();
    let mut registrations = Vec::new();

    for method in &parsed {
        let names = Names::of(&struct_ident, method);
        declarations.push(declaration(method, &names, self_ty)?);
        in_impl.push(renamed_body(method, &names));
        in_impl.push(dispatcher(method, &names));
        links.push(link(method, &names, self_ty));
        let tag = &names.tag;
        let link_ident = &names.link;
        registrations.push(quote! {
            model_manager.register_method::<#tag>(#link_ident, plugin_name);
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
        #(#declarations)*

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
    tag: TokenStream,
    args: TokenStream,
    /// Defined only when this struct declares the method rather than overriding one.
    owned_tag: Option<Ident>,
    owned_args: Option<Ident>,
    /// Alias through which the argument struct is built, which is what lets an override construct
    /// the declaring struct's type without naming it.
    args_alias: Ident,
    body: Ident,
    link: Ident,
}

impl Names {
    fn of(struct_ident: &Ident, method: &ParsedMethod) -> Self {
        let method_name = method.name.to_string();
        let camel = method_name.replace('_', " ").to_camel_case();
        let prefix = format!("{struct_ident}{camel}");
        let args_alias = Ident::new(
            &format!("__ErpArgs{struct_ident}{camel}"),
            Span::call_site(),
        );
        let body = Ident::new(&format!("__erp_impl_{method_name}"), Span::call_site());
        let link = Ident::new(
            &format!("__erp_link_{struct_ident}_{method_name}"),
            Span::call_site(),
        );

        match &method.role {
            MethodRole::Overridable => {
                let tag = Ident::new(&prefix, Span::call_site());
                let args = Ident::new(&format!("{prefix}Args"), Span::call_site());
                Self {
                    tag: quote! { #tag },
                    args: quote! { #args },
                    owned_tag: Some(tag),
                    owned_args: Some(args),
                    args_alias,
                    body,
                    link,
                }
            }
            MethodRole::Overrides(path) => Self {
                tag: quote! { #path },
                args: quote! {
                    <#path as erp::types::method::MethodTag>::Args
                },
                owned_tag: None,
                owned_args: None,
                args_alias,
                body,
                link,
            },
        }
    }
}

/// The tag and argument struct, emitted once by the struct that declares the method.
fn declaration(method: &ParsedMethod, names: &Names, self_ty: &Type) -> Result<TokenStream> {
    let args_alias = &names.args_alias;
    let args_ty = &names.args;
    let alias = quote! {
        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        type #args_alias = #args_ty;
    };

    let (Some(tag), Some(args)) = (&names.owned_tag, &names.owned_args) else {
        return Ok(alias);
    };

    let name = method.name.to_string();
    let ret = &method.ret;
    let fields = method.args.iter().map(|(ident, ty)| {
        quote! { pub #ident: #ty }
    });
    let doc = format!("Arguments of the overridable method `{name}`.");
    let tag_doc = format!(
        "Override point for `{name}`. A plugin extending this model names it in \
         `#[erp(overrides = \"...\")]`, which is what makes a mismatched signature a compile error."
    );

    Ok(quote! {
        #[doc = #tag_doc]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
        pub struct #tag;

        #[doc = #doc]
        #[derive(Debug, Clone)]
        pub struct #args {
            #(#fields,)*
        }

        impl erp::types::method::MethodTag for #tag {
            type Model = <#self_ty as erp::types::model::CommonModel<
                erp::types::field::MultipleIds,
            >>::BaseModel;
            type Args = #args;
            type Ret = #ret;
            const NAME: &'static str = #name;
        }

        #alias
    })
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

    let tag = &names.tag;
    if let Some(syn::FnArg::Typed(sup)) = func.sig.inputs.last_mut() {
        *sup.ty = syn::parse_quote! {
            erp::types::method::Super<'_, #tag>
        };
    }
    quote! {
        #[doc(hidden)]
        #func
    }
}

/// The name the author wrote, now resolving to the top of the chain.
fn dispatcher(method: &ParsedMethod, names: &Names) -> TokenStream {
    let name = &method.name;
    let tag = &names.tag;
    let args_alias = &names.args_alias;
    let output = &method.item.sig.output;
    let vis = &method.item.vis;
    let params = method.args.iter().map(|(ident, ty)| quote! { #ident: #ty });
    let fields = method.args.iter().map(|(ident, _)| quote! { #ident });
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
            let ids: erp::types::field::MultipleIds =
                erp::types::model::CommonModel::get_id_mode(self).clone();
            let args = #args_alias { #(#fields,)* };
            env.call_method::<#tag>(&ids, &args)
        }
    }
}

/// The entry stored in the chain.
///
/// Takes the recordset as ids and rebuilds this struct from them, because the contributors to one
/// chain are different Rust types and cannot share a pointer naming any one of them.
fn link(method: &ParsedMethod, names: &Names, self_ty: &Type) -> TokenStream {
    let tag = &names.tag;
    let body = &names.body;
    let link = &names.link;
    let forwarded = method.args.iter().map(|(ident, _)| {
        quote! { ::core::clone::Clone::clone(&args.#ident) }
    });

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #link(
            ids: erp::types::field::MultipleIds,
            env: &mut dyn erp::types::environment::ErasedEnvironment,
            args: &<#tag as erp::types::method::MethodTag>::Args,
            sup: erp::types::method::Super<'_, #tag>,
        ) -> ::core::result::Result<
            <#tag as erp::types::method::MethodTag>::Ret,
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
