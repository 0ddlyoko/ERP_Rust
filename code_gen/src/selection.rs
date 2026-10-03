use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::spanned::Spanned;
use syn::{Attribute, Fields, Ident, ItemEnum, LitStr, Path, Token, Variant};

/// What `#[selection(...)]` says about the enum: the one it extends, if any.
pub struct EnumArgs {
    extends: Option<Path>,
}

impl syn::parse::Parse for EnumArgs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        if input.is_empty() {
            return Ok(EnumArgs { extends: None });
        }
        let name: Ident = input.parse()?;
        if name != "extends" {
            return Err(syn::Error::new(
                name.span(),
                "#[selection] takes nothing, or `extends = OtherEnum`",
            ));
        }
        input.parse::<Token![=]>()?;
        let extends: Path = input.parse()?;
        Ok(EnumArgs {
            extends: Some(extends),
        })
    }
}

/// What `#[selection(...)]` says about one variant.
struct VariantArgs {
    key: Option<LitStr>,
    label: Option<LitStr>,
    after: Option<LitStr>,
    before: Option<LitStr>,
}

fn variant_args(attrs: &[Attribute]) -> syn::Result<VariantArgs> {
    let mut args = VariantArgs {
        key: None,
        label: None,
        after: None,
        before: None,
    };
    for attr in attrs
        .iter()
        .filter(|attr| attr.path().is_ident("selection"))
    {
        attr.parse_nested_meta(|meta| {
            let value: LitStr = meta.value()?.parse()?;
            let slot = if meta.path.is_ident("key") {
                &mut args.key
            } else if meta.path.is_ident("label") {
                &mut args.label
            } else if meta.path.is_ident("after") {
                &mut args.after
            } else if meta.path.is_ident("before") {
                &mut args.before
            } else {
                return Err(meta.error("expected key, label, after or before"));
            };
            *slot = Some(value);
            Ok(())
        })?;
    }
    if let (Some(_), Some(before)) = (&args.after, &args.before) {
        return Err(syn::Error::new(
            before.span(),
            "a value goes after one value or before one, not both",
        ));
    }
    Ok(args)
}

/// `QuotationSent` as a key: `quotation_sent`.
fn key_of(name: &str) -> String {
    let mut key = String::new();
    for (at, letter) in name.chars().enumerate() {
        if letter.is_uppercase() && at > 0 {
            key.push('_');
        }
        key.extend(letter.to_lowercase());
    }
    key
}

/// `QuotationSent` as a label: `Quotation sent`.
fn label_of(name: &str) -> String {
    let words = key_of(name);
    let mut letters = words.chars();
    match letters.next() {
        Some(first) => first
            .to_uppercase()
            .chain(letters.map(|letter| if letter == '_' { ' ' } else { letter }))
            .collect(),
        None => String::new(),
    }
}

