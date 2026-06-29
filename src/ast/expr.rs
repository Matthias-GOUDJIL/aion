use super::Span;
use super::stmt::MatchArm;
use crate::ast::Statement;
use crate::lexer::token::Token;

#[derive(Debug, Clone)]
pub enum Expression {
    Integer(i64, Span),
    Float(f64, Span),
    Char(char, Span),
    String(String, Span),
    Duration(u64, u32, Span),
    Date(i64, Span),
    Identifier(String, Span),
    Boolean(bool, Span),
    Infix {
        left: Box<Expression>,
        operator: Token,
        right: Box<Expression>,
        span: Span,
    },
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
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
        span: Span,
    },
    Block {
        statements: Vec<Statement>,
        is_unsafe: bool,
        span: Span,
    },
    If {
        condition: Box<Expression>,
        then_branch: Vec<Statement>,
        else_branch: Option<Vec<Statement>>,
        span: Span,
    },
    Deref {
        expr: Box<Expression>,
        span: Span,
    },
    Cast {
        expr: Box<Expression>,
        target: String,
        span: Span,
    },
    Intrinsic {
        name: String,
        arguments: Vec<Expression>,
        span: Span,
    },
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
    Match {
        condition: Box<Expression>,
        arms: Vec<MatchArm>,
        span: Span,
    },
}

impl Expression {
    pub fn span(&self) -> Span {
        match self {
            Expression::Integer(_, s)
            | Expression::Float(_, s)
            | Expression::Char(_, s)
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
            | Expression::TypeRef { span: s, .. }
            | Expression::Match { span: s, .. } => *s,
        }
    }
}
