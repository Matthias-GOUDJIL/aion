pub mod decl;
pub mod expr;
pub mod stmt;

pub use decl::*;
pub use expr::*;
pub use stmt::*;

use crate::lexer::token::Token;

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
        Self {
            line: tok.line,
            col: tok.col,
        }
    }
}
