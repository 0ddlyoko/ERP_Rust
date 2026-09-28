#[derive(Clone)]
pub struct FieldCompute {
    /// Overridable method that fills this field.
    ///
    /// A compute is an ordinary overridable method with no arguments and no return value; this is
    /// the name the registry holds its implementations under. Every struct contributing to the
    /// field has to name the same one, or the field would have two chains and only one of them
    /// would ever run.
    pub method: String,
    pub depends: Vec<&'static str>,
}
