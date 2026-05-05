use crate::token::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

impl Span {
    pub const fn zero() -> Self {
        Self { line: 0, col: 0 }
    }

    pub fn from_token(tok: &Token) -> Self {
        Self { line: tok.line, col: tok.col }
    }
}

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
    pub methods: Vec<Function>,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ImplBlock {
    pub target_name: String,
    pub generic_params: Vec<String>,
    pub interface_name: Option<String>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub generic_params: Vec<String>,
    pub params: Vec<(String, String, Option<Box<Expression>>)>,
    pub return_type: String,
    pub body: Option<Vec<Statement>>,
    pub modifiers: Vec<Token>,
    pub attributes: Vec<(String, String)>,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub generic_params: Vec<String>,
    pub fields: Vec<(String, String)>,
    pub attributes: Vec<String>,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Enum {
    pub name: String,
    pub generic_params: Vec<String>,
    pub variants: Vec<EnumVariant>,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub data_types: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let { name: String, value: Expression, intent: Option<String>, is_mut: bool, span: Span },
    Return { value: Expression, intent: Option<String>, span: Span },
    ExpressionStmt(Expression, Span),
    If { condition: Expression, then_branch: Vec<Statement>, else_branch: Option<Vec<Statement>>, span: Span },
    Assignment { target: Expression, value: Expression, span: Span },
    While { condition: Expression, body: Vec<Statement>, span: Span },
    For { var: String, range: Expression, body: Vec<Statement>, span: Span },
    Spawn(Vec<Statement>, Span),
    Match { condition: Expression, arms: Vec<MatchArm>, span: Span },
    UnsafeBlock(Vec<Statement>, Span),
    NoOp,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: String,
    pub patterns: Vec<String>,
    pub guard: Option<Box<Expression>>,
    pub params: Vec<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Expression {
    Integer(i64, Span),
    Float(f64, Span),
    String(String, Span),
    Duration(u64, u32, Span),
    Date(i64, Span),
    Identifier(String, Span),
    Boolean(bool, Span),
    Infix { left: Box<Expression>, operator: Token, right: Box<Expression>, span: Span },
    Call {
        function: String,
        generic_args: Vec<String>,
        arguments: Vec<Expression>,
        span: Span,
    },
    StructInst {
        name: String,
        generic_args: Vec<String>,
        fields: Vec<(String, Expression)>,
        span: Span,
    },
    Range { start: Box<Expression>, end: Box<Expression>, span: Span },
    Block { statements: Vec<Statement>, is_unsafe: bool, span: Span },
    If { condition: Box<Expression>, then_branch: Vec<Statement>, else_branch: Option<Vec<Statement>>, span: Span },
    Deref { expr: Box<Expression>, span: Span },
    Cast { expr: Box<Expression>, target: String, span: Span },
    Intrinsic { name: String, arguments: Vec<Expression>, span: Span },
    EnumInst {
        name: String,
        variant: String,
        generic_args: Vec<String>,
        arguments: Vec<Expression>,
        span: Span,
    },
    MemberAccess {
        receiver: Box<Expression>,
        member: String,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expression>,
        method: String,
        generic_args: Vec<String>,
        arguments: Vec<Expression>,
        span: Span,
    },
    TypeRef {
        name: String,
        generic_args: Vec<String>,
        span: Span,
    },
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Integer(_, s)
            | Expression::Float(_, s)
            | Expression::String(_, s)
            | Expression::Duration(_, _, s)
            | Expression::Date(_, s)
            | Expression::Identifier(_, s)
            | Expression::Boolean(_, s)
            | Expression::Infix { span: s, .. }
            | Expression::Call { span: s, .. }
            | Expression::StructInst { span: s, .. }
            | Expression::Range { span: s, .. }
            | Expression::Block { span: s, .. }
            | Expression::If { span: s, .. }
            | Expression::Deref { span: s, .. }
            | Expression::Cast { span: s, .. }
            | Expression::Intrinsic { span: s, .. }
            | Expression::EnumInst { span: s, .. }
            | Expression::MemberAccess { span: s, .. }
            | Expression::MethodCall { span: s, .. }
            | Expression::TypeRef { span: s, .. } => *s,
        }
    }
}
