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
        if let Some(t) = self.store.get(name) {
            eprintln!("DEBUG: env.get('{}') found exact match: {:?}", name, t);
            return Some(t.clone());
        }
        
        // Fuzzy lookup: check if name matches a fully qualified name suffix
        for (key, val) in &self.store {
            if key.ends_with(name) && (key.len() == name.len() || key.as_bytes()[key.len() - name.len() - 1] == b'.') {
                eprintln!("DEBUG: env.get('{}') found fuzzy match '{}': {:?}", name, key, val);
                return Some(val.clone());
            }
        }

        match &self.outer {
            Some(outer) => outer.get(name),
            None => None,
        }
    }

    pub fn set(&mut self, name: String, val: Type) {
        self.store.insert(name, val);
    }
}
