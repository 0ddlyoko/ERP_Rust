/// Type-erased handle to the framework's `Environment`.
///
/// It exists only to break the dependency cycle between the framework and generated model code:
/// `erp_types` cannot name `Environment`, which lives in `erp`.
///
/// Implementing this trait outside the framework is a soundness hazard, because the framework
/// recovers the concrete type from it through a pointer cast. [`ErasedEnvironment::erased_type_name`]
/// lets that cast be checked instead of assumed.
pub trait ErasedEnvironment {
    /// Name of the concrete type behind this handle.
    ///
    /// Used by the framework as an identity check before casting back. Not meant to be called or
    /// implemented by anything else.
    #[doc(hidden)]
    fn erased_type_name(&self) -> &'static str;
}
