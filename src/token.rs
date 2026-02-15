#[derive(Debug, PartialEq, Clone)]
pub enum Token {
    // System Keywords
    Fn, Let, Mut, Struct, Enum, Return, If, Else, Match,
    Use, Pub, Async, Unsafe, Require,
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
    DoubleColon, 
    Range,       
    At,
    Question,

    // End of File
    EOF,
    
    // Error
    Illegal(char),
}
