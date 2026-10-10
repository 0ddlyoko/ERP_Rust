use crate::methods::parse::{is_super, unwrap_result};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{Error, FnArg, ImplItem, ImplItemFn, ItemImpl, LitStr, Pat, PatType, Result, Type};

/// One method of an `#[erp_routes]` block.
struct ParsedRoute {
    name: Ident,
    args: Vec<(Ident, Type)>,
    ret: Type,
    has_sup: bool,
    route: Option<RouteArgs>,
    item: ImplItemFn,
}

/// `#[erp_routes]`: the overridable methods of a controller, and the URLs some of them answer.
///
/// Mirrors `#[erp_methods]`. Every method of the block is overridable by name; one with
/// What a route attribute says: the URL, its verbs, whether it needs a CSRF token, and whether
/// it answers anyone without looking the session up.
struct RouteArgs {
    pattern: LitStr,
    verbs: Vec<String>,
    csrf: bool,
    anyone: bool,
}

/// `#[erp(route = "...")]` also answers that URL. The route's parameters and the query string
/// fill the method's arguments by name.
pub fn expand(mut item: ItemImpl) -> Result<TokenStream> {
    let self_ty = item.self_ty.clone();
    let struct_ident = self_struct_ident(&self_ty)?;

    let mut parsed = Vec::new();
    let mut kept = Vec::new();
    for entry in std::mem::take(&mut item.items) {
        match entry {
            ImplItem::Fn(func) => parsed.push(parse(func)?),
            other => kept.push(other),
        }
    }
    item.items = kept;

    let mut in_impl = Vec::new();
    let mut free = Vec::new();
    let mut registrations = Vec::new();
    for method in &parsed {
        let name = &method.name;
        let name_str = name.to_string();
        let body = Ident::new(&format!("__erp_impl_{name}"), Span::call_site());
        let link = Ident::new(
            &format!("__erp_ctl_{struct_ident}_{name}"),
            Span::call_site(),
        );
        let http = Ident::new(
            &format!("__erp_http_{struct_ident}_{name}"),
            Span::call_site(),
        );
        let args_tuple = args_tuple(method);
        let ret = &method.ret;
        let sup_ty = quote! { erp::types::method::Super<'_, #args_tuple, #ret> };
        let body_sup_ty = quote! { erp::model::Super<'_, #args_tuple, #ret> };

        in_impl.push(renamed_body(method, &body, &body_sup_ty));
        in_impl.push(dispatcher(method));
        free.push(link_fn(
            method,
            &link,
            &body,
            &self_ty,
            &args_tuple,
            &sup_ty,
        ));
        registrations.push(quote! {
            registry.register_method::<#args_tuple, #ret>(
                <#self_ty as erp::http::Controller>::controller_name(),
                #name_str,
                #link,
            );
        });

        if let Some(RouteArgs {
            pattern,
            verbs,
            csrf,
            anyone,
        }) = &method.route
        {
            let auth = if *anyone {
                quote! { erp::http::Auth::None }
            } else {
                quote! { erp::http::Auth::User }
            };
            free.push(http_fn(method, &http, &self_ty));
            registrations.push(quote! {
                registry.register_route(
                    <#self_ty as erp::http::Controller>::controller_name(),
                    #name_str,
                    #pattern,
                    &[#(#verbs),*],
                    #csrf,
                    #auth,
                    #http,
                );
            });
        }
    }

    Ok(quote! {
        #item

        impl #self_ty {
            #(#in_impl)*
        }

        #(#free)*

        impl erp::http::HasRoutes for #self_ty {
            fn register_routes(registry: &mut erp::http::ControllerRegistry) {
                #(#registrations)*
            }
        }
    })
}

