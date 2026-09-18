/// One sort key of a search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderBy {
    pub field: String,
    pub descending: bool,
}

impl OrderBy {
    /// Sort on `field`, smallest first.
    pub fn asc(field: &str) -> Self {
        Self {
            field: field.to_string(),
            descending: false,
        }
    }

    /// Sort on `field`, largest first.
    pub fn desc(field: &str) -> Self {
        Self {
            field: field.to_string(),
            descending: true,
        }
    }
}

/// What a search asks for beyond the domain itself.
///
/// The default asks for nothing: every matching record, in the backend's natural order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Keep at most this many records. `None` keeps them all.
    pub limit: Option<usize>,
    /// Skip this many records before taking any.
    pub offset: usize,
    /// Sort keys, applied in order. Records that tie on all of them keep a stable order.
    pub order: Vec<OrderBy>,
}

impl SearchOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    pub fn order_by(mut self, order: OrderBy) -> Self {
        self.order.push(order);
        self
    }

    /// True when the options ask for nothing, which lets a backend skip the paging work.
    pub fn is_unbounded(&self) -> bool {
        self.limit.is_none() && self.offset == 0 && self.order.is_empty()
    }

    /// Apply `offset` then `limit` to an already-ordered list.
    pub fn paginate<T>(&self, items: Vec<T>) -> Vec<T> {
        let mut items = items.into_iter().skip(self.offset);
        match self.limit {
            Some(limit) => items.by_ref().take(limit).collect(),
            None => items.collect(),
        }
    }
}
