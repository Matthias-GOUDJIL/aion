#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, col: usize) -> Self {
        Self { kind, line, col }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    // System Keywords
    Fn, Let, Mut, Struct, Enum, Return, If, Else, Match, As, While,
    Use, Pub, Async, Unsafe, Require, Extern,
    Interface, Impl, Channel,
    For, In, Type, SelfToken, Spawn,
    
    // AI-Native & Logic
    Intent, 
    Invariant,
    Inside, 

    // Identifiers and Literals
    Identifier(String),
    StringLiteral(String),
    FString(String),
    IntLiteral(i64),
    FloatLiteral(f64),
    DurationLiteral(u64, u32),
    DateLiteral(i64),
    True, False, // Keywords

    // Symbols and Operators
    Plus, Minus, Star, Slash, Percent, Caret, // Arithmetic (+ % ^)
    Eq, EqEq, NotEq, Arrow, 
    LArrow, // <- (Send/Receive)
    Pipeline, // |>
    And, Or, Bang, // Logic (&& || !)
    Gt, Lt, GtEq, LtEq,
    LParen, RParen, LBrace, RBrace, LBracket, RBracket, 
    Colon, Semicolon, Comma, Dot, 
    Pipe,
    DoubleColon, 
    Range,       
    At,
    Question,

    // End of File
    EOF,
    
    // Error
    Illegal(char),
}
