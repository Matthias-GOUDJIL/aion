use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Integer,
    Float,
    Boolean,
    String,
    Duration,
    Date,
    Function { is_unsafe: bool, return_type: Box<Type> },
    Enum { name: String },
    Struct { name: String },
    Placeholder(String),
    GenericInstance(String, Vec<Type>),
    Pointer(Box<Type>),
    Unknown,
}

impl Type {
    pub fn from_str(s: &str) -> Self {
        let trimmed = s.trim();
        if let Some(stripped) = trimmed.strip_prefix('*') {
            return Type::Pointer(Box::new(Type::from_str(stripped.trim())));
        }
        match trimmed {
            "i64" | "u64" | "i32" | "u32" | "i8" | "u8" => Type::Integer,
            "f64" => Type::Float,
            "bool" => Type::Boolean,
            "String" => Type::String,
            "Date" => Type::Date,
            "Duration" => Type::Duration,
            "void" | "Unit" => Type::Unit,
            _ => {
                if trimmed.contains('<') && trimmed.ends_with('>') {
                    if let Some(start) = trimmed.find('<') {
                        let base = trimmed[..start].to_string();
                        let args_str = &trimmed[start + 1..trimmed.len() - 1];
                        let mut ga = Vec::new();
                        for part in args_str.split(',') {
                            let pt = part.trim().to_string();
                            if !pt.is_empty() {
                                ga.push(Type::from_str(&pt));
                            }
                        }
                        return Type::GenericInstance(base, ga);
                    }
                }
                Type::Placeholder(trimmed.to_string())
            }
        }
    }

    pub fn name(&self) -> String {
        match self {
            Type::Unit => "void".to_string(),
            Type::Integer => "i64".to_string(),
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
            Type::Function { return_type, .. } => format!("fn() -> {}", return_type.name()),
            Type::Unknown => "unknown".to_string(),
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}
