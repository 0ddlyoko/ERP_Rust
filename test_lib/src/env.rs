use std::any::Any;
use std::collections::HashMap;
use crate::ModelManager;

// Specific environment-stuff
#[derive(Debug)]
pub struct Environment<'env> {
    global: &'env GlobalEnvironment,
    context: HashMap<String, String>,
}

impl<'env> Environment<'env> {
    pub fn new(global: &'env GlobalEnvironment) -> Environment<'env> {
        Self {
            global: global,
            context: HashMap::new(),
        }
    }

    pub fn global(&self) -> &GlobalEnvironment {
        &self.global
    }

    pub fn models(&self) -> &ModelManager {
        &self.global.model_manager
    }

    pub fn with_context(&mut self, key: &str, value: String) {
        self.context.insert(key.to_string(), value);
    }

    pub fn with_new_context(&mut self, key: &str, value: String) -> Environment {
        let mut new_env = self.clone();
        new_env.with_context(key, value);
        new_env
    }
}

// impl<'env> Copy for Environment<'env> {}

impl<'env> Clone for Environment<'env> {
    fn clone(&self) -> Self {
        let mut context = HashMap::new();
        self.context.iter().for_each(|(key, value)| {
            context.insert(key.clone(), (*value).clone());
        });
        Self {
            global: self.global,
            context: context,
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
    fn restore_env(&mut self, env: &'env Environment<'env>);
}