/// Turn an enum into a selection: its variants, plus `Empty` for no value and `Extended` for the
/// values other enums of its family add; the conversions to and from keys; and what it declares,
/// for the registry.
pub fn expand(args: EnumArgs, mut item: ItemEnum) -> syn::Result<TokenStream> {
    let name = item.ident.clone();
    if !item.generics.params.is_empty() {
        return Err(syn::Error::new(
            item.generics.span(),
            "a selection takes no generics",
        ));
    }
    let mut variants = Vec::new();
    let mut values = Vec::new();
    let mut has_default = false;
    for variant in &item.variants {
        let Variant {
            ident,
            fields,
            discriminant,
            attrs,
            ..
        } = variant;
        if !matches!(fields, Fields::Unit) || discriminant.is_some() {
            return Err(syn::Error::new(
                variant.span(),
                "a selection's values are plain names, without fields or numbers",
            ));
        }
        if ident == "Empty" {
            return Err(syn::Error::new(
                ident.span(),
                "`Empty` is the variant every selection gets for no value",
            ));
        }
        if ident == "Extended" {
            return Err(syn::Error::new(
                ident.span(),
                "`Extended` is the variant every selection gets for the values others add",
            ));
        }
        has_default |= attrs.iter().any(|attr| attr.path().is_ident("default"));
        let VariantArgs {
            key,
            label,
            after,
            before,
        } = variant_args(attrs)?;
        if args.extends.is_none()
            && let Some(position) = after.as_ref().or(before.as_ref())
        {
            return Err(syn::Error::new(
                position.span(),
                "only an enum extending another places its values among others'",
            ));
        }
        let key = key
            .as_ref()
            .map(LitStr::value)
            .unwrap_or_else(|| key_of(&ident.to_string()));
        let label_given = label.is_some();
        let label = label
            .as_ref()
            .map(LitStr::value)
            .unwrap_or_else(|| label_of(&ident.to_string()));
        let placement = match (&after, &before) {
            (Some(after), _) => quote! { erp::types::field::Placement::After(#after) },
            (_, Some(before)) => quote! { erp::types::field::Placement::Before(#before) },
            _ => quote! { erp::types::field::Placement::Unchanged },
        };
        values.push(quote! {
            erp::types::field::SelectionValue {
                key: #key,
                label: #label,
                label_given: #label_given,
                placement: #placement,
            }
        });
        variants.push((ident.clone(), key));
    }

    for variant in item.variants.iter_mut() {
        variant
            .attrs
            .retain(|attr| !attr.path().is_ident("selection"));
    }
    item.variants.push(syn::parse_quote! {
        /// No value: what a field of an empty record reads as.
        Empty
    });
    item.variants.push(syn::parse_quote! {
        /// A value of the family this enum does not name: one another enum added.
        Extended(erp::types::field::SelectionKey)
    });
    let default = has_default.then(|| quote! { #[derive(Default)] });

    let (root, parent, family) = match &args.extends {
        Some(parent) => (
            quote! { <#parent as erp::types::field::Selection>::Root },
            quote! { #parent },
            quote! { <#parent as erp::types::field::Selection>::FAMILY },
        ),
        None => (
            quote! { #name },
            quote! { #name },
            quote! { concat!(module_path!(), "::", stringify!(#name)) },
        ),
    };
    let to_key = variants.iter().map(|(ident, key)| {
        quote! { #name::#ident => erp::types::field::SelectionKey::from_static(#key), }
    });
    let from_key = variants.iter().map(|(ident, key)| {
        quote! { #key => &#name::#ident, }
    });
    let span = Span::call_site();
    let extended = Ident::new("Extended", span);

    Ok(quote! {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #default
        #item

        impl erp::types::field::Selection for #name {
            type Root = #root;
            type Parent = #parent;
            const FAMILY: &'static str = #family;
            const VALUES: &'static [erp::types::field::SelectionValue] = &[#(#values),*];

            fn key(&self) -> erp::types::field::SelectionKey {
                match self {
                    #(#to_key)*
                    #name::Empty => erp::types::field::SelectionKey::from_static(""),
                    #name::#extended(key) => *key,
                }
            }

            fn from_key(key: &str) -> Self {
                *Self::from_key_ref(key)
            }

            fn empty_ref() -> &'static Self {
                &#name::Empty
            }

            fn from_key_ref(key: &str) -> &'static Self {
                match key {
                    "" => &#name::Empty,
                    #(#from_key)*
                    _ => {
                        static EXTENDED: ::std::sync::LazyLock<
                            ::std::sync::Mutex<::std::collections::HashMap<&'static str, &'static #name>>,
                        > = ::std::sync::LazyLock::new(::std::default::Default::default);
                        let key = erp::types::field::SelectionKey::new(key);
                        let mut known = EXTENDED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                        known
                            .entry(key.as_str())
                            .or_insert_with(|| ::std::boxed::Box::leak(::std::boxed::Box::new(#name::#extended(key))))
                    }
                }
            }
        }

        impl<V> erp::types::field::Accepts<V> for #name
        where
            V: erp::types::field::Selection<Root = <#name as erp::types::field::Selection>::Root>,
        {
            fn accept(value: V) -> Self {
                erp::types::field::Selection::to(value)
            }
        }
    })
}
