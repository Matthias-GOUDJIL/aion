use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    /// Sized integer. `signed` distinguishes i*/u*; `bits` is 8/16/32/64.
    /// All integer arithmetic requires both operands to share the same
    /// `(signed, bits)`; mismatched sizes are a type error (#52). Integer
    /// literals default to i64 (`signed: true, bits: 64`) for backward
    /// compatibility, and coerce to any integer type at `let`/`return`.
    Integer {
        signed: bool,
        bits: u8,
    },
    Float,
    Boolean,
    String,
    Duration,
    Date,
    Function {
        is_unsafe: bool,
        params: Vec<Type>,
        return_type: Box<Type>,
    },
    Enum {
        name: String,
    },
    Struct {
        name: String,
    },
    Placeholder(String),
    GenericInstance(String, Vec<Type>),
    Pointer(Box<Type>),
    /// Heterogeneous fixed-size sequence: `(i64, String, bool)`. Codegen
    /// lowers to an anonymous LLVM struct type. #53.
    Tuple(Vec<Type>),
    Unknown,
}

impl Type {
    /// i64 — the default integer type for literals and unannotated bindings.
    /// Every existing `i64` program keeps this type. #52.
    pub fn i64() -> Self {
        Type::Integer {
            signed: true,
            bits: 64,
        }
    }
    pub fn u64() -> Self {
        Type::Integer {
            signed: false,
            bits: 64,
        }
    }
    pub fn i32() -> Self {
        Type::Integer {
            signed: true,
            bits: 32,
        }
    }
    pub fn u32() -> Self {
        Type::Integer {
            signed: false,
            bits: 32,
        }
    }
    pub fn i8() -> Self {
        Type::Integer {
            signed: true,
            bits: 8,
        }
    }
    pub fn u8() -> Self {
        Type::Integer {
            signed: false,
            bits: 8,
        }
    }

    /// True for any `Type::Integer { .. }` regardless of signedness/width. #52.
    pub fn is_integer(&self) -> bool {
        matches!(self, Type::Integer { .. })
    }

    pub fn parse(s: &str) -> Self {
        let trimmed = s.trim();
        if let Some(stripped) = trimmed.strip_prefix('*') {
            return Type::Pointer(Box::new(Type::parse(stripped.trim())));
        }
        // Tuple type: `(T, U, ...)`. #53.
        if trimmed.starts_with('(') && trimmed.ends_with(')') {
            let inner = &trimmed[1..trimmed.len() - 1];
            let mut elems = Vec::new();
            for part in split_top_level_commas(inner) {
                let p = part.trim();
                if !p.is_empty() {
                    elems.push(Type::parse(p));
                }
            }
            if elems.len() >= 2 {
                return Type::Tuple(elems);
            }
            // `(T)` is a parenthesized single type — return the inner type.
            if elems.len() == 1 {
                return elems.remove(0);
            }
            return Type::Unit;
        }
        match trimmed {
            "i64" => Type::i64(),
            "u64" => Type::u64(),
            "i32" => Type::i32(),
            "u32" => Type::u32(),
            "i8" => Type::i8(),
            "u8" => Type::u8(),
            // Legacy aliases that previously collapsed to Type::Integer.
            "int" | "Integer" => Type::i64(),
            "f64" => Type::Float,
            "bool" => Type::Boolean,
            "String" => Type::String,
            "Date" => Type::Date,
            "Duration" => Type::Duration,
            "void" | "Unit" => Type::Unit,
            _ => {
                if trimmed.contains('<')
                    && trimmed.ends_with('>')
                    && let Some(start) = trimmed.find('<')
                {
                    let base = trimmed[..start].to_string();
                    let args_str = &trimmed[start + 1..trimmed.len() - 1];
                    let mut ga = Vec::new();
                    for part in args_str.split(',') {
                        let pt = part.trim().to_string();
                        if !pt.is_empty() {
                            ga.push(Type::parse(&pt));
                        }
                    }
                    return Type::GenericInstance(base, ga);
                }
                Type::Placeholder(trimmed.to_string())
            }
        }
    }

    pub fn name(&self) -> String {
        match self {
            Type::Unit => "void".to_string(),
            Type::Integer {
                signed: true,
                bits: 64,
            } => "i64".to_string(),
            Type::Integer {
                signed: false,
                bits: 64,
            } => "u64".to_string(),
            Type::Integer {
                signed: true,
                bits: 32,
            } => "i32".to_string(),
            Type::Integer {
                signed: false,
                bits: 32,
            } => "u32".to_string(),
            Type::Integer {
                signed: true,
                bits: 8,
            } => "i8".to_string(),
            Type::Integer {
                signed: false,
                bits: 8,
            } => "u8".to_string(),
            // Other widths render generically.
            Type::Integer { signed: true, bits } => format!("i{}", bits),
            Type::Integer {
                signed: false,
                bits,
            } => format!("u{}", bits),
            Type::Float => "f64".to_string(),
            Type::Boolean => "bool".to_string(),
            Type::String => "String".to_string(),
            Type::Duration => "Duration".to_string(),
            Type::Date => "Date".to_string(),
            Type::Pointer(inner) => format!("*{}", inner.name()),
            Type::Tuple(elems) => {
                format!(
                    "({})",
                    elems
                        .iter()
                        .map(|t| t.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Type::Struct { name } => name.clone(),
            Type::Enum { name } => name.clone(),
            Type::GenericInstance(base, args) => {
                let args_str: Vec<String> = args.iter().map(|a| a.name()).collect();
                format!("{}<{}>", base, args_str.join(", "))
            }
            Type::Placeholder(name) => name.clone(),
            Type::Function {
                params,
                return_type,
                ..
            } => {
                let params_str: Vec<String> = params.iter().map(|p| p.name()).collect();
                format!("fn({}) -> {}", params_str.join(", "), return_type.name())
            }
            Type::Unknown => "unknown".to_string(),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Split `s` on top-level commas (depth 0), respecting nested `<>` and `()`
/// so that `(i64, (String, bool))` splits into `["i64", " (String, bool)"]`.
/// Used by `Type::parse` for tuple type element extraction. #53.
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth: i32 = 0;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '<' | '(' => {
                depth += 1;
                cur.push(c);
            }
            '>' | ')' => {
                depth -= 1;
                cur.push(c);
            }
            ',' if depth == 0 => {
                parts.push(cur.clone());
                cur.clear();
            }
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() || !parts.is_empty() {
        parts.push(cur);
    }
    parts
}
