use erp_types::field::{
    ComputeFn, FieldCompute, FieldDepend, FieldKind, FieldReference, FieldType,
};
use std::any::TypeId;
use std::collections::HashSet;

/// Field descriptor represented by a single field in a single struct model
pub struct InternalField {
    pub name: String,
    pub kind: FieldKind,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    pub compute: Option<FieldCompute>,
    pub field_ref: Option<FieldReference>,
}

/// Final descriptor of a field.
///
/// Represent all combined InternalModel
pub struct FinalInternalField {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub kind: FieldKind,
    /// Value given to the field when a record is created without one. `None` means the field
    /// simply starts empty.
    pub default_value: Option<FieldType>,
    pub compute: Option<FieldCompute>,
    // If the type is M2O, O2M or M2M, there is an inverse here (but the field could be empty)
    pub inverse: Option<FieldReference>,
    pub depends: Vec<Vec<FieldDepend>>,
    /// Implementations of this field's compute, most-derived first.
    ///
    /// Each one receives a cursor over the rest, which is how a plugin reaches the implementation
    /// it overrides.
    pub compute_chain: Vec<ComputeFn>,
    is_init: bool,
}

impl FinalInternalField {
    pub fn new(field_name: &str) -> Self {
        FinalInternalField {
            name: field_name.to_string(),
            description: field_name.to_string(),
            required: false,
            kind: FieldKind::String,
            default_value: None,
            compute: None,
            inverse: None,
            depends: Vec::new(),
            compute_chain: Vec::new(),
            is_init: false,
        }
    }

    /// Whether the field lives in a column of its own.
    pub fn is_stored(&self) -> bool {
        self.kind.is_stored()
    }

    pub fn register_internal_field(
        &mut self,
        field_descriptor: &InternalField,
        type_id: &TypeId,
        compute_fn: ComputeFn,
    ) {
        // Every struct contributing to a field must agree on its type.
        if self.is_init && self.kind != field_descriptor.kind {
            panic!(
                "Field {} is declared as {} and as {} by two different structs",
                self.name, self.kind, field_descriptor.kind
            );
        }
        self.kind = field_descriptor.kind;
        if field_descriptor.default_value.is_some() {
            self.default_value = field_descriptor.default_value.clone();
        }
        if let Some(description) = &field_descriptor.description {
            self.description = description.clone();
        }
        self.required = field_descriptor.required;
        if let Some(new_compute) = &field_descriptor.compute {
            // Registration follows plugin load order, so the newest contributor is the most
            // derived and must run first.
            self.compute_chain.insert(0, compute_fn);
            if let Some(existing_compute) = &mut self.compute {
                existing_compute.type_id = *type_id;
                existing_compute
                    .depends
                    .append(&mut new_compute.depends.clone());
                // Remove duplicates
                let mut seen = HashSet::new();
                existing_compute
                    .depends
                    .retain(|dep| seen.insert(dep.clone()));
            } else {
                self.compute = Some(FieldCompute {
                    type_id: *type_id,
                    depends: new_compute.depends.clone(),
                });
            }
        }
        if let Some(inverse) = &field_descriptor.field_ref {
            self.inverse = Some(inverse.clone());
        }
        self.is_init = true;
    }
}
