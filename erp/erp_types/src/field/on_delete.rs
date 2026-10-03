use std::fmt;

/// What becomes of the records a many2one points from, when the record it points to is deleted.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum OnDelete {
    /// The field is emptied — refused when it is required.
    #[default]
    SetNull,
    /// The deletion is refused while records point to it.
    Restrict,
    /// The records pointing to it are deleted with it.
    Cascade,
}

impl OnDelete {
    pub const KEYS: [&str; 3] = ["set_null", "restrict", "cascade"];

    pub fn from_key(key: &str) -> Option<OnDelete> {
        match key {
            "set_null" => Some(OnDelete::SetNull),
            "restrict" => Some(OnDelete::Restrict),
            "cascade" => Some(OnDelete::Cascade),
            _ => None,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            OnDelete::SetNull => "set_null",
            OnDelete::Restrict => "restrict",
            OnDelete::Cascade => "cascade",
        }
    }
}

impl fmt::Display for OnDelete {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.key())
    }
}
