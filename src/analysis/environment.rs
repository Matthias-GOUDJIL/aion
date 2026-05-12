use std::collections::HashMap;
use super::types::Type;

#[derive(Clone, Debug)]
pub struct Environment {
    pub store: HashMap<String, Type>,
    pub outer: Option<Box<Environment>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
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
            return Some(t.clone());
        }
        
        // Fuzzy lookup: check if name matches a fully qualified name suffix
        for (key, val) in &self.store {
            if key.ends_with(name) && (key.len() == name.len() || {
                let sep_idx = key.len() - name.len() - 1;
                let b = key.as_bytes()[sep_idx];
                b == b'.' || (sep_idx >= 1 && key.as_bytes()[sep_idx - 1] == b':' && b == b':')
            }) {
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
