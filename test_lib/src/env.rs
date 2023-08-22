mod context;

use std::mem;
use crate::env::context::Context;
use crate::{CachedModels, ModelManager};
use crate::cache::CachedFieldDescriptor;

// Specific environment-stuff
#[derive(Debug)]
pub struct Environment<'env> {
    global: &'env GlobalEnvironment,
    context: Context,
    cache: CachedModels,
    pub counter: u32,
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
            let fields: Vec<CachedFieldDescriptor> = model_descriptor.get_fields().iter().map(|(_, f)| {
                CachedFieldDescriptor::from_field_descriptor(f)
            }).collect();
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

    pub fn cache_mut(&mut self) -> &mut CachedModels {
        &mut self.cache
    }
}

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

    pub fn models_mut(&mut self) -> &mut ModelManager {
        &mut self.model_manager
    }
}

// TODO Check what to add in this trait
pub trait ModelEnvironment<'env> {
    // fn new<IMD>(env: std::rc::Weak<std::cell::RefCell<Environment<'env>>>) -> IMD where IMD: InternalModelGetterDescriptor<'env> {
    //     let name = IMD::_name();
    //     let (id, new_fields) = match env.upgrade() {
    //         Some(env_borrow) => {
    //             let mut env = env_borrow.borrow_mut();
    //             env.counter += 1;
    //             let id = env.counter;
    //             let cached_record = env.cache.new_cached_record(name, id);
    //             (id, cached_record.get_new_fields())
    //         }
    //         None => {
    //             // Should be there
    //             panic!("Environment should exist!")
    //         }
    //     };
    //     IMD::_from_map(id, new_fields, env)
    // }
}
