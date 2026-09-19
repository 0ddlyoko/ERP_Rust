use erp_types::method::{MethodFn, MethodTag, model_of};
use std::any::{Any, TypeId};

/// Every implementation of one overridable method, most-derived first.
///
/// The links are stored type-erased because a registry holds methods of many different
/// signatures, and recovered with a single downcast per call: the arguments and the return value
/// are never erased, so a call costs one type check and then runs fully typed.
pub struct MethodChain {
    /// Identity of the tag the links were registered under.
    ///
    /// Two structs contributing to the same method under different tags disagree on its
    /// signature, which is what this catches.
    tag: TypeId,
    /// `Vec<MethodFn<T>>` for the `T` that `tag` identifies.
    links: Box<dyn Any + Send + Sync>,
    /// Plugins that contributed, in registration order, to name them in an error.
    contributors: Vec<String>,
}

impl MethodChain {
    fn new<T: MethodTag>() -> Self {
        Self {
            tag: TypeId::of::<T>(),
            links: Box::new(Vec::<MethodFn<T>>::new()),
            contributors: Vec::new(),
        }
    }

    /// Whether this chain carries the signature `T` describes.
    fn holds<T: MethodTag>(&self) -> bool {
        self.tag == TypeId::of::<T>()
    }

    fn push<T: MethodTag>(&mut self, link: MethodFn<T>, plugin_name: &str) {
        let Some(links) = self.links.downcast_mut::<Vec<MethodFn<T>>>() else {
            panic!(
                "Method \"{}\".\"{}\" is declared with two different signatures: by {} and now by \
                 {plugin_name}. Every struct overriding a method must name the tag of the struct \
                 that declared it.",
                model_of::<T>(),
                T::NAME,
                self.contributors.join(", "),
            );
        };
        // Registration follows plugin load order, so the newest contributor is the most derived
        // and must run first.
        links.insert(0, link);
        self.contributors.push(plugin_name.to_string());
    }

    fn links<T: MethodTag>(&self) -> Option<&[MethodFn<T>]> {
        self.links
            .downcast_ref::<Vec<MethodFn<T>>>()
            .map(Vec::as_slice)
    }

    /// Plugins that contributed an implementation, in registration order.
    pub fn contributors(&self) -> &[String] {
        &self.contributors
    }
}

/// Registry of the overridable methods of one model.
#[derive(Default)]
pub struct MethodRegistry {
    chains: std::collections::HashMap<String, MethodChain>,
}

impl MethodRegistry {
    pub fn register<T: MethodTag>(&mut self, link: MethodFn<T>, plugin_name: &str) {
        self.chains
            .entry(T::NAME.to_string())
            .or_insert_with(MethodChain::new::<T>)
            .push(link, plugin_name);
    }

    /// Implementations of `T`, most-derived first.
    ///
    /// `None` when nothing registered under that name, or when what did registered a different
    /// signature — both are programming errors the caller reports with the name in hand.
    pub fn chain<T: MethodTag>(&self) -> Option<&[MethodFn<T>]> {
        let chain = self.chains.get(T::NAME)?;
        chain.holds::<T>().then(|| chain.links::<T>())?
    }

    pub fn get(&self, name: &str) -> Option<&MethodChain> {
        self.chains.get(name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.chains.keys().map(String::as_str)
    }
}
