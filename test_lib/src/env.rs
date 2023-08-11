use std::any::Any;
use std::collections::HashMap;
use crate::ModelManager;

// Specific environment-stuff
#[derive(Debug)]
pub struct Environment<'a> {
    global: &'a GlobalEnvironment,
    context: HashMap<String, Box<dyn Any>>,
}

impl<'a> Environment<'a> {
    pub fn new(global: &'a GlobalEnvironment) -> Environment<'a> {
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

    pub fn with_context<T: Any>(&mut self, key: &str, value: T) {
        self.context.insert(key.to_string(), Box::new(value));
    }

    pub fn with_new_context<T: Any>(&mut self, key: &str, value: T) -> Environment {

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
