//! Values built from records and kept across requests, until those records change.
//!
//! A plugin names a cache and the models it is built from. Creating, writing or deleting a record
//! of one of them empties the cache, and so does loading a plugin. What a transaction builds after
//! changing one of them is not kept: another request would read what that transaction has not
//! committed. A cache may hold at most so many values, the one used least recently making room.

use std::any::Any;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};

type Value = Arc<dyn Any + Send + Sync>;

#[derive(Default)]
struct Slot {
    generation: u64,
    /// Each value, with when it was last used.
    values: HashMap<String, (Value, u64)>,
    /// The keys by when they were last used, the least recent first.
    by_use: BTreeMap<u64, String>,
    uses: u64,
}

impl Slot {
    fn get(&mut self, key: &str) -> Option<Value> {
        self.uses += 1;
        let (value, used) = self.values.get_mut(key)?;
        self.by_use.remove(used);
        *used = self.uses;
        self.by_use.insert(self.uses, key.to_string());
        Some(value.clone())
    }

    /// Keep a value, then let go of the least recently used ones beyond `capacity`.
    fn insert(&mut self, key: &str, value: Value, capacity: Option<usize>) {
        self.uses += 1;
        if let Some((_, used)) = self.values.insert(key.to_string(), (value, self.uses)) {
            self.by_use.remove(&used);
        }
        self.by_use.insert(self.uses, key.to_string());
        let Some(capacity) = capacity else {
            return;
        };
        while self.values.len() > capacity {
            let Some((_, oldest)) = self.by_use.pop_first() else {
                break;
            };
            self.values.remove(&oldest);
        }
    }

    fn clear(&mut self) {
        self.generation += 1;
        self.values.clear();
        self.by_use.clear();
    }
}

/// Every cache the loaded plugins declared.
#[derive(Default)]
pub struct SharedCaches {
    watched: HashMap<&'static str, Vec<&'static str>>,
    capacities: HashMap<&'static str, usize>,
    slots: Mutex<HashMap<&'static str, Slot>>,
}

impl SharedCaches {
    /// Declare a cache, built from records of these models.
    pub fn register(&mut self, name: &'static str, models: &[&'static str]) {
        self.watched.entry(name).or_default().extend(models);
    }

    /// Declare a cache holding at most `capacity` values: one kept per user or per session must
    /// not grow with the number of accounts.
    pub fn register_bounded(
        &mut self,
        name: &'static str,
        models: &[&'static str],
        capacity: usize,
    ) {
        self.register(name, models);
        self.capacities.insert(name, capacity);
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.watched.contains_key(name)
    }

    /// Whether some cache is built from this model.
    pub fn watches(&self, model_name: &str) -> bool {
        self.watched
            .values()
            .any(|models| models.contains(&model_name))
    }

    /// Whether this cache is built from one of these models.
    pub fn depends_on<'a>(&self, name: &str, mut models: impl Iterator<Item = &'a str>) -> bool {
        self.watched
            .get(name)
            .is_some_and(|watched| models.any(|model| watched.contains(&model)))
    }

    /// Empty every cache built from this model.
    pub fn forget_model(&self, model_name: &str) {
        let mut slots = self.lock();
        for (name, models) in &self.watched {
            if models.contains(&model_name) {
                slots.entry(name).or_default().clear();
            }
        }
    }

    /// Empty every cache.
    pub fn forget_all(&self) {
        for slot in self.lock().values_mut() {
            slot.clear();
        }
    }

    /// What a cache holds under a key, and the generation to hand back when storing a new value.
    pub(crate) fn get(&self, name: &'static str, key: &str) -> (Option<Value>, u64) {
        let mut slots = self.lock();
        let slot = slots.entry(name).or_default();
        (slot.get(key), slot.generation)
    }

    /// Keep a value, unless the cache was emptied since `generation` was read: it would then have
    /// been built from records that changed meanwhile.
    pub(crate) fn store(&self, name: &'static str, key: &str, value: Value, generation: u64) {
        let capacity = self.capacities.get(name).copied();
        let mut slots = self.lock();
        let slot = slots.entry(name).or_default();
        if slot.generation == generation {
            slot.insert(key, value, capacity);
        }
    }

    /// A poisoned lock only means a request panicked while holding it; what it guards stays valid.
    fn lock(&self) -> MutexGuard<'_, HashMap<&'static str, Slot>> {
        self.slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
