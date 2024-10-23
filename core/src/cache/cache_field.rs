
macro_rules! make_eq {
    ( $self:expr, $other:expr, $( $path:path ),* ) => {
        match $self {
            $($path(ref self_value) => {
                if let $path(ref other_value) = $other {
                    self_value == other_value
                } else {
                    false
                }
            })*
        }
    };
}

#[derive(Debug)]
pub enum CacheFieldValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl Clone for CacheFieldValue {
    fn clone(&self) -> Self {
        match self {
            CacheFieldValue::String(ref v) => CacheFieldValue::String(v.clone()),
            CacheFieldValue::Int(ref v) => CacheFieldValue::Int(*v),
            CacheFieldValue::Float(ref v) => CacheFieldValue::Float(*v),
            CacheFieldValue::Bool(ref v) => CacheFieldValue::Bool(*v),
        }
    }
}

impl PartialEq<Self> for CacheFieldValue {
    fn eq(&self, other: &Self) -> bool {
        make_eq!(
            self, other,
            CacheFieldValue::String,
            CacheFieldValue::Int,
            CacheFieldValue::Float,
            CacheFieldValue::Bool
        )
    }
}

/// Cache for a single field
pub struct CacheField {
    value: Option<CacheFieldValue>,
    dirty: bool,
}

impl CacheField {
    pub fn new() -> Self {
        Self { value: None, dirty: false }
    }

    pub fn new_with_value(value: CacheFieldValue) -> Self {
        Self { value: Some(value), dirty: false }
    }

    pub fn get(&self) -> Option<&CacheFieldValue> {
        match &self.value {
            Some(v) => Some(v),
            None => None,
        }
    }

    pub fn set(&mut self, value: CacheFieldValue) {
        self.value = Some(value);
    }
    
    pub fn is_set(&self) -> bool {
        self.value.is_some()
    }

    pub fn clear(&mut self) {
        self.value = None;
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    pub fn clean_dirty(&mut self) {
        self.dirty = false;
    }
}

mod test {
    use crate::cache::cache_field::{CacheField, CacheFieldValue};

    #[test]
    fn test() {
        let mut cache_field = CacheField::new();
        assert!(cache_field.get().is_none());

        cache_field.set(CacheFieldValue::Int(1));
        let result = cache_field.get();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &CacheFieldValue::Int(1));
    }
}
