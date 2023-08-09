use std::any::Any;
use std::collections::HashMap;
use crate::ModelManager;

// Specific environment-stuff
pub struct Environment<'a> {
    global: &'a GlobalEnvironment,
    context: HashMap<String, dyn Any>,
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

    pub fn with_context<T: Any>(&self, key: &str, value: T) -> Environment<'a> {
        let mut context = self.context.clone();
        context.insert(key.to_string(), Box::new(value));
        Self {
            global: self.global,
            context: context,
        }
    }
}

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
}