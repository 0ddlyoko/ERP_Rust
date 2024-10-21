use std::collections::HashMap;

#[derive(Debug)]
pub struct Context {
    context: HashMap<String, String>,
}

impl Context {
    pub fn new() -> Context {
        Self {
            context: HashMap::new(),
        }
    }

    pub fn register(&mut self, key: &str, value: &str) -> &mut Self {
        self.context.insert(key.to_string(), value.to_string());
        self
    }

    pub fn unregister(&mut self, key: &str) -> &mut Self {
        self.context.remove(key);
        self
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.context.get(key)
    }

    pub fn get_mut(&mut self, key: &str) -> Option<&mut String> {
        self.context.get_mut(key)
    }

    pub fn context(&self) -> &HashMap<String, String> {
        &self.context
    }

    pub fn clear(&mut self) {
        self.context.clear()
    }
}

impl Clone for Context {
    fn clone(&self) -> Self {
        let mut context = HashMap::new();
        self.context().iter().for_each(|(key, value)| {
            context.insert(key.clone(), value.clone());
        });
        Self {
            context: context,
        }
    }
}
