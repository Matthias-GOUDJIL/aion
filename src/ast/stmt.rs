use super::{Expression, Span};

#[derive(Debug, Clone)]
pub enum Statement {
    Let {
        name: String,
        value: Expression,
        explicit_type: Option<String>,
        intent: Option<String>,
        is_mut: bool,
        span: Span,
    },
    /// Destructuring let: `let (a, b) = pair`. #53.
    LetTuple {
        names: Vec<String>,
        value: Expression,
        span: Span,
    },
    Return {
        value: Expression,
        intent: Option<String>,
        span: Span,
    },
    ExpressionStmt(Expression, Span),
    If {
        condition: Expression,
        then_branch: Vec<Statement>,
        else_branch: Option<Vec<Statement>>,
        span: Span,
    },
    Assignment {
        target: Expression,
        value: Expression,
        span: Span,
    },
    While {
        condition: Expression,
        body: Vec<Statement>,
        span: Span,
    },
    For {
        var: String,
        range: Expression,
        body: Vec<Statement>,
        span: Span,
    },
    Spawn(Vec<Statement>, Span),
    Match {
        condition: Expression,
        arms: Vec<MatchArm>,
        span: Span,
    },
    UnsafeBlock(Vec<Statement>, Span),
    Break(Span),
    Continue(Span),
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

impl Statement {
    /// Source location of the statement (first token). Zero for `NoOp`.
    pub fn span(&self) -> Span {
        match self {
            Statement::Let { span, .. } => *span,
            Statement::LetTuple { span, .. } => *span,
            Statement::Return { span, .. } => *span,
            Statement::ExpressionStmt(_, span) => *span,
            Statement::If { span, .. } => *span,
            Statement::Assignment { span, .. } => *span,
            Statement::While { span, .. } => *span,
            Statement::For { span, .. } => *span,
            Statement::Spawn(_, span) => *span,
            Statement::Match { span, .. } => *span,
            Statement::UnsafeBlock(_, span) => *span,
            Statement::Break(span) | Statement::Continue(span) => *span,
            Statement::NoOp => Span::zero(),
        }
    }
}
