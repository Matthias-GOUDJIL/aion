use super::types::Type;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct Environment {
    pub store: HashMap<String, Type>,
    pub immutable: HashSet<String>,
    pub outer: Option<Box<Environment>>,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    pub fn new() -> Self {
        Self {
            store: HashMap::new(),
            immutable: HashSet::new(),
            outer: None,
        }
    }

    pub fn new_enclosed(outer: Environment) -> Self {
        Self {
            store: HashMap::new(),
            immutable: HashSet::new(),
            outer: Some(Box::new(outer)),
        }
    }

    pub fn get(&self, name: &str) -> Option<Type> {
        if let Some(t) = self.store.get(name) {
            return Some(t.clone());
        }

        // Fuzzy lookup: check if name matches a fully qualified name suffix
        for (key, val) in &self.store {
            if key.ends_with(name)
                && (key.len() == name.len() || {
                    let sep_idx = key.len() - name.len() - 1;
                    let b = key.as_bytes()[sep_idx];
                    b == b'.' || (sep_idx >= 1 && key.as_bytes()[sep_idx - 1] == b':' && b == b':')
                })
            {
                return Some(val.clone());
            }
        }

        match &self.outer {
            Some(outer) => outer.get(name),
            None => None,
        }
    }

    pub fn set(&mut self, name: String, val: Type) {
        self.immutable.remove(&name);
        self.store.insert(name, val);
    }

    pub fn set_with_mut(&mut self, name: String, val: Type, is_mut: bool) {
        if is_mut {
            self.immutable.remove(&name);
        } else {
            self.immutable.insert(name.clone());
        }
        self.store.insert(name, val);
    }

    pub fn is_immutable(&self, name: &str) -> bool {
        if self.store.contains_key(name) {
            return self.immutable.contains(name);
        }
        match &self.outer {
            Some(outer) => outer.is_immutable(name),
            None => false,
        }
    }

    /// Iterator over all visible names in this environment and its enclosing
    /// scopes. Used by the type checker for "did you mean X?" suggestions. #40.
    pub fn visible_names(&self) -> impl Iterator<Item = &String> {
        // Build an explicit owned-vector chain to avoid lifetime gymnastics
        // across the recursive `outer` boxes; scopes stay small.
        let mut seen: std::collections::HashSet<&String> = std::collections::HashSet::new();
        let mut chain: Vec<&Environment> = vec![self];
        let mut idx = 0;
        while idx < chain.len() {
            let cur = chain[idx];
            for k in cur.store.keys() {
                seen.insert(k);
            }
            if let Some(o) = cur.outer.as_deref() {
                chain.push(o);
            }
            idx += 1;
        }
        seen.into_iter()
    }
}
