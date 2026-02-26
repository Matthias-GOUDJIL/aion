use crate::token::Token;

#[derive(Debug, Clone)]
pub struct Program {
    pub module_name: Option<String>,
    pub imports: Vec<Import>,
    pub declarations: Vec<Declaration>,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub path: Vec<String>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Declaration {
    Function(Function),
    Struct(Struct),
    Enum(Enum),
    Interface(Interface),
    Impl(ImplBlock),
}

#[derive(Debug, Clone)]
pub struct Interface {
    pub name: String,
    pub methods: Vec<Function>, // Method signatures
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub target_name: String,
    pub generic_params: Vec<String>,
    pub interface_name: Option<String>, // Optional: impl Interface for Target
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub generic_params: Vec<String>,
    pub params: Vec<(String, String)>,
    pub return_type: String,
    pub body: Option<Vec<Statement>>, 
    pub modifiers: Vec<Token>,        
    pub attributes: Vec<(String, String)>, 
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub generic_params: Vec<String>,
    pub fields: Vec<(String, String)>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub generic_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub data_types: Vec<String>, 
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let { name: String, value: Expression, intent: Option<String>, is_mut: bool },
    Return { value: Expression, intent: Option<String> },
    ExpressionStmt(Expression),
    If { condition: Expression, then_branch: Vec<Statement>, else_branch: Option<Vec<Statement>> },
    Assignment { target: Expression, value: Expression },
    While { condition: Expression, body: Vec<Statement> },
    For { var: String, range: Expression, body: Vec<Statement> },
    Spawn(Vec<Statement>),
    Match { condition: Expression, arms: Vec<MatchArm> },
    UnsafeBlock(Vec<Statement>),
    NoOp,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: String,
    pub params: Vec<String>, // Variables bound in the pattern (e.g., val in Ok(val))
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Integer(i64),
    Float(f64),
    String(String),
    Duration(u64, u32), // secs, nanos
    Date(i64),          // timestamp
    Identifier(String),
    Boolean(bool),
    Infix { left: Box<Expression>, operator: Token, right: Box<Expression> },
    Call { 
        function: String, 
        generic_args: Vec<String>,
        arguments: Vec<Expression> 
    },
    StructInst { 
        name: String, 
        generic_args: Vec<String>,
        fields: Vec<(String, Expression)> 
    },
    Range { start: Box<Expression>, end: Box<Expression> },
    Block { statements: Vec<Statement>, is_unsafe: bool },
    If { condition: Box<Expression>, then_branch: Vec<Statement>, else_branch: Option<Vec<Statement>> },
    Deref { expr: Box<Expression> },
    Cast { expr: Box<Expression>, target: String },
    Intrinsic { name: String, arguments: Vec<Expression> },
    EnumInst { 
        name: String, 
        variant: String,
        generic_args: Vec<String>,
        arguments: Vec<Expression> 
    },
    MemberAccess {
        receiver: Box<Expression>,
        member: String,
    },
    MethodCall {
        receiver: Box<Expression>,
        method: String,
        generic_args: Vec<String>,
        arguments: Vec<Expression>,
    },
    TypeRef {
        name: String,
        generic_args: Vec<String>,
    },
}
