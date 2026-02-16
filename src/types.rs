#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Unit,
    Integer,
    Float,
    Boolean,
    String,
    Duration,
    Date,
    Function { is_unsafe: bool },
    Enum { name: String },
    Placeholder(String), // Placeholder like 'T'
    GenericInstance(String, Vec<Type>), // Concrete like 'Vector<i64>'
    Unknown,
}