fn parse(item: ImplItemFn) -> Result<ParsedRoute> {
    let route = read_route(&item)?;
    let help = "A controller method takes &self, an &mut Environment, the &Request, its own \
                arguments, and may end with a `sup: Super` cursor";
    let inputs: Vec<&FnArg> = item.sig.inputs.iter().collect();
    let Some(FnArg::Receiver(_)) = inputs.first() else {
        return Err(Error::new(item.sig.span(), help));
    };
    if inputs.len() < 3 {
        return Err(Error::new(item.sig.span(), help));
    }
    let has_sup = inputs.last().is_some_and(|last| is_super(last));
    let end = if has_sup {
        inputs.len() - 1
    } else {
        inputs.len()
    };

    let mut args = Vec::new();
    for input in &inputs[3..end] {
        let FnArg::Typed(PatType { pat, ty, .. }) = input else {
            return Err(Error::new(input.span(), help));
        };
        let Pat::Ident(ident) = pat.as_ref() else {
            return Err(Error::new(
                pat.span(),
                "An argument of a controller method must be a plain name: it is the name of the \
                 parameter it is read from",
            ));
        };
        args.push((ident.ident.clone(), (**ty).clone()));
    }

    if let Some(RouteArgs { pattern, .. }) = &route {
        let value = pattern.value();
        let parts: Vec<&str> = value.split('/').filter(|part| !part.is_empty()).collect();
        for (index, part) in parts.iter().enumerate() {
            let Some(param) = part.strip_prefix('<').and_then(|p| p.strip_suffix('>')) else {
                continue;
            };
            let param = match param.strip_prefix('*') {
                Some(rest) if index + 1 == parts.len() => rest,
                Some(_) => {
                    return Err(Error::new(
                        pattern.span(),
                        "<*name> takes the rest of the path, so it is the last segment",
                    ));
                }
                None => param,
            };
            if !args.iter().any(|(ident, _)| ident == param) {
                return Err(Error::new(
                    pattern.span(),
                    format!("Route parameter <{param}> is not an argument of this method"),
                ));
            }
        }
    }

    Ok(ParsedRoute {
        name: item.sig.ident.clone(),
        ret: unwrap_result(&item.sig.output)?,
        args,
        has_sup,
        route,
        item,
    })
}

/// `#[erp(route = "/path/<param>", methods = ["GET", "POST"], csrf = false, auth = "none")]`,
/// GET when no method is named. A request changing something needs a CSRF token unless
/// `csrf = false`; the caller is the session's user unless `auth = "none"`.
fn read_route(item: &ImplItemFn) -> Result<Option<RouteArgs>> {
    let mut route: Option<LitStr> = None;
    let mut verbs: Vec<String> = Vec::new();
    let mut csrf: Option<syn::LitBool> = None;
    let mut auth: Option<LitStr> = None;
    for attr in item.attrs.iter().filter(|attr| attr.path().is_ident("erp")) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("route") {
                let pattern: LitStr = meta.value()?.parse()?;
                if !pattern.value().starts_with('/') {
                    return Err(Error::new(pattern.span(), "A route starts with /"));
                }
                route = Some(pattern);
                Ok(())
            } else if meta.path.is_ident("methods") {
                let list: syn::ExprArray = meta.value()?.parse()?;
                for verb in list.elems {
                    let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(verb),
                        ..
                    }) = verb
                    else {
                        return Err(Error::new(
                            verb.span(),
                            "Expected a method name, as \"GET\"",
                        ));
                    };
                    verbs.push(verb.value().to_ascii_uppercase());
                }
                Ok(())
            } else if meta.path.is_ident("csrf") {
                csrf = Some(meta.value()?.parse()?);
                Ok(())
            } else if meta.path.is_ident("auth") {
                let value: LitStr = meta.value()?.parse()?;
                if !matches!(value.value().as_str(), "user" | "none") {
                    return Err(Error::new(value.span(), "auth is \"user\" or \"none\""));
                }
                auth = Some(value);
                Ok(())
            } else {
                Err(meta.error(
                    "Unknown key. The keys on a controller method are: route, methods, csrf, auth",
                ))
            }
        })?;
    }
    match route {
        Some(route) => {
            if verbs.is_empty() {
                verbs.push("GET".to_string());
            }
            let anyone = auth.is_some_and(|auth| auth.value() == "none");
            Ok(Some(RouteArgs {
                pattern: route,
                verbs,
                csrf: csrf.is_none_or(|csrf| csrf.value),
                anyone,
            }))
        }
        None if !verbs.is_empty() || csrf.is_some() || auth.is_some() => Err(Error::new(
            item.sig.ident.span(),
            "`methods`, `csrf` and `auth` are about a route, so they need a `route`",
        )),
        None => Ok(None),
    }
}

