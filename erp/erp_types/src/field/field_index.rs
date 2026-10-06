use std::fmt;

/// How a field's column is indexed, for searches on it to be quick.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum FieldIndex {
    /// For equality, ranges and sorting: a code, a state, a date.
    #[default]
    Btree,
    /// For a text searched anywhere in it, `ilike "%word%"`: a name.
    Trigram,
}

impl FieldIndex {
    pub const KEYS: [&str; 2] = ["btree", "trigram"];

    pub fn from_key(key: &str) -> Option<FieldIndex> {
        match key {
            "btree" => Some(FieldIndex::Btree),
            "trigram" => Some(FieldIndex::Trigram),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            FieldIndex::Btree => "btree",
            FieldIndex::Trigram => "trigram",
        }
    }
}

impl fmt::Display for FieldIndex {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.key())
    }
}
