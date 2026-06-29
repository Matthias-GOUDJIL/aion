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
