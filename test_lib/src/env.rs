mod context;

use std::any::Any;
use std::collections::HashMap;
use std::mem;
use crate::env::context::Context;
use crate::ModelManager;

// Specific environment-stuff
#[derive(Debug)]
pub struct Environment<'env> {
    global: &'env GlobalEnvironment,
    context: Context,
}

impl<'env> Environment<'env> {
    pub fn new(global: &'env GlobalEnvironment) -> Environment<'env> {
        Self {
            global: global,
            context: Context::new(),
        }
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
}

// impl<'env> Copy for Environment<'env> {}

impl<'env> Clone for Environment<'env> {
    fn clone(&self) -> Self {
        let cloned_context = self.context.clone();
        Self {
            global: self.global,
            context: cloned_context,
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
