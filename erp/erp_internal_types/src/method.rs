use erp_types::method::MethodFn;
use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::fmt;

/// What two contributors to one method have to agree on.
///
/// Held as the two type names rather than the whole function pointer, because those are what an
/// author writes and what they have to change to agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    pub args: &'static str,
    pub returns: &'static str,
}

impl Signature {
    fn of<A: 'static, R: 'static>() -> Self {
        Self {
            args: type_name::<A>(),
            returns: type_name::<R>(),
        }
    }
}

impl fmt::Display for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "arguments {}, returning {}", self.args, self.returns)
    }
}

/// Every implementation of one overridable method, most-derived first.
///
/// The links are stored type-erased because a registry holds methods of many different
/// signatures, and recovered with a single downcast per call: the arguments and the return value
/// are never erased, so a call costs one type check and then runs fully typed.
pub struct MethodChain {
    /// `Vec<MethodFn<A, R>>` for the argument and return types the contributors agreed on.
    links: Box<dyn Any + Send + Sync>,
    /// Spelled out for the error a disagreeing contributor gets.
    signature: Signature,
    /// Plugins that contributed, in registration order, to name them in that error.
    contributors: Vec<String>,
}

impl MethodChain {
    fn new<A: 'static, R: 'static>() -> Self {
        Self {
            links: Box::new(Vec::<MethodFn<A, R>>::new()),
            signature: Signature::of::<A, R>(),
            contributors: Vec::new(),
        }
    }

    fn holds<A: 'static, R: 'static>(&self) -> bool {
        self.links.as_ref().type_id() == TypeId::of::<Vec<MethodFn<A, R>>>()
    }

    /// Plugins that contributed an implementation, in registration order.
    pub fn contributors(&self) -> &[String] {
        &self.contributors
    }

    /// The signature every contributor has to agree on.
    pub fn signature(&self) -> Signature {
        self.signature
    }
}

/// Registry of the overridable methods of one model.
///
/// Keyed by the method name alone. Two structs contributing the same name to the same model land
/// on the same chain without either naming anything the other declared — the relationship a
/// computed field already gets from its field declaration.
#[derive(Default)]
pub struct MethodRegistry {
    chains: HashMap<String, MethodChain>,
}

impl MethodRegistry {
    /// Add one implementation, ahead of the ones registered before it.
    ///
    /// Panics when a contributor disagrees on the signature. It happens at startup, with both
    /// plugin names in hand, rather than at the first call.
    pub fn register<A: 'static, R: 'static>(
        &mut self,
        model_name: &str,
        method_name: &str,
        link: MethodFn<A, R>,
        plugin_name: &str,
    ) {
        let chain = self
            .chains
            .entry(method_name.to_string())
            .or_insert_with(MethodChain::new::<A, R>);

        let Some(links) = chain.links.downcast_mut::<Vec<MethodFn<A, R>>>() else {
            panic!(
                "Method \"{model_name}\".\"{method_name}\" is declared with two different \
                 signatures.\n  {} declared: {}\n  {plugin_name} declares: {}\n\
                 Overriding a method means matching the arguments and the return type of the one \
                 already declared.",
                chain.contributors.join(", "),
                chain.signature,
                Signature::of::<A, R>(),
            );
        };
        // Registration follows plugin load order, so the newest contributor is the most derived
        // and must run first.
        links.insert(0, link);
        chain.contributors.push(plugin_name.to_string());
    }

    /// Implementations of a method, most-derived first.
    ///
    /// `None` when nothing registered under that name; a signature that does not match is a
    /// programming error the registration already refused.
    pub fn chain<A: 'static, R: 'static>(&self, method_name: &str) -> Option<&[MethodFn<A, R>]> {
        let chain = self.chains.get(method_name)?;
        chain
            .holds::<A, R>()
            .then(|| chain.links.downcast_ref::<Vec<MethodFn<A, R>>>())?
            .map(Vec::as_slice)
    }

    pub fn get(&self, method_name: &str) -> Option<&MethodChain> {
        self.chains.get(method_name)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.chains.keys().map(String::as_str)
    }
}
