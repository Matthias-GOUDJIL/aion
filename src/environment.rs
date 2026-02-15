use std::collections::HashMap;
use crate::types::Type;

#[derive(Clone, Debug)]
pub struct Environment {
    pub store: HashMap<String, Type>,
    pub outer: Option<Box<Environment>>,
}

impl Environment {
    pub fn new() -> Self {
        Self { store: HashMap::new(), outer: None }
    }
    
    pub fn new_enclosed(outer: Environment) -> Self {
        Self { store: HashMap::new(), outer: Some(Box::new(outer)) }
    }

    pub fn get(&self, name: &str) -> Option<Type> {
        match self.store.get(name) {
            Some(t) => Some(t.clone()),
            None => match &self.outer {
                Some(outer) => outer.get(name),
                None => None,
            },
        }
    }

    pub fn set(&mut self, name: String, val: Type) {
        self.store.insert(name, val);
    }
}
