use super::types::Type;
use std::collections::HashMap;

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
        Self {
            store: HashMap::new(),
            outer: None,
        }
    }

    pub fn new_enclosed(outer: Environment) -> Self {
        Self {
            store: HashMap::new(),
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
        self.store.insert(name, val);
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
