//! Values built from records and kept across requests, until those records change.
//!
//! A plugin names a cache and the models it is built from. Creating, writing or deleting a record
//! of one of them empties the cache, and so does loading a plugin. What a transaction builds after
//! changing one of them is not kept: another request would read what that transaction has not
//! committed.

use std::any::Any;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

type Value = Arc<dyn Any + Send + Sync>;

#[derive(Default)]
struct Slot {
    generation: u64,
    values: HashMap<String, Value>,
}

/// Every cache the loaded plugins declared.
#[derive(Default)]
pub struct SharedCaches {
    watched: HashMap<&'static str, Vec<&'static str>>,
    slots: Mutex<HashMap<&'static str, Slot>>,
}

impl SharedCaches {
    /// Declare a cache, built from records of these models.
    pub fn register(&mut self, name: &'static str, models: &[&'static str]) {
        self.watched.entry(name).or_default().extend(models);
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
                let slot = slots.entry(name).or_default();
                slot.generation += 1;
                slot.values.clear();
            }
        }
    }

    /// Empty every cache.
    pub fn forget_all(&self) {
        for slot in self.lock().values_mut() {
            slot.generation += 1;
            slot.values.clear();
        }
    }

    /// What a cache holds under a key, and the generation to hand back when storing a new value.
    pub(crate) fn get(&self, name: &'static str, key: &str) -> (Option<Value>, u64) {
        let mut slots = self.lock();
        let slot = slots.entry(name).or_default();
        (slot.values.get(key).cloned(), slot.generation)
    }

    /// Keep a value, unless the cache was emptied since `generation` was read: it would then have
    /// been built from records that changed meanwhile.
    pub(crate) fn store(&self, name: &'static str, key: &str, value: Value, generation: u64) {
        let mut slots = self.lock();
        let slot = slots.entry(name).or_default();
        if slot.generation == generation {
            slot.values.insert(key.to_string(), value);
        }
    }

    /// A poisoned lock only means a request panicked while holding it; what it guards stays valid.
    fn lock(&self) -> MutexGuard<'_, HashMap<&'static str, Slot>> {
        self.slots
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
