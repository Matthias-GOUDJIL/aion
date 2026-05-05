use super::{Expression, Statement};
use crate::lexer::token::Token;

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
