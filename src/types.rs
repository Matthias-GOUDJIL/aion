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
    Unknown,
}
