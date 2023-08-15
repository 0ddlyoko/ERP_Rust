mod context;

use std::mem;
use crate::env::context::Context;
use crate::{CachedModels, InternalModelGetterDescriptor, ModelManager};
use crate::cache::CachedFieldDescriptor;

// Specific environment-stuff
#[derive(Debug)]
pub struct Environment<'env> {
    global: &'env GlobalEnvironment,
    context: Context,
    cache: CachedModels,
    counter: u32,
}

impl<'env> Environment<'env> {
    pub fn new(global: &'env GlobalEnvironment) -> Environment<'env> {
        let mut env = Self {
            global: global,
            context: Context::new(),
            cache: CachedModels::new(),
            counter: 1,
        };
        // Load cache
        global.model_manager.models().values().for_each(|model_descriptor| {
            let table_name = model_descriptor.get_table_name();
            let fields: Vec<CachedFieldDescriptor> = vec![];
            // TODO Retrieve field type & default value
            env.cache.add_cache_model(table_name, fields);
        });

        env
    }

    pub fn global(&self) -> &GlobalEnvironment {
        &self.global
    }

    pub fn models(&self) -> &ModelManager {
        &self.global.model_manager
    }

    pub fn with_context(&mut self, key: &str, value: &str) -> &mut Environment<'env> {
        self.context.register(key, value);
        self
    }

    pub fn with_new_context(&mut self, key: &str, value: &str) -> Context {
        let new_context = self.context.clone();
        let old_context = mem::replace(&mut self.context, new_context);
        self.with_context(key, value);
        old_context
    }

    pub fn remove_context(&mut self, key: &str) -> &mut Environment<'env> {
        self.context.unregister(key);
        self
    }

    pub fn restore_context(&mut self, context: Context) -> &mut Environment<'env> {
        self.context = context;
        self
    }

    pub fn clear_context(&mut self) -> &mut Environment<'env> {
        self.context.clear();
        self
    }

    pub fn cache(&self) -> &CachedModels {
        &self.cache
    }
    //
    // // TODO Move to correct class
    // // TODO No need to have a mut class
    // pub fn new_empty_model<IMD>(&mut self) where IMD: InternalModelGetterDescriptor {
    //     let name = IMD::_name();
    //     let id = self.counter;
    //     self.counter += 1;
    // }
}

// impl<'env> Copy for Environment<'env> {}

impl<'env> Clone for Environment<'env> {
    fn clone(&self) -> Self {
        Self {
            global: self.global,
            context: self.context.clone(),
            // TODO Clone cache ????
            cache: CachedModels::new(),
            counter: 1,
        }
    }
}

#[derive(Debug)]
pub struct GlobalEnvironment {
    model_manager: ModelManager,
}

impl GlobalEnvironment {
    pub fn new() -> GlobalEnvironment {
        Self {
            model_manager: ModelManager::new(),
        }
    }

    pub fn new_env(&self) -> Environment {
        Environment::new(&self)
    }

    pub fn models(&self) -> &ModelManager {
        &self.model_manager
    }
}

pub trait ModelEnvironment<'env> {
    fn env(&self) -> &Environment<'env>;
    fn env_mut(&mut self) -> &mut Environment<'env>;
    fn restore_env(&mut self, env: &'env mut Environment<'env>);
}
