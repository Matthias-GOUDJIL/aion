use std::collections::HashMap;
use crate::types::Type;

#[derive(Clone, Debug)]
pub struct Environment {
    store: HashMap<String, Type>,
    outer: Option<Box<Environment>>,
}

impl Environment {
    pub fn new() -> Self {
        Self { store: HashMap::new(), outer: None }
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
