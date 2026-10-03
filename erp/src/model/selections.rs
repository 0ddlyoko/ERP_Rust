use erp_types::field::{Placement, Selection, SelectionFamily, SelectionValue};
use std::collections::{HashMap, HashSet};

/// A value of a family, as fields of its enums now show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Choice {
    pub key: String,
    pub label: String,
    placed: Placement,
}

/// The values of every family of enums, in the order fields show them: the root enum's, then
/// what each enum extending it named, added, moved or relabelled, in the order plugins
/// registered them.
#[derive(Default)]
pub struct Selections {
    families: HashMap<&'static str, Vec<Choice>>,
    applied: HashSet<&'static str>,
}

impl Selections {
    /// Start a family from its root enum, unless it already is.
    pub fn ensure(&mut self, family: SelectionFamily) {
        self.families.entry(family.family).or_insert_with(|| {
            family
                .root_values
                .iter()
                .map(|value| Choice {
                    key: value.key.to_string(),
                    label: value.label.to_string(),
                    placed: Placement::Unchanged,
                })
                .collect()
        });
    }

    /// Apply what an enum declares to its family, after the enums it extends.
    ///
    /// A key the family has is named: moved if the enum places it, relabelled if it writes a
    /// label. A new key is added where the enum places it, else last. Applying an enum twice
    /// changes nothing.
    ///
    /// # Panics
    /// When a value is placed after or before a key the family does not have yet — the plugin
    /// declaring that key must load first — or when the enum's own placements go round in a
    /// circle.
    pub fn extend<E: Selection>(&mut self) {
        let name = std::any::type_name::<E>();
        if std::any::type_name::<E::Parent>() != name {
            self.extend::<E::Parent>();
        }
        if !self.applied.insert(name) {
            return;
        }
        self.ensure(SelectionFamily::of::<E>());
        refuse_circles(name, E::VALUES);
        let choices = self
            .families
            .get_mut(E::FAMILY)
            .expect("the family was just started");
        for value in E::VALUES {
            place(name, choices, value);
        }
    }

    /// The values of a family, in order.
    pub fn choices(&self, family: &str) -> &[Choice] {
        self.families.get(family).map_or(&[], Vec::as_slice)
    }

    pub fn contains(&self, family: &str, key: &str) -> bool {
        self.choices(family).iter().any(|choice| choice.key == key)
    }
}

/// Name, add, move or relabel one value.
///
/// Values placed after the same one keep the order they were placed in: the later goes after
/// those already there.
fn place(enum_name: &str, choices: &mut Vec<Choice>, value: &SelectionValue) {
    let existing = choices.iter().position(|choice| choice.key == value.key);
    let mut choice = match existing {
        Some(at) if value.placement == Placement::Unchanged => {
            if value.label_given {
                choices[at].label = value.label.to_string();
            }
            return;
        }
        Some(at) => choices.remove(at),
        None => Choice {
            key: value.key.to_string(),
            label: value.label.to_string(),
            placed: Placement::Unchanged,
        },
    };
    if value.label_given {
        choice.label = value.label.to_string();
    }
    let at = match value.placement {
        Placement::Unchanged => choices.len(),
        Placement::After(target) | Placement::Before(target) => {
            let Some(found) = choices.iter().position(|choice| choice.key == target) else {
                panic!(
                    "{enum_name} places \"{}\" next to \"{target}\", which its family does not \
                     have: the plugin declaring \"{target}\" must load first",
                    value.key
                );
            };
            if matches!(value.placement, Placement::After(_)) {
                let mut at = found + 1;
                while choices
                    .get(at)
                    .is_some_and(|next| next.placed == value.placement)
                {
                    at += 1;
                }
                at
            } else {
                found
            }
        }
    };
    choice.placed = value.placement;
    choices.insert(at, choice);
}

/// Refuse an enum whose values are placed relative to each other in a circle: `a` after `c` and
/// `c` after `a` say nothing about where either goes.
fn refuse_circles(enum_name: &str, values: &[SelectionValue]) {
    let next: HashMap<&str, &str> = values
        .iter()
        .filter_map(|value| match value.placement {
            Placement::After(target) | Placement::Before(target) => Some((value.key, target)),
            Placement::Unchanged => None,
        })
        .collect();
    for start in next.keys() {
        let mut seen = vec![*start];
        let mut at = *start;
        while let Some(target) = next.get(at) {
            if *target == *start {
                seen.push(target);
                panic!(
                    "{enum_name} places its values relative to each other in a circle: {}",
                    seen.join(" → ")
                );
            }
            if seen.contains(target) {
                break;
            }
            seen.push(target);
            at = target;
        }
    }
}
