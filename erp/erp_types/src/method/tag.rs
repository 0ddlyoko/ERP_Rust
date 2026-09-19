use crate::model::BaseModel;

/// Identity and signature of one overridable method.
///
/// Rust has no runtime reflection, so the link between the name a caller uses and the functions
/// implementing it has to be built at compile time. A tag is that link: a zero-sized type,
/// generated once by whichever struct declares the method, naming the model, the method, and the
/// types every contributor must agree on.
///
/// A plugin overriding the method names the *declaring* plugin's tag, which is what makes a
/// mismatched signature a compile error in the overriding crate rather than a surprise at
/// startup.
pub trait MethodTag: 'static {
    /// Model the method belongs to.
    ///
    /// An associated type rather than a name, so that a tag cannot be attached to a model that
    /// does not exist, and so the model name comes from the one place that already holds it.
    type Model: BaseModel;
    /// Arguments, as a single struct generated from the declared parameter list.
    type Args: 'static;
    /// What the method returns.
    type Ret: 'static;
    /// Name the method answers to.
    const NAME: &'static str;
}

/// Model a tag belongs to.
pub fn model_of<T: MethodTag>() -> &'static str {
    <T::Model as BaseModel>::_get_model_name()
}
