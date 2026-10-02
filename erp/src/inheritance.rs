//! Markup that other plugins change with `<xpath>` — templates, views — and resolving its final
//! form.
//!
//! A piece of markup without a parent is a base one. One inheriting in `extension` mode changes its
//! parent in place; one in `primary` mode is new markup, its parent's final form changed by its own
//! specifications. Extensions apply in their `order`, lowest first.

use crate::xml::{Node, XmlError, apply_extension, parse_fragment};
use std::collections::{HashMap, HashSet};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Where an extension stands among those of one parent: compared field by field, lowest first.
pub type Order = (usize, String, i64);

/// One piece of markup, and what its kind keeps beside it.
pub struct Arch<T> {
    pub id: u32,
    /// How an error names it: `Template web.Layout`, `View view_users_list`.
    pub label: String,
    pub arch: String,
    pub inherit: Option<u32>,
    pub primary: bool,
    pub order: Order,
    pub data: T,
}

/// Every piece of markup of one kind, to resolve any of them without going back to the database.
pub struct Archs<T> {
    rows: HashMap<u32, Arch<T>>,
}

impl<T> Archs<T> {
    pub fn new(rows: Vec<Arch<T>>) -> Self {
        Archs {
            rows: rows.into_iter().map(|row| (row.id, row)).collect(),
        }
    }

    pub fn get(&self, id: u32) -> Option<&Arch<T>> {
        self.rows.get(&id)
    }

    pub fn all(&self) -> impl Iterator<Item = &Arch<T>> {
        self.rows.values()
    }

    /// The base and primary pieces — those resolved on their own — in their order.
    pub fn primaries(&self) -> Vec<&Arch<T>> {
        let mut primaries: Vec<&Arch<T>> = self.rows.values().filter(|row| row.primary).collect();
        primaries.sort_by(|a, b| (&a.order, a.id).cmp(&(&b.order, b.id)));
        primaries
    }

    /// The base piece a piece derives from, through every parent: `None` past a missing parent or
    /// round a cycle.
    pub fn root_of<'a>(&'a self, row: &'a Arch<T>) -> Option<&'a Arch<T>> {
        let mut row = row;
        let mut seen = HashSet::new();
        while let Some(parent) = row.inherit {
            if !seen.insert(row.id) {
                return None;
            }
            row = self.rows.get(&parent)?;
        }
        Some(row)
    }

    /// A piece's final markup: its own, or its parent's changed by it, then its extensions.
    pub fn resolve(&self, id: u32) -> Result<Vec<Node>> {
        self.resolve_within(id, &mut HashSet::new())
    }

    fn resolve_within(&self, id: u32, visiting: &mut HashSet<u32>) -> Result<Vec<Node>> {
        let row = self
            .rows
            .get(&id)
            .ok_or_else(|| format!("No markup #{id}"))?;
        if !visiting.insert(id) {
            return Err(format!("{} inherits from itself", row.label).into());
        }
        let mut nodes = match row.inherit {
            None => parse_fragment(&row.arch).map_err(|error| failed(row, &error))?,
            Some(parent) => {
                let mut nodes = self.resolve_within(parent, visiting)?;
                apply(row, &mut nodes)?;
                nodes
            }
        };
        let mut extensions: Vec<&Arch<T>> = self
            .rows
            .values()
            .filter(|other| other.inherit == Some(id) && !other.primary)
            .collect();
        extensions.sort_by(|a, b| (&a.order, a.id).cmp(&(&b.order, b.id)));
        for extension in extensions {
            apply(extension, &mut nodes)?;
        }
        visiting.remove(&id);
        Ok(nodes)
    }
}

fn apply<T>(row: &Arch<T>, nodes: &mut Vec<Node>) -> Result<()> {
    let spec = parse_fragment(&row.arch).map_err(|error| failed(row, &error))?;
    apply_extension(nodes, &spec).map_err(|error| failed(row, &error))?;
    Ok(())
}

/// An error about a piece of markup, naming it.
pub fn failed<T>(row: &Arch<T>, error: &XmlError) -> Box<dyn Error + Send + Sync> {
    format!("{}: {error}", row.label).into()
}