/// The arguments every contributor's chain is keyed on: the request, then the method's own.
fn args_tuple(method: &ParsedRoute) -> TokenStream {
    let types = method.args.iter().map(|(_, ty)| ty);
    quote! { (erp::http::Request, #(#types,)*) }
}

fn renamed_body(method: &ParsedRoute, body: &Ident, sup_ty: &TokenStream) -> TokenStream {
    let mut func = method.item.clone();
    func.attrs.retain(|attr| !attr.path().is_ident("erp"));
    func.sig.ident = body.clone();
    func.vis = syn::Visibility::Inherited;
    if method.has_sup
        && let Some(FnArg::Typed(sup)) = func.sig.inputs.last_mut()
    {
        *sup.ty = syn::parse_quote! { #sup_ty };
    }
    quote! {
        #[doc(hidden)]
        #func
    }
}

/// The name the author wrote, now resolving to the top of the chain, answering the error type
/// the author declared.
fn dispatcher(method: &ParsedRoute) -> TokenStream {
    let name = &method.name;
    let name_str = name.to_string();
    let vis = &method.item.vis;
    let output = &method.item.sig.output;
    let params = method.args.iter().map(|(ident, ty)| quote! { #ident: #ty });
    let values = method.args.iter().map(|(ident, _)| quote! { #ident });
    let docs = method
        .item
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"));
    quote! {
        #(#docs)*
        ///
        /// Dispatched from the most derived implementation, so calling it reaches an override
        /// rather than the implementation next to it.
        #vis fn #name(
            &self,
            env: &mut erp::environment::Environment,
            request: &erp::http::Request,
            #(#params,)*
        ) #output {
            let _ = self;
            let answer = env.call_controller(
                <Self as erp::http::Controller>::controller_name(),
                #name_str,
                &(request.clone(), #(#values,)*),
            );
            ::core::result::Result::map_err(answer, ::core::convert::Into::into)
        }
    }
}

fn link_fn(
    method: &ParsedRoute,
    link: &Ident,
    body: &Ident,
    self_ty: &Type,
    args_tuple: &TokenStream,
    sup_ty: &TokenStream,
) -> TokenStream {
    let ret = &method.ret;
    let forwarded = (0..method.args.len()).map(|index| {
        let index = syn::Index::from(index + 1);
        quote! { ::core::clone::Clone::clone(&args.#index) }
    });
    let call = if method.has_sup {
        quote! { controller.#body(env, &args.0, #(#forwarded,)* erp::model::Super::new(sup)) }
    } else {
        quote! {
            let _ = sup;
            controller.#body(env, &args.0, #(#forwarded,)*)
        }
    };
    // Boxed for the chain, which lives below the ORM; boxing an `erp::Error` keeps it whole.
    let call = quote! {
        let answer = { #call };
        ::core::result::Result::map_err(answer, ::core::convert::Into::into)
    };
    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #link(
            _ids: erp::types::field::MultipleIds,
            env: &mut dyn erp::types::environment::ErasedEnvironment,
            args: &#args_tuple,
            sup: #sup_ty,
        ) -> ::core::result::Result<
            #ret,
            ::std::boxed::Box<dyn ::std::error::Error + Send + Sync>,
        > {
            let env = erp::environment::Environment::from_erased(env);
            let controller = <#self_ty as erp::http::Controller>::instance();
            #call
        }
    }
}

/// What the route calls: every argument read from the request by its name, then the method.
fn http_fn(method: &ParsedRoute, http: &Ident, self_ty: &Type) -> TokenStream {
    let name = &method.name;
    let reads = method.args.iter().map(|(ident, ty)| {
        let param = ident.to_string();
        quote_spanned! {ty.span()=>
            let #ident: #ty =
                <#ty as erp::http::FromParam>::from_param(env, request.param(#param))
                    .map_err(|error| erp::http::HttpError::about_parameter(#param, error))?;
        }
    });
    let values = method.args.iter().map(|(ident, _)| quote! { #ident });
    let call = quote_spanned! {method.ret.span()=>
        let response: erp::http::Response =
            <#self_ty as erp::http::Controller>::instance().#name(env, request, #(#values,)*)?;
    };
    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #http(
            env: &mut erp::environment::Environment,
            request: &erp::http::Request,
        ) -> ::core::result::Result<erp::http::Response, erp::Error> {
            #(#reads)*
            #call
            Ok(response)
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
