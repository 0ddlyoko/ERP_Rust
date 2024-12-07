use crate::field::FieldType;

/// Cache for a single field
#[derive(Default, Clone)]
pub struct CacheField {
    value: Option<FieldType>,
}

impl CacheField {
    pub fn new_with_value(value: FieldType) -> Self {
        Self { value: Some(value) }
    }

    pub fn get(&self) -> Option<&FieldType> {
        match &self.value {
            Some(v) => Some(v),
            None => None,
        }
    }

    pub fn set(&mut self, value: FieldType) {
        self.value = Some(value);
    }

    pub fn is_set(&self) -> bool {
        self.value.is_some()
    }

    pub fn clear(&mut self) {
        self.value = None;
    }
}

#[cfg(test)]
mod tests {
    use crate::cache::{CacheField, FieldType};

    #[test]
    fn test() {
        let mut cache_field = CacheField::default();
        assert!(cache_field.get().is_none());

        cache_field.set(FieldType::Integer(1));
        let result = cache_field.get();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &FieldType::Integer(1));
    }
}
