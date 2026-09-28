use erp_types::field::{FieldCompute, FieldDepend, FieldKind, FieldReference, FieldType};
use std::collections::HashSet;

/// Field descriptor represented by a single field in a single struct model
pub struct InternalField {
    pub name: String,
    pub kind: FieldKind,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    pub private: bool,
    pub asks_for_storage: bool,
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
    /// Whether the field never leaves the process. Any struct declaring it so makes it so: a
    /// plugin may hide a field another declared, never reveal one.
    pub private: bool,
    /// Whether the value is meant to live in a column.
    ///
    /// Settled by [`FinalInternalField::settle_storage`] once every struct has been seen, never
    /// by one of them alone. False only for a computed field that nobody asked to keep — its
    /// value is worked out on each read. Distinct from [`FinalInternalField::is_stored`], which
    /// also asks whether a value of this kind could have a column at all.
    pub stored: bool,
    /// Whether any struct asked for a column.
    asked_for_storage: bool,
    pub kind: FieldKind,
    /// Value given to the field when a record is created without one. `None` means the field
    /// simply starts empty.
    pub default_value: Option<FieldType>,
    pub compute: Option<FieldCompute>,
    // If the type is M2O, O2M or M2M, there is an inverse here (but the field could be empty)
    pub inverse: Option<FieldReference>,
    pub depends: Vec<Vec<FieldDepend>>,
    is_init: bool,
}

impl FinalInternalField {
    pub fn new(field_name: &str) -> Self {
        FinalInternalField {
            name: field_name.to_string(),
            description: field_name.to_string(),
            required: false,
            private: false,
            stored: false,
            asked_for_storage: false,
            kind: FieldKind::String,
            default_value: None,
            compute: None,
            inverse: None,
            depends: Vec::new(),
            is_init: false,
        }
    }

    /// Decide whether this field is kept, now that every struct has had its say.
    ///
    /// It cannot be decided earlier. A struct may mention a field another one computes, without
    /// repeating the computation — and on its own that struct looks like it is declaring a plain
    /// field, which would be kept. Only the merged view knows the field is computed at all.
    ///
    /// Returns what is wrong when a struct asked to keep a field that nothing computes — every
    /// other field is kept anyway, so saying so means the author expected something else — or a
    /// list of references, which no column can hold.
    pub fn settle_storage(&mut self) -> Result<(), String> {
        if self.asked_for_storage && self.compute.is_none() {
            return Err(format!(
                "Field {} is asked to be stored, but nothing computes it. Every other field is \
                 kept anyway: there is nowhere else its value could live.",
                self.name
            ));
        }
        if self.asked_for_storage && !self.kind.is_stored() {
            return Err(format!(
                "Field {} is asked to be stored, but a list of references has no column to keep \
                 it in: it is worked out on each read.",
                self.name
            ));
        }
        self.stored = self.asked_for_storage || self.compute.is_none();
        Ok(())
    }

    /// Whether the field lives in a column of its own.
    ///
    /// Both halves have to agree: the field must be meant to be kept, and its kind must be one a
    /// column can hold — the "many" side of a relation is neither.
    pub fn is_stored(&self) -> bool {
        self.stored && self.kind.is_stored()
    }

    pub fn register_internal_field(&mut self, field_descriptor: &InternalField) {
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
        // Never taken back: a struct extending a model can hide a field, and no struct can
        // reveal one that another decided to hide. A password is hidden whatever it declares,
        // because a field whose whole purpose is to be unreadable should not depend on the
        // author of every struct touching it remembering to say so.
        self.private |= field_descriptor.private || field_descriptor.kind == FieldKind::Password;
        // Asking for storage wins, the same way hiding does: a struct extending a model may keep
        // a computed field another worked out on every read, and none can take that back.
        self.asked_for_storage |= field_descriptor.asks_for_storage;
        if let Some(new_compute) = &field_descriptor.compute {
            if let Some(existing_compute) = &mut self.compute {
                // The field has one chain, held under one method name. Two structs naming
                // different methods would each get their own, and only one would ever run.
                if existing_compute.method != new_compute.method {
                    panic!(
                        "Field {} is computed by {} and by {} in two different structs. Every \
                         struct contributing to a computed field must name the same method.",
                        self.name, existing_compute.method, new_compute.method
                    );
                }
                existing_compute
                    .depends
                    .append(&mut new_compute.depends.clone());
                // Remove duplicates
                let mut seen = HashSet::new();
                existing_compute.depends.retain(|dep| seen.insert(*dep));
            } else {
                self.compute = Some(FieldCompute {
                    method: new_compute.method.clone(),
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
