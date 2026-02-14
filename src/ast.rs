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
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, String)>,
    pub return_type: String,
    pub body: Option<Vec<Statement>>, 
    pub modifiers: Vec<Token>,        
    pub attributes: Vec<(String, String)>, 
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub name: String,
    pub fields: Vec<(String, String)>,
    pub attributes: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let { name: String, value: Expression, intent: Option<String>, is_mut: bool },
    Return { value: Expression, intent: Option<String> },
    ExpressionStmt(Expression),
    If { condition: Expression, then_branch: Vec<Statement>, else_branch: Option<Vec<Statement>> },
    For { var: String, range: Expression, body: Vec<Statement> }, // Ajout de For
    Spawn(Vec<Statement>), 
}

#[derive(Debug, Clone)]
pub enum Expression {
    Integer(i64),
    Float(f64),
    String(String),
    Identifier(String),
    Boolean(bool),
    Infix { left: Box<Expression>, operator: Token, right: Box<Expression> },
    Call { function: String, arguments: Vec<Expression> },
    StructInst { name: String, fields: Vec<(String, Expression)> },
    Range { start: Box<Expression>, end: Box<Expression> },
}
